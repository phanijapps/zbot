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
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateToolSettingsRequest {
    #[serde(default)]
    pub grep: bool,
    #[serde(default)]
    pub glob: bool,
    #[serde(default)]
    pub python: bool,
    #[serde(default)]
    pub load_skill: bool,
    #[serde(default)]
    pub ui_tools: bool,
    #[serde(default)]
    pub knowledge_graph: bool,
    #[serde(default)]
    pub create_agent: bool,
    #[serde(default)]
    pub introspection: bool,
}

impl From<UpdateToolSettingsRequest> for ToolSettings {
    fn from(req: UpdateToolSettingsRequest) -> Self {
        ToolSettings {
            grep: req.grep,
            glob: req.glob,
            python: req.python,
            load_skill: req.load_skill,
            ui_tools: req.ui_tools,
            knowledge_graph: req.knowledge_graph,
            create_agent: req.create_agent,
            introspection: req.introspection,
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
