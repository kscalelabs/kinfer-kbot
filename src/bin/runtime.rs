use ::clap::Parser;
use ::kinfer::model::ModelRunner;
use ::std::path::Path;
use ::std::sync::Arc;

use kinfer_kbot::{
    initialize_file_and_console_logging, initialize_logging, provider::KBotProvider,
    runtime::ModelRuntime,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the model to run
    #[arg(long)]
    model_path: String,
    /// Duration of the model step in milliseconds
    #[arg(long, default_value_t = 20)]
    dt: u64,
    /// Slowdown factor
    #[arg(long, default_value_t = 1)]
    slowdown_factor: i32,
    /// Magnitude factor
    #[arg(long, default_value_t = 1.0)]
    magnitude_factor: f32,
    /// Torque enabled
    #[arg(long, default_value = "false")]
    torque_enabled: bool,
    /// Torque scale
    #[arg(long, default_value_t = 1.0)]
    torque_scale: f32,
    /// File logging
    #[arg(long, default_value = "false")]
    file_logging: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.file_logging {
        initialize_file_and_console_logging();
    } else {
        initialize_logging();
    }

    let model_path = Path::new(&args.model_path);

    let model_provider = Arc::new(KBotProvider::new(args.torque_enabled, args.torque_scale).await?);
    let model_runner = ModelRunner::new(model_path, model_provider.clone()).await?;

    // Initialize and start the model runtime.
    let mut model_runtime = ModelRuntime::new(model_provider, Arc::new(model_runner), args.dt);
    model_runtime.set_slowdown_factor(args.slowdown_factor);
    model_runtime.set_magnitude_factor(args.magnitude_factor);
    model_runtime.start()?;

    // Wait for the Ctrl-C signal
    tokio::signal::ctrl_c().await?;

    // Stop the model runtime
    model_runtime.stop();

    Ok(())
}
