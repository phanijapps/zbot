//! # Session Archive Endpoints
//!
//! HTTP API for archiving and restoring session transcripts.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use gateway_execution::{SessionState, SessionStateBuilder};
use serde::{Deserialize, Serialize};

// ============================================================================
// REQUEST / RESPONSE TYPES
// ============================================================================

/// Request body for archiving old sessions.
#[derive(Debug, Deserialize)]
pub struct ArchiveRequest {
    /// Archive sessions older than this many days (default: 7)
    #[serde(default = "default_older_than_days")]
    pub older_than_days: u32,
}

fn default_older_than_days() -> u32 {
    7
}

/// Response for the archive endpoint.
#[derive(Debug, Serialize)]
pub struct ArchiveResponse {
    pub archived: usize,
    pub results: Vec<ArchiveResultEntry>,
}

/// Single session archive result.
#[derive(Debug, Serialize)]
pub struct ArchiveResultEntry {
    pub session_id: String,
    pub messages_archived: usize,
    pub logs_archived: usize,
    pub file_size: u64,
}

/// Response for the restore endpoint.
#[derive(Debug, Serialize)]
pub struct RestoreResponse {
    pub session_id: String,
    pub records_restored: usize,
}

/// Error response.
#[derive(Debug, Serialize)]
pub struct SessionErrorResponse {
    pub error: String,
}

// ============================================================================
// HANDLERS
// ============================================================================

/// POST /api/sessions/archive
/// Archive old session transcripts to compressed JSONL files.
pub async fn archive_sessions(
    State(state): State<AppState>,
    Json(body): Json<ArchiveRequest>,
) -> Result<Json<ArchiveResponse>, (StatusCode, Json<SessionErrorResponse>)> {
    let archiver = match &state.session_archiver {
        Some(a) => a,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(SessionErrorResponse {
                    error: "Session archiver not available".to_string(),
                }),
            ));
        }
    };

    match archiver.archive_old_sessions(body.older_than_days) {
        Ok(results) => {
            let entries: Vec<ArchiveResultEntry> = results
                .iter()
                .map(|r| ArchiveResultEntry {
                    session_id: r.session_id.clone(),
                    messages_archived: r.messages_archived,
                    logs_archived: r.logs_archived,
                    file_size: r.file_size,
                })
                .collect();
            let count = entries.len();
            Ok(Json(ArchiveResponse {
                archived: count,
                results: entries,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SessionErrorResponse {
                error: format!("Archive failed: {}", e),
            }),
        )),
    }
}

/// POST /api/sessions/restore/:id
/// Restore an archived session from its compressed JSONL file.
pub async fn restore_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<RestoreResponse>, (StatusCode, Json<SessionErrorResponse>)> {
    let archiver = match &state.session_archiver {
        Some(a) => a,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(SessionErrorResponse {
                    error: "Session archiver not available".to_string(),
                }),
            ));
        }
    };

    match archiver.restore_session(&session_id) {
        Ok(records_restored) => Ok(Json(RestoreResponse {
            session_id,
            records_restored,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SessionErrorResponse {
                error: format!("Restore failed: {}", e),
            }),
        )),
    }
}

/// GET /api/sessions/:id/state — returns structured session snapshot
pub async fn get_session_state(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionState>, (StatusCode, Json<SessionErrorResponse>)> {
    let builder = SessionStateBuilder::new(state.log_service.clone(), state.conversations.clone());

    match builder.build(&session_id) {
        Ok(Some(session_state)) => Ok(Json(session_state)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(SessionErrorResponse {
                error: format!("Session not found: {}", session_id),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SessionErrorResponse {
                error: format!("Failed to build session state: {}", e),
            }),
        )),
    }
}

/// DELETE /api/sessions/:id — hard-delete session and per-session data.
///
/// Cascades to `messages`, `agent_executions`, `execution_logs`, `artifacts`
/// (DB rows only — files on disk stay), `distillation_runs`, `bridge_outbox`,
/// and `recall_log`. Preserves `memory_facts`, `memory_facts_index` (vec0),
/// and the knowledge graph so cross-session memory survives a single-session
/// cleanup.
///
/// Returns 204 on success, 404 if the session doesn't exist, 500 on DB error.
pub async fn delete_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<SessionErrorResponse>)> {
    // Presence check — distinguish 404 from silent no-op on bogus ids.
    match state.state_service.get_session(&session_id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(SessionErrorResponse {
                    error: format!("Session not found: {}", session_id),
                }),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SessionErrorResponse {
                    error: format!("Lookup failed: {}", e),
                }),
            ));
        }
    }

    match state.state_service.delete_session_cascade(&session_id) {
        Ok(rows) => {
            tracing::info!(session_id = %session_id, rows, "deleted session cascade");
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SessionErrorResponse {
                error: format!("Delete failed: {}", e),
            }),
        )),
    }
}
