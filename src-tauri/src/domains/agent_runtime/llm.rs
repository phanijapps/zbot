// ============================================================================
// LLM CLIENT
// OpenAI-compatible API client for various providers
// ============================================================================

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::settings::AppDirs;

// ============================================================================
// CONFIGURATION
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider_id: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
}

/// Message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Tool call from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: ToolCallType,
    pub function: ToolCallFunction,
}

/// Tool call type (always "function" for OpenAI)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolCallType {
    Function,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,  // JSON string
}

impl ToolCall {
    /// Create a new tool call
    pub fn new(id: String, name: String, arguments: Value) -> Result<Self, String> {
        let arguments_str = serde_json::to_string(&arguments)
            .map_err(|e| format!("Failed to serialize arguments: {}", e))?;
        Ok(ToolCall {
            id,
            call_type: ToolCallType::Function,
            function: ToolCallFunction {
                name,
                arguments: arguments_str,
            },
        })
    }

    /// Get the function name (for backward compatibility)
    pub fn name(&self) -> &str {
        &self.function.name
    }

    /// Get the arguments as Value (for backward compatibility)
    pub fn arguments(&self) -> Result<Value, String> {
        serde_json::from_str(&self.function.arguments)
            .map_err(|e| format!("Failed to parse arguments: {}", e))
    }
}

/// Tool result to send back to LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub output: String,
    pub error: Option<String>,
}

/// Chat response from LLM
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
    pub tokens_used: Option<u32>,
}

// ============================================================================
// LLM CLIENT TRAIT
// ============================================================================

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Value>,
    ) -> Result<ChatResponse, String>;

    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Value>,
        callback: Arc<Mutex<dyn StreamingCallback + Send>>,
    ) -> Result<ChatResponse, String>;
}

/// Callback for streaming responses
#[async_trait]
pub trait StreamingCallback {
    fn on_token(&mut self, token: &str);
    fn on_tool_call(&mut self, tool_call: &ToolCall);
}

// ============================================================================
// OPENAI-COMPATIBLE CLIENT
// ============================================================================

pub struct OpenAiClient {
    config: LlmConfig,
    client: reqwest::Client,
}

impl OpenAiClient {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Load provider configuration from the providers file
    pub async fn from_provider(provider_id: &str, model: &str) -> Result<Self, String> {
        let dirs = AppDirs::get().map_err(|e| e.to_string())?;
        let providers_file = dirs.config_dir.join("providers.json");

        let content = std::fs::read_to_string(&providers_file)
            .map_err(|e| format!("Failed to read providers file: {}", e))?;

        let providers: Vec<Value> = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse providers: {}", e))?;

        let provider = providers
            .into_iter()
            .find(|p| p.get("id").and_then(|i| i.as_str()) == Some(provider_id))
            .ok_or_else(|| format!("Provider not found: {}", provider_id))?;

        let api_key = provider.get("apiKey")
            .and_then(|k| k.as_str())
            .ok_or_else(|| format!("Provider missing apiKey"))?
            .to_string();

        let base_url = provider.get("baseUrl")
            .and_then(|u| u.as_str())
            .ok_or_else(|| format!("Provider missing baseUrl"))?
            .to_string();

        let config = LlmConfig {
            provider_id: provider_id.to_string(),
            api_key,
            base_url,
            model: model.to_string(),
            temperature: 0.7,
            max_tokens: 2000,
        };

        Ok(Self::new(config))
    }

    fn build_request_body(&self, messages: Vec<ChatMessage>, tools: Option<Value>) -> Value {
        json!({
            "model": self.config.model,
            "messages": messages,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
            "stream": false,
            "tools": tools
        })
    }

    async fn make_request(&self, body: Value) -> Result<Value, String> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to make request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("API error ({}): {}", status, error_text));
        }

        response
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    fn parse_response(&self, response: Value) -> ChatResponse {
        let content = response
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let finish_reason = response
            .pointer("/choices/0/finish_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let tokens_used = response
            .pointer("/usage/total_tokens")
            .and_then(|v| v.as_u64())
            .map(|t| t as u32);

        // Parse tool calls if present
        let tool_calls = if let Some(calls) = response.pointer("/choices/0/message/tool_calls") {
            if let Some(calls_array) = calls.as_array() {
                calls_array
                    .iter()
                    .filter_map(|call| {
                        let id = call.get("id")?.as_str()?.to_string();
                        let name = call.get("function")?.get("name")?.as_str()?.to_string();
                        let arguments_str = call.get("function")?.get("arguments")?.as_str()?.to_string();

                        // Parse arguments from string to Value for internal use
                        let arguments = serde_json::from_str(&arguments_str).ok()?;

                        ToolCall::new(id, name, arguments).ok()
                    })
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        ChatResponse {
            content,
            tool_calls,
            finish_reason,
            tokens_used,
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Value>,
    ) -> Result<ChatResponse, String> {
        let body = self.build_request_body(messages, tools);
        let response = self.make_request(body).await?;
        Ok(self.parse_response(response))
    }

    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Value>,
        callback: Arc<Mutex<dyn StreamingCallback + Send>>,
    ) -> Result<ChatResponse, String> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let mut body_obj = self.build_request_body(messages, tools);
        // Enable streaming
        if let Some(obj) = body_obj.as_object_mut() {
            obj.insert("stream".to_string(), json!(true));
        }

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body_obj)
            .send()
            .await
            .map_err(|e| format!("Failed to make request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("API error ({}): {}", status, error_text));
        }

        let mut full_content = String::new();
        let mut tool_calls = Vec::new();

        // Read streaming response
        let mut stream = response.bytes_stream();
        use tokio_stream::StreamExt;

        let mut buffer = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
            buffer.extend_from_slice(&chunk);

            // Process complete SSE lines
            let data_str = String::from_utf8_lossy(&buffer).to_string();
            let lines: Vec<&str> = data_str.split('\n').collect();

            // Find the last complete line and keep incomplete data in buffer
            let last_newline_idx = data_str.rfind('\n');
            let keep_in_buffer = if let Some(idx) = last_newline_idx {
                // There's at least one complete line
                // Calculate byte offset of the incomplete part
                let complete_bytes = data_str[..idx].as_bytes().len();
                // Keep the incomplete part
                if idx + 1 < data_str.len() {
                    buffer.split_off(complete_bytes)
                } else {
                    buffer.split_off(buffer.len())
                }
            } else {
                // No complete lines yet, keep everything in buffer
                Vec::new()
            };

            // Process all complete lines (excluding the last incomplete one)
            for line in lines.iter().take(lines.len().saturating_sub(if keep_in_buffer.is_empty() { 0 } else { 1 })) {
                let line = line.trim();
                if line.starts_with("data: ") {
                    let data_str = &line[6..];
                    if data_str == "[DONE]" {
                        continue;
                    }

                    if let Ok(json_data) = serde_json::from_str::<Value>(data_str) {
                        // Check for delta content
                        if let Some(delta) = json_data.pointer("/choices/0/delta") {
                            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                full_content.push_str(content);
                                callback.lock().await.on_token(content);
                            }

                            // Check for tool calls
                            if let Some(calls) = delta.get("tool_calls").and_then(|c| c.as_array()) {
                                for call in calls {
                                    let id = call.get("id")
                                        .and_then(|i| i.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let name = call.get("function")
                                        .and_then(|f| f.get("name"))
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    let args = call.get("function")
                                        .and_then(|f| f.get("arguments"))
                                        .and_then(|a| a.as_str())
                                        .unwrap_or("{}");

                                    if !name.is_empty() {
                                        // Parse arguments string to Value
                                        let args_value = serde_json::from_str(args).unwrap_or(json!({}));

                                        if let Ok(tool_call) = ToolCall::new(id, name, args_value) {
                                            callback.lock().await.on_tool_call(&tool_call);
                                            tool_calls.push(tool_call);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // If there was incomplete data, keep it in buffer
            if !keep_in_buffer.is_empty() {
                buffer = keep_in_buffer;
            }
        }

        let finish_reason = None;
        let tokens_used = Some(full_content.len() as u32);

        Ok(ChatResponse {
            content: full_content,
            tool_calls,
            finish_reason,
            tokens_used,
        })
    }
}

// ============================================================================
// MODEL FACTORY
// ============================================================================

/// Create an LLM client from agent configuration
pub async fn create_llm_client(
    agent_id: &str,
) -> Result<Arc<dyn LlmClient>, String> {
    // Load agent configuration
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    let agent_dir = dirs.config_dir.join("agents").join(agent_id);
    let config_file = agent_dir.join("config.yaml");

    if !config_file.exists() {
        return Err(format!("Agent config not found: {}", config_file.display()));
    }

    let config_content = std::fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read agent config: {}", e))?;

    let agent_config: serde_yaml::Value = serde_yaml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse agent config: {}", e))?;

    let provider_id = agent_config.get("providerId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Agent missing providerId"))?;

    let model = agent_config.get("model")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Agent missing model"))?;

    let client = OpenAiClient::from_provider(provider_id, model).await?;
    Ok(Arc::new(client))
}

/// Simple model wrapper for compatibility
pub struct LlmModel {
    client: Arc<dyn LlmClient>,
}

impl LlmModel {
    pub fn new(client: Arc<dyn LlmClient>) -> Self {
        Self { client }
    }

    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<ChatResponse, String> {
        self.client.chat(messages, None).await
    }
}
