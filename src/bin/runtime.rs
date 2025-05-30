use ::clap::Parser;
use ::kinfer::model::ModelRunner;
use ::std::path::Path;
use ::std::sync::Arc;

use kinfer_kbot::initialize_logging;
use kinfer_kbot::keyboard;
use kinfer_kbot::provider::KBotProvider;
use kinfer_kbot::runtime::ModelRuntime;

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
    /// Enable keyboard commands
    #[arg(long, default_value = "false")]
    keyboard_commands: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize_logging();

    let args = Args::parse();
    let model_path = Path::new(&args.model_path);

    // Just prepare the keyboard info (but don't start anything yet)
    if args.keyboard_commands {
        keyboard::prepare_keyboard_listener().await?;
    }

    let model_provider = Arc::new(KBotProvider::new(args.torque_enabled, args.torque_scale).await?);
    let model_runner = ModelRunner::new(model_path, model_provider.clone()).await?;

    // Pass the keyboard_enabled flag to the runtime
    let mut model_runtime = ModelRuntime::new(
        model_provider,
        Arc::new(model_runner),
        args.dt,
        args.keyboard_commands,
    );
    model_runtime.set_slowdown_factor(args.slowdown_factor);
    model_runtime.set_magnitude_factor(args.magnitude_factor);

    model_runtime.start()?;

    tokio::signal::ctrl_c().await?;
    println!("\nCtrl+C received");

    // Stop the model runtime and cleanup
    model_runtime.stop();
    if args.keyboard_commands {
        keyboard::cleanup_keyboard();
    }

    Ok(())
}
