use ::clap::Parser;
use ::kinfer::model::ModelRunner;
use ::std::path::Path;
use ::std::sync::Arc;
use kinfer_kbot::initialize_file_and_console_logging;

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
    /// File logging
    #[arg(long, default_value = "false")]
    file_logging: bool,
    /// Go to zero
    #[arg(long, default_value = "false")]
    go_to_zero: bool,
    /// JSON logging
    #[arg(long)]
    json_logging: Option<String>,  // Path to JSON log file
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

    // Just prepare the keyboard info (but don't start anything yet)
    if args.keyboard_commands {
        keyboard::prepare_keyboard_listener().await?;
    }

    let model_provider =
        Arc::new(KBotProvider::new(args.torque_enabled, args.torque_scale, args.go_to_zero, args.json_logging).await?);
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

    // Wait for either Ctrl-C signal OR keyboard ESC signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nCtrl+C received - shutting down gracefully...");
        }
        _ = async {
            // Poll for keyboard shutdown signal
            if args.keyboard_commands {
                while !keyboard::is_shutdown_requested() {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
                println!("ESC shutdown signal received");
            } else {
                // If no keyboard, just wait (until Ctrl+C)
                std::future::pending::<()>().await;
            }
        } => {}
    }

    // Stop the model runtime and cleanup
    println!("Stopping model runtime...");
    model_runtime.stop();

    if args.keyboard_commands {
        println!("Cleaning up keyboard...");
        keyboard::cleanup_keyboard();
    }

    println!("Graceful shutdown complete");
    Ok(())
}
