//! # HTTP Handlers
//!
//! Axum handlers for the session state API.

use crate::repository::StateDbProvider;
use crate::service::StateService;
use crate::types::*;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

// ============================================================================
// SESSION HANDLERS
// ============================================================================

/// List sessions (basic).
///
/// GET /sessions?status=...&limit=...
pub async fn list_sessions<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Query(filter): Query<SessionFilter>,
) -> Result<Json<Vec<Session>>, ApiError> {
    let sessions = service
        .list_sessions(&filter)
        .map_err(ApiError::Database)?;

    Ok(Json(sessions))
}

/// List sessions with executions (for dashboard).
///
/// GET /sessions/full?status=...&limit=...
pub async fn list_sessions_full<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Query(filter): Query<SessionFilter>,
) -> Result<Json<Vec<SessionWithExecutions>>, ApiError> {
    let sessions = service
        .list_sessions_with_executions(&filter)
        .map_err(ApiError::Database)?;

    Ok(Json(sessions))
}

/// Get a single session by ID.
///
/// GET /sessions/:id
pub async fn get_session<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(session_id): Path<String>,
) -> Result<Json<Session>, ApiError> {
    let session = service
        .get_session(&session_id)
        .map_err(ApiError::Database)?;

    match session {
        Some(s) => Ok(Json(s)),
        None => Err(ApiError::NotFound(format!(
            "Session not found: {}",
            session_id
        ))),
    }
}

/// Get session with all executions.
///
/// GET /sessions/:id/full
pub async fn get_session_full<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionWithExecutions>, ApiError> {
    let session = service
        .get_session_with_executions(&session_id)
        .map_err(ApiError::Database)?;

    match session {
        Some(s) => Ok(Json(s)),
        None => Err(ApiError::NotFound(format!(
            "Session not found: {}",
            session_id
        ))),
    }
}

/// Delete a session.
///
/// DELETE /sessions/:id
pub async fn delete_session<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(session_id): Path<String>,
) -> Result<Json<DeleteResponse>, ApiError> {
    let deleted = service
        .delete_session(&session_id)
        .map_err(ApiError::Database)?;

    Ok(Json(DeleteResponse {
        deleted,
        id: session_id,
    }))
}

// ============================================================================
// EXECUTION HANDLERS
// ============================================================================

/// List executions with filtering.
///
/// GET /executions?session_id=...&agent_id=...
pub async fn list_executions<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Query(filter): Query<ExecutionFilter>,
) -> Result<Json<Vec<AgentExecution>>, ApiError> {
    let executions = service
        .list_executions(&filter)
        .map_err(ApiError::Database)?;

    Ok(Json(executions))
}

// ============================================================================
// LEGACY COMPATIBILITY HANDLERS
// ============================================================================

/// Legacy execution session format for UI compatibility.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LegacyExecutionSession {
    pub id: String,
    pub conversation_id: String,
    pub agent_id: String,
    pub parent_session_id: Option<String>,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub checkpoint: Option<String>,
    pub error: Option<String>,
}

impl From<AgentExecution> for LegacyExecutionSession {
    fn from(exec: AgentExecution) -> Self {
        Self {
            id: exec.id.clone(),
            conversation_id: exec.session_id.clone(), // Map session_id to conversation_id
            agent_id: exec.agent_id,
            parent_session_id: exec.parent_execution_id,
            status: exec.status.as_str().to_string(),
            created_at: exec.started_at.clone().unwrap_or_default(),
            started_at: exec.started_at,
            completed_at: exec.completed_at,
            tokens_in: exec.tokens_in,
            tokens_out: exec.tokens_out,
            checkpoint: exec.checkpoint.and_then(|c| serde_json::to_string(&c).ok()),
            error: exec.error,
        }
    }
}

/// List execution sessions in legacy format.
///
/// GET /sessions (legacy - returns only ROOT executions as sessions for UI)
pub async fn list_legacy_sessions<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Query(filter): Query<ExecutionFilter>,
) -> Result<Json<Vec<LegacyExecutionSession>>, ApiError> {
    let executions = service
        .list_executions(&filter)
        .map_err(ApiError::Database)?;

    // Only return root executions (one per session) - filter out subagents
    let legacy: Vec<LegacyExecutionSession> = executions
        .into_iter()
        .filter(|e| e.delegation_type == DelegationType::Root)
        .map(|e| e.into())
        .collect();
    Ok(Json(legacy))
}

/// Get a single execution by ID.
///
/// GET /executions/:id
pub async fn get_execution<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(execution_id): Path<String>,
) -> Result<Json<AgentExecution>, ApiError> {
    let execution = service
        .get_execution(&execution_id)
        .map_err(ApiError::Database)?;

    match execution {
        Some(e) => Ok(Json(e)),
        None => Err(ApiError::NotFound(format!(
            "Execution not found: {}",
            execution_id
        ))),
    }
}

/// Get child executions for a parent.
///
/// GET /executions/:id/children
pub async fn get_child_executions<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(execution_id): Path<String>,
) -> Result<Json<Vec<AgentExecution>>, ApiError> {
    let children = service
        .get_child_executions(&execution_id)
        .map_err(ApiError::Database)?;

    Ok(Json(children))
}

// ============================================================================
// CONTROL HANDLERS
// ============================================================================

/// Pause a running session.
///
/// POST /sessions/:id/pause
pub async fn pause_session<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(session_id): Path<String>,
) -> Result<Json<StatusResponse>, ApiError> {
    service
        .pause_session(&session_id)
        .map_err(ApiError::InvalidTransition)?;

    Ok(Json(StatusResponse {
        id: session_id,
        status: "paused".to_string(),
    }))
}

/// Resume a paused session.
///
/// POST /sessions/:id/resume
pub async fn resume_session<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(session_id): Path<String>,
) -> Result<Json<StatusResponse>, ApiError> {
    service
        .resume_session(&session_id)
        .map_err(ApiError::InvalidTransition)?;

    Ok(Json(StatusResponse {
        id: session_id,
        status: "running".to_string(),
    }))
}

/// Cancel a session.
///
/// POST /sessions/:id/cancel
pub async fn cancel_session<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(session_id): Path<String>,
) -> Result<Json<StatusResponse>, ApiError> {
    service
        .cancel_session(&session_id)
        .map_err(ApiError::InvalidTransition)?;

    Ok(Json(StatusResponse {
        id: session_id,
        status: "cancelled".to_string(),
    }))
}

// ============================================================================
// STATS HANDLERS
// ============================================================================

/// Get dashboard stats (pre-computed, ready to display).
///
/// GET /stats
pub async fn get_dashboard_stats<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
) -> Result<Json<DashboardStats>, ApiError> {
    let stats = service
        .get_dashboard_stats()
        .map_err(ApiError::Database)?;

    Ok(Json(stats))
}

/// Get stats as counts map (legacy format for UI compatibility).
///
/// GET /stats/counts
pub async fn get_stats_counts<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
) -> Result<Json<std::collections::HashMap<String, u64>>, ApiError> {
    let stats = service
        .get_dashboard_stats()
        .map_err(ApiError::Database)?;

    let mut counts = std::collections::HashMap::new();
    counts.insert("running".to_string(), stats.running);
    counts.insert("paused".to_string(), stats.paused);
    counts.insert("completed".to_string(), stats.completed);
    counts.insert("crashed".to_string(), stats.crashed);
    counts.insert("today_count".to_string(), stats.today_count);
    counts.insert("today_tokens".to_string(), stats.today_tokens);

    Ok(Json(counts))
}

// ============================================================================
// MESSAGES HANDLERS
// ============================================================================

/// Get messages for an execution.
///
/// GET /executions/:id/messages
pub async fn get_execution_messages<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(execution_id): Path<String>,
) -> Result<Json<Vec<Message>>, ApiError> {
    let messages = service
        .get_messages(&execution_id)
        .map_err(ApiError::Database)?;

    Ok(Json(messages))
}

// ============================================================================
// CLEANUP HANDLERS
// ============================================================================

/// Parameters for cleanup endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct CleanupParams {
    pub older_than: Option<String>,
}

/// Response for cleanup operations.
#[derive(Debug, serde::Serialize)]
pub struct CleanupResponse {
    pub deleted: u64,
}

/// Cleanup old sessions.
///
/// DELETE /cleanup?older_than=<RFC3339 timestamp>
pub async fn cleanup_old_sessions<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Query(params): Query<CleanupParams>,
) -> Result<Json<CleanupResponse>, ApiError> {
    let older_than = params
        .older_than
        .ok_or_else(|| ApiError::BadRequest("older_than parameter required".to_string()))?;

    let deleted = service
        .delete_old_sessions(&older_than)
        .map_err(ApiError::Database)?;

    Ok(Json(CleanupResponse { deleted }))
}

// ============================================================================
// RESPONSE TYPES
// ============================================================================

/// Response for delete operations.
#[derive(Debug, serde::Serialize)]
pub struct DeleteResponse {
    pub deleted: bool,
    pub id: String,
}

/// Response for status change operations.
#[derive(Debug, serde::Serialize)]
pub struct StatusResponse {
    pub id: String,
    pub status: String,
}

/// Message type for API responses.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub id: String,
    pub execution_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_results: Option<serde_json::Value>,
}
