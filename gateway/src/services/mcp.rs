//! # MCP Service
//!
//! Service for managing MCP server configurations.

use agent_runtime::McpServerConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// MCP service for loading and managing MCP server configurations.
pub struct McpService {
    config_path: PathBuf,
}

/// Summary of an MCP server for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerSummary {
    /// Server ID
    pub id: String,
    /// Server name
    pub name: String,
    /// Server description
    pub description: String,
    /// Transport type (stdio, http, sse, streamable-http)
    #[serde(rename = "type")]
    pub transport_type: String,
    /// Whether the server is enabled
    pub enabled: bool,
}

impl McpService {
    /// Create a new MCP service.
    ///
    /// The config_dir should be the agentzero configuration directory
    /// (e.g., ~/Documents/agentzero). The service will look for mcps.json
    /// in that directory.
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            config_path: config_dir.join("mcps.json"),
        }
    }

    /// Get the config file path.
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    /// List all MCP server configurations.
    pub fn list(&self) -> Result<Vec<McpServerConfig>, String> {
        if !self.config_path.exists() {
            return Ok(vec![]);
        }

        let content = fs::read_to_string(&self.config_path)
            .map_err(|e| format!("Failed to read mcps.json: {}", e))?;

        let configs: Vec<McpServerConfig> = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse mcps.json: {}", e))?;

        Ok(configs)
    }

    /// List MCP server summaries (lightweight for UI).
    pub fn list_summaries(&self) -> Result<Vec<McpServerSummary>, String> {
        let configs = self.list()?;
        Ok(configs.into_iter().map(|c| self.config_to_summary(&c)).collect())
    }

    /// Get a specific MCP server configuration by ID.
    pub fn get(&self, id: &str) -> Result<McpServerConfig, String> {
        let configs = self.list()?;

        configs
            .into_iter()
            .find(|c| c.id() == id)
            .ok_or_else(|| format!("MCP server not found: {}", id))
    }

    /// Get multiple MCP server configurations by IDs.
    ///
    /// Returns only the configs that exist and are enabled.
    /// Missing or disabled configs are silently skipped.
    pub fn get_multiple(&self, ids: &[String]) -> Vec<McpServerConfig> {
        let Ok(configs) = self.list() else {
            return vec![];
        };

        configs
            .into_iter()
            .filter(|c| ids.contains(&c.id()) && c.enabled())
            .collect()
    }

    /// Save MCP server configurations to disk.
    pub fn save(&self, configs: &[McpServerConfig]) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = serde_json::to_string_pretty(configs)
            .map_err(|e| format!("Failed to serialize mcps.json: {}", e))?;

        fs::write(&self.config_path, content)
            .map_err(|e| format!("Failed to write mcps.json: {}", e))?;

        Ok(())
    }

    /// Add a new MCP server configuration.
    pub fn add(&self, config: McpServerConfig) -> Result<(), String> {
        let mut configs = self.list().unwrap_or_default();

        // Check for duplicate ID
        let new_id = config.id();
        if configs.iter().any(|c| c.id() == new_id) {
            return Err(format!("MCP server with ID '{}' already exists", new_id));
        }

        configs.push(config);
        self.save(&configs)
    }

    /// Update an existing MCP server configuration.
    pub fn update(&self, id: &str, config: McpServerConfig) -> Result<(), String> {
        let mut configs = self.list()?;

        let index = configs
            .iter()
            .position(|c| c.id() == id)
            .ok_or_else(|| format!("MCP server not found: {}", id))?;

        configs[index] = config;
        self.save(&configs)
    }

    /// Delete an MCP server configuration.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let mut configs = self.list()?;

        let index = configs
            .iter()
            .position(|c| c.id() == id)
            .ok_or_else(|| format!("MCP server not found: {}", id))?;

        configs.remove(index);
        self.save(&configs)
    }

    /// Convert a config to a summary.
    fn config_to_summary(&self, config: &McpServerConfig) -> McpServerSummary {
        match config {
            McpServerConfig::Stdio { id, name, description, enabled, .. } => McpServerSummary {
                id: id.clone().unwrap_or_else(|| name.clone()),
                name: name.clone(),
                description: description.clone(),
                transport_type: "stdio".to_string(),
                enabled: *enabled,
            },
            McpServerConfig::Http { id, name, description, enabled, .. } => McpServerSummary {
                id: id.clone().unwrap_or_else(|| name.clone()),
                name: name.clone(),
                description: description.clone(),
                transport_type: "http".to_string(),
                enabled: *enabled,
            },
            McpServerConfig::Sse { id, name, description, enabled, .. } => McpServerSummary {
                id: id.clone().unwrap_or_else(|| name.clone()),
                name: name.clone(),
                description: description.clone(),
                transport_type: "sse".to_string(),
                enabled: *enabled,
            },
            McpServerConfig::StreamableHttp { id, name, description, enabled, .. } => McpServerSummary {
                id: id.clone().unwrap_or_else(|| name.clone()),
                name: name.clone(),
                description: description.clone(),
                transport_type: "streamable-http".to_string(),
                enabled: *enabled,
            },
        }
    }
}
