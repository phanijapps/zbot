//! Structured-output builders over the host's existing LlmClient transport.

use std::sync::Arc;

use rig::AgentBuilder;

use super::model::LlmCompletionModel;
use crate::llm::LlmClient;

/// Structured-output builders backed by an AgentZero LlmClient.
#[derive(Clone)]
pub struct LlmCompletionClient {
    pub(crate) client: Arc<dyn LlmClient>,
}

impl LlmCompletionClient {
    /// Wrap an AgentZero LLM client for use as a Rig completion client.
    #[must_use]
    pub fn new(client: Arc<dyn LlmClient>) -> Self {
        Self { client }
    }
}

impl LlmCompletionClient {
    pub fn agent(&self, _model: impl Into<String>) -> AgentBuilder {
        AgentBuilder::new(LlmCompletionModel::new(self.client.clone()).erase())
    }
    pub fn extractor<T>(&self, _model: impl Into<String>) -> rig::extractor::ExtractorBuilder<T>
    where
        T: schemars::JsonSchema
            + serde::de::DeserializeOwned
            + serde::Serialize
            + Send
            + Sync
            + 'static,
    {
        rig::extractor::ExtractorBuilder::new(LlmCompletionModel::new(self.client.clone()).erase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ChatResponse, LlmClient, LlmError, StreamCallback};
    use crate::types::{ChatMessage, ToolCall};
    use async_trait::async_trait;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    // The extractor works via tool-calling: it registers a `submit` tool whose
    // args are T, and the model calls `submit({...T...})`. This test proves the
    // extractor + LlmCompletionClient + LlmCompletionModel bridge end-to-end.
    #[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
    struct Person {
        name: String,
        age: u32,
    }

    struct StubLlm {
        submitted: Person,
    }

    #[async_trait]
    impl LlmClient for StubLlm {
        fn model(&self) -> &str {
            "stub"
        }
        fn provider(&self) -> &str {
            "stub"
        }
        async fn chat(
            &self,
            _messages: Vec<ChatMessage>,
            _tools: Option<Value>,
        ) -> Result<ChatResponse, LlmError> {
            // Immediately "submit" the structured data, as a capable model would.
            Ok(ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "c1".to_string(),
                    name: "submit".to_string(),
                    arguments: serde_json::to_value(&self.submitted).unwrap(),
                }]),
                reasoning: None,
                usage: None,
            })
        }
        async fn chat_stream(
            &self,
            _messages: Vec<ChatMessage>,
            _tools: Option<Value>,
            _callback: StreamCallback,
        ) -> Result<ChatResponse, LlmError> {
            Err(LlmError::ApiError(
                "chat_stream not used by extractor".to_string(),
            ))
        }
    }

    #[tokio::test]
    async fn extractor_returns_typed_struct_via_submit_tool() {
        let client = LlmCompletionClient::new(Arc::new(StubLlm {
            submitted: Person {
                name: "Ada".to_string(),
                age: 36,
            },
        }));
        let extractor = client.extractor::<Person>("stub").build();
        let person = extractor
            .extract("extract the person")
            .await
            .expect("extraction should yield the typed struct via the submit tool");
        assert_eq!(person.output.name, "Ada");
        assert_eq!(person.output.age, 36);
    }
}
