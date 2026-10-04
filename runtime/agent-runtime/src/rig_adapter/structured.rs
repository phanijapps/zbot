//! Structured-output helpers that keep Rig types inside the runtime boundary.

use std::sync::Arc;

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
    use rig::completion::{AssistantContent, CompletionRequest, Message, ToolDefinition};
    if user_prompts.is_empty() {
        return Err(LlmError::ApiError("rig_request_configuration".into()));
    }
    let prompt = user_prompts.pop().unwrap();
    let mut request = CompletionRequest::new(prompt);
    request.chat_history = std::iter::once(Message::system(system_prompt))
        .chain(user_prompts.into_iter().map(Message::user))
        .chain(request.chat_history)
        .collect();
    request.output_schema = output_schema
        .map(schemars::Schema::try_from)
        .transpose()
        .map_err(|_| LlmError::ApiError("rig_request_configuration".into()))?;
    if let Some(parameters) = submit_schema {
        request.tools.push(ToolDefinition {
            name: "submit_intent".into(),
            description: "Submit the intent decision as data. This never executes a tool.".into(),
            parameters,
        });
    }
    let response = super::model::LlmCompletionModel::new(llm_client)
        .erase()
        .call(request)
        .await
        .map_err(completion_error)?;
    let mut content = String::new();
    let mut calls = Vec::new();
    for choice in response.choice {
        match choice {
            AssistantContent::Text(text) => content.push_str(&text.text),
            // Host intent completion keeps provider reasoning separate from decision data.
            AssistantContent::Reasoning(_) => {}
            AssistantContent::ToolCall(call) => calls.push(crate::types::ToolCall {
                id: call.id.wire().into_owned(),
                name: call.function.name.to_string(),
                arguments: call.function.arguments,
            }),
            _ => return Err(LlmError::ParseError("invalid_provider_response".into())),
        }
    }
    Ok(ChatResponse {
        content,
        tool_calls: (!calls.is_empty()).then_some(calls),
        reasoning: None,
        usage: response
            .usage
            .input_tokens
            .zip(response.usage.output_tokens)
            .zip(response.usage.total_tokens)
            .map(|((input, output), total)| TokenUsage {
                prompt_tokens: input.min(u64::from(u32::MAX)) as u32,
                completion_tokens: output.min(u64::from(u32::MAX)) as u32,
                total_tokens: total.min(u64::from(u32::MAX)) as u32,
                cached_prompt_tokens: response
                    .usage
                    .cached_input_tokens
                    .map(|n| n.min(u64::from(u32::MAX)) as u32),
            }),
    })
}

fn completion_error(error: rig::error::ProviderError) -> LlmError {
    if let rig::error::ProviderError::Request(error) = error {
        if error
            .downcast_ref::<rig::completion::message::EmptyToolName>()
            .is_some()
        {
            return LlmError::ParseError("invalid_provider_response".into());
        }
        if let Some(error) = error
            .downcast_ref::<super::model::HostProviderError>()
            .and_then(|error| error.take())
        {
            return error;
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
    T: JsonSchema + DeserializeOwned + Send + Sync + 'static,
{
    let model_id = llm_client.model().to_string();
    let system_prompt = system_prompt.into();
    let client = LlmCompletionClient::new(llm_client);
    let agent = client.agent(model_id).preamble(&system_prompt).build();

    agent
        .prompt_typed::<T>(user_prompt.into())
        .await
        .map(|response| response.output)
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
        metered: bool,
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
                reasoning: Some("provider reasoning".into()),
                usage: self.metered.then_some(TokenUsage {
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
            metered: true,
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
            metered: true,
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
    #[tokio::test]
    async fn single_completion_preserves_unavailable_usage() {
        let provider = Arc::new(Provider {
            calls: AtomicUsize::new(0),
            fail: false,
            metered: false,
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
        assert!(response.usage.is_none());
        assert_eq!(response.tool_calls.unwrap().len(), 2);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    }
}
