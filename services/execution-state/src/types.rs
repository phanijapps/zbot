//! # Session State Types
//!
//! Core types for session tracking, agent executions, and checkpointing.

use serde::{Deserialize, Serialize};

// ============================================================================
// SESSION STATUS
// ============================================================================

/// Status of a session (top-level container).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// At least one execution is running
    Running,
    /// User paused the session
    Paused,
    /// All executions completed successfully
    Completed,
    /// Root execution crashed
    Crashed,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Crashed => "crashed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Crashed)
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
            "paused" => Ok(Self::Paused),
            "completed" => Ok(Self::Completed),
            "crashed" => Ok(Self::Crashed),
            _ => Err(format!("Invalid session status: {}", s)),
        }
    }
}

// ============================================================================
// EXECUTION STATUS
// ============================================================================

/// Status of an agent execution within a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    /// Created but not yet started
    Queued,
    /// Actively executing
    Running,
    /// Paused (session paused or waiting)
    Paused,
    /// Failed with error
    Crashed,
    /// Cancelled by user or parent
    Cancelled,
    /// Successfully finished
    Completed,
}

impl ExecutionStatus {
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

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Cancelled | Self::Completed | Self::Crashed)
    }

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
// DELEGATION TYPE
// ============================================================================

/// How an agent execution was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DelegationType {
    /// Root agent (started by user)
    Root,
    /// Delegated sequentially (parent waits)
    Sequential,
    /// Delegated in parallel (parent continues)
    Parallel,
}

impl DelegationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Sequential => "sequential",
            Self::Parallel => "parallel",
        }
    }
}

impl std::fmt::Display for DelegationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for DelegationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "root" => Ok(Self::Root),
            "sequential" => Ok(Self::Sequential),
            "parallel" => Ok(Self::Parallel),
            _ => Err(format!("Invalid delegation type: {}", s)),
        }
    }
}

// ============================================================================
// SESSION
// ============================================================================

/// A user's work session - top-level container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session ID (sess-{uuid})
    pub id: String,

    /// Current status
    pub status: SessionStatus,

    /// The root agent for this session
    pub root_agent_id: String,

    /// Optional title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// When the session was created (RFC3339)
    pub created_at: String,

    /// When first execution started (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,

    /// When session completed (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,

    /// Total input tokens across all executions
    pub total_tokens_in: u64,

    /// Total output tokens across all executions
    pub total_tokens_out: u64,

    /// JSON metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl Session {
    /// Create a new session in RUNNING state.
    pub fn new(root_agent_id: impl Into<String>) -> Self {
        Self {
            id: format!("sess-{}", uuid::Uuid::new_v4()),
            status: SessionStatus::Running,
            root_agent_id: root_agent_id.into(),
            title: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            started_at: None,
            completed_at: None,
            total_tokens_in: 0,
            total_tokens_out: 0,
            metadata: None,
        }
    }

    /// Total tokens (in + out).
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens_in + self.total_tokens_out
    }
}

// ============================================================================
// AGENT EXECUTION
// ============================================================================

/// An agent's participation in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecution {
    /// Unique execution ID (exec-{uuid})
    pub id: String,

    /// Session this execution belongs to
    pub session_id: String,

    /// Agent being executed
    pub agent_id: String,

    /// Parent execution ID (for delegated subagents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_execution_id: Option<String>,

    /// How this execution was created
    pub delegation_type: DelegationType,

    /// Task description (for subagents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,

    /// Current status
    pub status: ExecutionStatus,

    /// When execution started (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,

    /// When execution completed (RFC3339)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,

    /// Input tokens consumed
    pub tokens_in: u64,

    /// Output tokens consumed
    pub tokens_out: u64,

    /// Checkpoint for resumption
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<Checkpoint>,

    /// Error message if crashed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Relative path to log file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
}

impl AgentExecution {
    /// Create a new root execution in QUEUED state.
    pub fn new_root(session_id: impl Into<String>, agent_id: impl Into<String>) -> Self {
        Self {
            id: format!("exec-{}", uuid::Uuid::new_v4()),
            session_id: session_id.into(),
            agent_id: agent_id.into(),
            parent_execution_id: None,
            delegation_type: DelegationType::Root,
            task: None,
            status: ExecutionStatus::Queued,
            started_at: None,
            completed_at: None,
            tokens_in: 0,
            tokens_out: 0,
            checkpoint: None,
            error: None,
            log_path: None,
        }
    }

    /// Create a new delegated execution.
    pub fn new_delegated(
        session_id: impl Into<String>,
        agent_id: impl Into<String>,
        parent_execution_id: impl Into<String>,
        delegation_type: DelegationType,
        task: impl Into<String>,
    ) -> Self {
        Self {
            id: format!("exec-{}", uuid::Uuid::new_v4()),
            session_id: session_id.into(),
            agent_id: agent_id.into(),
            parent_execution_id: Some(parent_execution_id.into()),
            delegation_type,
            task: Some(task.into()),
            status: ExecutionStatus::Queued,
            started_at: None,
            completed_at: None,
            tokens_in: 0,
            tokens_out: 0,
            checkpoint: None,
            error: None,
            log_path: None,
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

    /// Check if this is a root execution.
    pub fn is_root(&self) -> bool {
        self.delegation_type == DelegationType::Root
    }
}

// ============================================================================
// SESSION WITH EXECUTIONS (API response)
// ============================================================================

/// Session with all its executions - for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionWithExecutions {
    /// The session
    #[serde(flatten)]
    pub session: Session,

    /// All executions in this session
    pub executions: Vec<AgentExecution>,

    /// Number of subagent executions
    pub subagent_count: u32,
}

// ============================================================================
// CHECKPOINT
// ============================================================================

/// Checkpoint data for resuming a paused or crashed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// LLM turn number
    pub llm_turn: u32,

    /// ID of the last processed message
    pub last_message_id: String,

    /// Tool calls that were in progress
    #[serde(default)]
    pub pending_tool_calls: Vec<PendingToolCall>,

    /// Snapshot of context state
    #[serde(default)]
    pub context_state: serde_json::Value,

    /// IDs of child executions that were active
    #[serde(default)]
    pub child_executions: Vec<String>,
}

impl Checkpoint {
    pub fn new(llm_turn: u32, last_message_id: impl Into<String>) -> Self {
        Self {
            llm_turn,
            last_message_id: last_message_id.into(),
            pending_tool_calls: Vec::new(),
            context_state: serde_json::Value::Null,
            child_executions: Vec::new(),
        }
    }
}

/// A tool call that was pending when execution was paused.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

// ============================================================================
// STATS (API response - ready to display)
// ============================================================================

/// Dashboard stats - pre-computed, ready for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    /// Number of running sessions
    pub running: u64,

    /// Number of paused sessions
    pub paused: u64,

    /// Number of completed sessions
    pub completed: u64,

    /// Number of crashed sessions
    pub crashed: u64,

    /// Total sessions today
    pub today_count: u64,

    /// Total tokens today
    pub today_tokens: u64,
}

// ============================================================================
// FILTERS
// ============================================================================

/// Filter criteria for querying sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<SessionStatus>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_agent_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_time: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_time: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

/// Filter criteria for querying executions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ExecutionStatus>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_execution_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,

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

// ============================================================================
// BACKWARDS COMPAT (temporary aliases)
// ============================================================================

/// Alias for backwards compatibility during migration.
pub type ExecutionSession = AgentExecution;
