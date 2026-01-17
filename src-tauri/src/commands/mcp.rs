// ============================================================================
// MCP COMMANDS
// Model Context Protocol server management
// Supports both stdio (command-based) and HTTP-based MCP servers
// ============================================================================

use crate::settings::AppDirs;
use crate::domains::agent_runtime::mcp_manager::McpServerConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::collections::HashMap;
use std::process::{Command, Stdio};

/// MCP Server data structure (shared with frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub server_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    pub enabled: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validated: Option<bool>,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

impl MCPServer {
    /// Convert from McpServerConfig to MCPServer
    pub fn from_config(config: McpServerConfig) -> Self {
        match config {
            McpServerConfig::Stdio { id, name, description, command, args, env, enabled, validated } => {
                MCPServer {
                    id,
                    name,
                    description,
                    server_type: "stdio".to_string(),
                    command: Some(command),
                    args: Some(args),
                    env,
                    url: None,
                    headers: None,
                    enabled,
                    status: "stopped".to_string(),
                    validated,
                    created_at: None,
                }
            }
            McpServerConfig::Http { id, name, description, url, headers, enabled, validated } => {
                MCPServer {
                    id,
                    name,
                    description,
                    server_type: "http".to_string(),
                    command: None,
                    args: None,
                    env: None,
                    url: Some(url),
                    headers,
                    enabled,
                    status: "stopped".to_string(),
                    validated,
                    created_at: None,
                }
            }
            McpServerConfig::Sse { id, name, description, url, headers, enabled, validated } => {
                MCPServer {
                    id,
                    name,
                    description,
                    server_type: "sse".to_string(),
                    command: None,
                    args: None,
                    env: None,
                    url: Some(url),
                    headers,
                    enabled,
                    status: "stopped".to_string(),
                    validated,
                    created_at: None,
                }
            }
            McpServerConfig::StreamableHttp { id, name, description, url, headers, enabled, validated } => {
                MCPServer {
                    id,
                    name,
                    description,
                    server_type: "streamable-http".to_string(),
                    command: None,
                    args: None,
                    env: None,
                    url: Some(url),
                    headers,
                    enabled,
                    status: "stopped".to_string(),
                    validated,
                    created_at: None,
                }
            }
        }
    }

    /// Convert to McpServerConfig
    pub fn to_config(&self) -> Result<McpServerConfig, String> {
        match self.server_type.as_str() {
            "stdio" => {
                let command = self.command.clone().ok_or("Missing command")?;
                let args = self.args.clone().ok_or("Missing args")?;
                Ok(McpServerConfig::Stdio {
                    id: self.id.clone(),
                    name: self.name.clone(),
                    description: self.description.clone(),
                    command,
                    args,
                    env: self.env.clone(),
                    enabled: self.enabled,
                    validated: self.validated,
                })
            }
            "http" => {
                let url = self.url.clone().ok_or("Missing url")?;
                Ok(McpServerConfig::Http {
                    id: self.id.clone(),
                    name: self.name.clone(),
                    description: self.description.clone(),
                    url,
                    headers: self.headers.clone(),
                    enabled: self.enabled,
                    validated: self.validated,
                })
            }
            "sse" => {
                let url = self.url.clone().ok_or("Missing url")?;
                Ok(McpServerConfig::Sse {
                    id: self.id.clone(),
                    name: self.name.clone(),
                    description: self.description.clone(),
                    url,
                    headers: self.headers.clone(),
                    enabled: self.enabled,
                    validated: self.validated,
                })
            }
            "streamable-http" => {
                let url = self.url.clone().ok_or("Missing url")?;
                Ok(McpServerConfig::StreamableHttp {
                    id: self.id.clone(),
                    name: self.name.clone(),
                    description: self.description.clone(),
                    url,
                    headers: self.headers.clone(),
                    enabled: self.enabled,
                    validated: self.validated,
                })
            }
            _ => Err(format!("Unknown server type: {}", self.server_type)),
        }
    }
}

/// Test result for MCP server
#[derive(Debug, Clone, Serialize)]
pub struct MCPTestResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
}

/// Get the MCP servers config file path
fn get_mcp_config_path() -> Result<std::path::PathBuf, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    Ok(dirs.config_dir.join("mcps.json"))
}

/// Read all MCP servers from config file
fn read_mcp_configs() -> Result<Vec<McpServerConfig>, String> {
    let config_path = get_mcp_config_path()?;

    if !config_path.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read MCP config: {}", e))?;

    // Support both array format and single object format
    if content.trim().starts_with('[') {
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse MCP servers array: {}", e))
    } else {
        // Single object - wrap in array
        let server: McpServerConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse MCP server: {}", e))?;
        Ok(vec![server])
    }
}

/// Write MCP server configs to config file
fn write_mcp_configs(servers: &[McpServerConfig]) -> Result<(), String> {
    let config_path = get_mcp_config_path()?;

    let content = serde_json::to_string_pretty(servers)
        .map_err(|e| format!("Failed to serialize MCP servers: {}", e))?;

    fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write MCP config: {}", e))?;

    Ok(())
}

/// Read all MCP servers (converted to MCPServer for frontend)
fn read_mcp_servers() -> Result<Vec<MCPServer>, String> {
    let configs = read_mcp_configs()?;
    Ok(configs.into_iter().map(MCPServer::from_config).collect())
}

/// Write MCP servers from MCPServer (converts to McpServerConfig)
fn write_mcp_servers(servers: &[MCPServer]) -> Result<(), String> {
    let configs: Result<Vec<McpServerConfig>, String> = servers
        .iter()
        .map(|s| s.to_config())
        .collect();
    write_mcp_configs(&configs?)
}

/// Lists all MCP servers
#[tauri::command]
pub async fn list_mcp_servers() -> Result<Vec<MCPServer>, String> {
    read_mcp_servers()
}

/// Gets a single MCP server by ID
#[tauri::command]
pub async fn get_mcp_server(id: String) -> Result<MCPServer, String> {
    let servers = read_mcp_servers()?;
    servers
        .into_iter()
        .find(|s| s.id.as_deref() == Some(id.as_str()))
        .ok_or_else(|| format!("MCP server not found: {}", id))
}

/// Creates a new MCP server configuration
#[tauri::command]
pub async fn create_mcp_server(server: MCPServer) -> Result<MCPServer, String> {
    let mut servers = read_mcp_servers()?;

    // Ensure server has an ID
    let server_id = server.id.as_ref().ok_or("Server must have an ID")?;

    // Check for duplicate ID
    if servers.iter().any(|s| s.id.as_deref() == Some(server_id)) {
        return Err(format!("MCP server with ID {} already exists", server_id));
    }

    servers.push(server.clone());
    write_mcp_servers(&servers)?;

    Ok(server)
}

/// Updates an existing MCP server
#[tauri::command]
pub async fn update_mcp_server(id: String, server: MCPServer) -> Result<MCPServer, String> {
    let mut servers = read_mcp_servers()?;

    let index = servers
        .iter()
        .position(|s| s.id.as_deref() == Some(id.as_str()))
        .ok_or_else(|| format!("MCP server not found: {}", id))?;

    // Preserve validation status if not explicitly set
    let mut updated_server = server.clone();
    if updated_server.validated.is_none() {
        updated_server.validated = servers[index].validated;
    }

    servers[index] = updated_server.clone();
    write_mcp_servers(&servers)?;

    Ok(updated_server)
}

/// Deletes an MCP server
#[tauri::command]
pub async fn delete_mcp_server(id: String) -> Result<(), String> {
    let mut servers = read_mcp_servers()?;

    let initial_len = servers.len();
    servers.retain(|s| s.id.as_deref() != Some(id.as_str()));

    if servers.len() == initial_len {
        return Err(format!("MCP server not found: {}", id));
    }

    write_mcp_servers(&servers)?;
    Ok(())
}

/// Starts an MCP server
#[tauri::command]
pub async fn start_mcp_server(id: String) -> Result<(), String> {
    let mut servers = read_mcp_servers()?;

    let server = servers
        .iter_mut()
        .find(|s| s.id.as_deref() == Some(id.as_str()))
        .ok_or_else(|| format!("MCP server not found: {}", id))?;

    server.enabled = true;
    server.status = "running".to_string();

    write_mcp_servers(&servers)?;
    Ok(())
}

/// Stops an MCP server
#[tauri::command]
pub async fn stop_mcp_server(id: String) -> Result<(), String> {
    let mut servers = read_mcp_servers()?;

    let server = servers
        .iter_mut()
        .find(|s| s.id.as_deref() == Some(id.as_str()))
        .ok_or_else(|| format!("MCP server not found: {}", id))?;

    server.enabled = false;
    server.status = "stopped".to_string();

    write_mcp_servers(&servers)?;
    Ok(())
}

/// Tests an MCP server configuration
#[tauri::command]
pub async fn test_mcp_server(server: MCPServer) -> Result<MCPTestResult, String> {
    match server.server_type.as_str() {
        "stdio" => {
            // Test stdio server by running the command
            let command = server.command.clone().ok_or("Missing command")?;
            let args = server.args.clone().ok_or("Missing args")?;
            let env_vars = server.env.clone();

            let handle = tokio::task::spawn_blocking(move || {
                let mut cmd = Command::new(&command);
                cmd.args(&args);
                if let Some(env) = &env_vars {
                    for (key, value) in env {
                        cmd.env(key, value);
                    }
                }
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());
                cmd.output()
            });

            let timeout_result = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;

            match timeout_result {
                Ok(inner) => match inner {
                    Ok(io_result) => match io_result {
                        Ok(output) => {
                            if output.status.success() {
                                Ok(MCPTestResult {
                                    success: true,
                                    message: "Server configuration validated successfully".to_string(),
                                    tools: None,
                                })
                            } else {
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                let error_msg = if !stderr.is_empty() {
                                    stderr.to_string()
                                } else if !stdout.is_empty() {
                                    stdout.to_string()
                                } else {
                                    format!("Command exited with status: {}", output.status)
                                };
                                Ok(MCPTestResult {
                                    success: false,
                                    message: format!("Command failed: {}", error_msg.lines().next().unwrap_or(&error_msg)),
                                    tools: None,
                                })
                            }
                        }
                        Err(e) => Ok(MCPTestResult {
                            success: false,
                            message: format!("Command execution failed: {}", e),
                            tools: None,
                        })
                    },
                    Err(e) => Ok(MCPTestResult {
                        success: false,
                        message: format!("Task failed: {}", e),
                        tools: None,
                    })
                },
                Err(_) => Ok(MCPTestResult {
                    success: false,
                    message: "Command timed out after 5 seconds. The server may be hanging or taking too long to start.".to_string(),
                    tools: None,
                })
            }
        }
        "http" | "sse" | "streamable-http" => {
            // Test HTTP/SSE server by making a request to the URL
            let url = server.url.clone().ok_or("Missing url")?;
            let headers = server.headers.clone().unwrap_or_default();

            let result = async move {
                let client = reqwest::Client::new();
                let mut req = client.get(&url);
                for (key, value) in &headers {
                    req = req.header(key, value);
                }
                req.timeout(std::time::Duration::from_secs(5)).send().await
            }.await;

            match result {
                Ok(response) => {
                    if response.status().is_success() {
                        Ok(MCPTestResult {
                            success: true,
                            message: "Server endpoint is reachable".to_string(),
                            tools: None,
                        })
                    } else {
                        Ok(MCPTestResult {
                            success: false,
                            message: format!("Server returned status: {}", response.status()),
                            tools: None,
                        })
                    }
                }
                Err(e) => Ok(MCPTestResult {
                    success: false,
                    message: format!("HTTP request failed: {}", e),
                    tools: None,
                })
            }
        }
        _ => Ok(MCPTestResult {
            success: false,
            message: format!("Unknown server type: {}", server.server_type),
            tools: None,
        })
    }
}
