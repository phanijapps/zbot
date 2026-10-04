//! Raw-attempt model observations; install inside retry and below protected context assembly.
use super::*;
use crate::{ChatMessage, ChatResponse, LlmClient, LlmError, StreamCallback, StreamChunk};
use std::sync::Arc;

pub struct HookedLlmClient {
    inner: Arc<dyn LlmClient>,
    run: Arc<HookRun>,
}
impl HookedLlmClient {
    pub fn new(inner: Arc<dyn LlmClient>, run: Arc<HookRun>) -> Self {
        Self { inner, run }
    }
    async fn begin(&self, messages: &mut Vec<ChatMessage>) -> Result<String, LlmError> {
        let attempt = self
            .run
            .begin_model(self.inner.provider(), self.inner.model())
            .await?;
        self.run.append_context(messages);
        Ok(attempt)
    }
    async fn end(&self, attempt: String, result: &Result<ChatResponse, LlmError>) {
        self.run
            .end_model(
                &attempt,
                if result.is_ok() {
                    HookOperationStatus::Completed
                } else {
                    HookOperationStatus::Failed
                },
            )
            .await;
    }
}
#[async_trait::async_trait]
impl LlmClient for HookedLlmClient {
    fn model(&self) -> &str {
        self.inner.model()
    }
    fn provider(&self) -> &str {
        self.inner.provider()
    }
    async fn chat(
        &self,
        mut messages: Vec<ChatMessage>,
        tools: Option<serde_json::Value>,
    ) -> Result<ChatResponse, LlmError> {
        let attempt = self.begin(&mut messages).await?;
        let result = self.inner.chat(messages, tools).await;
        self.end(attempt, &result).await;
        result
    }
    async fn chat_with_schema(
        &self,
        mut messages: Vec<ChatMessage>,
        tools: Option<serde_json::Value>,
        schema: Option<serde_json::Value>,
    ) -> Result<ChatResponse, LlmError> {
        let attempt = self.begin(&mut messages).await?;
        let result = self.inner.chat_with_schema(messages, tools, schema).await;
        self.end(attempt, &result).await;
        result
    }
    async fn chat_stream(
        &self,
        mut messages: Vec<ChatMessage>,
        tools: Option<serde_json::Value>,
        callback: StreamCallback,
    ) -> Result<ChatResponse, LlmError> {
        let mut fallback_messages = messages.clone();
        let fallback_tools = tools.clone();
        let callback: Arc<dyn Fn(StreamChunk) + Send + Sync> = callback.into();
        let streamed_callback = callback.clone();
        let attempt = self.begin(&mut messages).await?;
        let result = self
            .inner
            .chat_stream_attempt(
                messages,
                tools,
                Box::new(move |chunk| streamed_callback(chunk)),
            )
            .await;
        self.end(attempt, &result).await;
        if !matches!(result, Err(LlmError::StreamFallbackRequired)) {
            return result;
        }
        // Start from the protected input again; append the current hook context
        // once, including any new data admitted for this distinct attempt.
        let attempt = self.begin(&mut fallback_messages).await?;
        let result = self
            .inner
            .chat_stream_fallback(fallback_messages, fallback_tools)
            .await;
        self.end(attempt, &result).await;
        if let Ok(response) = &result {
            if !response.content.is_empty() {
                callback(StreamChunk::Token(response.content.clone()));
            }
        }
        result
    }
    fn supports_tools(&self) -> bool {
        self.inner.supports_tools()
    }
    fn supports_reasoning(&self) -> bool {
        self.inner.supports_reasoning()
    }
}
