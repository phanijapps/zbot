//! # Plugin Configuration
//!
//! Types for plugin manifest parsing and plugin state management.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors from the plugin system.
#[derive(Debug, Error)]
pub enum PluginError {
    /// Plugin not found.
    #[error("Plugin '{0}' not found")]
    NotFound(String),

    /// Plugin already running.
    #[error("Plugin '{0}' is already running")]
    AlreadyRunning(String),

    /// Plugin not running.
    #[error("Plugin '{0}' is not running")]
    NotRunning(String),

    /// Failed to read plugin manifest.
    #[error("Failed to read plugin manifest: {0}")]
    ManifestRead(String),

    /// Failed to parse plugin manifest.
    #[error("Failed to parse plugin manifest: {0}")]
    ManifestParse(String),

    /// Failed to spawn plugin process.
    #[error("Failed to spawn plugin process: {0}")]
    SpawnFailed(String),

    /// npm install failed.
    #[error("npm install failed: {0}")]
    NpmInstallFailed(String),

    /// Plugin process exited unexpectedly.
    #[error("Plugin process exited: {0}")]
    ProcessExited(String),

    /// Failed to communicate with plugin.
    #[error("Plugin communication error: {0}")]
    CommunicationError(String),

    /// Plugin handshake failed.
    #[error("Plugin handshake failed: {0}")]
    HandshakeFailed(String),

    /// Plugin is disabled.
    #[error("Plugin '{0}' is disabled")]
    Disabled(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Runtime state of a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// Plugin discovered but not started.
    Discovered,
    /// Installing npm dependencies.
    Installing,
    /// Starting the plugin process.
    Starting,
    /// Plugin is running and connected.
    Running,
    /// Plugin process exited or failed.
    Stopped,
    /// Plugin failed to start or crashed.
    Failed,
    /// Plugin is disabled in config.
    Disabled,
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginState::Discovered => write!(f, "discovered"),
            PluginState::Installing => write!(f, "installing"),
            PluginState::Starting => write!(f, "starting"),
            PluginState::Running => write!(f, "running"),
            PluginState::Stopped => write!(f, "stopped"),
            PluginState::Failed => write!(f, "failed"),
            PluginState::Disabled => write!(f, "disabled"),
        }
    }
}

/// Plugin manifest loaded from plugin.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Unique plugin identifier (used as adapter_id in bridge protocol).
    #[serde(default)]
    pub id: String,
    /// Human-readable plugin name.
    #[serde(default)]
    pub name: String,
    /// Plugin version (semver recommended).
    #[serde(default)]
    pub version: String,
    /// Plugin description.
    #[serde(default)]
    pub description: String,
    /// Entry point script (default: "index.js").
    #[serde(default = "default_entry")]
    pub entry: String,
    /// Whether the plugin is enabled (default: true).
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Environment variables to pass to the plugin.
    /// Values can reference env vars with ${VAR_NAME} syntax.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Whether to auto-restart on crash (default: true).
    #[serde(default = "default_auto_restart")]
    pub auto_restart: bool,
    /// Delay before auto-restart in milliseconds (default: 5000).
    #[serde(default = "default_restart_delay")]
    pub restart_delay_ms: u64,
}

fn default_entry() -> String {
    "index.js".to_string()
}

fn default_enabled() -> bool {
    true
}

fn default_auto_restart() -> bool {
    true
}

fn default_restart_delay() -> u64 {
    5000
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: String::new(),
            description: String::new(),
            entry: default_entry(),
            enabled: true,
            env: HashMap::new(),
            auto_restart: true,
            restart_delay_ms: 5000,
        }
    }
}

impl PluginConfig {
    /// Load plugin config from a directory containing plugin.json.
    pub fn from_dir(plugin_dir: &std::path::Path) -> Result<Self, PluginError> {
        let manifest_path = plugin_dir.join("plugin.json");
        let content = std::fs::read_to_string(&manifest_path).map_err(|e| {
            PluginError::ManifestRead(format!("{}: {}", manifest_path.display(), e))
        })?;

        let config: Self = serde_json::from_str(&content)
            .map_err(|e| PluginError::ManifestParse(e.to_string()))?;

        // Validate required fields
        if config.id.is_empty() {
            return Err(PluginError::ManifestParse(
                "plugin.json missing required 'id' field".to_string(),
            ));
        }
        if config.name.is_empty() {
            return Err(PluginError::ManifestParse(
                "plugin.json missing required 'name' field".to_string(),
            ));
        }

        Ok(config)
    }

    /// Resolve environment variable references in env values.
    /// ${VAR_NAME} is replaced with the value of VAR_NAME from the process environment.
    /// If the variable is not set, it's replaced with an empty string.
    pub fn resolve_env(&self) -> HashMap<String, String> {
        self.env
            .iter()
            .map(|(key, value)| {
                let resolved = resolve_env_vars(value);
                (key.clone(), resolved)
            })
            .collect()
    }
}

/// Resolve ${VAR_NAME} references in a string.
fn resolve_env_vars(value: &str) -> String {
    let mut result = value.to_string();
    // Simple regex-free implementation
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let env_value = std::env::var(var_name).unwrap_or_default();
            result = format!(
                "{}{}{}",
                &result[..start],
                env_value,
                &result[start + end + 1..]
            );
        } else {
            break;
        }
    }
    result
}

/// Summary of a plugin for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSummary {
    /// Plugin ID.
    pub id: String,
    /// Plugin name.
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// Plugin description.
    pub description: String,
    /// Current state.
    pub state: PluginState,
    /// Whether auto-restart is enabled.
    pub auto_restart: bool,
    /// Whether the plugin is enabled.
    pub enabled: bool,
    /// Error message if state is Failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// User-defined configuration for a plugin.
/// Stored in config/plugins/{plugin_id}.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginUserConfig {
    /// Override plugin enabled state (None = use plugin.json).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// User-defined settings (non-sensitive).
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,
    /// Secrets (sensitive values like API tokens).
    /// Values are masked when returned via API.
    #[serde(default)]
    pub secrets: HashMap<String, String>,
}

impl PluginUserConfig {
    /// Load user config from a file.
    pub fn load(path: &std::path::Path) -> Result<Self, PluginError> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| PluginError::ManifestRead(format!("{}: {}", path.display(), e)))?;

        let config: Self = serde_json::from_str(&content)
            .map_err(|e| PluginError::ManifestParse(format!("{}: {}", path.display(), e)))?;

        Ok(config)
    }

    /// Save user config to a file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), PluginError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PluginError::Io(e))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| PluginError::ManifestParse(format!("Serialization failed: {}", e)))?;

        std::fs::write(path, content)?;

        // Set file permissions to owner-only for security (on Unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| PluginError::Io(e))?;
        }

        Ok(())
    }

    /// Get a setting value.
    pub fn get_setting(&self, key: &str) -> Option<&serde_json::Value> {
        self.settings.get(key)
    }

    /// Set a setting value.
    pub fn set_setting(&mut self, key: String, value: serde_json::Value) {
        self.settings.insert(key, value);
    }

    /// Get a secret value.
    pub fn get_secret(&self, key: &str) -> Option<&String> {
        self.secrets.get(key)
    }

    /// Set a secret value.
    pub fn set_secret(&mut self, key: String, value: String) {
        self.secrets.insert(key, value);
    }

    /// Delete a secret.
    pub fn delete_secret(&mut self, key: &str) -> Option<String> {
        self.secrets.remove(key)
    }

    /// Get list of secret keys (for API responses without values).
    pub fn secret_keys(&self) -> Vec<String> {
        self.secrets.keys().cloned().collect()
    }

    /// Merge with environment variables from plugin.json.
    /// User config secrets take precedence over env var references.
    pub fn resolve_env_with_secrets(
        &self,
        plugin_env: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut result = HashMap::new();

        // First, resolve env vars from plugin.json
        for (key, value) in plugin_env {
            let resolved = resolve_env_vars(value);
            result.insert(key.clone(), resolved);
        }

        // Then, override with secrets from user config
        for (key, value) in &self.secrets {
            result.insert(key.clone(), value.clone());
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_plugin_config_minimal() {
        let json = r#"{"id": "test-plugin", "name": "Test Plugin"}"#;
        let config: PluginConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.id, "test-plugin");
        assert_eq!(config.name, "Test Plugin");
        assert_eq!(config.entry, "index.js");
        assert!(config.enabled);
        assert!(config.auto_restart);
        assert_eq!(config.restart_delay_ms, 5000);
    }

    #[test]
    fn test_plugin_config_full() {
        let json = r#"{
            "id": "slackbot",
            "name": "Slack Bot",
            "version": "1.0.0",
            "description": "Slack integration",
            "entry": "main.js",
            "enabled": false,
            "env": {"TOKEN": "${SLACK_TOKEN}"},
            "auto_restart": false,
            "restart_delay_ms": 10000
        }"#;
        let config: PluginConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.id, "slackbot");
        assert_eq!(config.entry, "main.js");
        assert!(!config.enabled);
        assert!(!config.auto_restart);
        assert_eq!(config.restart_delay_ms, 10000);
        assert_eq!(config.env.get("TOKEN").unwrap(), "${SLACK_TOKEN}");
    }

    #[test]
    fn test_plugin_config_from_dir() {
        let dir = tempdir().unwrap();
        let manifest_path = dir.path().join("plugin.json");
        let content = r#"{"id": "my-plugin", "name": "My Plugin", "version": "2.0.0"}"#;
        std::fs::write(&manifest_path, content).unwrap();

        let config = PluginConfig::from_dir(dir.path()).unwrap();
        assert_eq!(config.id, "my-plugin");
        assert_eq!(config.name, "My Plugin");
        assert_eq!(config.version, "2.0.0");
    }

    #[test]
    fn test_plugin_config_missing_id() {
        let json = r#"{"name": "No ID"}"#;
        let config: Result<PluginConfig, _> = serde_json::from_str(json);
        // Parsing succeeds, but validation would fail in from_dir
        assert!(config.is_ok());
        assert!(config.unwrap().id.is_empty());
    }

    #[test]
    fn test_resolve_env_vars() {
        // Test with a var that likely exists (PATH, HOME, etc)
        let value = "prefix_${HOME}_suffix";
        let resolved = resolve_env_vars(value);
        // HOME should exist and be non-empty on most systems
        if std::env::var("HOME").is_ok() {
            let home = std::env::var("HOME").unwrap();
            assert!(resolved.starts_with("prefix_"));
            assert!(resolved.ends_with("_suffix"));
            assert!(resolved.contains(&home));
        }

        // Test missing variable
        let value = "${MISSING_VAR_12345}";
        let resolved = resolve_env_vars(value);
        assert_eq!(resolved, "");

        // Test no variables
        let value = "no_vars_here";
        let resolved = resolve_env_vars(value);
        assert_eq!(resolved, "no_vars_here");
    }

    #[test]
    fn test_plugin_state_display() {
        assert_eq!(PluginState::Running.to_string(), "running");
        assert_eq!(PluginState::Failed.to_string(), "failed");
        assert_eq!(PluginState::Installing.to_string(), "installing");
    }
}
