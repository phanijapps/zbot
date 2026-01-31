// ============================================================================
// SEARCH INDEX
// Full-text search using Tantivy
// ============================================================================

pub mod error;
pub mod schema;
pub mod types;
pub mod manager;

pub use error::SearchError;
pub use schema::*;
pub use types::*;
pub use manager::SearchIndexManager;

// Re-export tantivy Schema for convenience
pub use tantivy::schema::Schema;

// Result type for this crate
pub type Result<T> = std::result::Result<T, SearchError>;
