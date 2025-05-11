pub mod actuators;
pub mod imu;

use actuators::{Actuator, ActuatorCommand};
use async_trait::async_trait;
use imu::IMU;
use kinfer::{ModelError, ModelProvider};
use ndarray::{Array, IxDyn};
use robstride::Supervisor;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing_subscriber::FmtSubscriber;

const ACTUATOR_NAME_TO_ID: &[(&str, u32)] = &[
    ("dof_left_shoulder_pitch_03", 11),
    ("dof_left_shoulder_roll_03", 12),
    ("dof_left_shoulder_yaw_02", 13),
    ("dof_left_elbow_02", 14),
    ("dof_left_wrist_00", 15),
    ("dof_right_shoulder_pitch_03", 21),
    ("dof_right_shoulder_roll_03", 22),
    ("dof_right_shoulder_yaw_02", 23),
    ("dof_right_elbow_02", 24),
    ("dof_right_wrist_00", 25),
    ("dof_left_hip_pitch_04", 31),
    ("dof_left_hip_roll_03", 32),
    ("dof_left_hip_yaw_03", 33),
    ("dof_left_knee_04", 34),
    ("dof_left_ankle_02", 35),
    ("dof_right_hip_pitch_04", 41),
    ("dof_right_hip_roll_03", 42),
    ("dof_right_hip_yaw_03", 43),
    ("dof_right_knee_04", 44),
    ("dof_right_ankle_02", 45),
];

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
            imu::IMU::new(&["/dev/ttyUSB0", "/dev/ttyCH341USB0"], 9600),
            actuators::Actuator::new(
                vec!["can0", "can1", "can2", "can3", "can4"],
                Duration::from_millis(100),
                Duration::from_millis(20),
                &kbot_actuators,
            )
        )?;

        // Disable torque on all actuators
        for id in &kbot_actuator_ids {
            let row = constants::ACTUATOR_KP_KD
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
                        torque_enabled: Some(torque_enabled),
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
                    .find(|(_, id)| *name == **id)
                    .map(|(_, id)| *id)
            })
            .collect::<Vec<_>>();
        let actuator_state = self.actuators.get_actuators_state(actuator_ids).await?;
        let joint_angles = actuator_state
            .iter()
            .map(|state| state.position)
            .collect::<Vec<_>>();
        Ok(Array::from_shape_vec((joint_names.len(),), joint_angles)?)
    }

    async fn get_joint_angular_velocities(
        &self,
        _joint_names: &[String],
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        let actuator_ids = _joint_names
            .iter()
            .map(|name| {
                ACTUATOR_NAME_TO_ID
                    .iter()
                    .find(|(_, id)| *name == **id)
                    .map(|(_, id)| *id)
            })
            .collect::<Vec<_>>();
        let actuator_state = self.actuators.get_actuators_state(actuator_ids).await?;
        let joint_angular_velocities = actuator_state
            .iter()
            .map(|state| state.velocity)
            .collect::<Vec<_>>();
        Ok(Array::from_shape_vec(
            (joint_names.len(),),
            joint_angular_velocities,
        )?)
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
        let values = self.imu.get_values()?;
        let accel_x = values.accel_x as f32;
        let accel_y = values.accel_y as f32;
        let accel_z = values.accel_z as f32;
        Ok(Array::from_shape_vec((3,), [accel_x, accel_y, accel_z])?)
    }

    async fn get_gyroscope(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        let values = self.imu.get_values()?;
        let gyro_x = values.gyro_x as f32;
        let gyro_y = values.gyro_y as f32;
        let gyro_z = values.gyro_z as f32;
        Ok(Array::from_shape_vec((3,), [gyro_x, gyro_y, gyro_z])?)
    }

    async fn get_command(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        Err(ModelError::Provider("Not implemented".to_string()))
    }

    async fn get_carry(&self, carry: Array<f32, IxDyn>) -> Result<Array<f32, IxDyn>, ModelError> {
        Ok(carry);
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

        let actuator_ids = joint_names
            .iter()
            .map(|name| {
                ACTUATOR_NAME_TO_ID
                    .iter()
                    .find(|(_, id)| *name == **id)
                    .map(|(_, id)| *id)
            })
            .collect::<Vec<_>>();

        self.actuators
            .command_actuators(
                joint_names
                    .iter()
                    .zip(action.iter())
                    .map(|(name, action)| ActuatorCommand {
                        actuator_id: ACTUATOR_NAME_TO_ID
                            .iter()
                            .find(|(_, id)| *name == **id)
                            .map(|(_, id)| *id)?,
                        position: Some(*action),
                        velocity: None,
                        torque: None,
                    })
                    .collect::<Vec<_>>(),
            )
            .await?;

        Ok(())
    }
}

pub fn initialize_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Setting default subscriber failed");
}
