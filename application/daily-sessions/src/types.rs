// ============================================================================
// DAILY SESSION TYPES
// Core types for daily session management
// ============================================================================

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Daily session for an agent
/// Represents a single day's conversation with an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySession {
    /// Unique session ID: session_{agent_id}_{YYYY_MM_DD}
    pub id: String,

    /// Agent ID this session belongs to
    pub agent_id: String,

    /// Session date in YYYY-MM-DD format
    pub session_date: String,

    /// End-of-day summary (if generated)
    pub summary: Option<String>,

    /// JSON array of previous session IDs referenced in this session
    pub previous_session_ids: Option<Vec<String>>,

    /// Number of messages in this session
    pub message_count: i64,

    /// Total token count for this session
    pub token_count: i64,

    /// When this session was created
    pub created_at: DateTime<Utc>,

    /// When this session was last updated
    pub updated_at: DateTime<Utc>,
}

impl DailySession {
    /// Generate a session ID for the given agent and date
    pub fn generate_id(agent_id: &str, session_date: &str) -> String {
        // Format: session_{agent_id}_{YYYY_MM_DD}
        // Replace hyphens in agent_id with underscores to avoid conflicts
        let safe_agent_id = agent_id.replace('-', "_");
        let safe_date = session_date.replace('-', "_");
        format!("session_{}_{}", safe_agent_id, safe_date)
    }

    /// Create a new daily session
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
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if this session is from today
    pub fn is_today(&self) -> bool {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        self.session_date == today
    }
}

/// Day summary for displaying in the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaySummary {
    /// Session ID
    pub session_id: String,

    /// Session date (YYYY-MM-DD)
    pub session_date: String,

    /// Summary of the day's conversation
    pub summary: Option<String>,

    /// Number of messages
    pub message_count: i64,

    /// Whether this day has been archived to Parquet
    pub is_archived: bool,
}

/// Message in a daily session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    /// Unique message ID
    pub id: String,

    /// Session ID this message belongs to
    pub session_id: String,

    /// Message role: user, assistant, system, or tool
    pub role: String,

    /// Message content
    pub content: String,

    /// When the message was created
    pub created_at: DateTime<Utc>,

    /// Token count for this message
    pub token_count: i64,

    /// Tool calls made in this message (JSON)
    pub tool_calls: Option<serde_json::Value>,

    /// Tool results from this message (JSON)
    pub tool_results: Option<serde_json::Value>,
}

impl SessionMessage {
    /// Create a new user message
    pub fn user_message(session_id: String, content: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            session_id,
            role: "user".to_string(),
            content,
            created_at: now,
            token_count: 0,
            tool_calls: None,
            tool_results: None,
        }
    }

    /// Create a new assistant message
    pub fn assistant_message(session_id: String, content: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            session_id,
            role: "assistant".to_string(),
            content,
            created_at: now,
            token_count: 0,
            tool_calls: None,
            tool_results: None,
        }
    }
}

/// Agent information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    /// Agent ID (matches directory name)
    pub id: String,

    /// Agent name (internal)
    pub name: String,

    /// Display name (shown in UI)
    pub display_name: String,

    /// Agent description
    pub description: Option<String>,

    /// Path to agent config file
    pub config_path: String,

    /// Current system prompt version
    pub system_prompt_version: i32,

    /// Current system prompt content
    pub current_system_prompt: Option<String>,

    /// When agent was created
    pub created_at: DateTime<Utc>,

    /// When agent was last updated
    pub updated_at: DateTime<Utc>,
}

/// Agent channel info for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChannel {
    /// Agent ID
    pub agent_id: String,

    /// Display name
    pub display_name: String,

    /// Number of messages today
    pub today_message_count: i64,

    /// Whether there's previous day history
    pub has_history: bool,

    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,

    /// Last activity as human-readable string (e.g., "2 hours ago")
    pub last_activity_text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_generation() {
        let agent_id = "story-time";
        let session_date = "2025-01-18";
        let id = DailySession::generate_id(agent_id, session_date);

        assert_eq!(id, "session_story_time_2025_01_18");
    }

    #[test]
    fn test_session_id_with_hyphen_agent() {
        let agent_id = "my-agent";
        let session_date = "2025-01-18";
        let id = DailySession::generate_id(agent_id, session_date);

        assert_eq!(id, "session_my_agent_2025_01_18");
    }

    #[test]
    fn test_daily_session_creation() {
        let session = DailySession::new(
            "story-time".to_string(),
            "2025-01-18".to_string(),
        );

        assert!(session.id.starts_with("session_"));
        assert_eq!(session.agent_id, "story-time");
        assert_eq!(session.session_date, "2025-01-18");
        assert_eq!(session.message_count, 0);
        assert_eq!(session.token_count, 0);
    }

    #[test]
    fn test_user_message_creation() {
        let session_id = "session_test_2025_01_18".to_string();
        let message = SessionMessage::user_message(session_id.clone(), "Hello".to_string());

        assert_eq!(message.session_id, session_id);
        assert_eq!(message.role, "user");
        assert_eq!(message.content, "Hello");
    }
}
