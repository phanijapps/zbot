//! `WikiArticle` and related domain types.

use serde::{Deserialize, Serialize};

/// A compiled wiki article for a ward.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiArticle {
    pub id: String,
    pub ward_id: String,
    pub agent_id: String,
    pub title: String,
    pub content: String,
    pub tags: Option<String>,
    pub source_fact_ids: Option<String>,
    /// Raw f32 embedding. Always `None` when loaded from a backend that
    /// stores vectors out-of-row. Callers may set this prior to upsert
    /// to have the vector persisted alongside.
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}
