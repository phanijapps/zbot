//! # Settings Endpoints
//!
//! HTTP endpoints for managing application settings.

use crate::state::AppState;
use agent_tools::ToolSettings;
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

/// Response for settings endpoints.
#[derive(Debug, Serialize)]
pub struct SettingsResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// GET /api/settings/tools - Get tool settings.
pub async fn get_tool_settings(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse<ToolSettings>>, (StatusCode, Json<SettingsResponse<()>>)> {
    match state.settings.get_tool_settings() {
        Ok(settings) => Ok(Json(SettingsResponse {
            success: true,
            data: Some(settings),
            error: None,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SettingsResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )),
    }
}

/// Request for updating tool settings.
///
/// Note: grep, load_skill are core tools and always enabled.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateToolSettingsRequest {
    #[serde(default)]
    pub python: bool,
    #[serde(default)]
    pub web_fetch: bool,
    #[serde(default)]
    pub ui_tools: bool,
    #[serde(default)]
    pub create_agent: bool,
    #[serde(default)]
    pub introspection: bool,
    #[serde(default)]
    pub file_tools: bool,
    #[serde(default)]
    pub todos: bool,
    #[serde(default)]
    pub offload_large_results: bool,
    #[serde(default = "default_offload_threshold")]
    pub offload_threshold_tokens: usize,
}

fn default_offload_threshold() -> usize {
    5000
}

impl From<UpdateToolSettingsRequest> for ToolSettings {
    fn from(req: UpdateToolSettingsRequest) -> Self {
        ToolSettings {
            python: req.python,
            web_fetch: req.web_fetch,
            ui_tools: req.ui_tools,
            create_agent: req.create_agent,
            introspection: req.introspection,
            file_tools: req.file_tools,
            todos: req.todos,
            offload_large_results: req.offload_large_results,
            offload_threshold_tokens: req.offload_threshold_tokens,
        }
    }
}

/// PUT /api/settings/tools - Update tool settings.
pub async fn update_tool_settings(
    State(state): State<AppState>,
    Json(request): Json<UpdateToolSettingsRequest>,
) -> Result<Json<SettingsResponse<ToolSettings>>, (StatusCode, Json<SettingsResponse<()>>)> {
    let settings: ToolSettings = request.into();

    match state.settings.update_tool_settings(settings.clone()) {
        Ok(()) => Ok(Json(SettingsResponse {
            success: true,
            data: Some(settings),
            error: None,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SettingsResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )),
    }
}
