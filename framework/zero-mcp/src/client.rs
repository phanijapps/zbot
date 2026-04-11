//! # MCP Client
//!
//! Core MCP client trait for tool integration.

use async_trait::async_trait;
use serde_json::Value;

use super::config::McpServerConfig;
use super::error::Result;

/// Core MCP client trait.
///
/// All MCP client implementations (stdio, HTTP, SSE) must implement this trait.
#[async_trait]
pub trait McpClient: Send + Sync {
    /// Get the server configuration.
    fn config(&self) -> &McpServerConfig;

    /// Connect to the MCP server.
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the MCP server.
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if the client is connected.
    fn is_connected(&self) -> bool;

    /// List available tools from this MCP server.
    async fn list_tools(&self) -> Result<Vec<McpToolDefinition>>;

    /// Call a tool from this MCP server.
    async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value>;

    /// Get the server's metadata/info.
    async fn get_info(&self) -> Result<McpServerInfo>;
}

/// Tool definition provided by an MCP server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpToolDefinition {
    /// Tool name
    pub name: String,

    /// Tool description
    pub description: String,

    /// JSON Schema for input parameters
    pub input_schema: Value,
}

/// Server information provided by an MCP server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpServerInfo {
    /// Server name
    pub name: String,

    /// Server version
    pub version: String,

    /// Protocol version
    pub protocol_version: String,
}

/// Simple in-memory MCP client for testing.
#[derive(Debug, Clone)]
pub struct MockMcpClient {
    config: McpServerConfig,
    connected: bool,
}

impl MockMcpClient {
    /// Create a new mock MCP client.
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            connected: false,
        }
    }
}

#[async_trait]
impl McpClient for MockMcpClient {
    fn config(&self) -> &McpServerConfig {
        &self.config
    }

    async fn connect(&mut self) -> Result<()> {
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    async fn list_tools(&self) -> Result<Vec<McpToolDefinition>> {
        Ok(vec![])
    }

    async fn call_tool(&self, _name: &str, _arguments: Value) -> Result<Value> {
        Ok(serde_json::json!({"result": "ok"}))
    }

    async fn get_info(&self) -> Result<McpServerInfo> {
        Ok(McpServerInfo {
            name: self.config.name.clone(),
            version: "1.0.0".to_string(),
            protocol_version: "2024-11-05".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpTransport;

    #[tokio::test]
    async fn test_mock_client() {
        let config = McpServerConfig::new("test", "Test Server", McpTransport::Http);
        let mut client = MockMcpClient::new(config);

        assert!(!client.is_connected());
        client.connect().await.unwrap();
        assert!(client.is_connected());

        let info = client.get_info().await.unwrap();
        assert_eq!(info.name, "Test Server");
    }

    #[tokio::test]
    async fn test_list_tools() {
        let config = McpServerConfig::stdio("test", "Test", "echo");
        let client = MockMcpClient::new(config);

        let tools = client.list_tools().await.unwrap();
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_call_tool() {
        let config = McpServerConfig::stdio("test", "Test", "echo");
        let client = MockMcpClient::new(config);

        let result = client
            .call_tool("test", serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result["result"], "ok");
    }
}
