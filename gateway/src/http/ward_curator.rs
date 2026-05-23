//! `POST /api/curator/cleanup` + `POST /api/curator/restore` — Phase B
//! endpoints. Spec: `memory-bank/future-state/2026-05-23-ward-curator-spec.md`.
//!
//! `cleanup` runs the Layer-1 heuristic transitions over the per-ward
//! sidecar, archives anything past `archive_days` and marks anything past
//! `stale_days`. Bundled / user-authored / pinned wards are skipped.
//! A `.tar.gz` snapshot is written before any mutation and an audit log
//! lands under `<vault>/data/curator_logs/<ts>/`.
//!
//! `restore` un-tars a named backup back over the wards tree.

use axum::{body::Bytes, extract::State, http::StatusCode, response::IntoResponse, Json};
use gateway_services::{CleanupReport, CleanupRequest, RestoreReport, RestoreRequest, WardCurator};

use crate::state::AppState;

fn make_curator(state: &AppState) -> WardCurator {
    WardCurator::new(state.paths.wards_dir(), state.paths.data_dir())
}

/// `POST /api/curator/cleanup` — body is an optional `CleanupRequest`. An
/// empty body or `{}` runs with defaults (stale=30d, archive=90d, dry_run=false).
pub async fn cleanup(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<CleanupReport>, (StatusCode, String)> {
    let req: CleanupRequest = if body.is_empty() {
        CleanupRequest::default()
    } else {
        serde_json::from_slice(&body)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("bad request body: {e}")))?
    };
    make_curator(&state)
        .cleanup(&req)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

/// `POST /api/curator/restore` — body `{ "backup": "<utc-iso>" }`.
pub async fn restore(
    State(state): State<AppState>,
    Json(req): Json<RestoreRequest>,
) -> Result<Json<RestoreReport>, impl IntoResponse> {
    make_curator(&state)
        .restore(&req.backup)
        .map(Json)
        .map_err(|e| {
            // 404 if the named backup doesn't exist; anything else is 500.
            let code = if e.contains("backup not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (code, e)
        })
}
