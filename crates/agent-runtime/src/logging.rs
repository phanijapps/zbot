// ============================================================================
// LOGGING MODULE
// Structured logging configuration
// ============================================================================

//! # Logging Module
//!
//! Structured logging utilities for the agent runtime framework.
//!
//! ## Features
//!
//! - Configurable log levels (Error, Warn, Info, Debug, Trace)
//! - Optional file and line number information
//! - Environment variable support via `RUST_LOG`
//! - Convenient macros for logging with context
//!
//! ## Usage
//!
//! ```rust,no_run
//! use agent_runtime::logging::{init_logging, LogLevel};
//!
//! // Initialize with INFO level
//! init_logging(LogLevel::Info, false);
//!
//! // Or use environment variable
//! init_logging_from_env(false);
//!
//! // Use the logging macros
//! agent_info!("Agent {} started", agent_id);
//! agent_warn!("Tool '{}' not found", tool_name);
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Log level for the application
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogLevel {
    /// Error level
    Error,
    /// Warning level
    Warn,
    /// Info level
    #[default]
    Info,
    /// Debug level
    Debug,
    /// Trace level
    Trace,
}

impl LogLevel {
    /// Convert to tracing Level
    #[must_use]
    pub const fn as_tracing(&self) -> tracing::Level {
        match self {
            Self::Error => tracing::Level::ERROR,
            Self::Warn => tracing::Level::WARN,
            Self::Info => tracing::Level::INFO,
            Self::Debug => tracing::Level::DEBUG,
            Self::Trace => tracing::Level::TRACE,
        }
    }

    /// Parse from string
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "error" => Some(Self::Error),
            "warn" | "warning" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }
}

/// Initialize logging for the agent runtime
///
/// # Arguments
/// * `level` - The minimum log level to display
/// * `with_file` - Whether to include file/line info (default: false for performance)
///
/// # Example
/// ```no_run
/// use agent_runtime::logging::{init_logging, LogLevel};
///
/// // Initialize with INFO level
/// init_logging(LogLevel::Info, false);
///
/// // Initialize with DEBUG level and file info
/// init_logging(LogLevel::Debug, true);
/// ```
pub fn init_logging(level: LogLevel, with_file: bool) {
    let env_filter = EnvFilter::builder()
        .with_default_directive(level.as_tracing().into())
        .from_env_lossy();

    let registry = tracing_subscriber::registry().with(env_filter);

    if with_file {
        registry
            .with(
                fmt::layer()
                    .with_file(true)
                    .with_line_number(true)
                    .with_target(true),
            )
            .init();
    } else {
        registry
            .with(
                fmt::layer()
                    .with_file(false)
                    .with_line_number(false)
                    .with_target(false),
            )
            .init();
    }
}

/// Initialize logging from environment variable
///
/// Reads the `RUST_LOG` environment variable to set the log level.
/// If not set, defaults to `INFO`.
///
/// # Example
/// ```no_run
/// use agent_runtime::logging::init_logging_from_env;
///
/// // Set RUST_LOG=debug,trace or run without it for default INFO
/// init_logging_from_env(false);
/// ```
pub fn init_logging_from_env(with_file: bool) {
    let env_filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::INFO.into())
        .from_env_lossy();

    let registry = tracing_subscriber::registry().with(env_filter);

    if with_file {
        registry
            .with(
                fmt::layer()
                    .with_file(true)
                    .with_line_number(true)
                    .with_target(true),
            )
            .init();
    } else {
        registry
            .with(
                fmt::layer()
                    .with_file(false)
                    .with_line_number(false)
                    .with_target(false),
            )
            .init();
    }
}

/// Macro for logging with appropriate context
///
/// These macros are convenience wrappers around tracing macros
/// that automatically include relevant context.
#[macro_export]
macro_rules! agent_info {
    ($($arg:tt)*) => {
        tracing::info!($($arg)*);
    };
}

#[macro_export]
macro_rules! agent_warn {
    ($($arg:tt)*) => {
        tracing::warn!($($arg)*);
    };
}

#[macro_export]
macro_rules! agent_error {
    ($($arg:tt)*) => {
        tracing::error!($($arg)*);
    };
}

#[macro_export]
macro_rules! agent_debug {
    ($($arg:tt)*) => {
        tracing::debug!($($arg)*);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("error"), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_str("INFO"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str("Debug"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_str("invalid"), None);
    }
}
