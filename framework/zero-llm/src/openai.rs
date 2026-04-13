//! # OpenAI LLM Client
//!
//! OpenAI-compatible LLM client implementation.

use super::config::LlmConfig;
use super::{
    Llm, LlmRequest, LlmResponse, LlmResponseChunk, LlmResponseStream, TokenUsage, ToolCall,
};
use async_trait::async_trait;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zero_core::error::{Result, ZeroError};
use zero_core::types::Part;

/// OpenAI-compatible LLM client.
pub struct OpenAiLlm {
    config: LlmConfig,
    client: reqwest::Client,
}

impl OpenAiLlm {
    /// Create a new OpenAI LLM client.
    pub fn new(config: LlmConfig) -> Result<Self> {
        Ok(Self {
            config,
            client: reqwest::Client::builder()
                .user_agent(concat!("Z-bot/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client"),
        })
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    /// Convert our LlmRequest to OpenAI format.
    fn to_openai_request(&self, request: &LlmRequest) -> OpenAiRequest {
        OpenAiRequest {
            model: self.config.model.clone(),
            messages: self.to_openai_messages(request),
            temperature: request.temperature.or(self.config.temperature),
            max_tokens: request.max_tokens.or(self.config.max_tokens),
            tools: request.tools.as_ref().map(|tools| {
                tools
                    .iter()
                    .map(|t| OpenAiTool {
                        r#type: "function".to_string(),
                        function: OpenAiFunction {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            parameters: t.parameters.clone().unwrap_or_else(|| {
                                serde_json::json!({
                                    "type": "object",
                                    "properties": {}
                                })
                            }),
                        },
                    })
                    .collect()
            }),
            stream: false,
        }
    }

    /// Convert our Content to OpenAI message format.
    fn to_openai_messages(&self, request: &LlmRequest) -> Vec<OpenAiMessage> {
        let mut messages = Vec::new();

        // Add system instruction if present
        if let Some(ref instruction) = request.system_instruction {
            messages.push(OpenAiMessage {
                role: "system".to_string(),
                content: Some(instruction.clone()),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        // Add contents as messages
        for content in &request.contents {
            let (text_parts, tool_calls, tool_responses) = self.extract_parts(content);

            // Special handling for role="tool" with multiple FunctionResponse parts
            // Create one OpenAI message per FunctionResponse
            if content.role == "tool" && !tool_responses.is_empty() {
                for (response_id, response_content) in tool_responses {
                    messages.push(OpenAiMessage {
                        role: "tool".to_string(),
                        content: Some(response_content),
                        tool_calls: None,
                        tool_call_id: Some(response_id),
                    });
                }
            } else if !tool_responses.is_empty() {
                // Content with tool responses but role != "tool"
                // Create tool messages for each response
                for (response_id, response_content) in tool_responses {
                    messages.push(OpenAiMessage {
                        role: "tool".to_string(),
                        content: Some(response_content),
                        tool_calls: None,
                        tool_call_id: Some(response_id),
                    });
                }
            } else {
                // Regular user/assistant message
                messages.push(OpenAiMessage {
                    role: content.role.clone(),
                    content: if text_parts.is_empty() {
                        None
                    } else {
                        Some(text_parts.join("\n"))
                    },
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                });
            }
        }

        messages
    }

    /// Extract text and tool calls from Content parts.
    /// Returns (text_parts, tool_calls, tool_responses)
    /// where tool_responses is a Vec of (tool_call_id, response_content) tuples
    fn extract_parts(
        &self,
        content: &zero_core::types::Content,
    ) -> (Vec<String>, Vec<OpenAiToolCall>, Vec<(String, String)>) {
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        let mut tool_responses = Vec::new();

        for part in &content.parts {
            match part {
                Part::Text { text } => text_parts.push(text.clone()),
                Part::FunctionCall { name, args, id } => {
                    let args_str = serde_json::to_string(args).unwrap_or_default();
                    tool_calls.push(OpenAiToolCall {
                        id: id
                            .clone()
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                        r#type: "function".to_string(),
                        function: OpenAiFunctionCall {
                            name: name.clone(),
                            arguments: args_str,
                        },
                    });
                }
                Part::FunctionResponse { id, response } => {
                    // Collect all tool responses (not just one)
                    tool_responses.push((id.clone(), response.clone()));
                }
                Part::Image { .. } | Part::File { .. } => {
                    // Multimodal parts are handled by the encoder layer
                }
            }
        }

        (text_parts, tool_calls, tool_responses)
    }

    /// Convert OpenAI response to our format.
    #[allow(clippy::wrong_self_convention)]
    fn from_openai_response(&self, response: OpenAiResponse) -> LlmResponse {
        // Check for tool calls in the message (OpenAI API location, not response root)
        let tool_calls = response
            .choices
            .first()
            .and_then(|c| c.message.tool_calls.as_ref());

        if let Some(tool_calls) = tool_calls {
            tracing::info!(
                "from_openai_response: Response has tool_calls, count={}",
                tool_calls.len()
            );

            // Check if response was truncated (finish_reason: "length")
            let is_truncated = response
                .choices
                .first()
                .and_then(|c| c.finish_reason.as_ref())
                .map(|r| r == "length")
                .unwrap_or(false);

            if is_truncated {
                tracing::warn!(
                    "Response was truncated (finish_reason: length). Tool calls may be incomplete."
                );
            }

            let our_tool_calls: Vec<ToolCall> = tool_calls
                .iter()
                .map(|tc| {
                    tracing::info!("Parsing tool call: name={}, arguments.len={}",
                        tc.function.name, tc.function.arguments.len());

                    // Validate JSON completeness before parsing
                    let args_str = &tc.function.arguments;
                    let is_complete_json = Self::is_json_complete(args_str);

                    if !is_complete_json {
                        tracing::error!("Tool '{}' arguments JSON is incomplete (truncated). Cannot execute tool. Arguments (first 200 chars): '{}...'",
                            tc.function.name, &args_str[..args_str.len().min(200)]);

                        return ToolCall {
                            id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            arguments: serde_json::json!({
                                "__error__": "TRUNCATED_ARGUMENTS",
                                "__message__": "The file content is too large to complete in one call. Split the content into smaller chunks and write them separately. For example: 1) Write the first half of the content to the file. 2) Use the edit tool or make multiple write calls with different filenames to write the remaining content.",
                                "__original_length__": args_str.len(),
                                "__truncated__": true
                            }),
                        };
                    }

                    let arguments = match serde_json::from_str::<serde_json::Value>(args_str) {
                        Ok(args) => {
                            tracing::info!("Successfully parsed arguments for tool '{}', keys: {:?}",
                                tc.function.name, args.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                            args
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse arguments for tool '{}': {}. Arguments were: '{}'",
                                tc.function.name, e, args_str);
                            serde_json::json!({
                                "__error__": "PARSE_ERROR",
                                "__message__": format!("JSON parse error: {}", e),
                                "__truncated__": is_complete_json // false if we got here but JSON was invalid
                            })
                        }
                    };
                    ToolCall {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        arguments,
                    }
                })
                .collect();

            // turn_complete=false when there are tool calls - we need to execute them and continue
            return LlmResponse::with_tool_calls(our_tool_calls, false);
        }

        // Regular text response
        let text = response
            .choices
            .first()
            .and_then(|c| c.message.content.as_ref())
            .cloned()
            .unwrap_or_default();

        tracing::debug!(
            "from_openai_response: Extracted text with len={}, text='{}'",
            text.len(),
            text
        );

        let usage = response.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        LlmResponse {
            content: Some(zero_core::types::Content {
                role: "assistant".to_string(),
                parts: vec![Part::Text { text }],
            }),
            turn_complete: true,
            usage,
        }
    }

    /// Check if a JSON string is complete (has balanced braces and strings)
    ///
    /// This is used to detect truncated tool call arguments when the LLM
    /// hits max_tokens limit.
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

            // Early exit if counts go negative (more closing than opening)
            if brace_count < 0 || bracket_count < 0 {
                return false;
            }
        }

        // JSON is complete if:
        // 1. Not in a string
        // 2. All braces are balanced
        // 3. All brackets are balanced
        !in_string && brace_count == 0 && bracket_count == 0
    }
}

#[async_trait]
impl Llm for OpenAiLlm {
    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse> {
        let openai_request = self.to_openai_request(&request);

        // Debug: Log the request being sent
        tracing::debug!("OpenAI LLM Request:");
        tracing::debug!("  model: {}", openai_request.model);
        tracing::debug!(
            "  system_instruction: {}",
            request
                .system_instruction
                .as_ref()
                .map(|s| format!("{} chars", s.len()))
                .unwrap_or_else(|| "None".to_string())
        );
        tracing::debug!("  messages.count: {}", openai_request.messages.len());
        for (i, msg) in openai_request.messages.iter().enumerate() {
            tracing::debug!(
                "  message[{}]: role={}, content.len={:?}, tool_calls={}",
                i,
                msg.role,
                msg.content.as_ref().map(|c| c.len()),
                msg.tool_calls.is_some()
            );
        }
        tracing::debug!(
            "  tools: {:?}",
            openai_request.tools.as_ref().map(|t| t.len())
        );

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url()))
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.config.api_key),
            )
            .header(header::CONTENT_TYPE, "application/json")
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| ZeroError::Llm(format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ZeroError::Llm(format!(
                "API error {}: {}",
                status, error_text
            )));
        }

        let openai_response: OpenAiResponse = response
            .json()
            .await
            .map_err(|e| ZeroError::Llm(format!("Response parse failed: {}", e)))?;

        // Debug: Log the API response
        tracing::debug!("OpenAI LLM Response:");
        tracing::debug!("  choices.count: {}", openai_response.choices.len());
        if let Some(choice) = openai_response.choices.first() {
            tracing::debug!(
                "  choice[0].message.content: {}",
                choice
                    .message
                    .content
                    .as_ref()
                    .map(|c| format!("{} chars", c.len()))
                    .as_deref()
                    .unwrap_or("None")
            );
            // Redacted: tool_calls may contain sensitive arguments
            tracing::debug!(
                "  choice[0].message.tool_calls.count: {:?}",
                choice.message.tool_calls.as_ref().map(|t| t.len())
            );
            tracing::debug!("  choice[0].finish_reason: {:?}", choice.finish_reason);
        }
        tracing::debug!("  usage: {:?}", openai_response.usage);

        Ok(self.from_openai_response(openai_response))
    }

    async fn generate_stream(&self, request: LlmRequest) -> Result<LlmResponseStream> {
        // For now, use non-streaming and convert to a stream
        let response = self.generate(request).await?;

        // Create a single-element stream
        use async_stream::stream;
        let stream = stream! {
            yield Ok(LlmResponseChunk {
                delta: response.content.as_ref().and_then(|c| c.text()).map(|s| s.to_string()),
                tool_call: None,
                turn_complete: true,
                usage: response.usage,
            });
        };

        Ok(Box::pin(stream))
    }
}

// ============================================================================
// OPENAI API TYPES
// ============================================================================

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
    stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAiToolCall {
    id: String,
    r#type: String,
    function: OpenAiFunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OpenAiTool {
    r#type: String,
    function: OpenAiFunction,
}

#[derive(Debug, Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessageContent,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessageContent {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_llm_creation() {
        let config = LlmConfig::new("sk-test", "gpt-4o-mini");
        let llm = OpenAiLlm::new(config);
        assert!(llm.is_ok());
    }

    #[test]
    fn test_request_conversion() {
        let config = LlmConfig::new("sk-test", "gpt-4o-mini");
        let llm = OpenAiLlm::new(config).unwrap();

        let request = LlmRequest::new().with_content(zero_core::types::Content::user("Hello"));

        let openai_req = llm.to_openai_request(&request);
        assert_eq!(openai_req.model, "gpt-4o-mini");
        assert_eq!(openai_req.messages.len(), 1);
    }
}
