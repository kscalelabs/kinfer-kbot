use crate::actuators::{Actuator, ActuatorCommand, ActuatorState, ConfigureRequest};
use crate::constants::{ACTUATOR_KP_KD, ACTUATOR_NAME_TO_ID};
use crate::imu::IMU;
use ::async_trait::async_trait;
use ::imu::{Quaternion, Vector3};
use ::kinfer::{ModelError, ModelProvider};
use ::ndarray::{Array, IxDyn};
use ::std::time::Duration;
use ::tokio::sync::Mutex;

pub struct KBotProvider {
    actuators: Actuator,
    imu: IMU,
    imu_read_lock: Mutex<()>,
}

impl KBotProvider {
    pub async fn new(torque_enabled: bool, torque_scale: f32) -> Result<Self, ModelError> {
        let kbot_actuators = Actuator::create_kbot_actuators();
        let kbot_actuator_ids = kbot_actuators.iter().map(|(id, _)| *id).collect::<Vec<_>>();

        let (imu, actuators) = tokio::try_join!(
            IMU::new(&["/dev/ttyUSB0", "/dev/ttyCH341USB0"], 230400),
            Actuator::new(
                vec!["can0", "can1", "can2", "can3", "can4"],
                Duration::from_millis(100),
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
                    .configure_actuator(ConfigureRequest {
                        actuator_id: *id as u32,
                        kp: Some(kp),
                        kd: Some(kd),
                        max_torque: Some(max_torque * torque_scale),
                        torque_enabled: Some(torque_enabled),
                        zero_position: None,
                        new_actuator_id: None,
                        max_velocity: None,
                        max_current: None,
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
            imu_read_lock: Mutex::new(()),
        })
    }

    fn get_actuator_ids(&self, joint_names: &[String]) -> Result<Vec<u32>, ModelError> {
        joint_names
            .iter()
            .map(|name| {
                ACTUATOR_NAME_TO_ID
                    .iter()
                    .find(|(const_name, _)| *name == *const_name)
                    .map(|(_, id)| *id)
                    .ok_or_else(|| ModelError::Provider(format!("Joint name not found: {}", name)))
            })
            .collect::<Result<Vec<u32>, _>>()
    }

    async fn get_actuator_state(
        &self,
        actuator_ids: &[u32],
    ) -> Result<Vec<ActuatorState>, ModelError> {
        self.actuators
            .get_actuators_state(actuator_ids.to_vec())
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))
    }

    pub async fn trigger_actuator_read(&self) -> Result<(), ModelError> {
        let actuator_ids = ACTUATOR_NAME_TO_ID
            .iter()
            .map(|(_, id)| *id)
            .collect::<Vec<u32>>();
        self.actuators
            .trigger_actuator_read(actuator_ids)
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl ModelProvider for KBotProvider {
    async fn get_joint_angles(
        &self,
        joint_names: &[String],
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        println!("pre 1");
        let actuator_ids = self.get_actuator_ids(joint_names)?;
        let actuator_state = self.get_actuator_state(&actuator_ids).await?;

        println!("get_joint_angles");

        let joint_angles = actuator_state
            .iter()
            .enumerate()
            .map(|(idx, state)| {
                state.position.map(|p| p as f32).ok_or_else(|| {
                    let joint_name_for_error = joint_names.get(idx).map_or_else(
                        || format!("<unknown joint at index {}>", idx),
                        |s: &String| s.to_string(),
                    );
                    ModelError::Provider(format!(
                        "Position not available for joint ID {} (name: {})",
                        state.actuator_id, joint_name_for_error
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
        joint_names: &[String],
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        println!("pre 2");
        let actuator_ids = self.get_actuator_ids(joint_names)?;
        let actuator_state = self.get_actuator_state(&actuator_ids).await?;

        println!("get_joint_anglular_velocities");

        let joint_angular_velocities: Vec<f32> = actuator_state
            .iter()
            .enumerate()
            .map(|(idx, state)| {
                state.velocity.map(|v| v as f32).ok_or_else(|| {
                    let joint_name_for_error = joint_names.get(idx).map_or_else(
                        || format!("<unknown joint at index {}>", idx),
                        |s: &String| s.to_string(),
                    );
                    ModelError::Provider(format!(
                        "Velocity data not available (is None) for joint ID {} (name: {})",
                        state.actuator_id, joint_name_for_error
                    ))
                })
            })
            .collect::<Result<Vec<f32>, ModelError>>()?;

        Ok(
            Array::from_shape_vec((joint_names.len(),), joint_angular_velocities)
                .map_err(|e| ModelError::Provider(e.to_string()))?
                .into_dyn(),
        )
    }

    async fn get_projected_gravity(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        let _guard = self.imu_read_lock.lock().await;
        println!("proj 1");
        let values = self
            .imu
            .get_values()
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        let projected_gravity = Quaternion {
            x: values.quat_x,
            y: values.quat_y,
            z: values.quat_z,
            w: values.quat_w,
        }
        .rotate_vector(Vector3::new(0.0, 0.0, -9.81), true);
        println!("proj 2");
        Ok(Array::from_shape_vec(
            (3,),
            vec![
                projected_gravity.x,
                projected_gravity.y,
                projected_gravity.z,
            ],
        )
        .map_err(|e| ModelError::Provider(e.to_string()))?
        .into_dyn())
    }

    async fn get_accelerometer(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        let _guard = self.imu_read_lock.lock().await;
        println!("acc 1");
        let values = self
            .imu
            .get_values()
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        let accel_x = values.accel_x as f32;
        let accel_y = values.accel_y as f32;
        let accel_z = values.accel_z as f32;
        println!("acc 2");
        Ok(Array::from_shape_vec((3,), vec![accel_x, accel_y, accel_z])
            .map_err(|e| ModelError::Provider(e.to_string()))?
            .into_dyn())
    }

    async fn get_gyroscope(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        let _guard = self.imu_read_lock.lock().await;
        println!("gyro 1");
        let values = self
            .imu
            .get_values()
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        let gyro_x = values.gyro_x as f32;
        let gyro_y = values.gyro_y as f32;
        let gyro_z = values.gyro_z as f32;
        println!("gyro 2");
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

        println!("taking action...");

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

        println!("took action");

        Ok(())
    }
}
