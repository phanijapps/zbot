//! # MCP Configuration
//!
//! Configuration types for MCP servers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Unique identifier for this server
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Transport type (stdio, http, sse)
    pub transport: McpTransport,

    /// Command for stdio transport
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<McpCommand>,

    /// URL for HTTP/SSE transport
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Additional headers for HTTP requests
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Environment variables for the process
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Whether this server is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// MCP transport types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    /// Standard input/output (subprocess)
    Stdio,

    /// HTTP transport
    Http,

    /// Server-Sent Events
    Sse,
}

/// Command configuration for stdio transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCommand {
    /// Command to run
    pub command: String,

    /// Arguments to pass
    #[serde(default)]
    pub args: Vec<String>,
}

impl McpServerConfig {
    /// Create a new MCP server config.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        transport: McpTransport,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            transport,
            command: None,
            url: None,
            headers: HashMap::new(),
            env: HashMap::new(),
            enabled: true,
        }
    }

    /// Create a stdio-based MCP server config.
    pub fn stdio(
        id: impl Into<String>,
        name: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            transport: McpTransport::Stdio,
            command: Some(McpCommand {
                command: command.into(),
                args: Vec::new(),
            }),
            url: None,
            headers: HashMap::new(),
            env: HashMap::new(),
            enabled: true,
        }
    }

    /// Create an HTTP-based MCP server config.
    pub fn http(id: impl Into<String>, name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            transport: McpTransport::Http,
            command: None,
            url: Some(url.into()),
            headers: HashMap::new(),
            env: HashMap::new(),
            enabled: true,
        }
    }

    /// Add arguments to the command.
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        if let Some(ref mut cmd) = self.command {
            cmd.args = args;
        }
        self
    }

    /// Add an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Add a header for HTTP requests.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Set whether this server is enabled.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), String> {
        match self.transport {
            McpTransport::Stdio => {
                if self.command.is_none() {
                    return Err("stdio transport requires command".to_string());
                }
            }
            McpTransport::Http | McpTransport::Sse => {
                if self.url.is_none() {
                    return Err(format!("{} transport requires url", self.transport_as_str()));
                }
            }
        }
        Ok(())
    }

    fn transport_as_str(&self) -> &str {
        match self.transport {
            McpTransport::Stdio => "stdio",
            McpTransport::Http => "http",
            McpTransport::Sse => "sse",
        }
    }
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdio_config() {
        let config = McpServerConfig::stdio("test", "Test Server", "mcp-server");
        assert_eq!(config.id, "test");
        assert_eq!(config.name, "Test Server");
        assert!(config.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_http_config() {
        let config = McpServerConfig::http("test", "Test Server", "http://localhost:3000");
        assert_eq!(config.id, "test");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_error() {
        let config = McpServerConfig::new("test", "Test Server", McpTransport::Stdio);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_with_args() {
        let config = McpServerConfig::stdio("test", "Test", "cmd")
            .with_args(vec!["--arg1".to_string(), "--arg2".to_string()]);

        assert_eq!(config.command.as_ref().unwrap().args.len(), 2);
    }

    #[test]
    fn test_serialization() {
        let config = McpServerConfig::stdio("test", "Test", "cmd");
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("stdio"));
        assert!(json.contains("test"));
    }
}
