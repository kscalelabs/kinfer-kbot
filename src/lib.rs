pub mod actuators;
pub mod constants;
pub mod imu;

use actuators::{Actuator, ActuatorCommand};
use async_trait::async_trait;
use constants::{ACTUATOR_KP_KD, ACTUATOR_NAME_TO_ID};
use imu::IMU;
use kinfer::{ModelError, ModelProvider};
use ndarray::{Array, IxDyn};
use std::time::Duration;
use tracing_subscriber::FmtSubscriber;

pub struct KBotProvider {
    actuators: Actuator,
    imu: IMU,
    dry_run: bool,
}

impl KBotProvider {
    pub async fn new(dry_run: bool) -> Result<Self, ModelError> {
        let kbot_actuators = actuators::Actuator::create_kbot_actuators();
        let kbot_actuator_ids = kbot_actuators.iter().map(|(id, _)| *id).collect::<Vec<_>>();

        let (imu, actuators) = tokio::try_join!(
            imu::IMU::new(&["/dev/ttyUSB0", "/dev/ttyCH341USB0"], 230400),
            actuators::Actuator::new(
                vec!["can0", "can1", "can2", "can3", "can4"],
                Duration::from_millis(100),
                Duration::from_millis(20),
                &kbot_actuators,
            )
        )
        .map_err(|e| ModelError::Provider(e.to_string()))?;

        // Disable torque on all actuators
        for id in &kbot_actuator_ids {
            let row = ACTUATOR_KP_KD
                .iter()
                .find(|(i, _, _, _)| *i == *id as usize);
            if let Some(row) = row {
                let kp = row.1;
                let kd = row.2;
                let max_torque = row.3;
                if let Err(e) = actuators
                    .configure_actuator(actuators::ConfigureRequest {
                        actuator_id: *id as u32,
                        kp: Some(kp as f64),
                        kd: Some(kd as f64),
                        max_torque: Some(max_torque as f64),
                        torque_enabled: Some(!dry_run),
                        zero_position: None,
                        new_actuator_id: None,
                    })
                    .await
                {
                    tracing::warn!("Failed to configure torque on actuator {}: {}", id, e);
                }
            } else {
                tracing::warn!("No kp and kd found for actuator {}", id);
            }
        }

        Ok(Self {
            actuators,
            imu,
            dry_run,
        })
    }
}

#[async_trait]
impl ModelProvider for KBotProvider {
    async fn get_joint_angles(
        &self,
        joint_names: &[String],
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        // TODO: Instead of just polling the position from the supervisor,
        // we should trigger the feedback command, wait for some amount of time,
        // and then read the position. We need to make sure that we only
        // trigger the feedback command once for both `get_joint_angles` and
        // `get_joint_angular_velocities` on each call.`
        let actuator_ids = joint_names
            .iter()
            .map(|name| {
                ACTUATOR_NAME_TO_ID
                    .iter()
                    .find(|(const_name, _)| *name == *const_name)
                    .map(|(_, id)| *id)
                    .ok_or_else(|| ModelError::Provider(format!("Joint name not found: {}", name)))
            })
            .collect::<Result<Vec<u32>, _>>()?;
        let actuator_state = self
            .actuators
            .get_actuators_state(actuator_ids)
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        let joint_angles = actuator_state
            .iter()
            .map(|state| {
                state.position.map(|p| p as f32).ok_or_else(|| {
                    ModelError::Provider(format!(
                        "Position not available for joint: {}",
                        state.actuator_id
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Array::from_shape_vec((joint_names.len(),), joint_angles)
            .map_err(|e| ModelError::Provider(e.to_string()))?
            .into_dyn())
    }

    async fn get_joint_angular_velocities(
        &self,
        _joint_names: &[String],
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        let actuator_ids: Vec<u32> = _joint_names
            .iter()
            .map(|name| {
                ACTUATOR_NAME_TO_ID
                    .iter()
                    .find(|(const_name, _)| *name == *const_name)
                    .map(|(_, id)| *id)
                    .ok_or_else(|| ModelError::Provider(format!("Joint name not found: {}", name)))
            })
            .collect::<Result<Vec<u32>, _>>()?;

        let actuator_state = self
            .actuators
            .get_actuators_state(actuator_ids)
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        let joint_angular_velocities: Vec<f32> = actuator_state
            .iter()
            .enumerate()
            .map(|(idx, state)| {
                state.velocity.map(|v| v as f32).ok_or_else(|| {
                    let joint_name_for_error = _joint_names
                        .get(idx)
                        .map_or("<unknown joint>", |s| s.as_str());
                    ModelError::Provider(format!(
                        "Velocity data not available (is None) for joint: {}",
                        joint_name_for_error
                    ))
                })
            })
            .collect::<Result<Vec<f32>, ModelError>>()?;

        Ok(
            Array::from_shape_vec((_joint_names.len(),), joint_angular_velocities)
                .map_err(|e| ModelError::Provider(e.to_string()))?
                .into_dyn(),
        )
    }

    async fn get_projected_gravity(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        // TODO: Use the quaternion to get the projected gravity vector.
        Err(ModelError::Provider("Not implemented".to_string()))
    }

    async fn get_accelerometer(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        // TODO: Right now we are polling the IMU to get the accelerometer
        // values. Instead, we should manually trigger an IMU read, then read
        // the accelerometer values. We need to make sure that we only
        // trigger the accelerometer read once for `get_accelerometer`,
        // `get_gyroscope` and `get_projected_gravity` on each call.
        let values = self
            .imu
            .get_values()
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        let accel_x = values.accel_x as f32;
        let accel_y = values.accel_y as f32;
        let accel_z = values.accel_z as f32;
        Ok(Array::from_shape_vec((3,), vec![accel_x, accel_y, accel_z])
            .map_err(|e| ModelError::Provider(e.to_string()))?
            .into_dyn())
    }

    async fn get_gyroscope(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        let values = self
            .imu
            .get_values()
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        let gyro_x = values.gyro_x as f32;
        let gyro_y = values.gyro_y as f32;
        let gyro_z = values.gyro_z as f32;
        Ok(Array::from_shape_vec((3,), vec![gyro_x, gyro_y, gyro_z])
            .map_err(|e| ModelError::Provider(e.to_string()))?
            .into_dyn())
    }

    async fn get_command(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        Err(ModelError::Provider("Not implemented".to_string()))
    }

    async fn get_carry(&self, carry: Array<f32, IxDyn>) -> Result<Array<f32, IxDyn>, ModelError> {
        Ok(carry)
    }

    async fn take_action(
        &self,
        joint_names: Vec<String>,
        action: Array<f32, IxDyn>,
    ) -> Result<(), ModelError> {
        assert_eq!(joint_names.len(), action.len());

        if self.dry_run {
            return Ok(());
        }

        let commands: Vec<ActuatorCommand> = joint_names
            .iter()
            .zip(action.iter())
            .map(|(name, action_value)| {
                let id = ACTUATOR_NAME_TO_ID
                    .iter()
                    .find(|(const_name, _)| *name == *const_name)
                    .map(|(_, found_id)| *found_id)
                    .ok_or_else(|| {
                        ModelError::Provider(format!(
                            "Joint name not found in ACTUATOR_NAME_TO_ID: {}",
                            name
                        ))
                    })?;

                Ok(ActuatorCommand {
                    actuator_id: id,
                    position: Some(*action_value as f64),
                    velocity: None,
                    torque: None,
                })
            })
            .collect::<Result<Vec<ActuatorCommand>, ModelError>>()?;

        self.actuators
            .command_actuators(commands)
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?;

        Ok(())
    }
}

pub fn initialize_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Setting default subscriber failed");
}
