//! # Plugin Service
//!
//! Manages plugin user configuration storage.
//! Config is stored in the plugin directory as .config.json for self-contained plugins.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Config file name (hidden file in plugin directory).
const CONFIG_FILE_NAME: &str = ".config.json";

/// User-defined configuration for a plugin.
/// Stored in plugins/{plugin_id}/.config.json
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
    pub fn load(path: &std::path::Path) -> Result<Self, PluginConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| PluginConfigError::Read(format!("{}: {}", path.display(), e)))?;

        let config: Self = serde_json::from_str(&content)
            .map_err(|e| PluginConfigError::Parse(format!("{}: {}", path.display(), e)))?;

        Ok(config)
    }

    /// Save user config to a file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), PluginConfigError> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| PluginConfigError::Parse(format!("Serialization failed: {}", e)))?;

        std::fs::write(path, content)?;

        // Set file permissions to owner-only for security (on Unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
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
}

/// Errors from plugin config operations.
#[derive(Debug, thiserror::Error)]
pub enum PluginConfigError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to read config file.
    #[error("Failed to read config: {0}")]
    Read(String),

    /// Failed to parse config file.
    #[error("Failed to parse config: {0}")]
    Parse(String),
}

/// Service for managing plugin configuration.
///
/// Config files are stored in the plugin directory itself as `.config.json`,
/// making plugins self-contained - deleting the plugin directory also deletes
/// the user configuration.
pub struct PluginService {
    /// Directory containing all plugins.
    plugins_dir: PathBuf,
}

impl PluginService {
    /// Create a new plugin service.
    ///
    /// # Arguments
    /// * `plugins_dir` - Path to the plugins directory (e.g., ~/Documents/agentzero/plugins)
    pub fn new(plugins_dir: PathBuf) -> Self {
        Self { plugins_dir }
    }

    /// Get the config file path for a plugin.
    /// Returns: plugins/{plugin_id}/.config.json
    fn config_path(&self, plugin_id: &str) -> PathBuf {
        self.plugins_dir
            .join(plugin_id)
            .join(CONFIG_FILE_NAME)
    }

    /// Get the plugin directory path.
    pub fn plugin_dir(&self, plugin_id: &str) -> PathBuf {
        self.plugins_dir.join(plugin_id)
    }

    /// Initialize config file for a plugin if it doesn't exist.
    ///
    /// This is called when a plugin is first discovered to create an empty
    /// config file that users can then populate via the API.
    pub fn initialize_config(&self, plugin_id: &str) -> Result<PathBuf, String> {
        let config_path = self.config_path(plugin_id);

        if config_path.exists() {
            tracing::debug!(
                plugin_id = %plugin_id,
                path = %config_path.display(),
                "Config file already exists"
            );
            return Ok(config_path);
        }

        // Create empty config
        let config = PluginUserConfig::default();
        config.save(&config_path).map_err(|e| {
            tracing::error!(
                plugin_id = %plugin_id,
                "Failed to initialize config: {}", e
            );
            format!("Failed to initialize config: {}", e)
        })?;

        tracing::info!(
            plugin_id = %plugin_id,
            path = %config_path.display(),
            "Initialized plugin config file"
        );

        Ok(config_path)
    }

    /// Check if a plugin directory exists.
    pub fn plugin_exists(&self, plugin_id: &str) -> bool {
        self.plugin_dir(plugin_id).join("plugin.json").exists()
    }

    /// Load user config for a plugin.
    pub fn load_config(&self, plugin_id: &str) -> PluginUserConfig {
        let path = self.config_path(plugin_id);

        match PluginUserConfig::load(&path) {
            Ok(config) => {
                tracing::debug!(
                    plugin_id = %plugin_id,
                    path = %path.display(),
                    "Loaded plugin user config"
                );
                config
            }
            Err(e) => {
                tracing::debug!(
                    plugin_id = %plugin_id,
                    "No user config found, using defaults: {}", e
                );
                PluginUserConfig::default()
            }
        }
    }

    /// Save user config for a plugin.
    pub fn save_config(&self, plugin_id: &str, config: &PluginUserConfig) -> Result<(), String> {
        let path = self.config_path(plugin_id);

        config.save(&path).map_err(|e| {
            tracing::error!(
                plugin_id = %plugin_id,
                "Failed to save plugin config: {}", e
            );
            format!("Failed to save plugin config: {}", e)
        })?;

        tracing::info!(
            plugin_id = %plugin_id,
            path = %path.display(),
            "Saved plugin user config"
        );

        Ok(())
    }

    /// Get a specific setting from a plugin's config.
    pub fn get_setting(&self, plugin_id: &str, key: &str) -> Option<serde_json::Value> {
        let config = self.load_config(plugin_id);
        config.get_setting(key).cloned()
    }

    /// Set a specific setting in a plugin's config.
    pub fn set_setting(
        &self,
        plugin_id: &str,
        key: String,
        value: serde_json::Value,
    ) -> Result<(), String> {
        let mut config = self.load_config(plugin_id);
        config.set_setting(key, value);
        self.save_config(plugin_id, &config)
    }

    /// Get a secret value from a plugin's config.
    pub fn get_secret(&self, plugin_id: &str, key: &str) -> Option<String> {
        let config = self.load_config(plugin_id);
        config.get_secret(key).cloned()
    }

    /// Set a secret value in a plugin's config.
    pub fn set_secret(
        &self,
        plugin_id: &str,
        key: String,
        value: String,
    ) -> Result<(), String> {
        let mut config = self.load_config(plugin_id);
        config.set_secret(key, value);
        self.save_config(plugin_id, &config)
    }

    /// Delete a secret from a plugin's config.
    pub fn delete_secret(&self, plugin_id: &str, key: &str) -> Result<bool, String> {
        let mut config = self.load_config(plugin_id);

        if config.delete_secret(key).is_some() {
            self.save_config(plugin_id, &config)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List secret keys for a plugin (without values).
    pub fn list_secret_keys(&self, plugin_id: &str) -> Vec<String> {
        let config = self.load_config(plugin_id);
        config.secret_keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_missing_config() {
        let dir = tempdir().unwrap();
        let service = PluginService::new(dir.path().join("plugins"));

        let config = service.load_config("nonexistent");
        assert!(config.settings.is_empty());
        assert!(config.secrets.is_empty());
    }

    #[test]
    fn test_initialize_config() {
        let dir = tempdir().unwrap();
        let plugins_dir = dir.path().join("plugins");
        let plugin_dir = plugins_dir.join("test-plugin");

        // Create plugin directory with manifest
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.json"), r#"{"id": "test-plugin"}"#).unwrap();

        let service = PluginService::new(plugins_dir);

        // Initialize config
        let config_path = service.initialize_config("test-plugin").unwrap();
        assert!(config_path.exists());
        assert!(config_path.ends_with(".config.json"));

        // Second call should be idempotent
        let config_path2 = service.initialize_config("test-plugin").unwrap();
        assert_eq!(config_path, config_path2);
    }

    #[test]
    fn test_save_and_load_config() {
        let dir = tempdir().unwrap();
        let plugins_dir = dir.path().join("plugins");
        let plugin_dir = plugins_dir.join("test-plugin");

        // Create plugin directory
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let service = PluginService::new(plugins_dir);

        let mut config = PluginUserConfig::default();
        config.enabled = Some(true);
        config.set_setting("agent".to_string(), serde_json::json!("assistant"));
        config.set_secret("token".to_string(), "secret123".to_string());

        service.save_config("test-plugin", &config).unwrap();

        // Verify config file location
        let config_path = plugin_dir.join(".config.json");
        assert!(config_path.exists());

        let loaded = service.load_config("test-plugin");
        assert_eq!(loaded.enabled, Some(true));
        assert_eq!(loaded.get_setting("agent").unwrap(), "assistant");
        assert_eq!(loaded.get_secret("token").unwrap(), "secret123");
    }

    #[test]
    fn test_set_secret() {
        let dir = tempdir().unwrap();
        let plugins_dir = dir.path().join("plugins");
        let plugin_dir = plugins_dir.join("slack");

        std::fs::create_dir_all(&plugin_dir).unwrap();

        let service = PluginService::new(plugins_dir);

        service.set_secret("slack", "bot_token".to_string(), "xoxb-123".to_string()).unwrap();

        let secret = service.get_secret("slack", "bot_token").unwrap();
        assert_eq!(secret, "xoxb-123");

        let keys = service.list_secret_keys("slack");
        assert_eq!(keys, vec!["bot_token"]);
    }

    #[test]
    fn test_delete_secret() {
        let dir = tempdir().unwrap();
        let plugins_dir = dir.path().join("plugins");
        let plugin_dir = plugins_dir.join("slack");

        std::fs::create_dir_all(&plugin_dir).unwrap();

        let service = PluginService::new(plugins_dir);

        service.set_secret("slack", "token".to_string(), "secret".to_string()).unwrap();
        assert!(service.delete_secret("slack", "token").unwrap());
        assert!(!service.delete_secret("slack", "token").unwrap());
    }

    #[test]
    fn test_plugin_exists() {
        let dir = tempdir().unwrap();
        let plugins_dir = dir.path().join("plugins");
        let plugin_dir = plugins_dir.join("test-plugin");

        let service = PluginService::new(plugins_dir.clone());

        // Plugin doesn't exist yet
        assert!(!service.plugin_exists("test-plugin"));

        // Create plugin directory with manifest
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.json"), r#"{"id": "test-plugin"}"#).unwrap();

        // Now it exists
        assert!(service.plugin_exists("test-plugin"));
        assert!(!service.plugin_exists("other-plugin"));
    }
}
