use clap::Parser;
use kinfer::model::ModelRunner;
use kinfer::runtime::ModelRuntime;
use kinfer_kbot::initialize_logging;
use kinfer_kbot::KBotProvider;
use std::path::Path;
use std::sync::Arc;

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize_logging();

    let args = Args::parse();
    let model_path = Path::new(&args.model_path);

    let model_runner = ModelRunner::new(model_path, Arc::new(KBotProvider)).await?;

    // Initialize and start the model runtime.
    let mut model_runtime = ModelRuntime::new(Arc::new(model_runner), args.dt);
    model_runtime.set_slowdown_factor(args.slowdown_factor);
    model_runtime.set_magnitude_factor(args.magnitude_factor);
    model_runtime.start()?;

    // Wait for the Ctrl-C signal
    tokio::signal::ctrl_c().await?;

    // Stop the model runtime
    model_runtime.stop();

    Ok(())
}
