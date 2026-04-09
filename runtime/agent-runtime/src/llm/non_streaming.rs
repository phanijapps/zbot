//! # Non-Streaming LLM Client Wrapper
//!
//! Wraps an LlmClient and converts `chat_stream()` calls into `chat()` calls.
//! Used for subagents where streaming adds no value and causes reliability issues.

use async_trait::async_trait;
use super::{
    LlmClient, LlmError, ChatResponse, StreamCallback, StreamChunk, ToolCallChunk,
    ChatMessage,
};
use serde_json::Value;
use std::sync::Arc;

/// Wraps an LlmClient to disable streaming — `chat_stream()` calls `chat()` internally.
pub struct NonStreamingLlmClient {
    inner: Arc<dyn LlmClient>,
}

impl NonStreamingLlmClient {
    /// Create a new non-streaming wrapper around an existing LLM client.
    pub fn new(inner: Arc<dyn LlmClient>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl LlmClient for NonStreamingLlmClient {
    fn model(&self) -> &str { self.inner.model() }
    fn provider(&self) -> &str { self.inner.provider() }
    fn supports_tools(&self) -> bool { self.inner.supports_tools() }
    fn supports_reasoning(&self) -> bool { self.inner.supports_reasoning() }

    async fn chat(&self, messages: Vec<ChatMessage>, tools: Option<Value>) -> Result<ChatResponse, LlmError> {
        self.inner.chat(messages, tools).await
    }

    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Value>,
        callback: StreamCallback,
    ) -> Result<ChatResponse, LlmError> {
        tracing::debug!("NonStreamingLlmClient: using chat() instead of chat_stream()");

        let response = self.inner.chat(messages, tools).await?;

        // Emit content as a single token chunk
        if !response.content.is_empty() {
            callback(StreamChunk::Token(response.content.clone()));
        }

        // Emit tool call chunks
        if let Some(ref tool_calls) = response.tool_calls {
            for tc in tool_calls {
                callback(StreamChunk::ToolCall(ToolCallChunk {
                    id: Some(tc.id.clone()),
                    name: Some(tc.name.clone()),
                    arguments: tc.arguments.to_string(),
                }));
            }
        }

        Ok(response)
    }
}
