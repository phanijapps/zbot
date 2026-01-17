// ============================================================================
// MCP MODULE
// Model Context Protocol support
// ============================================================================

//! # MCP Module
//!
//! Model Context Protocol client for external tool integration.
//!
//! Supports multiple transports (stdio, HTTP, SSE) for connecting
//! to MCP servers that provide additional tools and capabilities.

#![warn(missing_docs)]
#![warn(clippy::all)]

// TODO: Extract from src-tauri/src/domains/agent_runtime/mcp_manager.rs

use serde_json::Value;

/// Manager for MCP server connections
pub struct McpManager {
    // TODO: Implement from existing code
    _private: (),
}

impl McpManager {
    /// Create a new MCP manager
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }

    /// Load MCP servers from configuration
    pub async fn load_servers(&mut self, _server_ids: &[String]) -> Result<(), McpError> {
        // TODO: Implement from existing code
        Ok(())
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Individual MCP client
pub struct McpClient {
    // TODO: Implement from existing code
}

/// MCP server configuration
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub transport: McpTransport,
}

/// Transport type for MCP connection
#[derive(Debug, Clone)]
pub enum McpTransport {
    Stdio {
        command: String,
        args: Vec<String>,
    },
    Http {
        url: String,
    },
    Sse {
        url: String,
    },
}

/// A tool provided by an MCP server
#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub parameters_schema: Value,
}

/// MCP errors
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Server not found: {0}")]
    ServerNotFound(String),
}
