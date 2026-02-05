//! # MCP Tool Wrapper
//!
//! Wraps MCP tools to implement the Zero Tool trait.

use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value;

use zero_core::{Tool, ToolContext, Result as ZeroResult};
use super::client::McpClient;
use super::error::{McpError, Result};

/// Tool that wraps an MCP server tool.
#[derive(Clone)]
pub struct McpTool {
    /// Server ID
    server_id: String,

    /// Tool definition from the server
    definition: super::client::McpToolDefinition,

    /// MCP client for making calls
    client: Arc<dyn McpClient>,
}

impl McpTool {
    /// Create a new MCP tool.
    pub fn new(
        server_id: impl Into<String>,
        definition: super::client::McpToolDefinition,
        client: Arc<dyn McpClient>,
    ) -> Self {
        Self {
            server_id: server_id.into(),
            definition,
            client,
        }
    }

    /// Get the server ID.
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Get the tool definition.
    pub fn definition(&self) -> &super::client::McpToolDefinition {
        &self.definition
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn description(&self) -> &str {
        &self.definition.description
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(self.definition.input_schema.clone())
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> ZeroResult<Value> {
        self.client
            .call_tool(&self.definition.name, args)
            .await
            .map_err(|e| zero_core::ZeroError::Tool(format!("MCP tool error: {}", e)))
    }
}

/// Builder for creating MCP tools.
pub struct McpToolBuilder {
    server_id: String,
    definition: Option<super::client::McpToolDefinition>,
    client: Option<Arc<dyn McpClient>>,
}

impl McpToolBuilder {
    /// Create a new builder.
    pub fn new(server_id: impl Into<String>) -> Self {
        Self {
            server_id: server_id.into(),
            definition: None,
            client: None,
        }
    }

    /// Set the tool definition.
    pub fn with_definition(mut self, definition: super::client::McpToolDefinition) -> Self {
        self.definition = Some(definition);
        self
    }

    /// Set the MCP client.
    pub fn with_client(mut self, client: Arc<dyn McpClient>) -> Self {
        self.client = Some(client);
        self
    }

    /// Build the tool.
    pub fn build(self) -> Result<McpTool> {
        let definition = self.definition.ok_or_else(|| {
            McpError::config("", "Tool definition is required")
        })?;

        let client = self.client.ok_or_else(|| {
            McpError::config("", "MCP client is required")
        })?;

        Ok(McpTool::new(self.server_id, definition, client))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::client::MockMcpClient;
    use super::super::config::McpServerConfig;

    fn create_mock_context() -> Arc<dyn ToolContext> {
        use zero_core::{ReadonlyContext, CallbackContext, ToolContext};
        use zero_core::EventActions;

        struct MockCtx;

        impl ReadonlyContext for MockCtx {
            fn invocation_id(&self) -> &str { "test" }
            fn agent_name(&self) -> &str { "test" }
            fn user_id(&self) -> &str { "test" }
            fn app_name(&self) -> &str { "test" }
            fn session_id(&self) -> &str { "test" }
            fn branch(&self) -> &str { "test" }
            fn user_content(&self) -> &zero_core::types::Content {
                static CONTENT: zero_core::types::Content = zero_core::types::Content {
                    role: String::new(),
                    parts: Vec::new(),
                };
                &CONTENT
            }
        }

        impl CallbackContext for MockCtx {
            fn get_state(&self, _key: &str) -> Option<Value> { None }
            fn set_state(&self, _key: String, _value: Value) {}
        }

        impl ToolContext for MockCtx {
            fn function_call_id(&self) -> String { "test".to_string() }
            fn actions(&self) -> EventActions { EventActions::default() }
            fn set_actions(&self, _actions: EventActions) {}
        }

        std::sync::Arc::new(MockCtx) as Arc<dyn ToolContext>
    }

    #[test]
    fn test_tool_builder() {
        let config = McpServerConfig::stdio("test", "Test", "echo");
        let client = Arc::new(MockMcpClient::new(config));
        let definition = super::super::client::McpToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        };

        let tool = McpToolBuilder::new("test-server")
            .with_definition(definition)
            .with_client(client)
            .build()
            .unwrap();

        assert_eq!(tool.name(), "test_tool");
        assert_eq!(tool.server_id(), "test-server");
    }

    #[test]
    fn test_tool_builder_missing_definition() {
        let result = McpToolBuilder::new("test-server").build();
        assert!(result.is_err());
    }
}
