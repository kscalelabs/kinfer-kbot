use ::eyre::Result;
use ::imu::{HiwonderOutput, HiwonderReader, ImuFrequency, ImuReader};
use ::std::time::Duration;
use ::tracing::{debug, trace, error, info};

const IMU_WRITE_TIMEOUT: Duration = Duration::from_secs(5);

pub struct IMU {
    imu_reader: HiwonderReader,
}

pub struct IMUData {
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub gyro_z: f32,
    pub quat_x: f32,
    pub quat_y: f32,
    pub quat_z: f32,
    pub quat_w: f32,
}

impl IMU {
    pub async fn new(interfaces: &[&str], baud_rate: u32) -> Result<Self> {
        if interfaces.is_empty() {
            return Err(eyre::eyre!("No interfaces provided"));
        }

        // Initialize IMU reader
        let mut imu_reader = None;
        for interface in interfaces {
            info!(
                "Attempting to initialize KBotIMU with interface: {} at {} baud",
                interface, baud_rate
            );

            match HiwonderReader::new(interface, baud_rate, Duration::from_secs(1), true) {
                Ok(imu) => {
                    info!("Successfully created IMU reader on {}", interface);
                    // info!("Setting and verifying params...");

                    // if let Err(e) = imu.set_output_mode(
                    //     HiwonderOutput::QUATERNION | HiwonderOutput::GYRO | HiwonderOutput::ACC,
                    //     IMU_WRITE_TIMEOUT,
                    // ) {
                    //     error!(
                    //         "Failed to set output mode for {}: {}. Params might be default.",
                    //         interface, e
                    //     );
                    // } else {
                    //     info!("Output mode set for {}", interface);
                    // }

                    // if let Err(e) = imu.set_frequency(ImuFrequency::Hz200, IMU_WRITE_TIMEOUT) {
                    //     error!(
                    //         "Failed to set frequency for {}: {}. Params might be default.",
                    //         interface, e
                    //     );
                    // } else {
                    //     info!("200Hz frequency set for {}", interface);
                    // }

                    // if let Err(e) = imu.set_bandwidth(42, IMU_WRITE_TIMEOUT) {
                    //     error!(
                    //         "Failed to set bandwidth for {}: {}. Params might be default.",
                    //         interface, e
                    //     );
                    // } else {
                    //     info!("Bandwidth set for {}", interface);
                    // }

                    imu_reader = Some(imu);
                    break;
                }
                Err(e) => {
                    error!("Failed to create IMU reader on {}: {}", interface, e);
                    continue;
                }
            }
        }

        let imu_reader = imu_reader
            .ok_or_else(|| eyre::eyre!("Failed to initialize IMU on any provided interface"))?;

        Ok(Self { imu_reader })
    }

    pub async fn get_values(&self) -> Result<IMUData> {
        let uuid = uuid::Uuid::new_v4();
        trace!("IMU log::get_values::START uuid={}", uuid);
        let direct_read = self.imu_reader.get_data()?;
        let accel = match direct_read.accelerometer {
            Some(accel) => accel,
            None => return Err(eyre::eyre!("Failed to read accelerometer")),
        };
        let gyro = match direct_read.gyroscope {
            Some(gyro) => gyro,
            None => return Err(eyre::eyre!("Failed to read gyroscope")),
        };
        let quat = match direct_read.quaternion {
            Some(quat) => quat,
            None => return Err(eyre::eyre!("Failed to read quaternion")),
        };
        trace!("IMU log::get_values::END uuid={}", uuid);
        Ok(IMUData {
            accel_x: accel.x,
            accel_y: accel.y,
            accel_z: accel.z,
            gyro_x: gyro.x,
            gyro_y: gyro.y,
            gyro_z: gyro.z,
            quat_x: quat.x,
            quat_y: quat.y,
            quat_z: quat.z,
            quat_w: quat.w,
        })
    }
}
