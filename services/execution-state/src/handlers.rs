//! # HTTP Handlers
//!
//! Axum handlers for the execution state API.

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

/// List execution sessions with optional filtering.
///
/// GET /sessions?agent_id=...&status=...&limit=...
pub async fn list_sessions<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Query(filter): Query<SessionFilter>,
) -> Result<Json<Vec<ExecutionSession>>, ApiError> {
    let sessions = service
        .list_sessions(&filter)
        .map_err(ApiError::Database)?;

    Ok(Json(sessions))
}

/// Get a single session by ID.
///
/// GET /sessions/:id
pub async fn get_session<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(session_id): Path<String>,
) -> Result<Json<ExecutionSession>, ApiError> {
    let session = service
        .find_session(&session_id)
        .map_err(ApiError::Database)?;

    match session {
        Some(s) => Ok(Json(s)),
        None => Err(ApiError::NotFound(format!(
            "Session not found: {}",
            session_id
        ))),
    }
}

/// Get child sessions for a parent.
///
/// GET /sessions/:id/children
pub async fn get_children<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(session_id): Path<String>,
) -> Result<Json<Vec<ExecutionSession>>, ApiError> {
    let children = service
        .get_child_sessions(&session_id)
        .map_err(ApiError::Database)?;

    Ok(Json(children))
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
        session_id,
    }))
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
        session_id,
        status: ExecutionStatus::Paused,
    }))
}

/// Resume a paused or crashed session.
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
        session_id,
        status: ExecutionStatus::Running,
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
        session_id,
        status: ExecutionStatus::Cancelled,
    }))
}

// ============================================================================
// AGGREGATE HANDLERS
// ============================================================================

/// Get status counts.
///
/// GET /stats/counts
pub async fn get_status_counts<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
) -> Result<Json<std::collections::HashMap<String, u64>>, ApiError> {
    let counts = service
        .get_status_counts()
        .map_err(ApiError::Database)?;

    Ok(Json(counts))
}

/// Get daily summary.
///
/// GET /stats/daily/:date
pub async fn get_daily_summary<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
    Path(date): Path<String>,
) -> Result<Json<DailySummary>, ApiError> {
    let summary = service
        .get_daily_summary(&date)
        .map_err(ApiError::Database)?;

    Ok(Json(summary))
}

/// Get resumable sessions.
///
/// GET /resumable
pub async fn get_resumable<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
) -> Result<Json<Vec<ExecutionSession>>, ApiError> {
    let sessions = service
        .get_resumable_sessions()
        .map_err(ApiError::Database)?;

    Ok(Json(sessions))
}

/// Get running sessions.
///
/// GET /running
pub async fn get_running<D: StateDbProvider + 'static>(
    State(service): State<Arc<StateService<D>>>,
) -> Result<Json<Vec<ExecutionSession>>, ApiError> {
    let sessions = service
        .get_running_sessions()
        .map_err(ApiError::Database)?;

    Ok(Json(sessions))
}

// ============================================================================
// CLEANUP HANDLERS
// ============================================================================

/// Cleanup old completed sessions.
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
        .cleanup_old_sessions(&older_than)
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
    pub session_id: String,
}

/// Response for status change operations.
#[derive(Debug, serde::Serialize)]
pub struct StatusResponse {
    pub session_id: String,
    pub status: ExecutionStatus,
}

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
