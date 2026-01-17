// ============================================================================
// OPENAI COMPATIBLE CLIENT
// OpenAI-compatible API implementation
// ============================================================================

// TODO: Extract from src-tauri/src/domains/agent_runtime/llm.rs
// This module will contain the OpenAiClient implementation

use std::sync::Arc;

use async_trait::async_trait;

use crate::llm::client::{LlmClient, LlmError, ChatResponse, StreamChunk, StreamCallback};
use crate::llm::config::LlmConfig;
use crate::types::ChatMessage;

/// OpenAI-compatible LLM client
///
/// This client works with any LLM provider that implements
/// the OpenAI API format (including many self-hosted models)
pub struct OpenAiClient {
    config: Arc<LlmConfig>,
    http_client: reqwest::Client,
}

impl OpenAiClient {
    /// Create a new OpenAI-compatible client
    pub fn new(config: LlmConfig) -> Result<Self, LlmError> {
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
}

#[async_trait]
impl LlmClient for OpenAiClient {
    fn model(&self) -> &str {
        &self.config.model
    }

    fn provider(&self) -> &str {
        &self.config.provider_id
    }

    async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<ChatResponse, LlmError> {
        // TODO: Implement from existing code
        Err(LlmError::ParseError("Not yet implemented".to_string()))
    }

    async fn chat_stream(
        &self,
        _messages: Vec<ChatMessage>,
        _callback: StreamCallback,
    ) -> Result<ChatResponse, LlmError> {
        // TODO: Implement from existing code
        Err(LlmError::ParseError("Not yet implemented".to_string()))
    }

    fn supports_reasoning(&self) -> bool {
        self.config.thinking_enabled
    }
}
