//! # Archive Schema
//!
//! Defines the Parquet schema for archived sessions.

use std::sync::Arc;

use arrow::array::*;
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Archived message stored in Parquet format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedMessage {
    /// Message ID
    pub id: String,
    /// Session ID
    pub session_id: String,
    /// Agent ID
    pub agent_id: String,
    /// Agent name (denormalized for convenience)
    pub agent_name: String,
    /// Message role (user, assistant, system, tool)
    pub role: String,
    /// Message content
    pub content: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Token count (if available)
    pub token_count: Option<i64>,
    /// Tool calls (JSON string, if any)
    pub tool_calls: Option<String>,
    /// Tool results (JSON string, if any)
    pub tool_results: Option<String>,
}

/// Archive metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMetadata {
    /// Agent ID
    pub agent_id: String,
    /// Session ID
    pub session_id: String,
    /// Session date (YYYY-MM-DD)
    pub session_date: String,
    /// Message count
    pub message_count: usize,
    /// Earliest message timestamp
    pub earliest_message: DateTime<Utc>,
    /// Latest message timestamp
    pub latest_message: DateTime<Utc>,
    /// Total tokens
    pub total_tokens: i64,
    /// File size in bytes
    pub file_size: u64,
    /// Parquet file path
    pub file_path: String,
}

/// Create the Arrow schema for archived messages
pub fn arrow_schema() -> arrow::datatypes::Schema {
    arrow::datatypes::Schema::new(vec![
        arrow::datatypes::Field::new("id", arrow::datatypes::DataType::Utf8, false),
        arrow::datatypes::Field::new("session_id", arrow::datatypes::DataType::Utf8, false),
        arrow::datatypes::Field::new("agent_id", arrow::datatypes::DataType::Utf8, false),
        arrow::datatypes::Field::new("agent_name", arrow::datatypes::DataType::Utf8, false),
        arrow::datatypes::Field::new("role", arrow::datatypes::DataType::Utf8, false),
        arrow::datatypes::Field::new("content", arrow::datatypes::DataType::LargeUtf8, false),
        arrow::datatypes::Field::new("created_at", arrow::datatypes::DataType::Timestamp(arrow::datatypes::TimeUnit::Millisecond, None), false),
        arrow::datatypes::Field::new("token_count", arrow::datatypes::DataType::Int64, true),
        arrow::datatypes::Field::new("tool_calls", arrow::datatypes::DataType::Utf8, true),
        arrow::datatypes::Field::new("tool_results", arrow::datatypes::DataType::Utf8, true),
    ])
}

/// Convert archived messages to a RecordBatch
pub fn messages_to_record_batch(messages: Vec<ArchivedMessage>) -> crate::error::ArchiveResult<RecordBatch> {
    let schema = arrow_schema();

    let ids: StringArray = messages.iter().map(|m| Some(m.id.as_str())).collect();
    let session_ids: StringArray = messages.iter().map(|m| Some(m.session_id.as_str())).collect();
    let agent_ids: StringArray = messages.iter().map(|m| Some(m.agent_id.as_str())).collect();
    let agent_names: StringArray = messages.iter().map(|m| Some(m.agent_name.as_str())).collect();
    let roles: StringArray = messages.iter().map(|m| Some(m.role.as_str())).collect();
    let contents: LargeStringArray = messages.iter().map(|m| Some(m.content.as_str())).collect();
    let created_at: TimestampMillisecondArray = messages.iter()
        .map(|m| Some(m.created_at.timestamp_millis()))
        .collect();
    let token_counts: Int64Array = messages.iter().map(|m| m.token_count).collect();
    let tool_calls: StringArray = messages.iter()
        .map(|m| m.tool_calls.as_deref())
        .collect();
    let tool_results: StringArray = messages.iter()
        .map(|m| m.tool_results.as_deref())
        .collect();

    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(ids),
            Arc::new(session_ids),
            Arc::new(agent_ids),
            Arc::new(agent_names),
            Arc::new(roles),
            Arc::new(contents),
            Arc::new(created_at),
            Arc::new(token_counts),
            Arc::new(tool_calls),
            Arc::new(tool_results),
        ],
    ).map_err(|e| crate::error::ArchiveError::Arrow(e))
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archived_message_serialization() {
        let msg = ArchivedMessage {
            id: "msg-1".to_string(),
            session_id: "session-1".to_string(),
            agent_id: "agent-1".to_string(),
            agent_name: "Test Agent".to_string(),
            role: "user".to_string(),
            content: "Hello, world!".to_string(),
            created_at: Utc::now(),
            token_count: Some(5),
            tool_calls: None,
            tool_results: None,
        };

        let json_str = serde_json::to_string(&msg).unwrap();
        let parsed: ArchivedMessage = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.id, "msg-1");
        assert_eq!(parsed.role, "user");
        assert_eq!(parsed.content, "Hello, world!");
        assert_eq!(parsed.token_count, Some(5));
    }

    #[test]
    fn test_archived_message_with_tool_calls() {
        let tool_calls = serde_json::json!([{"id": "call_1", "name": "search"}]);

        let msg = ArchivedMessage {
            id: "msg-1".to_string(),
            session_id: "session-1".to_string(),
            agent_id: "agent-1".to_string(),
            agent_name: "Test Agent".to_string(),
            role: "assistant".to_string(),
            content: "".to_string(),
            created_at: Utc::now(),
            token_count: None,
            tool_calls: Some(tool_calls.to_string()),
            tool_results: None,
        };

        assert!(msg.tool_calls.is_some());
        assert!(msg.tool_results.is_none());
    }

    #[test]
    fn test_archive_metadata() {
        let now = Utc::now();
        let metadata = ArchiveMetadata {
            agent_id: "agent-1".to_string(),
            session_id: "session-1".to_string(),
            session_date: "2025-01-20".to_string(),
            message_count: 10,
            earliest_message: now,
            latest_message: now + chrono::Duration::hours(1),
            total_tokens: 500,
            file_size: 1024,
            file_path: "/archive/session.parquet".to_string(),
        };

        assert_eq!(metadata.agent_id, "agent-1");
        assert_eq!(metadata.message_count, 10);
        assert_eq!(metadata.total_tokens, 500);
        assert_eq!(metadata.file_size, 1024);
    }

    #[test]
    fn test_archive_metadata_serialization() {
        let now = Utc::now();
        let metadata = ArchiveMetadata {
            agent_id: "agent-1".to_string(),
            session_id: "session-1".to_string(),
            session_date: "2025-01-20".to_string(),
            message_count: 10,
            earliest_message: now,
            latest_message: now,
            total_tokens: 0,
            file_size: 0,
            file_path: "/archive/session.parquet".to_string(),
        };

        let json_str = serde_json::to_string(&metadata).unwrap();
        let parsed: ArchiveMetadata = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.agent_id, "agent-1");
        assert_eq!(parsed.session_date, "2025-01-20");
    }

    #[test]
    fn test_arrow_schema() {
        let schema = arrow_schema();
        assert_eq!(schema.fields().len(), 10);

        let field_names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
        assert!(field_names.contains(&"id"));
        assert!(field_names.contains(&"session_id"));
        assert!(field_names.contains(&"agent_id"));
        assert!(field_names.contains(&"role"));
        assert!(field_names.contains(&"content"));
        assert!(field_names.contains(&"created_at"));
        assert!(field_names.contains(&"token_count"));
        assert!(field_names.contains(&"tool_calls"));
        assert!(field_names.contains(&"tool_results"));
    }

    #[test]
    fn test_messages_to_record_batch() {
        let now = Utc::now();
        let messages = vec![
            ArchivedMessage {
                id: "msg-1".to_string(),
                session_id: "session-1".to_string(),
                agent_id: "agent-1".to_string(),
                agent_name: "Agent".to_string(),
                role: "user".to_string(),
                content: "Hello".to_string(),
                created_at: now,
                token_count: Some(2),
                tool_calls: None,
                tool_results: None,
            },
            ArchivedMessage {
                id: "msg-2".to_string(),
                session_id: "session-1".to_string(),
                agent_id: "agent-1".to_string(),
                agent_name: "Agent".to_string(),
                role: "assistant".to_string(),
                content: "Hi there!".to_string(),
                created_at: now + chrono::Duration::seconds(1),
                token_count: Some(3),
                tool_calls: None,
                tool_results: None,
            },
        ];

        let result = messages_to_record_batch(messages);
        assert!(result.is_ok());

        let batch = result.unwrap();
        assert_eq!(batch.num_rows(), 2);
    }

    #[test]
    fn test_messages_to_record_batch_empty() {
        let result = messages_to_record_batch(vec![]);
        assert!(result.is_ok());

        let batch = result.unwrap();
        assert_eq!(batch.num_rows(), 0);
    }

    #[test]
    fn test_messages_to_record_batch_with_tool_calls() {
        let now = Utc::now();
        let tool_calls = serde_json::json!([{"id": "call_1"}]);

        let messages = vec![ArchivedMessage {
            id: "msg-1".to_string(),
            session_id: "session-1".to_string(),
            agent_id: "agent-1".to_string(),
            agent_name: "Agent".to_string(),
            role: "assistant".to_string(),
            content: "".to_string(),
            created_at: now,
            token_count: None,
            tool_calls: Some(tool_calls.to_string()),
            tool_results: None,
        }];

        let result = messages_to_record_batch(messages);
        assert!(result.is_ok());
    }
}
