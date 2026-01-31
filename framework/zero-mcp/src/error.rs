//! # MCP Errors
//!
//! Comprehensive error types for MCP operations.

use std::io;
use zero_core::ZeroError;

/// MCP-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// Configuration error
    #[error("MCP configuration error: {message}")]
    Config {
        /// Server ID
        server_id: String,
        /// Error message
        message: String,
    },

    /// Connection error
    #[error("MCP connection error for '{server_id}': {message}")]
    Connection {
        /// Server ID
        server_id: String,
        /// Error message
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Request error
    #[error("MCP request error for '{server_id}' calling '{method}': {message}")]
    Request {
        /// Server ID
        server_id: String,
        /// Method being called
        method: String,
        /// Error message
        message: String,
    },

    /// Response error
    #[error("MCP response error for '{server_id}': {message}")]
    Response {
        /// Server ID
        server_id: String,
        /// Error message
        message: String,
    },

    /// Tool execution error
    #[error("MCP tool error for '{server_id}.{tool_name}': {message}")]
    ToolExecution {
        /// Server ID
        server_id: String,
        /// Tool name
        tool_name: String,
        /// Error message
        message: String,
    },

    /// Parse error
    #[error("MCP parse error: {message}")]
    Parse {
        /// Error message
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// IO error
    #[error("MCP IO error: {0}")]
    Io(#[from] io::Error),

    /// HTTP error
    #[error("MCP HTTP error: {message}")]
    Http {
        /// Error message
        message: String,
        #[source]
        source: Option<reqwest::Error>,
    },

    /// JSON error
    #[error("MCP JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Protocol error
    #[error("MCP protocol error: {message}")]
    Protocol {
        /// Error message
        message: String,
    },

    /// Timeout error
    #[error("MCP timeout error for '{server_id}': operation timed out after {timeout_secs}s")]
    Timeout {
        /// Server ID
        server_id: String,
        /// Timeout in seconds
        timeout_secs: u64,
    },

    /// Not connected error
    #[error("MCP client for '{server_id}' is not connected")]
    NotConnected {
        /// Server ID
        server_id: String,
    },

    /// Initialization error
    #[error("MCP initialization error for '{server_id}': {message}")]
    Initialization {
        /// Server ID
        server_id: String,
        /// Error message
        message: String,
    },

    /// Schema validation error
    #[error("MCP schema error for '{server_id}.{tool_name}': {message}")]
    Schema {
        /// Server ID
        server_id: String,
        /// Tool name
        tool_name: String,
        /// Error message
        message: String,
    },
}

impl McpError {
    /// Create a config error.
    pub fn config(server_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Config {
            server_id: server_id.into(),
            message: message.into(),
        }
    }

    /// Create a connection error.
    pub fn connection(
        server_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Connection {
            server_id: server_id.into(),
            message: message.into(),
            source: None,
        }
    }

    /// Create a connection error with source.
    pub fn connection_with_source(
        server_id: impl Into<String>,
        message: impl Into<String>,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self::Connection {
            server_id: server_id.into(),
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Create a tool execution error.
    pub fn tool_execution(
        server_id: impl Into<String>,
        tool_name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::ToolExecution {
            server_id: server_id.into(),
            tool_name: tool_name.into(),
            message: message.into(),
        }
    }

    /// Get the server ID if available.
    pub fn server_id(&self) -> Option<&str> {
        match self {
            Self::Config { server_id, .. }
            | Self::Connection { server_id, .. }
            | Self::Request { server_id, .. }
            | Self::Response { server_id, .. }
            | Self::ToolExecution { server_id, .. }
            | Self::Timeout { server_id, .. }
            | Self::NotConnected { server_id, .. }
            | Self::Initialization { server_id, .. }
            | Self::Schema { server_id, .. } => Some(server_id),
            _ => None,
        }
    }
}

impl From<McpError> for ZeroError {
    fn from(err: McpError) -> Self {
        ZeroError::Mcp(err.to_string())
    }
}

/// Result type for MCP operations.
pub type Result<T> = std::result::Result<T, McpError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = McpError::config("test-server", "invalid config");
        // The error format is "MCP configuration error: {message}"
        assert!(err.to_string().contains("invalid config"));
        assert!(err.to_string().contains("configuration error"));
    }

    #[test]
    fn test_error_conversion() {
        let mcp_err = McpError::tool_execution("test", "my_tool", "failed");
        let zero_err: ZeroError = mcp_err.into();
        assert!(matches!(zero_err, ZeroError::Mcp(_)));
    }

    #[test]
    fn test_server_id_extraction() {
        let err = McpError::tool_execution("server1", "tool1", "error");
        assert_eq!(err.server_id(), Some("server1"));

        let err = McpError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        assert_eq!(err.server_id(), None);
    }

    #[test]
    fn test_connection_error() {
        let err = McpError::connection("my-server", "failed to connect");
        assert!(matches!(err, McpError::Connection { .. }));
    }
}
