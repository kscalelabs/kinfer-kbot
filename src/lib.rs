use async_trait::async_trait;
use kinfer::{ModelError, ModelProvider};
use ndarray::{Array, IxDyn};

pub struct KBotProvider;

#[async_trait]
impl ModelProvider for KBotProvider {
    async fn get_joint_angles(
        &self,
        _joint_names: &[String],
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        Err(ModelError::Provider("Not implemented".to_string()))
    }

    async fn get_joint_angular_velocities(
        &self,
        _joint_names: &[String],
    ) -> Result<Array<f32, IxDyn>, ModelError> {
        Err(ModelError::Provider("Not implemented".to_string()))
    }

    async fn get_projected_gravity(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        Err(ModelError::Provider("Not implemented".to_string()))
    }

    async fn get_accelerometer(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        Err(ModelError::Provider("Not implemented".to_string()))
    }

    async fn get_gyroscope(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        Err(ModelError::Provider("Not implemented".to_string()))
    }

    async fn get_command(&self) -> Result<Array<f32, IxDyn>, ModelError> {
        Err(ModelError::Provider("Not implemented".to_string()))
    }

    async fn get_carry(&self, _carry: Array<f32, IxDyn>) -> Result<Array<f32, IxDyn>, ModelError> {
        Err(ModelError::Provider("Not implemented".to_string()))
    }

    async fn take_action(
        &self,
        _joint_names: Vec<String>,
        _action: Array<f32, IxDyn>,
    ) -> Result<(), ModelError> {
        Err(ModelError::Provider("Not implemented".to_string()))
    }
}
