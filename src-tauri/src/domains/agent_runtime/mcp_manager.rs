// ============================================================================
// MCP MANAGER
// Manages Model Context Protocol server connections and tool execution
// ============================================================================

use std::sync::Arc;
use std::collections::HashMap;
use serde_json::Value;
use tokio::sync::RwLock;

use crate::settings::AppDirs;

// ============================================================================
// MCP SERVER CONFIG
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpServerConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub enabled: bool,
}

// ============================================================================
// MCP MANAGER
// ============================================================================

pub struct McpManager {
    servers: RwLock<HashMap<String, Arc<dyn McpClient>>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
        }
    }

    /// Load MCP servers from configuration and connect to them
    pub async fn load_servers(&self, agent_mcps: &[String]) -> Result<(), String> {
        let dirs = AppDirs::get().map_err(|e| e.to_string())?;
        let mcp_file = dirs.config_dir.join("mcps.json");

        eprintln!("Loading MCP servers from: {:?}", mcp_file);
        eprintln!("Agent MCPs: {:?}", agent_mcps);

        if !mcp_file.exists() {
            eprintln!("MCP servers file does not exist");
            return Ok(()); // No MCP servers configured
        }

        let content = std::fs::read_to_string(&mcp_file)
            .map_err(|e| format!("Failed to read MCP servers file: {}", e))?;

        let servers: Vec<McpServerConfig> = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse MCP servers: {}", e))?;

        eprintln!("Found {} MCP servers in config", servers.len());

        for server_config in servers {
            eprintln!("Checking server: id={:?}, name={}, enabled={}",
                server_config.id, server_config.name, server_config.enabled);
            if let Some(ref id) = server_config.id {
                if agent_mcps.contains(id) {
                    // Start this MCP server since the agent explicitly uses it
                    // (regardless of global enabled state)
                    eprintln!("Starting MCP server: {} (required by agent)", id);
                    self.start_server(server_config).await?;
                } else if server_config.enabled {
                    // Also start if it's globally enabled (for agents that don't specify)
                    eprintln!("Starting MCP server: {} (globally enabled)", id);
                    self.start_server(server_config).await?;
                } else {
                    eprintln!("Skipping MCP server {} (not used by agent and not enabled)", id);
                }
            } else {
                eprintln!("Skipping MCP server {} (no id)", server_config.name);
            }
        }

        Ok(())
    }

    /// Start an MCP server connection
    async fn start_server(&self, config: McpServerConfig) -> Result<(), String> {
        // For now, we'll create a placeholder client
        // In a full implementation, this would use rmcp to connect to the server
        let id = config.id.clone().unwrap_or_else(|| config.name.clone());
        let client = Arc::new(PlaceholderMcpClient::new(id.clone(), config.name));
        self.servers.write().await.insert(id, client);
        Ok(())
    }

    /// Get an MCP client by ID
    pub async fn get_client(&self, id: &str) -> Option<Arc<dyn McpClient>> {
        self.servers.read().await.get(id).cloned()
    }

    /// Execute a tool on an MCP server
    pub async fn execute_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, String> {
        let client = self.get_client(server_id).await
            .ok_or_else(|| format!("MCP server not found: {}", server_id))?;

        client.call_tool(tool_name, arguments).await
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// MCP CLIENT TRAIT
// ============================================================================

#[async_trait::async_trait]
pub trait McpClient: Send + Sync {
    fn name(&self) -> &str;

    async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value, String>;

    async fn list_tools(&self) -> Result<Vec<McpTool>, String>;
}

#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub parameters: Option<Value>,
}

// ============================================================================
// PLACEHOLDER MCP CLIENT
// ============================================================================

struct PlaceholderMcpClient {
    id: String,
    name: String,
}

impl PlaceholderMcpClient {
    fn new(id: String, name: String) -> Self {
        Self { id, name }
    }

    /// Get tool definitions for known MCP servers
    fn get_tools_for_server(&self) -> Vec<McpTool> {
        match self.id.as_str() {
            "timemcp" | "time-mcp" | "time" | "time-server" => vec![
                McpTool {
                    name: "get_current_time".to_string(),
                    description: "Get the current time in a specified timezone".to_string(),
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "timezone": {
                                "type": "string",
                                "description": "IANA timezone identifier (e.g., 'America/New_York', 'UTC')",
                                "default": "UTC"
                            }
                        }
                    })),
                },
                McpTool {
                    name: "get_timezones".to_string(),
                    description: "List available IANA timezones".to_string(),
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "filter": {
                                "type": "string",
                                "description": "Optional filter string to match timezone names"
                            }
                        }
                    })),
                },
            ],
            "filesystem" | "fs" | "filesystem-server" => vec![
                McpTool {
                    name: "read_file".to_string(),
                    description: "Read contents of a file".to_string(),
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "File path to read"
                            }
                        },
                        "required": ["path"]
                    })),
                },
                McpTool {
                    name: "write_file".to_string(),
                    description: "Write content to a file".to_string(),
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": {"type": "string"},
                            "content": {"type": "string"}
                        },
                        "required": ["path", "content"]
                    })),
                },
            ],
            _ => vec![],
        }
    }
}

#[async_trait::async_trait]
impl McpClient for PlaceholderMcpClient {
    fn name(&self) -> &str {
        &self.name
    }

    async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value, String> {
        // Placeholder implementation that simulates tool calls
        match self.id.as_str() {
            "timemcp" | "time-mcp" | "time" | "time-server" => {
                match tool_name {
                    "get_current_time" => {
                        let timezone = arguments.get("timezone")
                            .and_then(|v| v.as_str())
                            .unwrap_or("UTC");

                        // For now, return UTC time
                        Ok(serde_json::json!({
                            "timezone": timezone,
                            "datetime": chrono::Utc::now().to_rfc3339(),
                            "unix_timestamp": chrono::Utc::now().timestamp()
                        }))
                    }
                    "get_timezones" => {
                        Ok(serde_json::json!({
                            "timezones": vec!["UTC", "America/New_York", "America/Los_Angeles", "Europe/London", "Asia/Tokyo"]
                        }))
                    }
                    _ => Err(format!("Unknown tool: {}", tool_name))
                }
            }
            _ => {
                // Generic placeholder response
                Ok(serde_json::json!({
                    "result": format!("Called {} on {} with args: {}", tool_name, self.name, arguments)
                }))
            }
        }
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, String> {
        Ok(self.get_tools_for_server())
    }
}
