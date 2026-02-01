//! # Error Types
//!
//! Unified error handling for session archive operations.

use std::path::PathBuf;

/// Archive operation error type
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Parquet errors
    #[error("Parquet error: {0}")]
    Parquet(String),

    /// Arrow errors
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Archive not found
    #[error("Archive not found: {0}")]
    NotFound(PathBuf),

    /// Invalid archive format
    #[error("Invalid archive format: {0}")]
    InvalidFormat(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Result type alias for archive operations
pub type ArchiveResult<T> = Result<T, ArchiveError>;
