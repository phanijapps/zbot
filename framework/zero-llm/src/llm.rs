//! # LLM Abstractions
//!
//! Core LLM traits and types for the Zero framework.

use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;

use zero_core::Result;

// Re-export Content from zero-core for convenience
pub use zero_core::types::Content;

/// Response stream from LLM.
pub type LlmResponseStream = Pin<Box<dyn Stream<Item = Result<LlmResponseChunk>> + Send>>;

/// Core LLM trait.
///
/// All LLM implementations must implement this trait.
#[async_trait]
pub trait Llm: Send + Sync {
    /// Generate a response from the LLM.
    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse>;

    /// Generate a streaming response from the LLM.
    async fn generate_stream(&self, request: LlmRequest) -> Result<LlmResponseStream>;
}

/// Request to an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// The contents (messages) to send to the LLM.
    pub contents: Vec<Content>,

    /// Optional system instruction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<String>,

    /// Optional tool definitions for function calling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,

    /// Temperature for generation (0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

impl LlmRequest {
    /// Create a new LLM request.
    pub fn new() -> Self {
        Self {
            contents: Vec::new(),
            system_instruction: None,
            tools: None,
            temperature: None,
            max_tokens: None,
        }
    }

    /// Add content to the request.
    pub fn with_content(mut self, content: Content) -> Self {
        self.contents.push(content);
        self
    }

    /// Set system instruction.
    pub fn with_system_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.system_instruction = Some(instruction.into());
        self
    }

    /// Set temperature.
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set max tokens.
    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Set tools.
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }
}

impl Default for LlmRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// The content of the response.
    pub content: Option<Content>,

    /// Whether the turn is complete (no more tool calls to make).
    #[serde(default)]
    pub turn_complete: bool,

    /// Token usage information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
}

impl LlmResponse {
    /// Create a new empty response.
    pub fn new() -> Self {
        Self {
            content: None,
            turn_complete: true,
            usage: None,
        }
    }

    /// Create a text response.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: Some(Content::assistant(text.into())),
            turn_complete: true,
            usage: None,
        }
    }

    /// Create a response with tool calls.
    pub fn with_tool_calls(tool_calls: Vec<ToolCall>, turn_complete: bool) -> Self {
        use zero_core::types::Part;

        // Build content with function call parts
        let parts = tool_calls
            .into_iter()
            .map(|tc| Part::FunctionCall {
                name: tc.name,
                args: tc.arguments,
                id: Some(tc.id),
            })
            .collect();

        Self {
            content: Some(Content {
                role: "assistant".to_string(),
                parts,
            }),
            turn_complete,
            usage: None,
        }
    }
}

/// A single chunk in a streaming LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponseChunk {
    /// Content delta (text fragment).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,

    /// Tool call if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<ToolCall>,

    /// Whether this is the final chunk.
    #[serde(default)]
    pub turn_complete: bool,

    /// Token usage (only in final chunk).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
}

/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,

    /// Tool description.
    pub description: String,

    /// JSON Schema for parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
}

/// Tool call made by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub id: String,

    /// Name of the tool to call.
    pub name: String,

    /// Arguments to pass to the tool.
    pub arguments: Value,
}

/// Token usage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Prompt tokens used.
    pub prompt_tokens: u32,

    /// Completion tokens generated.
    pub completion_tokens: u32,

    /// Total tokens used.
    pub total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_builder() {
        let request = LlmRequest::new()
            .with_content(Content::user("Hello"))
            .with_temperature(0.7)
            .with_max_tokens(100);

        assert_eq!(request.contents.len(), 1);
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(100));
    }

    #[test]
    fn test_response_text() {
        let response = LlmResponse::text("Hello world");
        assert!(response.content.is_some());
        assert_eq!(
            response.content.as_ref().unwrap().text(),
            Some("Hello world")
        );
        assert!(response.turn_complete);
    }

    #[test]
    fn test_tool_definition() {
        let tool = ToolDefinition {
            name: "search".to_string(),
            description: "Search the web".to_string(),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            })),
        };

        assert_eq!(tool.name, "search");
        assert!(tool.parameters.is_some());
    }
}
