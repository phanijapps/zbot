//! # Health Endpoints
//!
//! Health check and status endpoints.

use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

/// Daemon version reported on `/api/health` and `/api/status`.
///
/// `option_env!("BUILD_VERSION")` resolves at compile time. When
/// `gateway/build.rs` runs with `ZBOT_INSTALL=1` (set by `make install`
/// and `scripts/install.sh`), it emits e.g. `2026.5.3.develop`. Plain
/// `cargo build` doesn't set `ZBOT_INSTALL`, so the env var stays
/// unset and the daemon reports the bare `CARGO_PKG_VERSION`.
const VERSION: &str = match option_env!("BUILD_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

/// Health check response.
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Detailed status response.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: String,
    pub version: String,
    #[serde(rename = "agentCount")]
    pub agent_count: usize,
}

/// GET /api/health - Basic health check.
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: VERSION.to_string(),
    })
}

/// GET /api/status - Detailed status.
pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let agent_count = state.agents.list().await.map(|a| a.len()).unwrap_or(0);

    Json(StatusResponse {
        status: "ok".to_string(),
        version: VERSION.to_string(),
        agent_count,
    })
}
