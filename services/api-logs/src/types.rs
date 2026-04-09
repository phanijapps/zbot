//! # Log Types
//!
//! Core types for execution logging.

use serde::{Deserialize, Serialize};

// ============================================================================
// LOG LEVELS
// ============================================================================

/// Log level for execution events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

// ============================================================================
// LOG CATEGORIES
// ============================================================================

/// Category of log entry for filtering and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogCategory {
    /// Session lifecycle (start, end)
    Session,
    /// Token streaming
    Token,
    /// Tool invocation
    ToolCall,
    /// Tool result
    ToolResult,
    /// Thinking/reasoning content
    Thinking,
    /// Delegation to subagent
    Delegation,
    /// System messages
    System,
    /// Errors
    Error,
    /// Agent's final response content
    Response,
    /// Intent analysis results
    Intent,
}

impl LogCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Token => "token",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
            Self::Thinking => "thinking",
            Self::Delegation => "delegation",
            Self::System => "system",
            Self::Error => "error",
            Self::Response => "response",
            Self::Intent => "intent",
        }
    }
}

impl std::fmt::Display for LogCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for LogCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "session" => Ok(Self::Session),
            "token" => Ok(Self::Token),
            "tool_call" => Ok(Self::ToolCall),
            "tool_result" => Ok(Self::ToolResult),
            "thinking" => Ok(Self::Thinking),
            "delegation" => Ok(Self::Delegation),
            "system" => Ok(Self::System),
            "error" => Ok(Self::Error),
            "response" => Ok(Self::Response),
            "intent" => Ok(Self::Intent),
            _ => Err(format!("Invalid log category: {}", s)),
        }
    }
}

// ============================================================================
// SESSION STATUS
// ============================================================================

/// Status of an execution session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Running,
    Completed,
    Error,
    Stopped,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Error => "error",
            Self::Stopped => "stopped",
        }
    }
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for SessionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "error" => Ok(Self::Error),
            "stopped" => Ok(Self::Stopped),
            _ => Err(format!("Invalid session status: {}", s)),
        }
    }
}

// ============================================================================
// EXECUTION LOG
// ============================================================================

/// A single execution log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLog {
    /// Unique log entry ID
    pub id: String,
    /// Session this log belongs to
    pub session_id: String,
    /// Conversation ID
    pub conversation_id: String,
    /// Agent ID
    pub agent_id: String,
    /// Parent session ID (for delegation tracking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    /// Timestamp (RFC3339)
    pub timestamp: String,
    /// Log level
    pub level: LogLevel,
    /// Log category
    pub category: LogCategory,
    /// Human-readable message
    pub message: String,
    /// Additional metadata (JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Duration in milliseconds (for timed operations like tool calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}

impl ExecutionLog {
    /// Create a new log entry with generated ID and current timestamp.
    pub fn new(
        session_id: impl Into<String>,
        conversation_id: impl Into<String>,
        agent_id: impl Into<String>,
        level: LogLevel,
        category: LogCategory,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            conversation_id: conversation_id.into(),
            agent_id: agent_id.into(),
            parent_session_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            level,
            category,
            message: message.into(),
            metadata: None,
            duration_ms: None,
        }
    }

    /// Set parent session ID.
    pub fn with_parent(mut self, parent_session_id: impl Into<String>) -> Self {
        self.parent_session_id = Some(parent_session_id.into());
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set duration.
    pub fn with_duration(mut self, duration_ms: i64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

// ============================================================================
// LOG SESSION
// ============================================================================

/// Summary of an execution session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSession {
    /// Unique session ID
    pub session_id: String,
    /// Conversation ID
    pub conversation_id: String,
    /// Agent ID
    pub agent_id: String,
    /// Agent display name
    pub agent_name: String,
    /// Title derived from the first user message in the session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Session start time (RFC3339)
    pub started_at: String,
    /// Session end time (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// Session status
    pub status: SessionStatus,
    /// Total tokens logged
    pub token_count: i32,
    /// Number of tool calls
    pub tool_call_count: i32,
    /// Number of errors
    pub error_count: i32,
    /// Total duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    /// Parent session ID (for delegated sessions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    /// Child session IDs
    #[serde(default)]
    pub child_session_ids: Vec<String>,
}

// ============================================================================
// SESSION DETAIL
// ============================================================================

/// Detailed view of a session with all logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetail {
    /// Session summary
    pub session: LogSession,
    /// All log entries for this session
    pub logs: Vec<ExecutionLog>,
}

// ============================================================================
// LOG FILTER
// ============================================================================

/// Filter criteria for querying logs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogFilter {
    /// Filter by agent ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Filter by conversation ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    /// Filter by log level (minimum)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// Filter logs after this time (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_time: Option<String>,
    /// Filter logs before this time (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_time: Option<String>,
    /// Maximum number of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Offset for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    /// Only return root sessions (no parent)
    #[serde(default)]
    pub root_only: bool,
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
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, message) = match &self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::Database(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        };

        let body = serde_json::json!({
            "error": message
        });

        (status, axum::Json(body)).into_response()
    }
}
