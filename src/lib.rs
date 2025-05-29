use ::tracing_subscriber::FmtSubscriber;

pub mod actuators;
pub mod constants;
pub mod imu;
pub mod keyboard;
pub mod provider;
pub mod runtime;

pub fn initialize_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Setting default subscriber failed");
}
