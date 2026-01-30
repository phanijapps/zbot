//! # Health Endpoints
//!
//! Health check and status endpoints.

use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

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
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /api/status - Detailed status.
pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let agent_count = state.agents.list().await.map(|a| a.len()).unwrap_or(0);

    Json(StatusResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        agent_count,
    })
}
