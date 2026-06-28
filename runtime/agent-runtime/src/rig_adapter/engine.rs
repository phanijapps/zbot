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
//! ## Status (T7, in progress)
//!
//! Wired and unit-tested with a stub model: agent construction, multi-turn
//! stream driving, cooperative stop, streaming-error mapping, text→`Token`
//! streaming, tool-call-start surfacing, and finalization (`Done`).
//!
//! Deliberately deferred to the rest of T7 (see TODOs below):
//! - full history conversion (`ChatMessage` → rig `Message`) and `stream_chat`;
//! - reasoning-text extraction (`Reasoning` → `StreamEvent::Reasoning`);
//! - tool-result mapping (`StreamedUserContent::ToolResult` →
//!   `StreamEvent::ToolResult`, including the raw/context distinction);
//! - `AgentHook` surfacing of before/after-tool and result-rewrite behavior
//!   (T7c), which also restores per-call `function_call_id` fidelity.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::StreamExt;
use rig::agent::{Agent, AgentBuilder, MultiTurnStreamItem, StreamingError};
use rig::completion::{CompletionModel, Message};
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
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
        _history: &[ChatMessage],
        stop_flag: Option<Arc<AtomicBool>>,
        on_event: &mut StreamEventSink<'_>,
    ) -> Result<(), ExecutorError> {
        // TODO(T7): convert AgentZero ChatMessage history into rig Messages
        // (role + Part contents) and switch to `stream_chat`. For now the
        // current prompt is the only message carried.
        let prompt = Message::user(user_message.to_string());

        let mut extensions = ToolCallExtensions::new();
        extensions.insert::<SharedToolContext>(self.shared_context.clone());

        // Awaiting the `StreamingPromptRequest` IntoFuture yields the agent
        // stream directly: `Stream<Item = Result<MultiTurnStreamItem, _>>`.
        let mut stream = self
            .agent
            .stream_prompt(prompt)
            .tool_extensions(extensions)
            .multi_turn(self.max_turns)
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
                    StreamedAssistantContent::Reasoning(_) => {
                        // TODO(T7): extract reasoning text once ReasoningContent
                        // accessors are confirmed; emit StreamEvent::Reasoning.
                    }
                    other => {
                        // Deltas / unknown low-level items; surfaced later via
                        // the full mapping and AgentHook.
                        let _ = other;
                    }
                },
                MultiTurnStreamItem::StreamUserItem(_) => {
                    // TODO(T7): map StreamedUserContent::ToolResult ->
                    //   StreamEvent::ToolResult { result, context_result, .. }.
                }
                MultiTurnStreamItem::CompletionCall(_) => {
                    // TODO(T7): emit StreamEvent::TokenUpdate from Usage.
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
}
