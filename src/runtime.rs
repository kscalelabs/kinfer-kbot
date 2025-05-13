use ::kinfer::model::{ModelError, ModelRunner};
use ::std::sync::atomic::{AtomicBool, Ordering};
use ::std::sync::Arc;
use ::std::time::Duration;
use ::tokio::runtime::Runtime;
use ::tokio::time::{interval, sleep};
use tracing::{debug, info};

use crate::provider::KBotProvider;

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
}

impl ModelRuntime {
    pub fn new(model_provider: Arc<KBotProvider>, model_runner: Arc<ModelRunner>, dt: u64) -> Self {
        assert!(dt > TRIGGER_READ_BEFORE.as_millis() as u64);

        Self {
            model_provider,
            model_runner,
            dt: Duration::from_millis(dt),
            slowdown_factor: 1,
            magnitude_factor: 1.0,
            running: Arc::new(AtomicBool::new(false)),
            runtime: None,
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

        let runtime = Runtime::new()?;
        running.store(true, Ordering::Relaxed);

        runtime.spawn(async move {
            info!("Starting model runtime");

            // Moves to home position.
            model_provider.move_to_home().await?;

            // Wait for user to press enter
            println!("Press enter to start...");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();

            for i in 1..5 {
                println!("Starting in {} seconds...", 5 - i);
                sleep(Duration::from_secs(1)).await;
            }

            let mut carry = model_runner
                .init()
                .await
                .map_err(|e| ModelError::Provider(e.to_string()))?;
            let mut joint_positions = model_runner
                .get_joint_angles()
                .await
                .map_err(|e| ModelError::Provider(e.to_string()))?;

            // Wait for the first tick, since it happens immediately.
            let mut read_interval = interval(dt);
            let mut command_interval = interval(dt);

            // Start the two intervals N milliseconds apart. The first tick is
            // always instantaneous and represents the start of the interval
            // ticks.
            read_interval.tick().await;
            sleep(dt - TRIGGER_READ_BEFORE).await;
            command_interval.tick().await;

            info!("Entering main control loop");
            while running.load(Ordering::Relaxed) {
                let uuid = uuid::Uuid::new_v4();
                let uuid_main_control_loop = uuid::Uuid::new_v4();
                debug!("runtime::model_runner_step::START uuid={}", uuid);
                debug!("runtime::main_control_loop::START uuid={}", uuid_main_control_loop);

                let (output, next_carry) = model_runner
                    .step(carry)
                    .await
                    .map_err(|e| ModelError::Provider(e.to_string()))?;
                carry = next_carry;
                debug!("runtime::model_runner_step::END uuid={}", uuid);

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
                    read_interval.tick().await;
                    model_provider.trigger_actuator_read().await?;
                    command_interval.tick().await;
                }

                joint_positions = output;
                debug!("runtime::main_control_loop::END uuid={}", uuid_main_control_loop);
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
