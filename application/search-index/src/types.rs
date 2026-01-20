// ============================================================================
// SEARCH TYPES
// Search query parameters and results
// ============================================================================

use crate::schema::MessageSource;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Search result with location info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub message_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub score: f32,
    pub source: MessageSource,
}

/// Search query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub start_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub end_date: Option<DateTime<Utc>>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// Index build progress for rebuilding index
#[derive(Debug, Clone, Serialize)]
pub struct IndexBuildProgress {
    pub total_messages: usize,
    pub indexed_messages: usize,
    pub stage: String,
    pub is_complete: bool,
}
