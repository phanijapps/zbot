//! # Logging Configuration
//!
//! Settings for daemon file logging with rolling file support.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Logging configuration for the daemon.
///
/// Controls file-based logging with rotation and retention policies.
/// When enabled, logs are written to files in addition to (or instead of) stdout.
///
/// # Example (settings.json)
/// ```json
/// {
///   "logs": {
///     "enabled": true,
///     "directory": null,
///     "level": "info",
///     "rotation": "daily",
///     "maxFiles": 7,
///     "suppressStdout": false
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogSettings {
    /// Enable file logging.
    ///
    /// When `false` (default), logs are only written to stdout.
    /// When `true`, logs are written to files in the configured directory.
    #[serde(default)]
    pub enabled: bool,

    /// Custom log directory path.
    ///
    /// When `None` (default), logs are written to `{data_dir}/logs/`.
    /// When `Some(path)`, logs are written to the specified directory.
    ///
    /// The directory will be created if it doesn't exist.
    #[serde(default)]
    pub directory: Option<PathBuf>,

    /// Log level threshold.
    ///
    /// One of: `trace`, `debug`, `info`, `warn`, `error`.
    /// Messages below this level are not logged.
    ///
    /// Default: `info`
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log file rotation strategy.
    ///
    /// - `daily`: Create new file each day (default)
    /// - `hourly`: Create new file each hour
    /// - `minutely`: Create new file each minute (useful for testing)
    /// - `never`: Never rotate (single file)
    #[serde(default = "default_rotation")]
    pub rotation: String,

    /// Maximum number of rotated log files to keep.
    ///
    /// When rotation is enabled, old log files beyond this count are deleted.
    /// Set to `0` for unlimited retention.
    ///
    /// Default: `7`
    #[serde(default = "default_max_files")]
    pub max_files: usize,

    /// Suppress stdout output when file logging is enabled.
    ///
    /// When `false` (default), logs go to both file and stdout.
    /// When `true`, logs only go to file (useful for daemon mode).
    #[serde(default)]
    pub suppress_stdout: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_rotation() -> String {
    "daily".to_string()
}

fn default_max_files() -> usize {
    7
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            directory: None,
            level: default_log_level(),
            rotation: default_rotation(),
            max_files: default_max_files(),
            suppress_stdout: false,
        }
    }
}

impl LogSettings {
    /// Create a new LogSettings with file logging enabled.
    #[must_use]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    /// Validate the settings.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `level` is not a valid log level
    /// - `rotation` is not a valid rotation strategy
    pub fn validate(&self) -> Result<(), String> {
        // Validate log level
        match self.level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            invalid => {
                return Err(format!(
                    "Invalid log level '{}'. Expected: trace, debug, info, warn, or error",
                    invalid
                ))
            }
        }

        // Validate rotation strategy
        match self.rotation.to_lowercase().as_str() {
            "daily" | "hourly" | "minutely" | "never" => {}
            invalid => {
                return Err(format!(
                    "Invalid rotation '{}'. Expected: daily, hourly, minutely, or never",
                    invalid
                ))
            }
        }

        Ok(())
    }

    /// Check if the log level is trace.
    #[must_use]
    pub fn is_trace(&self) -> bool {
        self.level.to_lowercase() == "trace"
    }

    /// Check if the log level is debug or lower.
    #[must_use]
    pub fn is_debug_or_lower(&self) -> bool {
        matches!(self.level.to_lowercase().as_str(), "trace" | "debug")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = LogSettings::default();
        assert!(!settings.enabled);
        assert!(settings.directory.is_none());
        assert_eq!(settings.level, "info");
        assert_eq!(settings.rotation, "daily");
        assert_eq!(settings.max_files, 7);
        assert!(!settings.suppress_stdout);
    }

    #[test]
    fn test_serialize_camel_case() {
        let settings = LogSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("maxFiles"));
        assert!(json.contains("suppressStdout"));
    }

    #[test]
    fn test_deserialize_camel_case() {
        let json = r#"{"enabled":true,"maxFiles":14,"suppressStdout":true}"#;
        let settings: LogSettings = serde_json::from_str(json).unwrap();
        assert!(settings.enabled);
        assert_eq!(settings.max_files, 14);
        assert!(settings.suppress_stdout);
    }

    #[test]
    fn test_validate_valid_settings() {
        let settings = LogSettings::default();
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_level() {
        let settings = LogSettings {
            level: "invalid".to_string(),
            ..LogSettings::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_rotation() {
        let settings = LogSettings {
            rotation: "weekly".to_string(),
            ..LogSettings::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_enabled_constructor() {
        let settings = LogSettings::enabled();
        assert!(settings.enabled);
        assert_eq!(settings.level, "info"); // Uses defaults for other fields
    }
}
