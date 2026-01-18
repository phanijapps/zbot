// ============================================================================
// SSE MCP CLIENT
// ============================================================================

//! # SSE MCP Client
//!
//! Server-Sent Events transport implementation for MCP clients.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use super::client::McpClient;
use super::error::McpError;
use super::tool::McpTool;

/// SSE-based MCP client
pub(super) struct SseMcpClient {
    id: String,
    name: String,
    url: String,
    headers: HashMap<String, String>,
    client: reqwest::Client,
}

impl SseMcpClient {
    pub(super) fn new(id: String, name: String, url: String, headers: HashMap<String, String>) -> Self {
        tracing::debug!("Creating SSE MCP client: {} at {}", name, url);
        Self {
            id,
            name,
            url,
            headers,
            client: reqwest::Client::new(),
        }
    }

    /// Send a JSON-RPC request via POST
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, McpError> {
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": uuid::Uuid::new_v4().to_string(),
            "method": method,
            "params": params
        });

        tracing::debug!("SSE MCP request to {}: {}", self.url, request_body);

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
            .map_err(|e| McpError::ProtocolError(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| McpError::ProtocolError(format!("Failed to read response: {}", e)))?;

        tracing::debug!("SSE MCP response status: {}, body: {}", status, response_text);

        if !status.is_success() {
            return Err(McpError::ProtocolError(format!("HTTP error {}: {}", status.as_u16(), response_text)));
        }

        let response_json: Value = serde_json::from_str(&response_text)
            .map_err(|e| McpError::ProtocolError(format!("Failed to parse JSON response: {}", e)))?;

        // Check for JSON-RPC error
        if let Some(error) = response_json.get("error") {
            return Err(McpError::ProtocolError(format!("MCP error: {}", error)));
        }

        Ok(response_json)
    }
}

#[async_trait]
impl McpClient for SseMcpClient {
    fn name(&self) -> &str {
        &self.name
    }

    async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value, McpError> {
        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments
        });

        let response = self.send_request("tools/call", params).await?;

        // Extract the result from the response
        response.get("result")
            .or_else(|| response.get("content"))
            .cloned()
            .ok_or_else(|| McpError::ProtocolError("No result in MCP response".to_string()))
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let response = self.send_request("tools/list", Value::Null).await?;

        let tools_array = response.get("result")
            .and_then(|v| v.get("tools"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| McpError::ProtocolError("No tools array in MCP response".to_string()))?;

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
