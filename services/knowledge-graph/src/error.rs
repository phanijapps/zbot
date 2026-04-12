//! # Error Types
//!
//! Error handling for knowledge graph operations.

/// Knowledge graph error type
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    /// Database errors
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// LLM errors
    #[error("LLM error: {0}")]
    Llm(String),

    /// Entity not found
    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    /// Invalid entity type
    #[error("Invalid entity type: {0}")]
    InvalidEntityType(String),

    /// Invalid relationship type
    #[error("Invalid relationship type: {0}")]
    InvalidRelationshipType(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Other/uncategorized errors (e.g., from entity resolver).
    #[error("{0}")]
    Other(String),
}

/// Result type alias for graph operations
pub type GraphResult<T> = Result<T, GraphError>;
