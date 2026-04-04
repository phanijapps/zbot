//! # Settings Service
//!
//! Service for managing application settings including tool and logging configuration.

use crate::logging::LogSettings;
use crate::paths::SharedVaultPaths;
use agent_tools::ToolSettings;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

/// Application settings.
///
/// Stored in `{data_dir}/settings.json` and persisted across restarts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Tool settings (enable/disable optional tools)
    #[serde(default)]
    pub tools: ToolSettings,

    /// Logging configuration (file logging, rotation, etc.)
    #[serde(default)]
    pub logs: LogSettings,

    /// Execution settings (concurrency, delegation limits, etc.)
    #[serde(default)]
    pub execution: ExecutionSettings,
}

/// Execution settings for controlling agent concurrency and delegation behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionSettings {
    /// Maximum number of subagents that can run in parallel across all sessions.
    /// Default: 2. Set lower for resource-constrained environments.
    #[serde(default = "default_max_parallel_agents")]
    pub max_parallel_agents: u32,
    /// Whether the first-time setup wizard has been completed.
    /// Default: false. Set to true after the wizard finishes.
    #[serde(default)]
    pub setup_complete: bool,
    /// The user-chosen name for the root agent (e.g., "Brahmi", "Jarvis").
    /// Used in SOUL.md and displayed in the UI.
    #[serde(default)]
    pub agent_name: Option<String>,
}

fn default_max_parallel_agents() -> u32 { 2 }

impl Default for ExecutionSettings {
    fn default() -> Self {
        Self {
            max_parallel_agents: default_max_parallel_agents(),
            setup_complete: false,
            agent_name: None,
        }
    }
}

/// Service for managing application settings.
pub struct SettingsService {
    paths: SharedVaultPaths,
    cache: RwLock<Option<AppSettings>>,
}

impl SettingsService {
    /// Create a new settings service.
    pub fn new(paths: SharedVaultPaths) -> Self {
        Self {
            paths,
            cache: RwLock::new(None),
        }
    }

    /// Create a legacy settings service with a direct config path.
    /// Used for early initialization before VaultPaths is available.
    pub fn new_legacy(config_dir: PathBuf) -> Self {
        Self {
            paths: std::sync::Arc::new(crate::paths::VaultPaths::new(config_dir)),
            cache: RwLock::new(None),
        }
    }

    /// Get the config file path.
    fn config_path(&self) -> PathBuf {
        self.paths.settings()
    }

    /// Invalidate the cache, forcing next read to go to disk.
    pub fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            *cache = None;
        }
    }

    /// Load settings from disk (bypasses cache).
    fn load_from_disk(&self) -> Result<AppSettings, String> {
        if !self.config_path().exists() {
            return Ok(AppSettings::default());
        }

        let content = fs::read_to_string(&self.config_path())
            .map_err(|e| format!("Failed to read settings.json: {}", e))?;

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse settings.json: {}", e))
    }

    /// Load settings (cached).
    pub fn load(&self) -> Result<AppSettings, String> {
        // Check cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(settings) = cache.as_ref() {
                return Ok(settings.clone());
            }
        }

        // Cache miss: read from disk
        let settings = self.load_from_disk()?;

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(settings.clone());
        }

        Ok(settings)
    }

    /// Save settings to disk and update cache.
    pub fn save(&self, settings: &AppSettings) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = self.config_path().parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = serde_json::to_string_pretty(settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        fs::write(&self.config_path(), content)
            .map_err(|e| format!("Failed to write settings.json: {}", e))?;

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(settings.clone());
        }

        Ok(())
    }

    /// Get tool settings.
    pub fn get_tool_settings(&self) -> Result<ToolSettings, String> {
        let settings = self.load()?;
        Ok(settings.tools)
    }

    /// Update tool settings.
    pub fn update_tool_settings(&self, tool_settings: ToolSettings) -> Result<(), String> {
        let mut settings = self.load().unwrap_or_default();
        settings.tools = tool_settings;
        self.save(&settings)
    }

    /// Get log settings.
    pub fn get_log_settings(&self) -> Result<LogSettings, String> {
        let settings = self.load()?;
        Ok(settings.logs)
    }

    /// Update log settings.
    ///
    /// Note: Changes to log settings require a daemon restart to take effect.
    pub fn update_log_settings(&self, log_settings: LogSettings) -> Result<(), String> {
        // Validate before saving
        log_settings.validate()?;

        let mut settings = self.load().unwrap_or_default();
        settings.logs = log_settings;
        self.save(&settings)
    }

    /// Get execution settings.
    pub fn get_execution_settings(&self) -> Result<ExecutionSettings, String> {
        let settings = self.load()?;
        Ok(settings.execution)
    }

    /// Update execution settings.
    ///
    /// Note: Changes to max_parallel_agents require a daemon restart to take effect.
    pub fn update_execution_settings(&self, execution_settings: ExecutionSettings) -> Result<(), String> {
        if execution_settings.max_parallel_agents == 0 {
            return Err("max_parallel_agents must be at least 1".to_string());
        }
        let mut settings = self.load().unwrap_or_default();
        settings.execution = execution_settings;
        self.save(&settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_settings() {
        let dir = tempdir().unwrap();
        let service = SettingsService::new_legacy(dir.path().to_path_buf());

        let settings = service.load().unwrap();
        // Optional tools are disabled by default
        assert!(!settings.tools.python);
        assert!(!settings.tools.web_fetch);
        // Logging is disabled by default
        assert!(!settings.logs.enabled);
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let service = SettingsService::new_legacy(dir.path().to_path_buf());

        let mut settings = AppSettings::default();
        settings.tools.python = true;
        settings.tools.web_fetch = true;

        service.save(&settings).unwrap();

        let loaded = service.load().unwrap();
        assert!(loaded.tools.python);
        assert!(loaded.tools.web_fetch);
    }

    #[test]
    fn test_log_settings_crud() {
        let dir = tempdir().unwrap();
        let service = SettingsService::new_legacy(dir.path().to_path_buf());

        // Default: logging disabled
        let log_settings = service.get_log_settings().unwrap();
        assert!(!log_settings.enabled);

        // Update: enable logging
        let mut new_log_settings = LogSettings::enabled();
        new_log_settings.max_files = 14;
        new_log_settings.level = "debug".to_string();

        service.update_log_settings(new_log_settings.clone()).unwrap();

        // Verify
        let loaded = service.get_log_settings().unwrap();
        assert!(loaded.enabled);
        assert_eq!(loaded.max_files, 14);
        assert_eq!(loaded.level, "debug");
    }

    #[test]
    fn test_log_settings_validation() {
        let dir = tempdir().unwrap();
        let service = SettingsService::new_legacy(dir.path().to_path_buf());

        // Invalid log level should fail
        let mut invalid_settings = LogSettings::default();
        invalid_settings.level = "invalid".to_string();

        let result = service.update_log_settings(invalid_settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_settings_json_format() {
        let dir = tempdir().unwrap();
        let service = SettingsService::new_legacy(dir.path().to_path_buf());

        let mut settings = AppSettings::default();
        settings.tools.python = true;
        settings.logs.enabled = true;
        settings.logs.max_files = 30;

        service.save(&settings).unwrap();

        // Read raw JSON to verify camelCase format
        let json_path = dir.path().join("config").join("settings.json");
        let json_content = fs::read_to_string(json_path).unwrap();

        assert!(json_content.contains("maxFiles"));
        assert!(json_content.contains("suppressStdout"));
    }
}
