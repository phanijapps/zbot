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
