//! Error types for the workflow executor

use std::path::PathBuf;

/// Workflow executor error types
#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    /// Workflow directory not found
    #[error("Workflow directory not found: {0}")]
    DirectoryNotFound(PathBuf),

    /// Missing required file
    #[error("Missing required file: {0}")]
    MissingFile(PathBuf),

    /// Invalid workflow configuration
    #[error("Invalid workflow configuration: {0}")]
    InvalidConfig(String),

    /// Failed to parse YAML
    #[error("YAML parse error in {path}: {message}")]
    YamlParse {
        path: PathBuf,
        message: String,
    },

    /// Failed to parse JSON
    #[error("JSON parse error in {path}: {message}")]
    JsonParse {
        path: PathBuf,
        message: String,
    },

    /// Invalid workflow graph
    #[error("Invalid workflow graph: {0}")]
    InvalidGraph(String),

    /// Node not found in graph
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// Subagent not found
    #[error("Subagent not found: {0}")]
    SubagentNotFound(String),

    /// Cycle detected in workflow graph
    #[error("Cycle detected in workflow graph: {0}")]
    CycleDetected(String),

    /// Missing start node
    #[error("Workflow must have exactly one start node")]
    MissingStartNode,

    /// Missing end node
    #[error("Workflow must have at least one end node")]
    MissingEndNode,

    /// Invalid edge connection
    #[error("Invalid edge: {from} -> {to}: {reason}")]
    InvalidEdge {
        from: String,
        to: String,
        reason: String,
    },

    /// LLM configuration error
    #[error("LLM configuration error: {0}")]
    LlmConfig(String),

    /// Tool configuration error
    #[error("Tool configuration error: {0}")]
    ToolConfig(String),

    /// Execution error
    #[error("Execution error: {0}")]
    Execution(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Core framework error
    #[error("Framework error: {0}")]
    Framework(String),
}

impl From<zero_core::ZeroError> for WorkflowError {
    fn from(err: zero_core::ZeroError) -> Self {
        WorkflowError::Framework(err.to_string())
    }
}

/// Result type for workflow operations
pub type Result<T> = std::result::Result<T, WorkflowError>;
