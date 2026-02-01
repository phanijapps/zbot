// ============================================================================
// TOOL ERROR
// Error types for tool operations
// ============================================================================

use thiserror::Error;

/// Error type for tool execution
#[derive(Debug, Error)]
pub enum ToolError {
    /// Tool execution failed
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    /// Tool not found
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// Invalid tool arguments
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    /// Tool timeout
    #[error("Tool timed out")]
    Timeout,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Parse error
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Result type for tool operations
pub type ToolResult<T> = std::result::Result<T, ToolError>;
