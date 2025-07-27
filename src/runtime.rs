use ::kinfer::model::{ModelError, ModelRunner};
use ::ndarray::Array;
use ::std::sync::atomic::{AtomicBool, Ordering};
use ::std::sync::Arc;
use ::std::time::Duration;
use ::tokio::runtime::Runtime;
use ::tokio::time::sleep;
use nix::sys::timerfd::{ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags};

use crate::constants::ACTUATOR_NAME_TO_ID;
use crate::keyboard;
use crate::provider::KBotProvider;
use std::time::SystemTime;
use tracing::{debug, info};

// We trigger a read N milliseconds before reading the current actuator state,
// to account for the asynchronicity of the CAN RX buffer.
const TRIGGER_READ_BEFORE: Duration = Duration::from_millis(2);

pub struct ModelRuntime {
    model_provider: Arc<KBotProvider>,
    model_runner: Arc<ModelRunner>,
    dt: Duration,
    slowdown_factor: i32,
    magnitude_factor: f32,
    running: Arc<AtomicBool>,
    runtime: Option<Runtime>,
    keyboard_enabled: bool,
}

impl ModelRuntime {
    pub fn new(
        model_provider: Arc<KBotProvider>,
        model_runner: Arc<ModelRunner>,
        dt: u64,
        keyboard_enabled: bool,
    ) -> Self {
        assert!(dt > TRIGGER_READ_BEFORE.as_millis() as u64);

        Self {
            model_provider,
            model_runner,
            dt: Duration::from_millis(dt),
            slowdown_factor: 1,
            magnitude_factor: 1.0,
            running: Arc::new(AtomicBool::new(false)),
            runtime: None,
            keyboard_enabled,
        }
    }

    pub fn set_slowdown_factor(&mut self, slowdown_factor: i32) {
        assert!(slowdown_factor >= 1);
        self.slowdown_factor = slowdown_factor;
    }

    pub fn set_magnitude_factor(&mut self, magnitude_factor: f32) {
        assert!(magnitude_factor >= 0.0);
        assert!(magnitude_factor <= 1.0);
        self.magnitude_factor = magnitude_factor;
    }

    pub fn start(&mut self) -> Result<(), ModelError> {
        if self.running.load(Ordering::Relaxed) {
            return Ok(());
        }

        let running = self.running.clone();
        let model_runner = self.model_runner.clone();
        let model_provider = self.model_provider.clone();
        let dt = self.dt;
        let slowdown_factor = self.slowdown_factor;
        let magnitude_factor = self.magnitude_factor;
        let keyboard_enabled = self.keyboard_enabled;

        let runtime = Runtime::new()?;
        running.store(true, Ordering::Relaxed);

        runtime.spawn(async move {
            info!("Starting model runtime");

            println!("Press enter to Home...");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            // Moves to home position.
            model_provider.move_to_home().await?;

            // Wait for user to press enter
            println!("Press enter to start...");
            std::io::stdin().read_line(&mut input).unwrap();

            // NOW start the keyboard listener if enabled
            if keyboard_enabled {
                keyboard::start_keyboard_listener_now();
                println!("Keyboard controls are now active! Use ESC to exit or Ctrl+C.");
            }

            for i in 1..5 {
                println!("Starting in {} seconds...", 5 - i);
                sleep(Duration::from_secs(1)).await;
            }

            let mut carry = model_runner
                .init()
                .await
                .map_err(|e| ModelError::Provider(e.to_string()))?;

            // Get initial joint positions directly from actuator state
            let mut joint_positions = {
                let actuator_ids = ACTUATOR_NAME_TO_ID
                    .iter()
                    .map(|(_, id)| *id)
                    .collect::<Vec<u32>>();
                let actuator_states = model_provider.get_actuator_state(&actuator_ids).await?;

                let joint_angles: Vec<f32> = actuator_states
                    .iter()
                    .map(|state| state.position.map(|p| p as f32).unwrap_or(0.0))
                    .collect();

                Array::from_shape_vec((joint_angles.len(),), joint_angles)
                    .map_err(|e| ModelError::Provider(e.to_string()))?
                    .into_dyn()
            };

            // Wait for the first tick, since it happens immediately.
            let read_interval =
                TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::empty()).unwrap();
            read_interval
                .set(Expiration::Interval(dt.into()), TimerSetTimeFlags::empty())
                .map_err(|e| ModelError::Provider(format!("Failed to set timer: {}", e)))?;

            // Start the two intervals N milliseconds apart. The first tick is
            // always instantaneous and represents the start of the interval
            // ticks.
            read_interval
                .wait()
                .map_err(|e| ModelError::Provider(format!("Failed to wait for timer: {}", e)))?;

            info!("Entering main control loop");
            while running.load(Ordering::Relaxed) {
                let uuid = uuid::Uuid::new_v4();
                let uuid_main_control_loop = uuid::Uuid::new_v4();
                let start = SystemTime::now();
                debug!("runtime::model_runner_step::START uuid={}", uuid);
                debug!(
                    "runtime::main_control_loop::START uuid={}",
                    uuid_main_control_loop
                );

                let (output, next_carry) = model_runner
                    .step(carry)
                    .await
                    .map_err(|e| ModelError::Provider(e.to_string()))?;
                carry = next_carry;
                debug!(
                    "runtime::model_runner_step::END uuid={}, elapsed: {:?}",
                    uuid,
                    start.elapsed()
                );

                for i in 1..(slowdown_factor + 1) {
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }
                    let t = i as f32 / slowdown_factor as f32;
                    let interp_joint_positions = &joint_positions * (1.0 - t) + &output * t;
                    model_runner
                        .take_action(interp_joint_positions * magnitude_factor)
                        .await
                        .map_err(|e| ModelError::Provider(e.to_string()))?;

                    // Trigger an actuator read N milliseconds before the next
                    // command tick, to make sure the observations are as fresh
                    // as possible.
                    read_interval.wait().map_err(|e| {
                        ModelError::Provider(format!("Failed to wait for timer: {}", e))
                    })?;
                    model_provider.trigger_actuator_read().await?;
                    sleep(TRIGGER_READ_BEFORE).await;
                }

                joint_positions = output;
                debug!(
                    "runtime::main_control_loop::END uuid={}, elapsed: {:?}",
                    uuid_main_control_loop,
                    start.elapsed()
                );
            }
            info!("Exiting main control loop");
            Ok::<(), ModelError>(())
        });

        self.runtime = Some(runtime);
        Ok(())
    }

    pub fn stop(&mut self) {
        info!("Stopping model runtime");
        self.running.store(false, Ordering::Relaxed);
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
