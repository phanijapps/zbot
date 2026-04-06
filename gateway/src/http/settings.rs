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
use gateway_services::{ExecutionSettings, LogSettings};
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

// ============================================================================
// LOG SETTINGS ENDPOINTS
// ============================================================================

/// Response wrapper for log settings with restart warning.
#[derive(Debug, Serialize)]
pub struct LogSettingsResponse {
    /// The current log settings
    #[serde(flatten)]
    pub settings: LogSettings,
    /// Warning message about restart requirement
    pub restart_required: bool,
}

/// GET /api/settings/logs - Get log settings.
///
/// Returns the current logging configuration including file logging,
/// rotation, and retention settings.
pub async fn get_log_settings(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse<LogSettingsResponse>>, (StatusCode, Json<SettingsResponse<()>>)> {
    match state.settings.get_log_settings() {
        Ok(settings) => Ok(Json(SettingsResponse {
            success: true,
            data: Some(LogSettingsResponse {
                settings,
                restart_required: true, // Always true - log changes require restart
            }),
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

/// Request for updating log settings.
///
/// All fields are optional - only provided fields will be updated.
/// Changes require a daemon restart to take effect.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLogSettingsRequest {
    /// Enable file logging
    #[serde(default)]
    pub enabled: bool,
    /// Custom log directory (optional, defaults to {data_dir}/logs)
    #[serde(default)]
    pub directory: Option<std::path::PathBuf>,
    /// Log level: trace, debug, info, warn, error
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Rotation strategy: daily, hourly, minutely, never
    #[serde(default = "default_rotation")]
    pub rotation: String,
    /// Maximum log files to keep (0 = unlimited)
    #[serde(default = "default_max_files")]
    pub max_files: usize,
    /// Suppress stdout output (only log to file)
    #[serde(default)]
    pub suppress_stdout: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_rotation() -> String {
    "daily".to_string()
}

fn default_max_files() -> usize {
    7
}

impl From<UpdateLogSettingsRequest> for LogSettings {
    fn from(req: UpdateLogSettingsRequest) -> Self {
        LogSettings {
            enabled: req.enabled,
            directory: req.directory,
            level: req.level,
            rotation: req.rotation,
            max_files: req.max_files,
            suppress_stdout: req.suppress_stdout,
        }
    }
}

/// PUT /api/settings/logs - Update log settings.
///
/// Updates the logging configuration. Changes require a daemon restart
/// to take effect. The response includes the updated settings and
/// a reminder about the restart requirement.
pub async fn update_log_settings(
    State(state): State<AppState>,
    Json(request): Json<UpdateLogSettingsRequest>,
) -> Result<Json<SettingsResponse<LogSettingsResponse>>, (StatusCode, Json<SettingsResponse<()>>)> {
    let settings: LogSettings = request.into();

    match state.settings.update_log_settings(settings.clone()) {
        Ok(()) => Ok(Json(SettingsResponse {
            success: true,
            data: Some(LogSettingsResponse {
                settings,
                restart_required: true,
            }),
            error: None,
        })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(SettingsResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )),
    }
}

// ============================================================================
// EXECUTION SETTINGS ENDPOINTS
// ============================================================================

/// Response wrapper for execution settings with restart warning.
#[derive(Debug, Serialize)]
pub struct ExecutionSettingsResponse {
    /// The current execution settings
    #[serde(flatten)]
    pub settings: ExecutionSettings,
    /// Changes require daemon restart to take effect
    pub restart_required: bool,
}

/// GET /api/settings/execution - Get execution settings.
pub async fn get_execution_settings(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse<ExecutionSettingsResponse>>, (StatusCode, Json<SettingsResponse<()>>)> {
    match state.settings.get_execution_settings() {
        Ok(settings) => Ok(Json(SettingsResponse {
            success: true,
            data: Some(ExecutionSettingsResponse {
                settings,
                restart_required: false,
            }),
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

/// Request for updating execution settings.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExecutionSettingsRequest {
    /// Maximum parallel subagents (default: 2)
    #[serde(default = "default_max_parallel")]
    pub max_parallel_agents: u32,
    /// Whether the first-time setup wizard has been completed (default: false)
    #[serde(default)]
    pub setup_complete: bool,
    /// The user-chosen name for the root agent
    #[serde(default)]
    pub agent_name: Option<String>,
    /// Disable streaming for subagents (default: true)
    #[serde(default = "default_non_streaming")]
    pub subagent_non_streaming: bool,
    /// Orchestrator (root agent) configuration
    #[serde(default)]
    pub orchestrator: Option<gateway_services::OrchestratorConfig>,
}

fn default_max_parallel() -> u32 { 2 }
fn default_non_streaming() -> bool { true }

impl From<UpdateExecutionSettingsRequest> for ExecutionSettings {
    fn from(req: UpdateExecutionSettingsRequest) -> Self {
        ExecutionSettings {
            max_parallel_agents: req.max_parallel_agents,
            setup_complete: req.setup_complete,
            agent_name: req.agent_name,
            subagent_non_streaming: req.subagent_non_streaming,
            orchestrator: req.orchestrator.unwrap_or_default(),
            distillation: Default::default(),
        }
    }
}

/// PUT /api/settings/execution - Update execution settings.
///
/// Changes to max_parallel_agents require a daemon restart.
/// When agent_name is set, also updates SOUL.md with the new identity.
pub async fn update_execution_settings(
    State(state): State<AppState>,
    Json(request): Json<UpdateExecutionSettingsRequest>,
) -> Result<Json<SettingsResponse<ExecutionSettingsResponse>>, (StatusCode, Json<SettingsResponse<()>>)> {
    let settings: ExecutionSettings = request.into();

    // Update SOUL.md if agent_name is provided
    if let Some(ref name) = settings.agent_name {
        let soul_path = state.paths.vault_dir().join("config").join("SOUL.md");
        let current = std::fs::read_to_string(&soul_path).unwrap_or_default();
        // Replace the first line "You are **OldName**" with the new name
        let updated = if let Some(rest) = current.strip_prefix("You are **") {
            if let Some(after_name) = rest.find("**") {
                format!("You are **{}**{}", name, &rest[after_name + 2..])
            } else {
                format!("You are **{}**, an autonomous agent.\n\n{}", name, current)
            }
        } else {
            format!("You are **{}**, an autonomous agent.\n\n{}", name, current)
        };
        if let Err(e) = std::fs::write(&soul_path, &updated) {
            tracing::warn!("Failed to update SOUL.md: {}", e);
        }
    }

    match state.settings.update_execution_settings(settings.clone()) {
        Ok(()) => Ok(Json(SettingsResponse {
            success: true,
            data: Some(ExecutionSettingsResponse {
                settings,
                restart_required: true,
            }),
            error: None,
        })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(SettingsResponse {
                success: false,
                data: None,
                error: Some(e),
            }),
        )),
    }
}
