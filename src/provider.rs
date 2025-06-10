use ::async_trait::async_trait;
use ::imu::{Quaternion, Vector3};
use ::kinfer::{InputType, ModelError, ModelMetadata, ModelProvider};
use ::ndarray::{Array, IxDyn};
use ::std::collections::HashMap;
use ::std::time::{Duration, Instant};

use crate::actuators::{Actuator, ActuatorCommand, ActuatorState, ConfigureRequest};
use crate::constants::{ACTUATOR_KP_KD, ACTUATOR_NAME_TO_ID, HOME_POSITION};
use crate::imu::{quat_to_euler, rotate_quat, IMU};
use crate::keyboard;
use tracing::{debug, trace, info};

pub struct KBotProvider {
    actuators: Actuator,
    imu: IMU,
    start_time: Instant,
    initial_heading: f32,
}

impl KBotProvider {
    pub async fn new(torque_enabled: bool, torque_scale: f32) -> Result<Self, ModelError> {
        let kbot_actuators = Actuator::create_kbot_actuators();
        let kbot_actuator_ids = kbot_actuators.iter().map(|(id, _)| *id).collect::<Vec<_>>();

        let (imu, actuators) = tokio::try_join!(
            IMU::new(&["/dev/ttyIMU", "/dev/ttyCH341USB0"], 230400),
            Actuator::new(
                vec!["can0", "can1", "can2", "can3", "can4"],
                Duration::from_millis(100),
                &kbot_actuators,
            )
        )
        .map_err(|e| ModelError::Provider(e.to_string()))?;

        let initial_quat = imu
            .get_values()
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?
            .quat;
        let initial_heading = quat_to_euler(initial_quat).z;

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
            start_time: Instant::now(),
            initial_heading,
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

    pub async fn get_actuator_state(
        &self,
        actuator_ids: &[u32],
    ) -> Result<Vec<ActuatorState>, ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!("provider::get_actuator_state::START uuid={}", uuid);
        let result = self
            .actuators
            .get_actuators_state(actuator_ids.to_vec())
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        debug!("provider::get_actuator_state::END uuid={}", uuid);
        Ok(result)
    }

    pub async fn trigger_actuator_read(&self) -> Result<(), ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!("provider::trigger_actuator_read::START uuid={}", uuid);
        let actuator_ids = ACTUATOR_NAME_TO_ID
            .iter()
            .map(|(_, id)| *id)
            .collect::<Vec<u32>>();
        self.actuators
            .trigger_actuator_read(actuator_ids)
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        debug!("provider::trigger_actuator_read::END uuid={}", uuid);
        Ok(())
    }

    pub async fn move_to_home(&self) -> Result<(), ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!("provider::move_to_home::START uuid={}", uuid);
        let home_position = HOME_POSITION;
        let mut commands = vec![];
        for (id, position) in home_position {
            commands.push(ActuatorCommand {
                actuator_id: id as u32,
                position: Some(position as f64),
                velocity: None,
                torque: None,
            });
        }
        self.actuators
            .command_actuators(commands)
            .await
            .map_err(|e| ModelError::Provider(e.to_string()))?;
        debug!("provider::move_to_home::END uuid={}", uuid);
        Ok(())
    }
}

#[async_trait]
impl ModelProvider for KBotProvider {
    async fn get_inputs(
        &self,
        input_types: &[InputType],
        meta: &ModelMetadata,
    ) -> Result<HashMap<InputType, Array<f32, IxDyn>>, ModelError> {
        use InputType::*;
        let uuid = uuid::Uuid::new_v4();
        debug!("provider::get_inputs::START uuid={}", uuid);

        // Read values from hardware once
        let (act_state, imu_values) = tokio::try_join!(
            async {
                let actuator_ids = self.get_actuator_ids(&meta.joint_names)?;
                self.get_actuator_state(&actuator_ids).await
            },
            async {
                self.imu.get_values().await.map_err(|e| {
                    ModelError::Provider(format!("Failed to get IMU values: {}", e.to_string()))
                })
            }
        )?;

        // Populate the requested slots
        let mut out = HashMap::with_capacity(input_types.len());

        for t in input_types {
            match t {
                JointAngles => {
                    let arr = self.get_joint_angles_from_state(&meta.joint_names, &act_state)?;
                    out.insert(JointAngles, arr);
                }
                JointAngularVelocities => {
                    let arr = self
                        .get_joint_angular_velocities_from_state(&meta.joint_names, &act_state)?;
                    out.insert(JointAngularVelocities, arr);
                }
                Accelerometer => {
                    let arr = self.get_accelerometer_from_values(&imu_values)?;
                    out.insert(Accelerometer, arr);
                }
                InitialHeading => {
                    let arr = Array::from_shape_vec((1,), vec![self.initial_heading])
                        .map_err(|e| ModelError::Provider(e.to_string()))?
                        .into_dyn();
                    out.insert(InitialHeading, arr);
                }
                Quaternion => {
                    let arr = self.get_quat_from_values(&imu_values)?;
                    out.insert(Quaternion, arr);
                }
                Gyroscope => {
                    let arr = self.get_gyroscope_from_values(&imu_values)?;
                    out.insert(Gyroscope, arr);
                }
                ProjectedGravity => {
                    let arr = self.get_projected_gravity_from_values(&imu_values)?;
                    out.insert(ProjectedGravity, arr);
                }
                Time => {
                    let secs = self.start_time.elapsed().as_secs_f32();
                    let time_arr = Array::from_shape_vec((1,), vec![secs])
                        .map_err(|e| ModelError::Provider(e.to_string()))?
                        .into_dyn();
                    out.insert(Time, time_arr);
                }
                Command => {
                    out.insert(Command, self.get_command_internal(meta, &imu_values)?);
                }
                Carry => {
                    return Err(ModelError::Provider("Carry should come via step()".into()));
                }
            }
        }
        debug!("provider::get_inputs::END uuid={}", uuid);
        Ok(out)
    }

    async fn take_action(
        &self,
        action: Array<f32, IxDyn>,
        metadata: &ModelMetadata,
    ) -> Result<(), ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!("provider::take_action::START uuid={}", uuid);
        let joint_names_from_metadata = &metadata.joint_names;
        assert_eq!(joint_names_from_metadata.len(), action.len());

        let commands: Vec<ActuatorCommand> = joint_names_from_metadata
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

        tracing::debug!("took action {:?} at time {:?}", action, Instant::now());
        debug!("provider::take_action::END uuid={}", uuid);
        Ok(())
    }
}

impl KBotProvider {
    // Internal methods for getting specific input types
    fn get_joint_angles_from_state(
        &self,
        joint_names: &[String],
        actuator_state: &[ActuatorState],
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!("provider::get_joint_angles_from_state::START uuid={}", uuid);
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
        debug!("provider::get_joint_angles_from_state::END uuid={}", uuid);
        Ok(Array::from_shape_vec((joint_names.len(),), joint_angles)
            .map_err(|e| ModelError::Provider(e.to_string()))?
            .into_dyn())
    }

    fn get_joint_angular_velocities_from_state(
        &self,
        joint_names: &[String],
        actuator_state: &[ActuatorState],
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!(
            "provider::get_joint_angular_velocities_from_state::START uuid={}",
            uuid
        );
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
        debug!(
            "provider::get_joint_angular_velocities_from_state::END uuid={}",
            uuid
        );
        Ok(
            Array::from_shape_vec((joint_names.len(),), joint_angular_velocities)
                .map_err(|e| ModelError::Provider(e.to_string()))?
                .into_dyn(),
        )
    }

    fn get_projected_gravity_from_values(
        &self,
        imu_values: &crate::imu::IMUData,
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!(
            "provider::get_projected_gravity_from_values::START uuid={}",
            uuid
        );
        let projected_gravity = imu_values
            .quat
            .rotate_vector(Vector3::new(0.0, 0.0, -9.81), true);
        debug!(
            "provider::get_projected_gravity_from_values::END uuid={}",
            uuid
        );
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

    fn get_accelerometer_from_values(
        &self,
        imu_values: &crate::imu::IMUData,
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!(
            "provider::get_accelerometer_from_values::START uuid={}",
            uuid
        );
        let accel_x = imu_values.accel_x;
        let accel_y = imu_values.accel_y;
        let accel_z = imu_values.accel_z;
        debug!("provider::get_accelerometer_from_values::END uuid={}", uuid);
        Ok(Array::from_shape_vec((3,), vec![accel_x, accel_y, accel_z])
            .map_err(|e| ModelError::Provider(e.to_string()))?
            .into_dyn())
    }

    fn get_gyroscope_from_values(
        &self,
        imu_values: &crate::imu::IMUData,
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!("provider::get_gyroscope_from_values::START uuid={}", uuid);
        let gyro_x = imu_values.gyro_x;
        let gyro_y = imu_values.gyro_y;
        let gyro_z = imu_values.gyro_z;
        debug!("provider::get_gyroscope_from_values::END uuid={}", uuid);
        Ok(Array::from_shape_vec((3,), vec![gyro_x, gyro_y, gyro_z])
            .map_err(|e| ModelError::Provider(e.to_string()))?
            .into_dyn())
    }

    fn get_quat_from_values(
        &self,
        imu_values: &crate::imu::IMUData,
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!("provider::get_quat_from_values::START uuid={}", uuid);
        let quat = imu_values.quat;
        debug!("provider::get_quat_from_values::END uuid={}", uuid);
        Ok(
            Array::from_shape_vec((4,), vec![quat.w, quat.x, quat.y, quat.z])
                .map_err(|e| ModelError::Provider(e.to_string()))?
                .into_dyn(),
        )
    }

    fn get_command_internal(
        &self,
        metadata: &ModelMetadata,
        imu_values: &crate::imu::IMUData,
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        let uuid = uuid::Uuid::new_v4();
        debug!("provider::get_command_internal::START uuid={}", uuid);
        let num_commands = metadata.num_commands.unwrap_or(0);

        let result = match num_commands {
            3 => {
                let commands = keyboard::get_commands();
                let command_values = vec![commands[0], commands[1], commands[2]];

                Array::from_shape_vec((num_commands,), command_values)
                    .map_err(|e| ModelError::Provider(e.to_string()))?
                    .into_dyn()
            }
            6 => {
                let commands = keyboard::get_commands();
                // Skip yaw rate
                let command_values = vec![commands[0], commands[1], commands[3], commands[4], commands[5], commands[6]];

                Array::from_shape_vec((num_commands,), command_values)
                    .map_err(|e| ModelError::Provider(e.to_string()))?
                    .into_dyn()
            }
            7 => {
                let commands = keyboard::get_commands();

                let command_values = vec![
                    commands[0],
                    commands[1],
                    commands[2],
                    commands[3],
                    commands[4],
                    commands[5],
                    commands[6],
                ];

                info!("X: {}, Y: {}, Yaw_rate: {}, Yaw: {}", commands[0], commands[1], commands[2], commands[3]);

                Array::from_shape_vec((num_commands,), command_values)
                    .map_err(|e| ModelError::Provider(e.to_string()))?
                    .into_dyn()
            }
            _ => {
                let command_values = vec![0.0f32; num_commands];
                Array::from_shape_vec((num_commands,), command_values)
                    .map_err(|e| ModelError::Provider(e.to_string()))?
                    .into_dyn()
            }
        };

        debug!("provider::get_command_internal::END uuid={}", uuid);
        Ok(result)
    }
}
