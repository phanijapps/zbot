//! Rig-backed execution engine behind the [`AgentEngine`] facade.
//!
//! Drives a Rig [`Agent`](rig::agent::Agent) built from a [`RigAgentConfig`],
//! a [`CompletionModel`], and a set of bridged [`ToolDyn`] tools, and maps its
//! multi-turn stream onto AgentZero [`StreamEvent`]s. The existing
//! [`AgentExecutor`](crate::executor::AgentExecutor) stays the live engine;
//! `RigAgentEngine` is the T7 path that will replace it once parity is proven
//! (T11). It is generic over the model so a stub model can drive it in tests
//! without the LLM-client bridge (which lands as a separate T7a step).
//!
//! ## Status (T7)
//!
//! Wired and tested: agent construction, multi-turn stream driving via
//! `stream_chat` (with `ChatMessage`→`Message` history), cooperative stop,
//! streaming-error mapping, and mapping of `MultiTurnStreamItem` onto
//! `StreamEvent` — text→`Token`, reasoning→`Reasoning`, tool-call→
//! `ToolCallStart`, tool-result→`ToolResult`, terminal→`Done`. The
//! `LlmCompletionModel` bridge (`model.rs`) lets Rig drive the real
//! OpenAI-compatible `LlmClient`.
//!
//! Still deferred (see TODOs + T7c):
//! - token-usage accounting through the bridge (`TokenUpdate`);
//! - the raw/context/persisted/UI result distinction on `ToolResult`
//!   (currently the model-visible text only) and tool-role history conversion;
//! - `AgentHook` surfacing of before/after-tool and result-rewrite behavior
//!   (T7c), which also restores per-call `function_call_id` fidelity.
//! `tool_concurrency(1)` keeps the shared context race-free meanwhile.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::StreamExt;
use rig::agent::{Agent, AgentBuilder, MultiTurnStreamItem, StreamingError};
use rig::completion::message::{ToolResult as RigToolResult, ToolResultContent};
use rig::completion::{CompletionModel, Message};
use rig::streaming::{StreamedAssistantContent, StreamedUserContent, StreamingChat};
use rig::tool::{ToolCallExtensions, ToolDyn};

use crate::engine::{AgentEngine, StreamEventSink};
use crate::executor::ExecutorError;
use crate::rig_adapter::{RigAgentConfig, SharedToolContext};
use crate::types::events::current_timestamp;
use crate::types::{ChatMessage, StreamEvent};

const DEFAULT_MAX_TURNS: usize = 50;

/// Rig-backed implementation of the gateway-facing [`AgentEngine`] facade.
///
/// The agent is built once at construction (tools baked in) and reused across
/// `execute_*` calls; per-request hidden context is threaded through Rig's
/// `ToolCallExtensions` each run.
pub struct RigAgentEngine<M: CompletionModel> {
    #[allow(dead_code)]
    config: RigAgentConfig,
    agent: Agent<M>,
    shared_context: SharedToolContext,
    max_turns: usize,
}

impl<M: CompletionModel + Send + Sync + 'static> RigAgentEngine<M> {
    /// Build a Rig agent behind the facade.
    ///
    /// `tools` are the already actor-filtered, bridged [`ToolDyn`] set; this
    /// engine performs no executable filtering of its own (that stays in
    /// gateway-execution's actor gating, per AC7).
    #[must_use]
    pub fn new(
        config: RigAgentConfig,
        model: M,
        tools: Vec<Box<dyn ToolDyn>>,
        shared_context: SharedToolContext,
    ) -> Self {
        Self::with_max_turns(config, model, tools, shared_context, DEFAULT_MAX_TURNS)
    }

    /// Same as [`Self::new`] with an explicit multi-turn cap.
    #[must_use]
    pub fn with_max_turns(
        config: RigAgentConfig,
        model: M,
        tools: Vec<Box<dyn ToolDyn>>,
        shared_context: SharedToolContext,
        max_turns: usize,
    ) -> Self {
        let agent = AgentBuilder::new(model)
            .preamble(&config.instructions)
            .tools(tools)
            .default_max_turns(max_turns)
            .build();
        Self {
            config,
            agent,
            shared_context,
            max_turns,
        }
    }

    /// Drive the Rig agent stream and map it onto [`StreamEvent`]s.
    ///
    /// `stop_flag` enables cooperative cancellation: when set, the loop breaks
    /// after the current item and finalizes with whatever was accumulated.
    async fn run(
        &self,
        user_message: &str,
        history: &[ChatMessage],
        stop_flag: Option<Arc<AtomicBool>>,
        on_event: &mut StreamEventSink<'_>,
    ) -> Result<(), ExecutorError> {
        let prompt = Message::user(user_message.to_string());
        let chat_history = convert_history(history);

        let mut extensions = ToolCallExtensions::new();
        extensions.insert::<SharedToolContext>(self.shared_context.clone());

        // Awaiting the `StreamingPromptRequest` IntoFuture yields the agent
        // stream directly: `Stream<Item = Result<MultiTurnStreamItem, _>>`.
        // `tool_concurrency(1)` keeps tool execution sequential within a turn,
        // matching the legacy executor and keeping the shared `ToolContext`'s
        // per-call state (e.g. function_call_id) race-free until T7c moves it
        // onto a proper per-call carrier.
        let mut stream = self
            .agent
            .stream_chat(prompt, chat_history)
            .tool_extensions(extensions)
            .multi_turn(self.max_turns)
            .tool_concurrency(1)
            .await;

        let mut final_message = String::new();
        while let Some(item) = stream.next().await {
            if let Some(flag) = &stop_flag {
                if flag.load(Ordering::Acquire) {
                    break;
                }
            }
            let item = match item {
                Ok(item) => item,
                Err(error) => return Err(map_streaming_error(error)),
            };
            match item {
                MultiTurnStreamItem::StreamAssistantItem(content) => match content {
                    StreamedAssistantContent::Text(text) => {
                        final_message.push_str(&text.text);
                        on_event(StreamEvent::Token {
                            timestamp: current_timestamp(),
                            content: text.text,
                        });
                    }
                    StreamedAssistantContent::ToolCall { tool_call, .. } => {
                        let tool_id = tool_call
                            .call_id
                            .clone()
                            .unwrap_or_else(|| tool_call.id.clone());
                        on_event(StreamEvent::ToolCallStart {
                            timestamp: current_timestamp(),
                            tool_id,
                            tool_name: tool_call.function.name.clone(),
                            args: tool_call.function.arguments.clone(),
                        });
                    }
                    StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                        on_event(StreamEvent::Reasoning {
                            timestamp: current_timestamp(),
                            content: reasoning,
                        });
                    }
                    other => {
                        // Deltas / complete-reasoning / unknown low-level items
                        // are folded into the aggregated assistant message by
                        // Rig; full surfacing rides on the AgentHook (T7c).
                        let _ = other;
                    }
                },
                MultiTurnStreamItem::StreamUserItem(user_content) => match user_content {
                    StreamedUserContent::ToolResult { tool_result, .. } => {
                        on_event(StreamEvent::ToolResult {
                            timestamp: current_timestamp(),
                            tool_id: tool_result.id.clone(),
                            result: tool_result_text(&tool_result),
                            context_result: None,
                            error: None,
                            duration_ms: None,
                        });
                    }
                },
                MultiTurnStreamItem::CompletionCall(_) => {
                    // TODO(T7): emit StreamEvent::TokenUpdate once usage is
                    // threaded through the LlmCompletionModel bridge.
                }
                MultiTurnStreamItem::FinalResponse(_) => {
                    // Terminal; final_message accumulated from tokens above.
                }
                // `MultiTurnStreamItem` is #[non_exhaustive]; future variants
                // are ignored until the full mapping lands.
                _ => {}
            }
        }

        on_event(StreamEvent::Done {
            timestamp: current_timestamp(),
            final_message,
            token_count: 0,
        });
        Ok(())
    }
}

/// Convert AgentZero chat history into rig `Message`s (text-first).
///
/// Tool-result (`role: "tool"`) and multimodal content are not yet converted;
/// that fidelity rides on the remaining T7 work.
fn convert_history(history: &[ChatMessage]) -> Vec<Message> {
    history
        .iter()
        .filter_map(|message| match message.role.as_str() {
            "user" => Some(Message::user(message.text_content())),
            "assistant" => Some(Message::assistant(message.text_content())),
            "system" => Some(Message::system(message.text_content())),
            _ => None,
        })
        .collect()
}

/// Extract the model-visible text from a rig tool result.
fn tool_result_text(tool_result: &RigToolResult) -> String {
    tool_result
        .content
        .iter()
        .filter_map(|content| match content {
            ToolResultContent::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[async_trait::async_trait]
impl<M: CompletionModel + Send + Sync + 'static> AgentEngine for RigAgentEngine<M> {
    async fn execute_stream(
        &self,
        user_message: &str,
        history: &[ChatMessage],
        on_event: &mut StreamEventSink<'_>,
    ) -> Result<(), ExecutorError> {
        self.run(user_message, history, None, on_event).await
    }

    async fn execute_stream_with_stop_flag(
        &self,
        user_message: &str,
        history: &[ChatMessage],
        stop_flag: Option<Arc<AtomicBool>>,
        on_event: &mut StreamEventSink<'_>,
    ) -> Result<(), ExecutorError> {
        self.run(user_message, history, stop_flag, on_event).await
    }

    async fn execute(
        &self,
        user_message: &str,
        history: &[ChatMessage],
    ) -> Result<String, ExecutorError> {
        let mut accumulated = String::new();
        self.run(user_message, history, None, &mut |event| {
            if let StreamEvent::Token { content, .. } = &event {
                accumulated.push_str(content);
            }
        })
        .await?;
        Ok(accumulated)
    }
}

/// Map a Rig streaming error onto the AgentZero executor error.
///
/// Kept coarse for the first slice: tool/completion/prompt failures all surface
/// as `ExecutorError::LlmError`. T7c may split these once the engine handles
/// tool errors distinctly.
fn map_streaming_error(error: StreamingError) -> ExecutorError {
    ExecutorError::LlmError(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig::completion::{
        AssistantContent, CompletionError, CompletionModel, CompletionRequest, CompletionResponse,
        Usage,
    };
    use rig::one_or_many::OneOrMany;
    use rig::streaming::{RawStreamingChoice, StreamingCompletionResponse};

    /// Stub completion model that streams a fixed sequence of text chunks then
    /// a final-response marker. `type Response = type StreamingResponse = ()`
    /// because `()` already implements [`rig::completion::GetTokenUsage`].
    #[derive(Clone)]
    struct StubModel {
        chunks: Vec<String>,
    }

    impl StubModel {
        fn text(chunks: &[&str]) -> Self {
            Self {
                chunks: chunks.iter().map(|c| (*c).to_string()).collect(),
            }
        }
    }

    impl CompletionModel for StubModel {
        type Response = ();
        type StreamingResponse = ();
        type Client = ();

        fn make(_: &Self::Client, _: impl Into<String>) -> Self {
            Self { chunks: Vec::new() }
        }

        async fn completion(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse<Self::Response>, CompletionError> {
            Ok(CompletionResponse {
                choice: OneOrMany::one(AssistantContent::text(self.chunks.join(""))),
                usage: Usage::new(),
                raw_response: (),
                message_id: None,
            })
        }

        async fn stream(
            &self,
            _request: CompletionRequest,
        ) -> Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError> {
            let mut choices: Vec<Result<RawStreamingChoice<()>, CompletionError>> = self
                .chunks
                .iter()
                .map(|c| Ok(RawStreamingChoice::Message(c.clone())))
                .collect();
            // Terminal marker so the agent loop finalizes cleanly.
            choices.push(Ok(RawStreamingChoice::FinalResponse(())));
            let stream = futures::stream::iter(choices);
            Ok(StreamingCompletionResponse::stream(Box::pin(stream)))
        }
    }

    fn sample_config() -> RigAgentConfig {
        use crate::rig_adapter::RigModelConfig;
        RigAgentConfig::new(
            "agent-1",
            "Agent",
            "test agent",
            "You are a test agent.".to_string(),
            RigModelConfig {
                provider_id: "p".into(),
                base_url: "https://llm.local/v1".into(),
                api_key: "sk-test".into(),
                model: "m".into(),
                temperature: 0.0,
                max_tokens: 100,
                context_window_tokens: 1_000,
                thinking_enabled: false,
                provider_params: None,
            },
        )
    }

    async fn collect_events(engine: &RigAgentEngine<StubModel>, prompt: &str) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        engine
            .execute_stream(prompt, &[], &mut |event| events.push(event))
            .await
            .expect("engine stream should complete");
        events
    }

    #[tokio::test]
    async fn streams_tokens_then_done_for_simple_chat() {
        let engine = RigAgentEngine::new(
            sample_config(),
            StubModel::text(&["hel", "lo", " world"]),
            Vec::new(),
            Arc::new(crate::tools::context::ToolContext::default()),
        );

        let events = collect_events(&engine, "hi").await;
        assert!(!events.is_empty());

        // Token events preserve model order.
        let tokens: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::Token { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(tokens, vec!["hel".to_string(), "lo".to_string(), " world".to_string()]);

        // Terminal Done carries the concatenated text.
        let done = events.iter().rev().find(|e| e.is_terminal());
        assert!(
            matches!(done, Some(StreamEvent::Done { final_message, .. }) if final_message == "hello world"),
            "expected terminal Done with concatenated text, got {done:?}"
        );
    }

    #[tokio::test]
    async fn empty_model_still_finalizes() {
        let engine = RigAgentEngine::new(
            sample_config(),
            StubModel::text(&[]),
            Vec::new(),
            Arc::new(crate::tools::context::ToolContext::default()),
        );

        let events = collect_events(&engine, "hi").await;
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::Done { final_message, .. } if final_message.is_empty())));
    }

    #[tokio::test]
    async fn stop_flag_breaks_after_current_item() {
        let engine = RigAgentEngine::new(
            sample_config(),
            StubModel::text(&["a", "b", "c"]),
            Vec::new(),
            Arc::new(crate::tools::context::ToolContext::default()),
        );

        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_closure = stop.clone();
        let mut events = Vec::new();
        engine
            .execute_stream_with_stop_flag(
                "hi",
                &[],
                Some(stop.clone()),
                &mut |event| {
                    let is_token = matches!(event, StreamEvent::Token { .. });
                    events.push(event);
                    // Stop after the first token lands.
                    if is_token {
                        stop_for_closure.store(true, Ordering::Release);
                    }
                },
            )
            .await
            .expect("stopped run should still finalize");

        let tokens: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::Token { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        // The stop flag is checked before polling the next item, so exactly one
        // token is emitted before the loop breaks and finalizes.
        assert_eq!(tokens, vec!["a".to_string()]);
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::Done { .. })));
    }

    // End-to-end: the real LlmCompletionModel bridge over a stub AgentZero
    // LlmClient, driven through RigAgentEngine. Proves LlmClient -> Rig agent
    // loop -> StreamEvent without any real network call.
    #[tokio::test]
    async fn llm_completion_model_drives_engine_end_to_end() {
        use crate::llm::{ChatResponse, LlmClient, LlmError, StreamCallback, StreamChunk};
        use crate::rig_adapter::model::LlmCompletionModel;
        use serde_json::Value;

        struct StubLlm {
            chunks: Vec<String>,
        }
        #[async_trait::async_trait]
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
                Ok(ChatResponse {
                    content: self.chunks.join(""),
                    tool_calls: None,
                    reasoning: None,
                    usage: None,
                })
            }
            async fn chat_stream(
                &self,
                _messages: Vec<ChatMessage>,
                _tools: Option<Value>,
                callback: StreamCallback,
            ) -> Result<ChatResponse, LlmError> {
                for chunk in &self.chunks {
                    callback(StreamChunk::Token(chunk.clone()));
                }
                Ok(ChatResponse {
                    content: self.chunks.join(""),
                    tool_calls: None,
                    reasoning: None,
                    usage: None,
                })
            }
        }

        let client: Arc<dyn LlmClient> = Arc::new(StubLlm {
            chunks: vec!["ri".to_string(), "gged".to_string()],
        });
        let model = LlmCompletionModel::new(client, "stub");
        let engine = RigAgentEngine::new(
            sample_config(),
            model,
            Vec::new(),
            Arc::new(crate::tools::context::ToolContext::default()),
        );

        let mut events = Vec::new();
        engine
            .execute_stream("hi", &[], &mut |event| events.push(event))
            .await
            .expect("engine should complete");

        let tokens: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::Token { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(tokens, vec!["ri".to_string(), "gged".to_string()]);
        assert!(events.iter().any(|e| matches!(
            e,
            StreamEvent::Done { final_message, .. } if final_message == "rigged"
        )));
    }
}
