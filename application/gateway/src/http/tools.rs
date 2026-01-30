//! # Tool Endpoints
//!
//! Endpoints for listing available tools.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool response.
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResponse {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    #[serde(rename = "riskLevel")]
    pub risk_level: String,
}

/// GET /api/tools - List all available tools.
///
/// Note: This will be connected to the tool registry in Phase 3b.
pub async fn list_tools(State(_state): State<AppState>) -> Json<Vec<ToolResponse>> {
    // TODO: Connect to tool registry in Phase 3b
    // For now, return empty list
    Json(vec![])
}

/// GET /api/tools/:name - Get a tool by name.
pub async fn get_tool(
    State(_state): State<AppState>,
    Path(_name): Path<String>,
) -> Result<Json<ToolResponse>, StatusCode> {
    // TODO: Connect to tool registry in Phase 3b
    Err(StatusCode::NOT_FOUND)
}
