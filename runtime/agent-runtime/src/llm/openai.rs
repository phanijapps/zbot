// ============================================================================
// OPENAI COMPATIBLE CLIENT
// OpenAI-compatible API implementation
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_stream::StreamExt;

use crate::llm::client::{
    LlmClient, LlmError, ChatResponse, StreamChunk, StreamCallback, TokenUsage,
    ToolCallChunk,
};
use crate::llm::config::LlmConfig;
use crate::types::{ChatMessage, ToolCall};

/// OpenAI-compatible LLM client
///
/// This client works with any LLM provider that implements
/// the OpenAI API format (including many self-hosted models)
pub struct OpenAiClient {
    config: Arc<LlmConfig>,
    http_client: reqwest::Client,
}

/// Check if a JSON string is complete (has balanced braces, brackets, and strings).
///
/// Used to detect truncated tool call arguments when the LLM hits `max_tokens` mid-argument.
fn is_json_complete(json_str: &str) -> bool {
    let mut brace_count = 0i32;
    let mut bracket_count = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    for ch in json_str.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_string => {
                escape_next = true;
            }
            '"' => {
                in_string = !in_string;
            }
            '{' if !in_string => {
                brace_count += 1;
            }
            '}' if !in_string => {
                brace_count -= 1;
            }
            '[' if !in_string => {
                bracket_count += 1;
            }
            ']' if !in_string => {
                bracket_count -= 1;
            }
            _ => {}
        }

        if brace_count < 0 || bracket_count < 0 {
            return false;
        }
    }

    !in_string && brace_count == 0 && bracket_count == 0
}

impl OpenAiClient {
    /// Create a new OpenAI-compatible client
    pub fn new(config: LlmConfig) -> Result<Self, LlmError> {
        tracing::debug!("Creating OpenAI client for model: {}", config.model);
        Ok(Self {
            config: Arc::new(config),
            http_client: reqwest::Client::new(),
        })
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    /// Build the request body for the API
    fn build_request_body(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Value>,
    ) -> Value {
        let mut body_obj = json!({
            "model": self.config.model,
            "messages": messages,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
            "stream": false,
        });

        // Add tools if present
        if let Some(tools_val) = &tools {
            if let Some(body_map) = body_obj.as_object_mut() {
                body_map.insert("tools".to_string(), tools_val.clone());
            }
        }

        // Add thinking parameter if enabled (for DeepSeek, GLM, etc.)
        if self.config.thinking_enabled {
            if let Some(body_map) = body_obj.as_object_mut() {
                body_map.insert(
                    "thinking".to_string(),
                    json!({"type": "enabled"})
                );
            }
        }

        // Debug: log request size estimation
        let request_json = serde_json::to_string(&body_obj).unwrap_or_default();
        let estimated_chars = request_json.len();
        let estimated_tokens = estimated_chars / 4; // rough estimate: ~4 chars per token

        tracing::info!(
            "Request size: ~{} chars (~{} tokens estimated)",
            estimated_chars,
            estimated_tokens
        );

        // Log tools count and size
        if let Some(tools_val) = &tools {
            if let Some(tools_array) = tools_val.as_array() {
                let tools_json = serde_json::to_string(tools_val).unwrap_or_default();
                let tools_tokens = tools_json.len() / 4;
                tracing::info!(
                    "Tools: {} tools, ~{} chars (~{} tokens)",
                    tools_array.len(),
                    tools_json.len(),
                    tools_tokens
                );
            }
        }

        // Log messages size
        if let Some(messages_val) = body_obj.get("messages") {
            let messages_json = serde_json::to_string(messages_val).unwrap_or_default();
            let messages_tokens = messages_json.len() / 4;
            tracing::info!(
                "Messages: ~{} chars (~{} tokens)",
                messages_json.len(),
                messages_tokens
            );
        }

        if self.config.thinking_enabled {
            tracing::debug!("Thinking mode enabled");
        }

        body_obj
    }

    /// Make a non-streaming request to the API
    async fn make_request(&self, body: Value) -> Result<Value, LlmError> {
        let url = format!("{}/chat/completions", self.config.base_url);

        tracing::debug!("Making POST request to: {}", url);

        let response = self.http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("API error ({}): {}", status, error_text);
            return Err(LlmError::ApiError(format!("({}): {}", status, error_text)));
        }

        response
            .json::<Value>()
            .await
            .map_err(|e| LlmError::ParseError(format!("Failed to parse response: {}", e)))
    }

    /// Parse the API response
    fn parse_response(&self, response: Value) -> ChatResponse {
        let content = response
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Parse reasoning_content (for DeepSeek, GLM, etc.)
        let reasoning = response
            .pointer("/choices/0/message/reasoning_content")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse tool calls if present
        let tool_calls = self.parse_tool_calls(&response);

        // Parse token usage
        let usage = response.get("usage").and_then(|u| {
            Some(TokenUsage {
                prompt_tokens: u.get("prompt_tokens")?.as_u64()? as u32,
                completion_tokens: u.get("completion_tokens")?.as_u64()? as u32,
                total_tokens: u.get("total_tokens")?.as_u64()? as u32,
            })
        });

        ChatResponse {
            content,
            tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
            reasoning,
            usage,
        }
    }

    /// Parse tool calls from the response
    fn parse_tool_calls(&self, response: &Value) -> Vec<ToolCall> {
        if let Some(calls) = response.pointer("/choices/0/message/tool_calls") {
            if let Some(calls_array) = calls.as_array() {
                return calls_array
                    .iter()
                    .filter_map(|call| {
                        let id = call.get("id")?.as_str()?.to_string();
                        let name = call.get("function")?.get("name")?.as_str()?.to_string();
                        let arguments_str = call.get("function")?.get("arguments")?.as_str()?.to_string();

                        // Parse arguments from string to Value for internal use
                        let arguments = serde_json::from_str(&arguments_str).ok()?;

                        Some(ToolCall::new(id, name, arguments))
                    })
                    .collect();
            }
        }
        Vec::new()
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    fn model(&self) -> &str {
        &self.config.model
    }

    fn provider(&self) -> &str {
        &self.config.provider_id
    }

    async fn chat(&self, messages: Vec<ChatMessage>, tools: Option<Value>) -> Result<ChatResponse, LlmError> {
        tracing::info!("Starting chat with {} messages", messages.len());

        let body = self.build_request_body(messages, tools);
        let response = self.make_request(body).await?;
        let parsed = self.parse_response(response);

        tracing::info!("Chat completed, response length: {}", parsed.content.len());
        Ok(parsed)
    }

    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Value>,
        callback: StreamCallback,
    ) -> Result<ChatResponse, LlmError> {
        tracing::info!("Starting streaming chat with {} messages", messages.len());

        let url = format!("{}/chat/completions", self.config.base_url);

        let mut body_obj = self.build_request_body(messages, tools);
        // Enable streaming with usage reporting
        if let Some(obj) = body_obj.as_object_mut() {
            obj.insert("stream".to_string(), json!(true));
            obj.insert("stream_options".to_string(), json!({ "include_usage": true }));
        }

        tracing::debug!("Making streaming POST request to: {}", url);

        let response = self.http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body_obj)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("API error ({}): {}", status, error_text);
            return Err(LlmError::ApiError(format!("({}): {}", status, error_text)));
        }

        let mut full_content = String::new();
        let mut reasoning_content = String::new();
        let mut _finish_reason: Option<String> = None;
        let mut stream_usage: Option<TokenUsage> = None;

        // Accumulate streaming tool call deltas by index.
        // OpenAI sends tool calls as incremental deltas keyed by index:
        //   Delta 1: {index: 0, id: "call_123", function: {name: "write", arguments: ""}}
        //   Delta 2: {index: 0, function: {arguments: "{\"path\""}}
        //   Delta 3: {index: 0, function: {arguments: ": \"app.js\"}"}}
        // We must accumulate the id, name, and argument fragments per index,
        // then parse the complete JSON arguments after the stream ends.
        struct ToolCallAccumulator {
            id: String,
            name: String,
            arguments: String,
        }
        let mut tool_accumulators: std::collections::HashMap<u64, ToolCallAccumulator> =
            std::collections::HashMap::new();

        // Track provider-side accumulated text to handle providers that return
        // accumulated content instead of true deltas in streaming responses.
        // (e.g., Z.AI/GLM sends the full text so far in each delta.content)
        let mut provider_accumulated = String::new();

        // Read streaming response using line-buffered SSE parsing.
        // Each SSE line is processed exactly once — the buffer only retains
        // incomplete (partial) lines between HTTP chunks.
        let mut stream = response.bytes_stream();
        let mut sse_buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                tracing::error!("Stream error: {}", e);
                LlmError::HttpError(e)
            })?;
            sse_buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Find the last complete line boundary
            let Some(last_nl) = sse_buffer.rfind('\n') else {
                continue; // No complete line yet, keep buffering
            };

            // Split: everything up to last newline is complete; remainder is partial
            let complete = sse_buffer[..last_nl].to_string();
            sse_buffer = sse_buffer[last_nl + 1..].to_string();

            // Process each complete SSE line exactly once
            for line in complete.lines() {
                let line = line.trim();
                if !line.starts_with("data: ") {
                    continue;
                }
                let data_payload = &line[6..];
                if data_payload == "[DONE]" {
                    continue;
                }

                let json_data = match serde_json::from_str::<Value>(data_payload) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                // Capture finish_reason from the final chunk
                if let Some(reason) = json_data.pointer("/choices/0/finish_reason").and_then(|v| v.as_str()) {
                    _finish_reason = Some(reason.to_string());
                    if reason == "length" {
                        tracing::warn!("Stream finished with reason 'length' — response may be truncated");
                    }
                }

                // Capture usage from the final chunk (sent when stream_options.include_usage=true)
                if let Some(u) = json_data.get("usage") {
                    if let (Some(pt), Some(ct), Some(tt)) = (
                        u.get("prompt_tokens").and_then(|v| v.as_u64()),
                        u.get("completion_tokens").and_then(|v| v.as_u64()),
                        u.get("total_tokens").and_then(|v| v.as_u64()),
                    ) {
                        stream_usage = Some(TokenUsage {
                            prompt_tokens: pt as u32,
                            completion_tokens: ct as u32,
                            total_tokens: tt as u32,
                        });
                    }
                }

                let Some(delta) = json_data.pointer("/choices/0/delta") else {
                    continue;
                };

                // Regular content
                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                    // Handle both delta-style and accumulated-style providers:
                    // - OpenAI sends true deltas: "Hi", "!", " I", "'m"
                    // - Some providers (Z.AI/GLM) send accumulated text: "Hi", "Hi!", "Hi! I"
                    // Detect accumulated mode: if new content extends what we've seen so far,
                    // extract only the new suffix as the actual delta.
                    let actual_delta = if !provider_accumulated.is_empty()
                        && content.starts_with(&provider_accumulated)
                    {
                        &content[provider_accumulated.len()..]
                    } else {
                        content
                    };

                    // Update tracking
                    if !provider_accumulated.is_empty()
                        && content.starts_with(&provider_accumulated)
                    {
                        provider_accumulated = content.to_string();
                    } else {
                        provider_accumulated.push_str(content);
                    }

                    if !actual_delta.is_empty() {
                        full_content.push_str(actual_delta);
                        callback(StreamChunk::Token(actual_delta.to_string()));
                    }
                }

                // Reasoning content (for models with thinking enabled)
                if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_str()) {
                    reasoning_content.push_str(reasoning);
                    callback(StreamChunk::Reasoning(reasoning.to_string()));
                }

                // Tool calls — accumulate deltas by index
                if let Some(calls) = delta.get("tool_calls").and_then(|c| c.as_array()) {
                    for call in calls {
                        let index = call.get("index")
                            .and_then(|i| i.as_u64())
                            .unwrap_or(0);

                        let acc = tool_accumulators.entry(index).or_insert_with(|| {
                            ToolCallAccumulator {
                                id: String::new(),
                                name: String::new(),
                                arguments: String::new(),
                            }
                        });

                        // First delta for this index carries the id and name
                        if let Some(id) = call.get("id").and_then(|i| i.as_str()) {
                            if !id.is_empty() {
                                acc.id = id.to_string();
                            }
                        }
                        if let Some(name) = call.get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str())
                        {
                            if !name.is_empty() {
                                acc.name = name.to_string();
                            }
                        }

                        // Every delta may carry an argument fragment — append it
                        if let Some(args_fragment) = call.get("function")
                            .and_then(|f| f.get("arguments"))
                            .and_then(|a| a.as_str())
                        {
                            acc.arguments.push_str(args_fragment);
                        }

                        // Emit StreamChunk::ToolCall for UI feedback
                        callback(StreamChunk::ToolCall(ToolCallChunk {
                            id: if acc.id.is_empty() { None } else { Some(acc.id.clone()) },
                            name: if acc.name.is_empty() { None } else { Some(acc.name.clone()) },
                            arguments: acc.arguments.clone(),
                        }));
                    }
                }
            }
        }

        // Build final tool calls from accumulated deltas
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut indices: Vec<u64> = tool_accumulators.keys().copied().collect();
        indices.sort();
        for index in indices {
            if let Some(acc) = tool_accumulators.remove(&index) {
                if acc.name.is_empty() {
                    tracing::warn!("Skipping tool call at index {} with empty name", index);
                    continue;
                }
                let args_value = if !is_json_complete(&acc.arguments) {
                    tracing::error!(
                        "Tool '{}' arguments JSON is incomplete (truncated). Args (first 200): '{}'",
                        acc.name, &acc.arguments[..acc.arguments.len().min(200)]
                    );
                    json!({
                        "__error__": "TRUNCATED_ARGUMENTS",
                        "__message__": "Tool call arguments were truncated. Try a shorter command or split into multiple calls.",
                        "__original_length__": acc.arguments.len(),
                        "__truncated__": true
                    })
                } else {
                    match serde_json::from_str::<serde_json::Value>(&acc.arguments) {
                        Ok(args) => args,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to parse tool call arguments for '{}': {} — raw: {}",
                                acc.name, e, acc.arguments
                            );
                            json!({
                                "__error__": "PARSE_ERROR",
                                "__message__": format!("JSON parse error: {}", e),
                                "__truncated__": false
                            })
                        }
                    }
                };
                tool_calls.push(ToolCall::new(acc.id, acc.name, args_value));
            }
        }

        // Use provider-reported usage if available, otherwise estimate from character count
        let usage = stream_usage.unwrap_or_else(|| {
            let estimated_completion = (full_content.len() + reasoning_content.len()) as u32 / 4;
            TokenUsage {
                prompt_tokens: 0,
                completion_tokens: estimated_completion,
                total_tokens: estimated_completion,
            }
        });

        tracing::info!(
            "Streaming completed: prompt={} completion={} total={} tokens, {} tool calls",
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens,
            tool_calls.len()
        );

        Ok(ChatResponse {
            content: full_content,
            tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
            reasoning: if reasoning_content.is_empty() { None } else { Some(reasoning_content) },
            usage: Some(usage),
        })
    }

    fn supports_reasoning(&self) -> bool {
        self.config.thinking_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_client_creation() {
        let config = LlmConfig::new(
            "https://api.openai.com".to_string(),
            "test-key".to_string(),
            "gpt-4".to_string(),
            "openai".to_string(),
        );

        let client = OpenAiClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_tool_call_parsing() {
        let tool_call = ToolCall::new(
            "call_123".to_string(),
            "search".to_string(),
            json!({"query": "test"}),
        );

        assert_eq!(tool_call.id, "call_123");
        assert_eq!(tool_call.name, "search");
    }
}
