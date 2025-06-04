use ::eyre::Result;
use imu::Vector3;
use ::imu::{HiwonderReader, ImuReader, Quaternion};
use ::std::time::Duration;
use ::tracing::{error, info, trace};

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
    pub quat: Quaternion,
}

const EPS: f32 = 1e-6;

pub fn quat_to_euler(quat: Quaternion) -> Vector3 {

    let magnitude = (quat.w * quat.w + quat.x * quat.x + quat.y * quat.y + quat.z * quat.z).sqrt();
    

    let normalized_quat = Quaternion {
        w: quat.w / (magnitude + EPS),
        x: quat.x / (magnitude + EPS),
        y: quat.y / (magnitude + EPS),
        z: quat.z / (magnitude + EPS),
    };

    let roll = (2.0 * (normalized_quat.w * normalized_quat.x + normalized_quat.y * normalized_quat.z))
        .atan2(1.0 - 2.0 * (normalized_quat.x * normalized_quat.x + normalized_quat.y * normalized_quat.y));
    
    let pitch = (2.0 * (normalized_quat.w * normalized_quat.y - normalized_quat.z * normalized_quat.x)).asin();
    
    let yaw = (2.0 * (normalized_quat.w * normalized_quat.z + normalized_quat.x * normalized_quat.y))
        .atan2(1.0 - 2.0 * (normalized_quat.y * normalized_quat.y + normalized_quat.z * normalized_quat.z));

    Vector3::new(roll, pitch, yaw)
}

pub fn rotate_quat(quat1: Quaternion, quat2: Quaternion) -> Quaternion {
    let w1 = quat1.w;
    let x1 = quat1.x;
    let y1 = quat1.y;
    let z1 = quat1.z;
    
    let w2 = quat2.w;
    let x2 = quat2.x;
    let y2 = quat2.y;
    let z2 = quat2.z;
    
    Quaternion {
        w: w1*w2 - x1*x2 - y1*y2 - z1*z2,
        x: w1*x2 + x1*w2 + y1*z2 - z1*y2,
        y: w1*y2 - x1*z2 + y1*w2 + z1*x2,
        z: w1*z2 + x1*y2 - y1*x2 + z1*w2,
    }
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
        trace!("imu::get_values::START uuid={}", uuid);
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
        trace!("imu::get_values::END uuid={}", uuid);
        Ok(IMUData {
            accel_x: accel.x,
            accel_y: accel.y,
            accel_z: accel.z,
            gyro_x: gyro.x,
            gyro_y: gyro.y,
            gyro_z: gyro.z,
            quat,
        })
    }
}
