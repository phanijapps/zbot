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

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_query_default_limit() {
        let query = SearchQuery {
            query: "test".to_string(),
            agent_id: None,
            start_date: None,
            end_date: None,
            limit: 50,
        };
        assert_eq!(query.limit, 50);
    }

    #[test]
    fn test_search_query_serialization() {
        let query = SearchQuery {
            query: "test search".to_string(),
            agent_id: Some("agent-123".to_string()),
            start_date: None,
            end_date: None,
            limit: 100,
        };

        let json_str = serde_json::to_string(&query).unwrap();
        let parsed: SearchQuery = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.query, "test search");
        assert_eq!(parsed.agent_id, Some("agent-123".to_string()));
        assert_eq!(parsed.limit, 100);
    }

    #[test]
    fn test_search_query_with_dates() {
        let now = Utc::now();
        let query = SearchQuery {
            query: "test".to_string(),
            agent_id: None,
            start_date: Some(now),
            end_date: Some(now + chrono::Duration::days(1)),
            limit: 50,
        };

        assert!(query.start_date.is_some());
        assert!(query.end_date.is_some());
    }

    #[test]
    fn test_index_build_progress_complete() {
        let progress = IndexBuildProgress {
            total_messages: 100,
            indexed_messages: 100,
            stage: "Complete".to_string(),
            is_complete: true,
        };

        assert!(progress.is_complete);
        assert_eq!(progress.indexed_messages, progress.total_messages);
    }

    #[test]
    fn test_index_build_progress_partial() {
        let progress = IndexBuildProgress {
            total_messages: 100,
            indexed_messages: 50,
            stage: "Indexing".to_string(),
            is_complete: false,
        };

        assert!(!progress.is_complete);
        assert_eq!(progress.indexed_messages, 50);
    }
}
