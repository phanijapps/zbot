// ============================================================================
// MCP COMMANDS
// Model Context Protocol server management
// Supports both stdio (command-based) and HTTP-based MCP servers
// ============================================================================

use crate::settings::AppDirs;
use agent_runtime::McpServerConfig;
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
            // Test stdio server by starting it and sending an MCP initialize request
            let command = server.command.clone().ok_or("Missing command")?;
            let args = server.args.clone().ok_or("Missing args")?;
            let env_vars = server.env.clone();

            let handle = tokio::task::spawn_blocking(move || {
                use std::io::{BufRead, BufReader, Write};

                // On Windows, npx/uvx etc need full paths or proper extensions
                #[cfg(target_os = "windows")]
                let actual_command = {
                    let cmd_lower = command.to_lowercase();
                    if cmd_lower == "npx" || cmd_lower == "npm" || cmd_lower == "node" ||
                       cmd_lower == "uvx" || cmd_lower == "uv" || cmd_lower == "python" || cmd_lower == "pip" {
                        let user_profile = std::env::var("USERPROFILE").unwrap_or_default();

                        // Try to find the command in common locations
                        let possible_paths = vec![
                            // Standard uv/uvx installation location
                            format!("{}\\.local\\bin\\{}.exe", user_profile, command),
                            format!("{}\\.local\\bin\\{}", user_profile, command),
                            // Node.js paths
                            format!("C:\\Program Files\\nodejs\\{}.cmd", command),
                            format!("C:\\Program Files\\nodejs\\{}.exe", command),
                            format!("C:\\Program Files\\nodejs\\{}", command),
                            // Python paths
                            format!("{}\\AppData\\Local\\Programs\\Python\\Python312\\Scripts\\{}.exe", user_profile, command),
                            format!("{}\\AppData\\Local\\Programs\\Python\\Python311\\Scripts\\{}.exe", user_profile, command),
                            format!("{}\\AppData\\Local\\Programs\\Python\\Python313\\Scripts\\{}.exe", user_profile, command),
                            format!("{}\\AppData\\Roaming\\Python\\Python312\\Scripts\\{}.exe", user_profile, command),
                            format!("{}\\AppData\\Roaming\\Python\\Python311\\Scripts\\{}.exe", user_profile, command),
                            // Windows Apps
                            format!("{}\\AppData\\Local\\Microsoft\\WindowsApps\\{}.exe", user_profile, command),
                            // Scoop paths
                            format!("{}\\scoop\\shims\\{}.exe", user_profile, command),
                            format!("{}\\scoop\\shims\\{}.cmd", user_profile, command),
                            format!("{}\\scoop\\apps\\nodejs-lts\\current\\{}.cmd", user_profile, command),
                            format!("{}\\scoop\\apps\\nodejs\\current\\{}.cmd", user_profile, command),
                            format!("{}\\scoop\\apps\\uv\\current\\{}.exe", user_profile, command),
                            // Astral uv paths
                            format!("{}\\AppData\\Local\\Programs\\astral\\uv\\{}.exe", user_profile, command),
                            format!("{}\\AppData\\Roaming\\astral\\uv\\{}.exe", user_profile, command),
                            format!("{}\\AppData\\Local\\astral\\uv\\{}.exe", user_profile, command),
                            // Cargo bin (for Rust-based tools)
                            format!("{}\\.cargo\\bin\\{}.exe", user_profile, command),
                        ];

                        let found_path = possible_paths.iter().find(|p| std::path::Path::new(p).exists());

                        if let Some(path) = found_path {
                            path.clone()
                        } else {
                            // Fallback: just use the command name and let Windows find it
                            command.clone()
                        }
                    } else {
                        command.clone()
                    }
                };

                #[cfg(not(target_os = "windows"))]
                let actual_command = command.clone();

                let mut cmd = Command::new(&actual_command);
                cmd.args(&args);

                // On Windows, ensure PATH includes common tool locations
                #[cfg(target_os = "windows")]
                {
                    let current_path = std::env::var("PATH").unwrap_or_default();
                    let user_profile = std::env::var("USERPROFILE").unwrap_or_default();
                    let additional_paths = vec![
                        // Standard uv/uvx location (highest priority)
                        format!("{}\\.local\\bin", user_profile),
                        // Node.js
                        "C:\\Program Files\\nodejs".to_string(),
                        // Python
                        format!("{}\\AppData\\Local\\Programs\\Python\\Python313\\Scripts", user_profile),
                        format!("{}\\AppData\\Local\\Programs\\Python\\Python312\\Scripts", user_profile),
                        format!("{}\\AppData\\Local\\Programs\\Python\\Python311\\Scripts", user_profile),
                        // Astral uv paths
                        format!("{}\\AppData\\Local\\Programs\\astral\\uv", user_profile),
                        format!("{}\\AppData\\Roaming\\astral\\uv", user_profile),
                        format!("{}\\AppData\\Local\\astral\\uv", user_profile),
                        // Scoop
                        format!("{}\\scoop\\shims", user_profile),
                        // Windows Apps
                        format!("{}\\AppData\\Local\\Microsoft\\WindowsApps", user_profile),
                        // Cargo bin
                        format!("{}\\.cargo\\bin", user_profile),
                    ];
                    let new_path = format!("{};{}", additional_paths.join(";"), current_path);
                    cmd.env("PATH", new_path);
                }

                if let Some(env) = &env_vars {
                    for (key, value) in env {
                        cmd.env(key, value);
                    }
                }
                cmd.stdin(Stdio::piped());
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());

                let mut child = match cmd.spawn() {
                    Ok(c) => c,
                    Err(e) => {
                        // On Windows, provide more helpful error message
                        #[cfg(target_os = "windows")]
                        {
                            if e.kind() == std::io::ErrorKind::NotFound {
                                return Err(format!(
                                    "Program not found: '{}'. Make sure it's installed and in your PATH. \
                                    For npx: install Node.js from nodejs.org. \
                                    For uvx: install uv from astral.sh/uv",
                                    actual_command
                                ));
                            }
                        }
                        return Err(format!("Failed to start server: {}", e));
                    }
                };

                // Send MCP initialize request (JSON-RPC 2.0)
                let init_request = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {},
                        "clientInfo": {
                            "name": "agentzero-test",
                            "version": "1.0.0"
                        }
                    }
                });

                let request_str = format!("{}\n", init_request.to_string());

                // Write to stdin
                if let Some(ref mut stdin) = child.stdin {
                    if let Err(e) = stdin.write_all(request_str.as_bytes()) {
                        let _ = child.kill();
                        return Err(format!("Failed to send initialize request: {}", e));
                    }
                    if let Err(e) = stdin.flush() {
                        let _ = child.kill();
                        return Err(format!("Failed to flush stdin: {}", e));
                    }
                } else {
                    let _ = child.kill();
                    return Err("Failed to get stdin handle".to_string());
                }

                // Read response from stdout with timeout
                let stdout = child.stdout.take();
                if let Some(stdout) = stdout {
                    let reader = BufReader::new(stdout);

                    // Read lines until we get a valid JSON response or timeout
                    for line in reader.lines().take(10) {
                        match line {
                            Ok(line_str) => {
                                if line_str.trim().is_empty() {
                                    continue;
                                }
                                // Try to parse as JSON
                                if let Ok(response) = serde_json::from_str::<serde_json::Value>(&line_str) {
                                    // Check if it's a valid MCP response
                                    if response.get("jsonrpc").is_some() {
                                        let _ = child.kill();

                                        // Check for error response
                                        if let Some(error) = response.get("error") {
                                            let error_msg = error.get("message")
                                                .and_then(|m| m.as_str())
                                                .unwrap_or("Unknown error");
                                            return Ok(MCPTestResult {
                                                success: false,
                                                message: format!("Server error: {}", error_msg),
                                                tools: None,
                                            });
                                        }

                                        // Extract server info if available
                                        let server_name = response
                                            .get("result")
                                            .and_then(|r| r.get("serverInfo"))
                                            .and_then(|s| s.get("name"))
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("MCP Server");

                                        return Ok(MCPTestResult {
                                            success: true,
                                            message: format!("Connected to {}", server_name),
                                            tools: None,
                                        });
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }

                let _ = child.kill();
                Err("No valid response from server".to_string())
            });

            let timeout_result = tokio::time::timeout(std::time::Duration::from_secs(15), handle).await;

            match timeout_result {
                Ok(inner) => match inner {
                    Ok(result) => match result {
                        Ok(test_result) => Ok(test_result),
                        Err(e) => Ok(MCPTestResult {
                            success: false,
                            message: e,
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
                    message: "Server took too long to respond (15s timeout). It may still be downloading dependencies.".to_string(),
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

/// Validates an MCP server and updates its validated status in the config
#[tauri::command]
pub async fn validate_mcp_server(id: String) -> Result<MCPTestResult, String> {
    // Get the server
    let mut servers = read_mcp_servers()?;
    let server_index = servers
        .iter()
        .position(|s| s.id.as_deref() == Some(id.as_str()))
        .ok_or_else(|| format!("MCP server not found: {}", id))?;

    let server = servers[server_index].clone();

    // Test the server
    let test_result = test_mcp_server(server).await?;

    // Update validated status based on test result
    servers[server_index].validated = Some(test_result.success);

    // Save the updated config
    write_mcp_servers(&servers)?;

    Ok(test_result)
}
