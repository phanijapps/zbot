//! Structured-output helpers that keep Rig types inside the runtime boundary.

use std::sync::Arc;

use rig::client::CompletionClient;
use rig::completion::TypedPrompt;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use super::client::LlmCompletionClient;
use crate::llm::{ChatResponse, LlmClient, LlmError, TokenUsage};

/// One Rig Agent completion, with output tools treated only as response data.
/// Unlike an agent prompt loop, this cannot execute tools or silently retry.
pub async fn complete_once(
    llm_client: Arc<dyn LlmClient>,
    system_prompt: String,
    mut user_prompts: Vec<String>,
    output_schema: Option<serde_json::Value>,
    submit_schema: Option<serde_json::Value>,
) -> Result<ChatResponse, LlmError> {
    use rig::completion::{AssistantContent, Completion, Message, ToolDefinition};

    let model_id = llm_client.model().to_string();
    let agent = LlmCompletionClient::new(llm_client)
        .agent(model_id)
        .preamble(&system_prompt)
        .build();
    let prompt = user_prompts
        .pop()
        .ok_or_else(|| LlmError::ApiError("rig_request_configuration".into()))?;
    let history: Vec<Message> = user_prompts.into_iter().map(Message::user).collect();
    let schema = output_schema
        .map(schemars::Schema::try_from)
        .transpose()
        .map_err(|_| LlmError::ApiError("rig_request_configuration".into()))?;
    let mut request = agent
        .completion(prompt, history)
        .await
        .map_err(completion_error)?
        .output_schema_opt(schema);
    if let Some(parameters) = submit_schema {
        request = request.tool(ToolDefinition {
            name: "submit_intent".into(),
            description: "Submit the intent decision as data. This never executes a tool.".into(),
            parameters,
        });
    }
    let response = request.send().await.map_err(completion_error)?;
    let mut content = String::new();
    let mut calls = Vec::new();
    for choice in response.choice {
        match choice {
            AssistantContent::Text(text) => content.push_str(&text.text),
            AssistantContent::ToolCall(call) => calls.push(crate::types::ToolCall {
                id: call.call_id.unwrap_or(call.id),
                name: call.function.name,
                arguments: call.function.arguments,
            }),
            _ => return Err(LlmError::ParseError("invalid_provider_response".into())),
        }
    }
    Ok(ChatResponse {
        content,
        tool_calls: (!calls.is_empty()).then_some(calls),
        reasoning: None,
        usage: Some(TokenUsage {
            prompt_tokens: response.usage.input_tokens.min(u64::from(u32::MAX)) as u32,
            completion_tokens: response.usage.output_tokens.min(u64::from(u32::MAX)) as u32,
            total_tokens: response.usage.total_tokens.min(u64::from(u32::MAX)) as u32,
            cached_prompt_tokens: Some(
                response.usage.cached_input_tokens.min(u64::from(u32::MAX)) as u32
            ),
        }),
    })
}

fn completion_error(error: rig::completion::CompletionError) -> LlmError {
    if let rig::completion::CompletionError::RequestError(error) = error {
        if let Ok(error) = error.downcast::<LlmError>() {
            return *error;
        }
    }
    LlmError::ApiError("rig_request_failed".into())
}

/// Run a Rig typed prompt over the existing AgentZero LLM client.
pub async fn prompt_typed<T>(
    llm_client: Arc<dyn LlmClient>,
    system_prompt: impl Into<String>,
    user_prompt: impl Into<String>,
) -> Result<T, String>
where
    T: JsonSchema + DeserializeOwned + Send + 'static,
{
    let model_id = llm_client.model().to_string();
    let system_prompt = system_prompt.into();
    let client = LlmCompletionClient::new(llm_client);
    let agent = client.agent(model_id).preamble(&system_prompt).build();

    agent
        .prompt_typed::<T>(user_prompt.into())
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    use crate::llm::{ChatMessage, StreamCallback};
    use crate::types::ToolCall;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Provider {
        calls: AtomicUsize,
        fail: bool,
    }
    #[async_trait::async_trait]
    impl LlmClient for Provider {
        fn model(&self) -> &str {
            "test"
        }
        fn provider(&self) -> &str {
            "test"
        }
        async fn chat(
            &self,
            _: Vec<ChatMessage>,
            tools: Option<Value>,
        ) -> Result<ChatResponse, LlmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(LlmError::ApiError(
                    "(422 Unprocessable Entity): unsupported_tools".into(),
                ));
            }
            assert_eq!(tools.unwrap()[0]["function"]["name"], "submit_intent");
            Ok(ChatResponse {
                content: String::new(),
                tool_calls: Some(vec![
                    ToolCall::new("first".into(), "submit_intent".into(), json!({"value":1})),
                    ToolCall::new(
                        "second".into(),
                        "unexpected_tool".into(),
                        json!({"value":2}),
                    ),
                ]),
                reasoning: None,
                usage: Some(TokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 2,
                    total_tokens: 12,
                    cached_prompt_tokens: Some(4),
                }),
            })
        }
        async fn chat_stream(
            &self,
            _: Vec<ChatMessage>,
            _: Option<Value>,
            _: StreamCallback,
        ) -> Result<ChatResponse, LlmError> {
            panic!("single completion must not start an execution stream")
        }
    }

    #[tokio::test]
    async fn rig_completion_returns_all_tool_data_without_execution_or_retry() {
        let provider = Arc::new(Provider {
            calls: AtomicUsize::new(0),
            fail: false,
        });
        let response = complete_once(
            provider.clone(),
            "contract".into(),
            vec!["data".into()],
            None,
            Some(json!({"type":"object"})),
        )
        .await
        .unwrap();
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        let calls = response.tool_calls.unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "first");
        assert_eq!(calls[1].arguments, json!({"value":2}));
        let usage = response.usage.unwrap();
        assert_eq!(usage.total_tokens, 12);
        assert_eq!(usage.cached_prompt_tokens, Some(4));
    }

    #[tokio::test]
    async fn rig_completion_preserves_typed_provider_error_without_retry() {
        let provider = Arc::new(Provider {
            calls: AtomicUsize::new(0),
            fail: true,
        });
        let error = complete_once(
            provider.clone(),
            "contract".into(),
            vec!["data".into()],
            None,
            None,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error, LlmError::ApiError(code) if code == "(422 Unprocessable Entity): unsupported_tools")
        );
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    }
}
