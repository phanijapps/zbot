//! # MCP Endpoints
//!
//! CRUD operations for MCP server configurations.

use crate::services::mcp::McpServerSummary;
use crate::state::AppState;
use agent_runtime::McpServerConfig;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP server list response.
#[derive(Debug, Serialize)]
pub struct McpListResponse {
    pub servers: Vec<McpServerSummary>,
}

/// Error response.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Create MCP server request.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CreateMcpRequest {
    /// Stdio-based MCP server
    #[serde(rename = "stdio")]
    Stdio {
        id: Option<String>,
        name: String,
        description: String,
        command: String,
        args: Vec<String>,
        env: Option<HashMap<String, String>>,
        #[serde(default = "default_true")]
        enabled: bool,
    },
    /// HTTP-based MCP server
    #[serde(rename = "http")]
    Http {
        id: Option<String>,
        name: String,
        description: String,
        url: String,
        headers: Option<HashMap<String, String>>,
        #[serde(default = "default_true")]
        enabled: bool,
    },
    /// SSE-based MCP server
    #[serde(rename = "sse")]
    Sse {
        id: Option<String>,
        name: String,
        description: String,
        url: String,
        headers: Option<HashMap<String, String>>,
        #[serde(default = "default_true")]
        enabled: bool,
    },
    /// Streamable HTTP MCP server
    #[serde(rename = "streamable-http")]
    StreamableHttp {
        id: Option<String>,
        name: String,
        description: String,
        url: String,
        headers: Option<HashMap<String, String>>,
        #[serde(default = "default_true")]
        enabled: bool,
    },
}

fn default_true() -> bool {
    true
}

impl From<CreateMcpRequest> for McpServerConfig {
    fn from(req: CreateMcpRequest) -> Self {
        match req {
            CreateMcpRequest::Stdio {
                id,
                name,
                description,
                command,
                args,
                env,
                enabled,
            } => McpServerConfig::Stdio {
                id,
                name,
                description,
                command,
                args,
                env,
                enabled,
                validated: None,
            },
            CreateMcpRequest::Http {
                id,
                name,
                description,
                url,
                headers,
                enabled,
            } => McpServerConfig::Http {
                id,
                name,
                description,
                url,
                headers,
                enabled,
                validated: None,
            },
            CreateMcpRequest::Sse {
                id,
                name,
                description,
                url,
                headers,
                enabled,
            } => McpServerConfig::Sse {
                id,
                name,
                description,
                url,
                headers,
                enabled,
                validated: None,
            },
            CreateMcpRequest::StreamableHttp {
                id,
                name,
                description,
                url,
                headers,
                enabled,
            } => McpServerConfig::StreamableHttp {
                id,
                name,
                description,
                url,
                headers,
                enabled,
                validated: None,
            },
        }
    }
}

/// GET /api/mcps - List all MCP servers.
pub async fn list_mcps(State(state): State<AppState>) -> Json<McpListResponse> {
    match state.mcp_service.list_summaries() {
        Ok(servers) => Json(McpListResponse { servers }),
        Err(e) => {
            tracing::error!("Failed to list MCP servers: {}", e);
            Json(McpListResponse { servers: vec![] })
        }
    }
}

/// GET /api/mcps/:id - Get an MCP server by ID.
pub async fn get_mcp(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<McpServerConfig>, StatusCode> {
    match state.mcp_service.get(&id) {
        Ok(config) => Ok(Json(config)),
        Err(e) => {
            tracing::warn!("MCP server not found: {} - {}", id, e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

/// POST /api/mcps - Create a new MCP server.
pub async fn create_mcp(
    State(state): State<AppState>,
    Json(request): Json<CreateMcpRequest>,
) -> Result<Json<McpServerConfig>, (StatusCode, Json<ErrorResponse>)> {
    let config: McpServerConfig = request.into();

    match state.mcp_service.add(config.clone()) {
        Ok(()) => Ok(Json(config)),
        Err(e) => {
            tracing::error!("Failed to create MCP server: {}", e);
            Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))
        }
    }
}

/// PUT /api/mcps/:id - Update an MCP server.
pub async fn update_mcp(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<CreateMcpRequest>,
) -> Result<Json<McpServerConfig>, (StatusCode, Json<ErrorResponse>)> {
    // Verify the server exists
    if state.mcp_service.get(&id).is_err() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("MCP server not found: {}", id),
            }),
        ));
    }

    let config: McpServerConfig = request.into();

    match state.mcp_service.update(&id, config.clone()) {
        Ok(()) => Ok(Json(config)),
        Err(e) => {
            tracing::error!("Failed to update MCP server: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e }),
            ))
        }
    }
}

/// DELETE /api/mcps/:id - Delete an MCP server.
pub async fn delete_mcp(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state.mcp_service.delete(&id) {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            tracing::warn!("Failed to delete MCP server: {} - {}", id, e);
            Err((StatusCode::NOT_FOUND, Json(ErrorResponse { error: e })))
        }
    }
}

/// Test result response.
#[derive(Debug, Serialize)]
pub struct McpTestResult {
    pub success: bool,
    pub message: String,
    pub tools: Option<Vec<String>>,
}

/// POST /api/mcps/:id/test - Test an MCP server connection.
pub async fn test_mcp(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<McpTestResult>, (StatusCode, Json<ErrorResponse>)> {
    // Get the MCP config
    let config = match state.mcp_service.get(&id) {
        Ok(c) => c,
        Err(e) => {
            return Err((StatusCode::NOT_FOUND, Json(ErrorResponse { error: e })));
        }
    };

    // Try to start the server and list tools
    use agent_runtime::McpManager;

    let manager = McpManager::new();

    match manager.start_server(config).await {
        Ok(()) => {
            // Try to list tools
            match manager.list_all_tools().await {
                Ok(tools) => {
                    let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
                    let count = tool_names.len();
                    Ok(Json(McpTestResult {
                        success: true,
                        message: format!("Connected successfully. Found {} tools.", count),
                        tools: Some(tool_names),
                    }))
                }
                Err(e) => Ok(Json(McpTestResult {
                    success: false,
                    message: format!("Connected but failed to list tools: {}", e),
                    tools: None,
                })),
            }
        }
        Err(e) => Ok(Json(McpTestResult {
            success: false,
            message: format!("Failed to connect: {}", e),
            tools: None,
        })),
    }
}
