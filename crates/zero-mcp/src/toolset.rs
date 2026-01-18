//! # MCP Toolset
//!
//! Toolset implementation for MCP servers.

use std::sync::Arc;

use async_trait::async_trait;

use zero_core::{Tool, Toolset, Result as ZeroResult};

use super::client::McpClient;
use super::connection::McpConnection;
use super::error::{McpError, Result};
use super::filter::ToolFilter;
use super::tool::McpTool;

/// Toolset that provides tools from an MCP server.
pub struct McpToolset {
    /// Server ID
    server_id: String,

    /// MCP client for this server
    client: Arc<dyn McpClient>,

    /// Connection tracking
    connection: Arc<McpConnection>,

    /// Tool filter
    filter: Option<ToolFilter>,

    /// Cached tool definitions
    tools: Arc<tokio::sync::RwLock<Vec<McpTool>>>,
}

impl McpToolset {
    /// Create a new MCP toolset.
    pub fn new(
        server_id: impl Into<String>,
        client: Arc<dyn McpClient>,
        connection: Arc<McpConnection>,
    ) -> Self {
        Self {
            server_id: server_id.into(),
            client,
            connection,
            filter: None,
            tools: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Set a tool filter.
    pub fn with_filter(mut self, filter: ToolFilter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Refresh the tool list from the server.
    pub async fn refresh_tools(&self) -> Result<()> {
        if !self.connection.is_connected().await {
            return Err(McpError::NotConnected {
                server_id: self.server_id.clone(),
            });
        }

        let definitions = self.client.list_tools().await?;

        let mut tools = Vec::new();
        for def in definitions {
            // Apply filter if set
            if let Some(ref filter) = self.filter {
                if !filter.matches(&def) {
                    continue;
                }
            }

            let tool = McpTool::new(
                self.server_id.clone(),
                def,
                Arc::clone(&self.client),
            );
            tools.push(tool);
        }

        let tool_count = tools.len();
        *self.tools.write().await = tools;

        tracing::debug!(
            "Refreshed tools for '{}': {} tools available",
            self.server_id,
            tool_count
        );

        Ok(())
    }

    /// Get the server ID.
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Check if connected.
    pub async fn is_connected(&self) -> bool {
        self.connection.is_connected().await
    }
}

#[async_trait]
impl Toolset for McpToolset {
    fn name(&self) -> &str {
        &self.server_id
    }

    async fn tools(&self) -> ZeroResult<Vec<Arc<dyn Tool>>> {
        // Return cached tools as Arc<dyn Tool>
        let tools = self.tools.read().await;
        let result: Vec<Arc<dyn Tool>> = tools
            .iter()
            .map(|t| Arc::new(t.clone()) as Arc<dyn Tool>)
            .collect();
        Ok(result)
    }

    async fn filtered_tools(
        &self,
        predicate: zero_core::ToolPredicate,
    ) -> ZeroResult<Vec<Arc<dyn Tool>>> {
        let tools = self.tools.read().await;
        let result: Vec<Arc<dyn Tool>> = tools
            .iter()
            .map(|t| Arc::new(t.clone()) as Arc<dyn Tool>)
            .filter(|t| predicate(t.as_ref()))
            .collect();
        Ok(result)
    }
}

/// Builder for creating McpToolset instances.
pub struct McpToolsetBuilder {
    server_id: Option<String>,
    client: Option<Arc<dyn McpClient>>,
    connection: Option<Arc<McpConnection>>,
    filter: Option<ToolFilter>,
    auto_refresh: bool,
}

impl McpToolsetBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            server_id: None,
            client: None,
            connection: None,
            filter: None,
            auto_refresh: true,
        }
    }

    /// Set the server ID.
    pub fn with_server_id(mut self, server_id: impl Into<String>) -> Self {
        self.server_id = Some(server_id.into());
        self
    }

    /// Set the MCP client.
    pub fn with_client(mut self, client: Arc<dyn McpClient>) -> Self {
        self.client = Some(client);
        self
    }

    /// Set the connection.
    pub fn with_connection(mut self, connection: Arc<McpConnection>) -> Self {
        self.connection = Some(connection);
        self
    }

    /// Set a tool filter.
    pub fn with_filter(mut self, filter: ToolFilter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Enable/disable auto-refresh on build.
    pub fn with_auto_refresh(mut self, auto_refresh: bool) -> Self {
        self.auto_refresh = auto_refresh;
        self
    }

    /// Build the toolset.
    pub async fn build(self) -> Result<McpToolset> {
        let server_id = self
            .server_id
            .ok_or_else(|| McpError::config("", "server_id is required"))?;

        let client = self
            .client
            .ok_or_else(|| McpError::config(&server_id, "client is required"))?;

        let connection = self
            .connection
            .ok_or_else(|| McpError::config(&server_id, "connection is required"))?;

        let mut toolset = McpToolset::new(server_id, client, connection);

        if let Some(filter) = self.filter {
            toolset = toolset.with_filter(filter);
        }

        // Auto-refresh tools if connected
        if self.auto_refresh && toolset.is_connected().await {
            toolset.refresh_tools().await?;
        }

        Ok(toolset)
    }
}

impl Default for McpToolsetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{MockMcpClient, McpToolDefinition, McpServerInfo};
    use crate::config::{McpServerConfig, McpTransport};
    use crate::connection::McpConnection;

    #[tokio::test]
    async fn test_toolset_builder() {
        let config = McpServerConfig::stdio("test", "Test Server", "echo");
        let conn = Arc::new(McpConnection::new(config.clone()));

        let mut client = MockMcpClient::new(config);
        client.connect().await.unwrap();

        let toolset = McpToolsetBuilder::new()
            .with_server_id("test")
            .with_client(Arc::new(client))
            .with_connection(conn)
            .with_auto_refresh(false)
            .build()
            .await
            .unwrap();

        assert_eq!(toolset.name(), "test");
    }

    #[tokio::test]
    async fn test_toolset_builder_missing_server_id() {
        let result = McpToolsetBuilder::new()
            .with_auto_refresh(false)
            .build()
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_toolset_filter() {
        let config = McpServerConfig::stdio("test", "Test", "echo");
        let conn = Arc::new(McpConnection::new(config.clone()));

        let mut client = MockMcpClient::new(config);
        client.connect().await.unwrap();

        let filter = ToolFilter::new().with_name_prefix("test_");

        let toolset = McpToolsetBuilder::new()
            .with_server_id("test")
            .with_client(Arc::new(client))
            .with_connection(conn)
            .with_filter(filter)
            .with_auto_refresh(false)
            .build()
            .await
            .unwrap();

        // Filter is set internally
        assert!(toolset.filter.is_some());
    }
}
