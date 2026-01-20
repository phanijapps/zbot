// ============================================================================
// ERROR TYPES
// Search index error types
// ============================================================================

use thiserror::Error;

/// Search index errors
#[derive(Debug, Error)]
pub enum SearchError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("Index error: {0}")]
    Index(String),

    #[error("Writer not initialized")]
    WriterNotInitialized,

    #[error("Reader not initialized")]
    ReaderNotInitialized,

    #[error("Query error: {0}")]
    Query(#[from] tantivy::query::QueryParserError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Not found: {0}")]
    NotFound(String),
}
