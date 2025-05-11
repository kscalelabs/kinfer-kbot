use eyre::Result;
use imu::{HiwonderReader, ImuFrequency, ImuReader};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::task::JoinHandle;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct ImuValues {
    pub accel_x: f64,
    pub accel_y: f64,
    pub accel_z: f64,
    pub gyro_x: f64,
    pub gyro_y: f64,
    pub gyro_z: f64,
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
    pub quaternion_x: f64,
    pub quaternion_y: f64,
    pub quaternion_z: f64,
    pub quaternion_w: f64,
}

impl Default for ImuValues {
    fn default() -> Self {
        ImuValues {
            accel_x: 0.0,
            accel_y: 0.0,
            accel_z: 0.0,
            gyro_x: 0.0,
            gyro_y: 0.0,
            gyro_z: 0.0,
            roll: 0.0,
            pitch: 0.0,
            yaw: 0.0,
            quaternion_x: 0.0,
            quaternion_y: 0.0,
            quaternion_z: 0.0,
            quaternion_w: 0.0,
        }
    }
}

pub struct IMU {
    data: Arc<RwLock<ImuValues>>,
    _background_task: JoinHandle<()>, // Keep handle to prevent task from being dropped
}

impl IMU {
    pub async fn new(interfaces: &[&str], baud_rate: u32) -> Result<Self> {
        if interfaces.is_empty() {
            return Err(eyre::eyre!("No interfaces provided"));
        }

        // Initialize IMU hardware
        let mut imu_hardware = None;
        for interface in interfaces {
            info!(
                "Attempting to initialize KBotIMU with interface: {} at {} baud",
                interface, baud_rate
            );

            match HiwonderReader::new(interface, baud_rate, Duration::from_secs(1), true) {
                Ok(mut imu) => {
                    info!("Successfully created IMU reader on {}", interface);
                    if let Err(e) = imu.set_frequency(ImuFrequency::Hz100, Duration::from_secs(1)) {
                        error!("Failed to set IMU frequency: {}", e);
                        continue;
                    }
                    imu_hardware = Some(imu);
                    break;
                }
                Err(e) => {
                    error!("Failed to create IMU reader on {}: {}", interface, e);
                    continue;
                }
            }
        }

        let imu_hardware = imu_hardware
            .ok_or_else(|| eyre::eyre!("Failed to initialize IMU on any provided interface"))?;

        let data = Arc::new(RwLock::new(ImuValues::default()));
        let data_clone = data.clone();

        // Spawn background task to continuously read IMU values
        let background_task = tokio::spawn(async move {
            let mut read_errors = 0;
            loop {
                match imu_hardware.get_data() {
                    Ok(data) => {
                        if let Ok(mut imu_data) = data_clone.write() {
                            let accel = match data.accelerometer {
                                Some(accel) => accel,
                                None => {
                                    error!("Accelerometer data not available");
                                    continue;
                                }
                            };
                            let gyro = match data.gyroscope {
                                Some(gyro) => gyro,
                                None => {
                                    error!("Gyroscope data not available");
                                    continue;
                                }
                            };
                            let angle = match data.euler {
                                Some(angle) => angle,
                                None => {
                                    error!("Euler angles data not available");
                                    continue;
                                }
                            };
                            let quat = match data.quaternion {
                                Some(quat) => quat,
                                None => {
                                    error!("Quaternion data not available");
                                    continue;
                                }
                            };
                            imu_data.accel_x = accel.x as f64;
                            imu_data.accel_y = accel.y as f64;
                            imu_data.accel_z = accel.z as f64;
                            imu_data.gyro_x = gyro.x as f64;
                            imu_data.gyro_y = gyro.y as f64;
                            imu_data.gyro_z = gyro.z as f64;
                            imu_data.roll = angle.x as f64;
                            imu_data.pitch = angle.y as f64;
                            imu_data.yaw = angle.z as f64;
                            imu_data.quaternion_w = quat.w as f64;
                            imu_data.quaternion_x = quat.x as f64;
                            imu_data.quaternion_y = quat.y as f64;
                            imu_data.quaternion_z = quat.z as f64;
                        }
                        read_errors = 0;
                    }
                    Err(e) => {
                        read_errors += 1;
                        error!("Error reading from IMU: {} (count: {})", e, read_errors);
                        if read_errors > 100 {
                            error!("Too many IMU read errors, stopping background task");
                            break;
                        }
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        });

        Ok(Self {
            data,
            _background_task: background_task,
        })
    }

    pub fn get_values(&self) -> Result<ImuValues> {
        self.data
            .read()
            .map_err(|e| eyre::eyre!("Failed to read IMU data: {}", e))
            .map(|data| data.clone())
    }
}
