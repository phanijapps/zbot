// ============================================================================
// DAILY SESSION TYPES
// Core data structures for daily session management
// ============================================================================

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

    pub fn with_version(
        agent_id: String,
        session_date: String,
        system_prompt_version: i64,
    ) -> Self {
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

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_daily_session_generate_id() {
        let agent_id = "test-agent";
        let date = "2025-01-20";
        let id = DailySession::generate_id(agent_id, date);
        assert_eq!(id, "session_test-agent_2025_01_20");
    }

    #[test]
    fn test_daily_session_generate_id_with_hyphens() {
        let agent_id = "my-agent";
        let date = "2025-01-20";
        let id = DailySession::generate_id(agent_id, date);
        assert_eq!(id, "session_my-agent_2025_01_20");
    }

    #[test]
    fn test_daily_session_today_id() {
        let agent_id = "test-agent";
        let id = DailySession::today_id(agent_id);
        assert!(id.starts_with("session_test-agent_"));
        // ID should contain date components (underscores between YYYY, MM, DD)
        assert!(id.contains('_'));
        let parts: Vec<&str> = id.rsplit('_').collect(); // Get date parts from end
        assert!(parts.len() >= 3); // At least day, month, year
    }

    #[test]
    fn test_daily_session_new() {
        let session = DailySession::new("agent-123".to_string(), "2025-01-20".to_string());

        assert_eq!(session.agent_id, "agent-123");
        assert_eq!(session.session_date, "2025-01-20");
        assert_eq!(session.id, "session_agent-123_2025_01_20");
        assert_eq!(session.message_count, 0);
        assert_eq!(session.token_count, 0);
        assert_eq!(session.system_prompt_version, 1);
        assert!(session.summary.is_none());
        assert!(session.previous_session_ids.is_none());
    }

    #[test]
    fn test_daily_session_with_version() {
        let session =
            DailySession::with_version("agent-123".to_string(), "2025-01-20".to_string(), 3);

        assert_eq!(session.agent_id, "agent-123");
        assert_eq!(session.system_prompt_version, 3);
        assert_eq!(session.message_count, 0);
        assert_eq!(session.token_count, 0);
    }

    #[test]
    fn test_day_summary_serialization() {
        let summary = DaySummary {
            session_id: "session_test_2025_01_20".to_string(),
            session_date: "2025-01-20".to_string(),
            summary: Some("Test summary".to_string()),
            message_count: 10,
            is_archived: false,
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: DaySummary = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.session_id, summary.session_id);
        assert_eq!(parsed.summary, summary.summary);
        assert_eq!(parsed.message_count, 10);
        assert!(!parsed.is_archived);
    }

    #[test]
    fn test_session_message_serialization() {
        let message = SessionMessage {
            id: "msg-1".to_string(),
            session_id: "session_test".to_string(),
            role: "user".to_string(),
            content: "Hello, world!".to_string(),
            created_at: Utc::now(),
            token_count: 5,
            tool_calls: None,
            tool_results: None,
        };

        let json = serde_json::to_string(&message).unwrap();
        let parsed: SessionMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "msg-1");
        assert_eq!(parsed.role, "user");
        assert_eq!(parsed.content, "Hello, world!");
        assert_eq!(parsed.token_count, 5);
    }

    #[test]
    fn test_session_message_with_tool_calls() {
        let tool_calls = json!([
            {
                "id": "call_1",
                "name": "search",
                "arguments": {"query": "test"}
            }
        ]);

        let message = SessionMessage {
            id: "msg-1".to_string(),
            session_id: "session_test".to_string(),
            role: "assistant".to_string(),
            content: "".to_string(),
            created_at: Utc::now(),
            token_count: 50,
            tool_calls: Some(tool_calls.clone()),
            tool_results: None,
        };

        assert!(message.tool_calls.is_some());
        assert_eq!(message.tool_calls.unwrap(), tool_calls);
    }

    #[test]
    fn test_agent_serialization() {
        let agent = Agent {
            id: "agent-1".to_string(),
            name: "test-agent".to_string(),
            display_name: "Test Agent".to_string(),
            description: Some("A test agent".to_string()),
            config_path: "/path/to/config.yaml".to_string(),
            system_prompt_version: 2,
            current_system_prompt: Some("You are helpful".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&agent).unwrap();
        let parsed: Agent = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "agent-1");
        assert_eq!(parsed.name, "test-agent");
        assert_eq!(parsed.system_prompt_version, 2);
        assert!(parsed.current_system_prompt.is_some());
    }

    #[test]
    fn test_system_prompt_check_changed() {
        let check = SystemPromptCheck {
            has_changed: true,
            previous_version: Some(1),
            new_version: 2,
            previous_prompt: Some("Old prompt".to_string()),
        };

        assert!(check.has_changed);
        assert_eq!(check.previous_version, Some(1));
        assert_eq!(check.new_version, 2);
    }

    #[test]
    fn test_system_prompt_check_unchanged() {
        let check = SystemPromptCheck {
            has_changed: false,
            previous_version: None,
            new_version: 1,
            previous_prompt: None,
        };

        assert!(!check.has_changed);
        assert_eq!(check.new_version, 1);
    }

    #[test]
    fn test_daily_session_error_display() {
        let err = DailySessionError::NotFound("session-123".to_string());
        assert!(err.to_string().contains("Not found"));
        assert!(err.to_string().contains("session-123"));
    }

    #[test]
    fn test_daily_session_error_database() {
        let db_err = rusqlite::Error::QueryReturnedNoRows;
        let err = DailySessionError::Database(db_err);
        assert!(err.to_string().contains("Database error"));
    }

    #[test]
    fn test_daily_session_error_serialization() {
        // Create a serialization error by trying to deserialize invalid JSON
        let invalid_json = "not valid json";
        let ser_err = serde_json::from_str::<serde_json::Value>(invalid_json).unwrap_err();
        let err = DailySessionError::Serialization(ser_err);
        assert!(err.to_string().contains("Serialization error"));
    }
}
