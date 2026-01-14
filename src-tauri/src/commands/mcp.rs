// ============================================================================
// MCP COMMANDS
// Model Context Protocol server management
// ============================================================================

use crate::settings::AppDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::collections::HashMap;
use std::process::{Command, Stdio};

/// MCP Server data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
    pub enabled: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validated: Option<bool>,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
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
fn read_mcp_servers() -> Result<Vec<MCPServer>, String> {
    let config_path = get_mcp_config_path()?;

    if !config_path.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read MCP config: {}", e))?;

    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse MCP config: {}", e))
}

/// Write MCP servers to config file
fn write_mcp_servers(servers: &[MCPServer]) -> Result<(), String> {
    let config_path = get_mcp_config_path()?;

    let content = serde_json::to_string_pretty(servers)
        .map_err(|e| format!("Failed to serialize MCP servers: {}", e))?;

    fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write MCP config: {}", e))?;

    Ok(())
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

/// Starts an MCP server (placeholder - actual implementation would spawn process)
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
    // Build the command
    let command = server.command.clone();
    let args = server.args.clone();
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

    // Process the triple-nested Results with proper error messages:
    // 1. Result from timeout (Elapsed vs ...)
    // 2. Result from spawn_blocking (JoinError vs ...)
    // 3. Result from Command::output (io::Error vs Output)
    let timeout_result = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;

    let result = match timeout_result {
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
                        // Command ran but returned non-zero exit code
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
                Err(e) => {
                    // Command could not be executed (e.g., command not found)
                    Ok(MCPTestResult {
                        success: false,
                        message: format!("Command execution failed: {}", e),
                        tools: None,
                    })
                }
            },
            Err(e) => {
                // Task spawn failed (JoinError)
                Ok(MCPTestResult {
                    success: false,
                    message: format!("Task failed: {}", e),
                    tools: None,
                })
            }
        },
        Err(_) => {
            // Timeout elapsed
            Ok(MCPTestResult {
                success: false,
                message: "Command timed out after 5 seconds. The server may be hanging or taking too long to start.".to_string(),
                tools: None,
            })
        }
    };

    result
}
