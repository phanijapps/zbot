// ============================================================================
// MCP ERROR TYPES
// ============================================================================

//! # MCP Error Types
//!
//! Error types for MCP operations.

use thiserror::Error;

/// Errors from MCP operations
#[derive(Debug, Error)]
pub enum McpError {
    /// Connection to MCP server failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// MCP protocol error
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Server not found
    #[error("Server not found: {0}")]
    ServerNotFound(String),

    /// Tool execution error
    #[error("Tool error: {0}")]
    ToolError(String),
}
