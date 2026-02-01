// ============================================================================
// SEARCH INDEX SCHEMA
// Tantivy index schema for message search
// ============================================================================

use serde::{Deserialize, Serialize};
use tantivy::schema::*;

/// Message source location
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MessageSource {
    Sqlite { session_id: String },
    Parquet { session_id: String, file_path: String },
}

/// Create Tantivy index schema
pub fn create_index_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // Stored fields (returned with results)
    schema_builder.add_text_field("message_id", STRING | STORED);
    schema_builder.add_text_field("session_id", STRING | STORED);
    schema_builder.add_text_field("agent_id", STRING | STORED);
    schema_builder.add_text_field("agent_name", TEXT | STORED);
    schema_builder.add_text_field("role", STRING | STORED);
    schema_builder.add_text_field("source_type", STRING | STORED); // "sqlite" or "parquet"
    schema_builder.add_text_field("source_path", STRING | STORED); // parquet file path

    // Indexed fields (searchable)
    schema_builder.add_text_field("content", TEXT | STORED);

    // Fast range queries
    schema_builder.add_i64_field("timestamp", INDEXED | STORED);

    schema_builder.build()
}

/// Document to be indexed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedDocument {
    pub message_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    pub source_type: String, // "sqlite" or "parquet"
    pub source_path: Option<String>, // Parquet file path if archived
}
