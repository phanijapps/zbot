//! # HTTP Handlers
//!
//! Axum handlers for the logs API.

use crate::repository::DbProvider;
use crate::service::LogService;
use crate::types::*;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

// ============================================================================
// HANDLERS
// ============================================================================

/// List execution sessions with optional filtering.
///
/// GET /sessions?agent_id=...&from_time=...&limit=...
pub async fn list_sessions<D: DbProvider + 'static>(
    State(service): State<Arc<LogService<D>>>,
    Query(filter): Query<LogFilter>,
) -> Result<Json<Vec<LogSession>>, ApiError> {
    let sessions = service
        .list_sessions(&filter)
        .map_err(ApiError::Database)?;

    Ok(Json(sessions))
}

/// Get a single session with all its logs.
///
/// GET /sessions/:id
pub async fn get_session<D: DbProvider + 'static>(
    State(service): State<Arc<LogService<D>>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionDetail>, ApiError> {
    let detail = service
        .get_session_detail(&session_id)
        .map_err(ApiError::Database)?;

    match detail {
        Some(d) => Ok(Json(d)),
        None => Err(ApiError::NotFound(format!(
            "Session not found: {}",
            session_id
        ))),
    }
}

/// Delete a session and all its logs.
///
/// DELETE /sessions/:id
pub async fn delete_session<D: DbProvider + 'static>(
    State(service): State<Arc<LogService<D>>>,
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

/// Cleanup old logs.
///
/// DELETE /cleanup?older_than=<RFC3339 timestamp>
pub async fn cleanup_old_logs<D: DbProvider + 'static>(
    State(service): State<Arc<LogService<D>>>,
    Query(params): Query<CleanupParams>,
) -> Result<Json<CleanupResponse>, ApiError> {
    let older_than = params
        .older_than
        .ok_or_else(|| ApiError::BadRequest("older_than parameter required".to_string()))?;

    let deleted = service
        .delete_old_logs(&older_than)
        .map_err(ApiError::Database)?;

    Ok(Json(CleanupResponse { deleted }))
}

// ============================================================================
// RESPONSE TYPES
// ============================================================================

/// Response for delete operations.
#[derive(Debug, serde::Serialize)]
pub struct DeleteResponse {
    pub deleted: u64,
    pub session_id: String,
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
