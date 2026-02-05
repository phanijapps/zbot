//! # AgentZero Daemon
//!
//! Standalone server for the agent runtime.
//!
//! ## Usage
//!
//! ```bash
//! # Start with defaults
//! zerod
//!
//! # Start with custom ports
//! zerod --ws-port 19000 --http-port 19001
//!
//! # Start with custom data directory
//! zerod --data-dir /path/to/agentzero
//!
//! # Start with config file
//! zerod --config /path/to/daemon.yaml
//!
//! # Serve web dashboard from static files
//! zerod --static-dir /path/to/dashboard/dist
//!
//! # Disable web dashboard
//! zerod --no-dashboard
//! ```

use anyhow::Result;
use clap::Parser;
use gateway::{GatewayConfig, GatewayServer};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt,
    fmt::writer::MakeWriterExt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// AgentZero Daemon - AI agent runtime server
#[derive(Parser, Debug)]
#[command(name = "zerod")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// WebSocket port
    #[arg(long, default_value_t = gateway::DEFAULT_WS_PORT)]
    ws_port: u16,

    /// HTTP port
    #[arg(long, default_value_t = gateway::DEFAULT_HTTP_PORT)]
    http_port: u16,

    /// Host address to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Path to AgentZero data directory (default: ~/Documents/agentzero)
    #[arg(long)]
    data_dir: Option<PathBuf>,

    /// Path to daemon configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Directory for log files (enables file logging when set)
    #[arg(long)]
    log_dir: Option<PathBuf>,

    /// Log rotation strategy: daily, hourly, minutely, or never
    #[arg(long, default_value = "daily")]
    log_rotation: String,

    /// Maximum number of log files to keep (0 = unlimited)
    #[arg(long, default_value_t = 7)]
    log_max_files: usize,

    /// Disable logging to stdout (only log to file)
    #[arg(long)]
    log_no_stdout: bool,

    /// Path to static files for web dashboard
    #[arg(long)]
    static_dir: Option<PathBuf>,

    /// Disable serving the web dashboard
    #[arg(long)]
    no_dashboard: bool,
}

/// Setup logging with optional file output.
///
/// Returns a guard that must be held for the lifetime of the program
/// to ensure log files are properly flushed.
fn setup_logging(
    args: &Args,
    env_filter: EnvFilter,
) -> Result<Option<tracing_appender::non_blocking::WorkerGuard>> {
    // Determine rotation strategy
    let rotation = match args.log_rotation.to_lowercase().as_str() {
        "hourly" => Rotation::HOURLY,
        "minutely" => Rotation::MINUTELY,
        "never" => Rotation::NEVER,
        _ => Rotation::DAILY,
    };

    // Check if file logging is enabled
    if let Some(ref log_dir) = args.log_dir {
        // Ensure log directory exists
        if !log_dir.exists() {
            std::fs::create_dir_all(log_dir)?;
        }

        // Create rolling file appender
        let file_appender = RollingFileAppender::new(rotation, log_dir, "zerod.log");
        let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

        if args.log_no_stdout {
            // Only file logging
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_writer(file_writer)
                        .with_ansi(false)
                        .with_target(true)
                        .with_thread_ids(true)
                        .with_file(true)
                        .with_line_number(true),
                )
                .init();
        } else {
            // Both stdout and file logging using combined writer
            let combined_writer = file_writer.and(std::io::stdout);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_writer(combined_writer)
                        .with_ansi(false) // Disable ANSI for file compatibility
                        .with_target(true)
                        .with_thread_ids(false)
                        .with_file(false)
                        .with_line_number(false),
                )
                .init();
        }

        Ok(Some(file_guard))
    } else {
        // Only stdout logging (default behavior)
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                fmt::layer()
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_file(false)
                    .with_line_number(false),
            )
            .init();

        Ok(None)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = match args.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let env_filter = EnvFilter::from_default_env()
        .add_directive(log_level.into());

    // Setup logging based on configuration
    let _guard = setup_logging(&args, env_filter)?;

    info!("AgentZero Daemon v{}", env!("CARGO_PKG_VERSION"));

    // Determine data directory (default: ~/Documents/agentzero)
    let data_dir = args.data_dir.unwrap_or_else(|| {
        dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("agentzero")
    });

    // Ensure data directory exists
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir)?;
        info!("Created data directory: {:?}", data_dir);
    }

    info!("Data directory: {:?}", data_dir);

    // Load gateway configuration
    let mut gateway_config: GatewayConfig = if let Some(config_path) = args.config {
        info!("Loading configuration from {:?}", config_path);
        let content = std::fs::read_to_string(&config_path)?;
        serde_yaml::from_str(&content)?
    } else {
        GatewayConfig {
            host: args.host.parse()?,
            websocket_port: args.ws_port,
            http_port: args.http_port,
            ..Default::default()
        }
    };

    // Override with CLI args
    if let Some(static_dir) = args.static_dir {
        gateway_config.static_dir = Some(static_dir.to_string_lossy().to_string());
        info!("Static directory: {:?}", static_dir);
    }
    if args.no_dashboard {
        gateway_config.serve_dashboard = false;
    }

    // Create and start server
    let mut server = GatewayServer::new(gateway_config, data_dir);
    server.start().await?;

    info!("Daemon started. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");
    server.shutdown().await;

    info!("Daemon stopped.");
    Ok(())
}
