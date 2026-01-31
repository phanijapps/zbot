//! # Execution State Types
//!
//! Core types for session state tracking, token metrics, and checkpointing.

use serde::{Deserialize, Serialize};

// ============================================================================
// EXECUTION STATUS
// ============================================================================

/// Status of an execution session.
///
/// State transitions:
/// ```text
/// QUEUED → RUNNING → PAUSED ⇄ RUNNING → COMPLETED
///                  → CRASHED ⇄ RUNNING
///                  → CANCELLED
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    /// Created but not yet started
    Queued,
    /// Actively executing
    Running,
    /// User-initiated pause (resumable)
    Paused,
    /// Daemon crashed during execution (resumable)
    Crashed,
    /// User cancelled (terminal)
    Cancelled,
    /// Successfully finished (terminal)
    Completed,
}

impl ExecutionStatus {
    /// Get string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Crashed => "crashed",
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
        }
    }

    /// Check if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Cancelled | Self::Completed)
    }

    /// Check if this state is resumable.
    pub fn is_resumable(&self) -> bool {
        matches!(self, Self::Paused | Self::Crashed)
    }
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ExecutionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "crashed" => Ok(Self::Crashed),
            "cancelled" => Ok(Self::Cancelled),
            "completed" => Ok(Self::Completed),
            _ => Err(format!("Invalid execution status: {}", s)),
        }
    }
}

// ============================================================================
// EXECUTION SESSION
// ============================================================================

/// An execution session record.
///
/// Tracks the lifecycle of a single agent invocation, including:
/// - Status (queued, running, paused, etc.)
/// - Token consumption (input/output)
/// - Timing (start, end, duration)
/// - Recovery data (checkpoint for resume)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSession {
    /// Unique session ID
    pub id: String,

    /// Conversation this session belongs to
    pub conversation_id: String,

    /// Agent being executed
    pub agent_id: String,

    /// Parent session ID (for delegated subagents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,

    /// Current status
    pub status: ExecutionStatus,

    /// When the session was created (RFC3339)
    pub created_at: String,

    /// When execution started (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,

    /// When execution completed (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,

    /// Input tokens consumed (prompt tokens)
    pub tokens_in: u64,

    /// Output tokens consumed (completion tokens)
    pub tokens_out: u64,

    /// Checkpoint for resumption
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<Checkpoint>,

    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ExecutionSession {
    /// Create a new session in QUEUED state.
    pub fn new(
        conversation_id: impl Into<String>,
        agent_id: impl Into<String>,
        parent_session_id: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conversation_id.into(),
            agent_id: agent_id.into(),
            parent_session_id,
            status: ExecutionStatus::Queued,
            created_at: chrono::Utc::now().to_rfc3339(),
            started_at: None,
            completed_at: None,
            tokens_in: 0,
            tokens_out: 0,
            checkpoint: None,
            error: None,
        }
    }

    /// Calculate duration in milliseconds (if completed).
    pub fn duration_ms(&self) -> Option<i64> {
        let started = self.started_at.as_ref()?;
        let completed = self.completed_at.as_ref()?;

        let start = chrono::DateTime::parse_from_rfc3339(started).ok()?;
        let end = chrono::DateTime::parse_from_rfc3339(completed).ok()?;

        Some((end - start).num_milliseconds())
    }

    /// Total tokens (in + out).
    pub fn total_tokens(&self) -> u64 {
        self.tokens_in + self.tokens_out
    }
}

// ============================================================================
// CHECKPOINT
// ============================================================================

/// Checkpoint data for resuming a paused or crashed session.
///
/// Contains enough state to restore the executor and continue
/// from where it left off.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// LLM turn number (how many LLM calls have been made)
    pub llm_turn: u32,

    /// ID of the last processed message
    pub last_message_id: String,

    /// Tool calls that were in progress when paused
    #[serde(default)]
    pub pending_tool_calls: Vec<PendingToolCall>,

    /// Snapshot of context state (key-value pairs)
    #[serde(default)]
    pub context_state: serde_json::Value,

    /// IDs of child sessions (subagents) that were active
    #[serde(default)]
    pub child_sessions: Vec<String>,
}

impl Checkpoint {
    /// Create a new checkpoint.
    pub fn new(llm_turn: u32, last_message_id: impl Into<String>) -> Self {
        Self {
            llm_turn,
            last_message_id: last_message_id.into(),
            pending_tool_calls: Vec::new(),
            context_state: serde_json::Value::Null,
            child_sessions: Vec::new(),
        }
    }
}

/// A tool call that was pending when the session was paused.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingToolCall {
    /// Tool call ID
    pub id: String,

    /// Tool name
    pub name: String,

    /// Arguments passed to the tool
    pub arguments: serde_json::Value,
}

// ============================================================================
// TOKEN UPDATE
// ============================================================================

/// Token consumption update event.
///
/// Emitted after each LLM call to track token usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUpdate {
    /// Session being updated
    pub session_id: String,

    /// Cumulative input tokens
    pub tokens_in: u64,

    /// Cumulative output tokens
    pub tokens_out: u64,
}

// ============================================================================
// DAILY SUMMARY
// ============================================================================

/// Daily aggregate of token consumption and session metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySummary {
    /// Date (YYYY-MM-DD)
    pub date: String,

    /// Total input tokens
    pub total_tokens_in: u64,

    /// Total output tokens
    pub total_tokens_out: u64,

    /// Number of sessions
    pub session_count: u64,

    /// Number of completed sessions
    pub completed_count: u64,

    /// Number of failed sessions
    pub failed_count: u64,
}

impl DailySummary {
    /// Token ratio (in/out). Higher means more input-heavy.
    pub fn token_ratio(&self) -> f64 {
        if self.total_tokens_out == 0 {
            return 0.0;
        }
        self.total_tokens_in as f64 / self.total_tokens_out as f64
    }
}

// ============================================================================
// FILTER
// ============================================================================

/// Filter criteria for querying sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionFilter {
    /// Filter by agent ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,

    /// Filter by conversation ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,

    /// Filter by status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ExecutionStatus>,

    /// Filter by parent session (for subagents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,

    /// Sessions created after this time (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_time: Option<String>,

    /// Sessions created before this time (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_time: Option<String>,

    /// Maximum number of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,

    /// Offset for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

// ============================================================================
// API ERROR
// ============================================================================

/// API error type for handlers.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Invalid state transition: {0}")]
    InvalidTransition(String),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, message) = match &self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::Database(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::InvalidTransition(msg) => (StatusCode::CONFLICT, msg.clone()),
        };

        let body = serde_json::json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}
