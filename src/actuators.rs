use eyre::Result;
use robstride::{
    ActuatorConfiguration, ActuatorType, CH341Transport, ControlConfig, SocketCanTransport,
    Supervisor, TransportType,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::trace;

#[cfg(feature = "json_logging")]
use robstride::JsonLogger;

#[derive(Clone, Copy, Debug)]
pub struct ActuatorCommand {
    pub actuator_id: u32,
    pub position: Option<f64>,
    pub velocity: Option<f64>,
    pub torque: Option<f64>,
}

pub struct ConfigureRequest {
    pub actuator_id: u32,
    pub kp: Option<f32>,
    pub kd: Option<f32>,
    pub max_torque: Option<f32>,
    pub torque_enabled: Option<bool>,
    pub zero_position: Option<bool>,
    pub new_actuator_id: Option<u32>,
    pub max_velocity: Option<f32>,
    pub max_current: Option<f32>,
}

pub struct ActionResult {
    pub actuator_id: u32,
    pub success: bool,
    pub error: Option<String>,
}

pub struct ActionResponse {
    pub success: bool,
    pub error: Option<String>,
}

pub struct ActuatorState {
    pub actuator_id: u32,
    pub position: Option<f64>,
    pub velocity: Option<f64>,
    pub torque: Option<f64>,
    pub temperature: Option<f64>,
    pub online: bool,
}

pub struct Actuator {
    supervisor: Arc<Mutex<Supervisor>>,
    #[cfg(feature = "json_logging")]
    _json_logger: Option<Arc<JsonLogger>>,
}

impl Actuator {
    pub async fn new(
        ports: Vec<&str>,
        actuator_timeout: Duration,
        actuators_config: &[(u8, ActuatorConfiguration)],
        json_logging_path: Option<String>,
    ) -> Result<Self> {
        let mut supervisor = Supervisor::new(actuator_timeout)?;
        let mut found_motors = vec![false; actuators_config.len()];

        // Initialize transports for each port
        for port in &ports {
            let transport = match port {
                p if p.starts_with("/dev/tty") => {
                    let serial = CH341Transport::new(p.to_string()).await?;
                    TransportType::CH341(serial)
                }
                p if p.starts_with("can") => {
                    let can = SocketCanTransport::new(p.to_string()).await?;
                    TransportType::SocketCAN(can)
                }
                _ => return Err(eyre::eyre!("Invalid port: {}", port)),
            };

            supervisor
                .add_transport(port.to_string(), transport)
                .await?;
        }

        // Scan for motors on each port
        for port in &ports {
            let discovered_ids = supervisor.scan_bus(0xFD, port, actuators_config).await?;
            tracing::info!("Discovered IDs on {}: {:?}", port, discovered_ids);

            // Find unknown IDs by comparing against configured IDs
            let configured_ids: Vec<_> = actuators_config.iter().map(|(id, _)| id).collect();
            let unknown_ids: Vec<_> = discovered_ids
                .iter()
                .filter(|id| !configured_ids.contains(id))
                .collect();

            if !unknown_ids.is_empty() {
                tracing::warn!(
                    "Unknown motor IDs discovered on port {}: {:?}",
                    port,
                    unknown_ids
                );
            }

            // Mark found configured motors
            for (idx, (motor_id, _)) in actuators_config.iter().enumerate() {
                if discovered_ids.contains(motor_id) {
                    found_motors[idx] = true;
                }
            }
        }

        // Log warnings for missing motors
        for (idx, (motor_id, config)) in actuators_config.iter().enumerate() {
            if !found_motors[idx] {
                tracing::warn!(
                    "Configured motor not found - ID: {}, Type: {:?}",
                    motor_id,
                    config.actuator_type
                );
            }
        }

        // Initialize JSON logging if requested
        #[cfg(feature = "json_logging")]
        let json_logger = if let Some(log_path) = json_logging_path {
            match JsonLogger::new(
                log_path,
                1000,                                    // Buffer size
                Duration::from_millis(100),             // Flush interval
            ).await {
                Ok(logger) => {
                    tracing::info!("JSON logging enabled");
                    let logger_arc = Arc::new(logger);
                    
                    // Enable JSON logging on the supervisor
                    supervisor.enable_json_logging(logger_arc.clone()).await?;
                    
                    Some(logger_arc)
                }
                Err(e) => {
                    tracing::error!("Failed to initialize JSON logger: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let actuator = Self {
            supervisor: Arc::new(Mutex::new(supervisor)),
            #[cfg(feature = "json_logging")]
            _json_logger: json_logger,
        };

        Ok(actuator)
    }

    pub async fn command_actuators(
        &self,
        commands: Vec<ActuatorCommand>,
    ) -> Result<Vec<ActionResult>> {
        let uuid = uuid::Uuid::new_v4();
        trace!("actuator::command_actuators::START uuid={}", uuid);
        let mut results = vec![];
        let mut supervisor = self.supervisor.lock().await;

        for command in commands {
            let motor_id = command.actuator_id as u8;
            let result = supervisor
                .command(
                    motor_id,
                    command.position.map(|p| p as f32).ok_or(eyre::eyre!(
                        "No position specified for actuator {}",
                        command.actuator_id
                    ))?,
                    command.velocity.map(|v| v as f32).unwrap_or(0.0), // We assume default target velocity is 0 if not specified
                    command.torque.map(|t| t as f32).unwrap_or(0.0), // We assume default target torque is 0 if not specified
                )
                .await;

            results.push(ActionResult {
                actuator_id: command.actuator_id,
                success: result.is_ok(),
                error: result.err().map(|e| e.to_string()),
            });
        }
        trace!("actuator::command_actuators::END uuid={}", uuid);
        Ok(results)
    }

    pub async fn configure_actuator(&self, config: ConfigureRequest) -> Result<ActionResponse> {
        let uuid = uuid::Uuid::new_v4();
        trace!("actuator::configure_actuator::START uuid={}", uuid);
        let motor_id = config.actuator_id as u8;
        let mut supervisor = self.supervisor.lock().await;

        let control_config = ControlConfig {
            kp: config.kp.unwrap_or(0.0), // We assume default kp is 0 if not specified
            kd: config.kd.unwrap_or(0.0), // We assume default kd is 0 if not specified
            max_torque: config.max_torque,
            max_velocity: config.max_velocity,
            max_current: config.max_current,
        };

        let result = supervisor.configure(motor_id, control_config).await;

        if let Some(torque_enabled) = config.torque_enabled {
            if torque_enabled {
                supervisor.enable(motor_id).await?;
            } else {
                supervisor.disable(motor_id, true).await?;
            }
        }

        if let Some(true) = config.zero_position {
            supervisor.zero(motor_id).await?;
        }

        if let Some(new_id) = config.new_actuator_id {
            supervisor.change_id(motor_id, new_id as u8).await?;
        }

        trace!("actuator::configure_actuator::END uuid={}", uuid);
        Ok(ActionResponse {
            success: result.is_ok(),
            error: result.err().map(|e| e.to_string()),
        })
    }

    pub async fn trigger_actuator_read(&self, actuator_ids: Vec<u32>) -> Result<()> {
        let uuid = uuid::Uuid::new_v4();
        trace!("actuator::trigger_actuator_read::START uuid={}", uuid);
        let supervisor = self.supervisor.lock().await;
        for id in actuator_ids {
            supervisor.request_feedback(id as u8).await?;
        }
        trace!("actuator::trigger_actuator_read::END uuid={}", uuid);
        Ok(())
    }

    pub async fn get_actuators_state(&self, actuator_ids: Vec<u32>) -> Result<Vec<ActuatorState>> {
        let uuid = uuid::Uuid::new_v4();
        trace!("actuator::get_actuators_state::START uuid={}", uuid);
        let mut responses = vec![];

        // Reads the latest feedback from each actuator.
        let supervisor = self.supervisor.lock().await;
        for id in actuator_ids {
            if let Ok(Some((feedback, _))) = supervisor.get_feedback(id as u8).await {
                responses.push(ActuatorState {
                    actuator_id: id,
                    online: true,
                    position: Some(feedback.angle as f64),
                    velocity: Some(feedback.velocity as f64),
                    torque: Some(feedback.torque as f64),
                    temperature: Some(feedback.temperature as f64),
                });
            } else {
                tracing::warn!("No feedback or error for actuator ID: {}", id);
                responses.push(ActuatorState {
                    actuator_id: id,
                    online: false,
                    position: None,
                    velocity: None,
                    torque: None,
                    temperature: None,
                });
            }
        }
        trace!("actuator::get_actuators_state::END uuid={}", uuid);
        Ok(responses)
    }

    pub fn create_kbot_actuators() -> Vec<(u8, ActuatorConfiguration)> {
        let max_angle_change = 5.0f32; // Percent
        let max_velocity = 10.0f32.to_radians();
        let command_rate_hz = 50.0;

        vec![
            // Left Arm (11-16)
            (
                11,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride03,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                12,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride03,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                13,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride02,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                14,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride02,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                15,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride02,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            // (
            //     16,
            //     ActuatorConfiguration {
            //         actuator_type: ActuatorType::RobStride00,
            //         max_angle_change: Some(30.0f32.to_radians()),
            //         max_velocity: Some(10.0f32.to_radians()),
            //     },
            // ),
            // Right Arm (21-26)
            (
                21,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride03,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                22,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride03,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                23,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride02,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                24,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride02,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                25,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride02,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            // (
            //     26,
            //     ActuatorConfiguration {
            //         actuator_type: ActuatorType::RobStride00,
            //         max_angle_change: Some(30.0f32.to_radians()),
            //         max_velocity: Some(10.0f32.to_radians()),
            //     },
            // ),
            // Left Leg (31-35)
            (
                31,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride04,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                32,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride03,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                33,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride03,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                34,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride04,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                35,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride02,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            // Right Leg (41-45)
            (
                41,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride04,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                42,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride03,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                43,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride03,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                44,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride04,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
            (
                45,
                ActuatorConfiguration {
                    actuator_type: ActuatorType::RobStride02,
                    max_angle_change: Some(max_angle_change),
                    max_velocity: Some(max_velocity),
                    command_rate_hz: Some(command_rate_hz),
                },
            ),
        ]
    }
}
