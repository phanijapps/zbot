// ============================================================================
// DAILY SESSION TYPES
// Core data structures for daily session management
// ============================================================================

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Daily session for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySession {
    pub id: String,
    pub agent_id: String,
    pub session_date: String,
    pub summary: Option<String>,
    pub previous_session_ids: Option<Vec<String>>,
    pub message_count: i64,
    pub token_count: i64,
    pub system_prompt_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DailySession {
    pub fn generate_id(agent_id: &str, session_date: &str) -> String {
        format!("session_{}_{}", agent_id, session_date.replace('-', "_"))
    }

    pub fn today_id(agent_id: &str) -> String {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        Self::generate_id(agent_id, &today)
    }

    pub fn new(agent_id: String, session_date: String) -> Self {
        let id = Self::generate_id(&agent_id, &session_date);
        let now = Utc::now();

        Self {
            id,
            agent_id,
            session_date,
            summary: None,
            previous_session_ids: None,
            message_count: 0,
            token_count: 0,
            system_prompt_version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_version(agent_id: String, session_date: String, system_prompt_version: i64) -> Self {
        let id = Self::generate_id(&agent_id, &session_date);
        let now = Utc::now();

        Self {
            id,
            agent_id,
            session_date,
            summary: None,
            previous_session_ids: None,
            message_count: 0,
            token_count: 0,
            system_prompt_version,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Day summary for displaying in the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaySummary {
    pub session_id: String,
    pub session_date: String,
    pub summary: Option<String>,
    pub message_count: i64,
    pub is_archived: bool,
}

/// Session message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub token_count: i64,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_results: Option<serde_json::Value>,
}

/// Agent metadata with system prompt version tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub config_path: String,
    pub system_prompt_version: i64,
    pub current_system_prompt: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Result of system prompt check - indicates if prompt has changed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptCheck {
    pub has_changed: bool,
    pub previous_version: Option<i64>,
    pub new_version: i64,
    pub previous_prompt: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum DailySessionError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid session date format: {0}")]
    InvalidDateFormat(String),

    #[error("Session already exists: {0}")]
    AlreadyExists(String),
}
