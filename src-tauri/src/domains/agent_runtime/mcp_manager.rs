// ============================================================================
// MCP MANAGER
// Manages Model Context Protocol server connections and tool execution
// Supports both stdio (command-based) and HTTP-based MCP servers
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
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerConfig {
    #[serde(rename = "stdio")]
    Stdio {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        name: String,
        description: String,
        command: String,
        args: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        env: Option<HashMap<String, String>>,
        #[serde(default)]
        enabled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        validated: Option<bool>,
    },
    #[serde(rename = "http")]
    Http {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        name: String,
        description: String,
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        #[serde(default)]
        enabled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        validated: Option<bool>,
    },
    #[serde(rename = "sse")]
    Sse {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        name: String,
        description: String,
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        #[serde(default)]
        enabled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        validated: Option<bool>,
    },
    #[serde(rename = "streamable-http")]
    StreamableHttp {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        name: String,
        description: String,
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        #[serde(default)]
        enabled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        validated: Option<bool>,
    },
}

impl McpServerConfig {
    pub fn id(&self) -> String {
        match self {
            McpServerConfig::Stdio { id, name, .. } => id.clone().unwrap_or_else(|| name.clone()),
            McpServerConfig::Http { id, name, .. } => id.clone().unwrap_or_else(|| name.clone()),
            McpServerConfig::Sse { id, name, .. } => id.clone().unwrap_or_else(|| name.clone()),
            McpServerConfig::StreamableHttp { id, name, .. } => id.clone().unwrap_or_else(|| name.clone()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            McpServerConfig::Stdio { name, .. } => name,
            McpServerConfig::Http { name, .. } => name,
            McpServerConfig::Sse { name, .. } => name,
            McpServerConfig::StreamableHttp { name, .. } => name,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            McpServerConfig::Stdio { enabled, .. } => *enabled,
            McpServerConfig::Http { enabled, .. } => *enabled,
            McpServerConfig::Sse { enabled, .. } => *enabled,
            McpServerConfig::StreamableHttp { enabled, .. } => *enabled,
        }
    }
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

        // Support both array format and single object format
        let servers: Vec<McpServerConfig> = if content.trim().starts_with('[') {
            serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse MCP servers array: {}", e))?
        } else {
            // Single object - wrap in array
            let server: McpServerConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse MCP server: {}", e))?;
            vec![server]
        };

        eprintln!("Found {} MCP servers in config", servers.len());

        for server_config in servers {
            let id = server_config.id();
            let name = server_config.name().to_string();
            let enabled = server_config.enabled();

            eprintln!("Checking server: id={}, name={}, enabled={}", id, name, enabled);

            if agent_mcps.contains(&id) {
                // Start this MCP server since the agent explicitly uses it
                eprintln!("Starting MCP server: {} (required by agent)", id);
                self.start_server(server_config).await?;
            } else if enabled {
                // Also start if it's globally enabled
                eprintln!("Starting MCP server: {} (globally enabled)", id);
                self.start_server(server_config).await?;
            } else {
                eprintln!("Skipping MCP server {} (not used by agent and not enabled)", id);
            }
        }

        Ok(())
    }

    /// Start an MCP server connection
    async fn start_server(&self, config: McpServerConfig) -> Result<(), String> {
        match config {
            McpServerConfig::Stdio { id, name, command, args, env, .. } => {
                let id = id.unwrap_or_else(|| name.clone());
                let client = Arc::new(StdioMcpClient::new(
                    id.clone(),
                    name,
                    command,
                    args,
                    env.unwrap_or_default(),
                )?);
                self.servers.write().await.insert(id, client);
                Ok(())
            }
            McpServerConfig::Http { id, name, url, headers, .. } => {
                let id = id.unwrap_or_else(|| name.clone());
                let client = Arc::new(HttpMcpClient::new(id.clone(), name, url, headers.unwrap_or_default()));
                self.servers.write().await.insert(id, client);
                Ok(())
            }
            McpServerConfig::Sse { id, name, url, headers, .. } => {
                let id = id.unwrap_or_else(|| name.clone());
                let client = Arc::new(SseMcpClient::new(id.clone(), name, url, headers.unwrap_or_default()));
                self.servers.write().await.insert(id, client);
                Ok(())
            }
            McpServerConfig::StreamableHttp { id, name, url, headers, .. } => {
                let id = id.unwrap_or_else(|| name.clone());
                // Streamable-http uses the same client as HTTP for now
                let client = Arc::new(HttpMcpClient::new(id.clone(), name, url, headers.unwrap_or_default()));
                self.servers.write().await.insert(id, client);
                Ok(())
            }
        }
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
// HTTP MCP CLIENT
// ============================================================================

struct HttpMcpClient {
    id: String,
    name: String,
    url: String,
    headers: HashMap<String, String>,
    client: reqwest::Client,
}

impl HttpMcpClient {
    fn new(id: String, name: String, url: String, headers: HashMap<String, String>) -> Self {
        Self {
            id,
            name,
            url,
            headers,
            client: reqwest::Client::new(),
        }
    }

    /// Send a JSON-RPC request to the HTTP MCP server
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": uuid::Uuid::new_v4().to_string(),
            "method": method,
            "params": params
        });

        eprintln!("[HttpMcpClient] Sending request to {}: {}", self.url, request_body);

        let mut req = self.client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        // Add custom headers (e.g., Authorization)
        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let response = req
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        eprintln!("[HttpMcpClient] Response status: {}, body: {}", status, response_text);

        if !status.is_success() {
            return Err(format!("HTTP error {}: {}", status.as_u16(), response_text));
        }

        let response_json: Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

        // Check for JSON-RPC error
        if let Some(error) = response_json.get("error") {
            return Err(format!("MCP error: {}", error));
        }

        Ok(response_json)
    }
}

#[async_trait::async_trait]
impl McpClient for HttpMcpClient {
    fn name(&self) -> &str {
        &self.name
    }

    async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value, String> {
        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments
        });

        let response = self.send_request("tools/call", params).await?;

        // Extract the result from the response
        response.get("result")
            .or_else(|| response.get("content"))
            .cloned()
            .ok_or_else(|| "No result in MCP response".to_string())
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, String> {
        let response = self.send_request("tools/list", Value::Null).await?;

        let tools_array = response.get("result")
            .and_then(|v| v.get("tools"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| "No tools array in MCP response".to_string())?;

        let mut tools = Vec::new();
        for tool in tools_array {
            let name = tool.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let description = tool.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let parameters = tool.get("inputSchema").cloned();

            tools.push(McpTool {
                name,
                description,
                parameters,
            });
        }

        Ok(tools)
    }
}

// ============================================================================
// STDIO MCP CLIENT (for stdio-based servers with real subprocess execution)
// ============================================================================

struct StdioMcpClient {
    id: String,
    name: String,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
}

impl StdioMcpClient {
    fn new(
        id: String,
        name: String,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Result<Self, String> {
        eprintln!("[StdioMcpClient::new] id={}, name={}, command={}, args={:?}",
            id, name, command, args);

        Ok(Self {
            id,
            name,
            command,
            args,
            env,
        })
    }

    /// Spawn the MCP server process and execute a tool call
    async fn spawn_and_call(&self, tool_name: &str, arguments: &Value) -> Result<Value, String> {
        use tokio::process::Command;

        eprintln!("[StdioMcpClient] Spawning: {} with args: {:?}", self.command, self.args);

        // Build the command
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);

        // Set environment variables if provided
        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        // Create JSON-RPC requests for initialization and tool call
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "agentzero",
                    "version": "0.1.0"
                }
            }
        });

        let initialized_notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let tool_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        eprintln!("[StdioMcpClient] Sending tool call: {} with args: {}", tool_name, arguments);

        // Spawn the process and communicate via stdin/stdout
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn MCP process: {}", e))?;

        // Write all requests to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;

            // Send initialize request
            let init_str = format!("{}\n", init_request);
            stdin.write_all(init_str.as_bytes()).await
                .map_err(|e| format!("Failed to write init to stdin: {}", e))?;
            stdin.flush().await
                .map_err(|e| format!("Failed to flush init: {}", e))?;

            // Send initialized notification
            let notif_str = format!("{}\n", initialized_notification);
            stdin.write_all(notif_str.as_bytes()).await
                .map_err(|e| format!("Failed to write notification to stdin: {}", e))?;
            stdin.flush().await
                .map_err(|e| format!("Failed to flush notification: {}", e))?;

            // Send tool call request
            let tool_str = format!("{}\n", tool_request);
            stdin.write_all(tool_str.as_bytes()).await
                .map_err(|e| format!("Failed to write tool request to stdin: {}", e))?;
            stdin.flush().await
                .map_err(|e| format!("Failed to flush tool request: {}", e))?;
        }

        // Read response from stdout (no timeout - wait indefinitely)
        let output = child.wait_with_output().await
            .map_err(|e| format!("Failed to read from MCP process: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        eprintln!("[StdioMcpClient] Process exited with: {:?}", output.status);
        eprintln!("[StdioMcpClient] stdout: {}", stdout);
        if !stderr.is_empty() {
            eprintln!("[StdioMcpClient] stderr: {}", stderr);
        }

        if !output.status.success() {
            return Err(format!("MCP process failed: {}", stderr));
        }

        // Parse JSON responses - we need to find the tool call response (id: 2)
        let mut tool_result = None;

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(response) = serde_json::from_str::<Value>(line) {
                // Look for tool call response (id: 2)
                if response.get("id").and_then(|v| v.as_i64()) == Some(2) {
                    // Check for JSON-RPC error first
                    if let Some(error) = response.get("error") {
                        return Err(format!("MCP error: {}", error));
                    }

                    tool_result = response.get("result")
                        .or_else(|| response.get("content"))
                        .cloned();
                }
            }
        }

        tool_result.ok_or_else(|| "No tool result in MCP response".to_string())
    }

    /// List tools by spawning the process and calling tools/list
    async fn spawn_and_list(&self) -> Result<Vec<McpTool>, String> {
        use tokio::process::Command;

        eprintln!("[StdioMcpClient] Listing tools for: {}", self.name);

        // Build the command
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);

        // Set environment variables if provided
        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        // Create JSON-RPC requests for initialization and tools/list
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "agentzero",
                    "version": "0.1.0"
                }
            }
        });

        let initialized_notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let tools_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });

        // Spawn the process
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn MCP process: {}", e))?;

        // Write all requests to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;

            // Send initialize request
            let init_str = format!("{}\n", init_request);
            stdin.write_all(init_str.as_bytes()).await
                .map_err(|e| format!("Failed to write init to stdin: {}", e))?;
            stdin.flush().await
                .map_err(|e| format!("Failed to flush init: {}", e))?;

            // Send initialized notification
            let notif_str = format!("{}\n", initialized_notification);
            stdin.write_all(notif_str.as_bytes()).await
                .map_err(|e| format!("Failed to write notification to stdin: {}", e))?;
            stdin.flush().await
                .map_err(|e| format!("Failed to flush notification: {}", e))?;

            // Send tools/list request
            let tools_str = format!("{}\n", tools_request);
            stdin.write_all(tools_str.as_bytes()).await
                .map_err(|e| format!("Failed to write tools request to stdin: {}", e))?;
            stdin.flush().await
                .map_err(|e| format!("Failed to flush tools request: {}", e))?;
        }

        // Read response
        let output = child.wait_with_output().await
            .map_err(|e| format!("Failed to read from MCP process: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        eprintln!("[StdioMcpClient] Process exited with: {:?}", output.status);
        eprintln!("[StdioMcpClient] stdout: {}", stdout);
        if !stderr.is_empty() {
            eprintln!("[StdioMcpClient] stderr: {}", stderr);
        }

        if !output.status.success() {
            return Err(format!("MCP process failed: {}", stderr));
        }

        // Parse JSON responses - we need to find the tools/list response
        // The stdout will have multiple JSON-RPC responses, one per line
        let mut tools_array = None;

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(response) = serde_json::from_str::<Value>(line) {
                eprintln!("[StdioMcpClient] Parsed response line: {}", response);

                // Skip initialize response (id: 1)
                if response.get("id").and_then(|v| v.as_i64()) == Some(1) {
                    continue;
                }

                // Look for tools/list response (id: 2)
                if response.get("id").and_then(|v| v.as_i64()) == Some(2) {
                    // Check for JSON-RPC error first
                    if let Some(error) = response.get("error") {
                        return Err(format!("MCP error: {}", error));
                    }

                    if let Some(tools) = response.get("result")
                        .and_then(|v| v.get("tools"))
                        .and_then(|v| v.as_array())
                    {
                        tools_array = Some(tools.clone());
                    } else {
                        eprintln!("[StdioMcpClient] Response structure: {}",
                            serde_json::to_string_pretty(&response).unwrap_or_else(|_| "Cannot prettify".to_string()));
                        return Err("No tools array in MCP response".to_string());
                    }
                }
            }
        }

        let tools_array = tools_array.ok_or_else(|| "No tools/list response found".to_string())?;

        let mut tools = Vec::new();
        for tool in tools_array {
            let name = tool.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let description = tool.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let parameters = tool.get("inputSchema").cloned();

            tools.push(McpTool {
                name,
                description,
                parameters,
            });
        }

        Ok(tools)
    }
}

#[async_trait::async_trait]
impl McpClient for StdioMcpClient {
    fn name(&self) -> &str {
        &self.name
    }

    async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value, String> {
        self.spawn_and_call(tool_name, &arguments).await
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, String> {
        self.spawn_and_list().await
    }
}

// ============================================================================
// SSE MCP CLIENT (for Server-Sent Events based servers)
// ============================================================================

struct SseMcpClient {
    id: String,
    name: String,
    url: String,
    headers: HashMap<String, String>,
    client: reqwest::Client,
}

impl SseMcpClient {
    fn new(id: String, name: String, url: String, headers: HashMap<String, String>) -> Self {
        Self {
            id,
            name,
            url,
            headers,
            client: reqwest::Client::new(),
        }
    }

    /// Send a JSON-RPC request via POST (uses the URL as provided)
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": uuid::Uuid::new_v4().to_string(),
            "method": method,
            "params": params
        });

        eprintln!("[SseMcpClient] Sending request to {}: {}", self.url, request_body);

        let mut req = self.client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        // Add custom headers (e.g., Authorization)
        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let response = req
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        eprintln!("[SseMcpClient] Response status: {}, body: {}", status, response_text);

        if !status.is_success() {
            return Err(format!("HTTP error {}: {}", status.as_u16(), response_text));
        }

        let response_json: Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

        // Check for JSON-RPC error
        if let Some(error) = response_json.get("error") {
            return Err(format!("MCP error: {}", error));
        }

        Ok(response_json)
    }
}

#[async_trait::async_trait]
impl McpClient for SseMcpClient {
    fn name(&self) -> &str {
        &self.name
    }

    async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value, String> {
        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments
        });

        let response = self.send_request("tools/call", params).await?;

        // Extract the result from the response
        response.get("result")
            .or_else(|| response.get("content"))
            .cloned()
            .ok_or_else(|| "No result in MCP response".to_string())
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, String> {
        let response = self.send_request("tools/list", Value::Null).await?;

        let tools_array = response.get("result")
            .and_then(|v| v.get("tools"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| "No tools array in MCP response".to_string())?;

        let mut tools = Vec::new();
        for tool in tools_array {
            let name = tool.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let description = tool.get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let parameters = tool.get("inputSchema").cloned();

            tools.push(McpTool {
                name,
                description,
                parameters,
            });
        }

        Ok(tools)
    }
}
