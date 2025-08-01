use ::tracing_subscriber::FmtSubscriber;
use std::io::{self, Write};
use time::macros::format_description;
use tracing::info;
use tracing_subscriber::{
    fmt::{format::FmtSpan, time::UtcTime},
    prelude::*,
    EnvFilter,
};

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

pub fn initialize_file_and_console_logging() {
    let log_dir = std::path::PathBuf::from("logs");
    std::fs::create_dir_all(&log_dir).expect("Failed to create logs directory");

    let file_appender = tracing_appender::rolling::never(log_dir, "kbot.log");

    let timer = UtcTime::new(format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z"
    ));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_timer(timer.clone())
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_filter(EnvFilter::new("info,kinfer_kbot=debug"));

    let console_layer = tracing_subscriber::fmt::layer()
        .with_timer(timer)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(true)
        .with_writer(|| CrWriter(std::io::stderr()))
        .with_filter(EnvFilter::new("info"));

    let subscriber = tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer);

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    info!("Logging initialized - writing to logs/kbot.log");
}

#[doc(hidden)]
/// Wraps a writer and automatically inserts `\r` in front of every `\n`.
struct CrWriter<W: Write>(W);

impl<W: Write> Write for CrWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut start = 0;
        for (i, b) in buf.iter().enumerate() {
            if *b == b'\n' {
                if i > start {
                    self.0.write_all(&buf[start..i])?;
                }
                self.0.write_all(b"\r\n")?; // explicit CR before LF
                start = i + 1;
            }
        }
        if start < buf.len() {
            self.0.write_all(&buf[start..])?;
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
