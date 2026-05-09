// ============================================================================
// SUMMARIZATION MIDDLEWARE
// Automatically summarize conversation history when approaching token limits
//
// Inspired by LangChain's summarization middleware:
// https://docs.langchain.com/oss/javascript/langchain/middleware/built-in#summarization
// ============================================================================

//! # Summarization Middleware
//!
//! Automatically summarize conversation history when approaching token limits.

use super::config::SummarizationConfig;
use super::token_counter::{estimate_total_tokens, get_model_context_window};
use super::traits::{MiddlewareContext, MiddlewareEffect, PreProcessMiddleware};
use crate::llm::openai::OpenAiClient;
use crate::llm::{LlmClient, LlmConfig};
use crate::types::{ChatMessage, StreamEvent};
use std::sync::Arc;

/// Summarization middleware
///
/// Compresses older conversation messages into a summary when token limits
/// are approached, while keeping recent messages intact.
pub struct SummarizationMiddleware {
    /// Configuration
    config: SummarizationConfig,
    /// LLM client for generating summaries
    summary_client: Arc<dyn LlmClient>,
}

impl SummarizationMiddleware {
    /// Create a new summarization middleware
    ///
    /// # Arguments
    /// * `config` - Middleware configuration
    /// * `summary_client` - LLM client to use for summarization
    pub fn new(config: SummarizationConfig, summary_client: Arc<dyn LlmClient>) -> Self {
        Self {
            config,
            summary_client,
        }
    }

    /// Create from config with LLM client creation
    ///
    /// # Arguments
    /// * `config` - Middleware configuration
    /// * `provider_id` - Provider ID for the summary model (config provider or agent provider)
    /// * `summary_model` - Model from config, if specified (None = use agent model)
    /// * `agent_model` - The agent's model (used as fallback)
    /// * `api_key` - API key for the provider
    /// * `base_url` - Base URL for the provider API
    pub async fn from_config(
        config: SummarizationConfig,
        provider_id: &str,
        summary_model: Option<String>,
        agent_model: &str,
        api_key: String,
        base_url: String,
    ) -> Result<Self, String> {
        // Use configured summary model, or fall back to agent's model
        let model = summary_model.unwrap_or_else(|| agent_model.to_string());

        let llm_config = LlmConfig {
            provider_id: provider_id.to_string(),
            api_key,
            base_url,
            model,
            temperature: 0.3, // Lower temperature for more consistent summaries
            max_tokens: 1000,
            thinking_enabled: false,
        };

        let summary_client = Arc::new(
            OpenAiClient::new(llm_config)
                .map_err(|e| format!("Failed to create LLM client: {e}"))?,
        );

        Ok(Self::new(config, summary_client))
    }

    /// Generate a summary of the given messages
    async fn summarize(&self, messages: &[ChatMessage]) -> Result<String, String> {
        // Build conversation text for summarization
        let conversation_text = self.build_conversation_text(messages);

        // Build summarization prompt
        let prompt = if let Some(custom_prompt) = &self.config.summary_prompt {
            custom_prompt.replace("{messages}", &conversation_text)
        } else {
            format!(
                "Summarize the following conversation concisely. \
                 Preserve key information, decisions, and context:\n\n{conversation_text}"
            )
        };

        // Call LLM to generate summary
        let response = self
            .summary_client
            .chat(
                vec![ChatMessage::user(prompt)],
                None, // No tools needed for summarization
            )
            .await
            .map_err(|e| format!("Summarization failed: {e}"))?;

        Ok(response.content)
    }

    /// Build conversation text from messages
    fn build_conversation_text(&self, messages: &[ChatMessage]) -> String {
        messages
            .iter()
            .map(|msg| format!("{}: {}", msg.role, msg.text_content()))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Split messages into keep and summarize groups.
    ///
    /// IMPORTANT: Never splits between an assistant message with `tool_calls`
    /// and its tool responses. The split boundary is walked forward to find
    /// a clean break point.
    fn split_messages(
        &self,
        messages: &[ChatMessage],
        context_window: usize,
    ) -> (Vec<ChatMessage>, Vec<ChatMessage>) {
        let to_keep = self
            .config
            .keep
            .to_keep_count(messages.len(), context_window);

        // System messages are always kept
        let system_messages: Vec<_> = messages
            .iter()
            .filter(|m| m.role == "system")
            .cloned()
            .collect();

        let non_system_messages: Vec<_> = messages
            .iter()
            .filter(|m| m.role != "system")
            .cloned()
            .collect();

        let keep_count = to_keep.saturating_sub(system_messages.len());

        let (to_keep_messages, to_summarize_messages) = if non_system_messages.len() > keep_count {
            let target_split = non_system_messages.len() - keep_count;

            // Walk the split boundary forward to find a clean break point.
            // A clean break is NOT inside an assistant+tool pair.
            let mut split_idx = target_split;
            for (i, msg) in non_system_messages.iter().enumerate().skip(target_split) {
                // Clean boundary: user message or assistant message (not a tool response)
                if msg.role == "user" || (msg.role == "assistant" && msg.tool_call_id.is_none()) {
                    split_idx = i;
                    break;
                }
                // tool message = inside a pair, keep walking forward
            }

            let summarize = non_system_messages[..split_idx].to_vec();
            let keep = non_system_messages[split_idx..].to_vec();
            (keep, summarize)
        } else {
            (non_system_messages, Vec::new())
        };

        // Prepend system messages to keep group
        let mut final_keep = system_messages;
        final_keep.extend(to_keep_messages);

        (final_keep, to_summarize_messages)
    }
}

#[async_trait::async_trait]
impl PreProcessMiddleware for SummarizationMiddleware {
    fn name(&self) -> &'static str {
        "summarization"
    }

    fn clone_box(&self) -> Box<dyn PreProcessMiddleware> {
        Box::new(Self {
            config: self.config.clone(),
            summary_client: Arc::clone(&self.summary_client),
        })
    }

    fn enabled(&self) -> bool {
        self.config.enabled
    }

    async fn process(
        &self,
        messages: Vec<ChatMessage>,
        context: &MiddlewareContext,
    ) -> Result<MiddlewareEffect, String> {
        // Validate configuration
        if !self.config.trigger.is_valid() {
            return Ok(MiddlewareEffect::Proceed);
        }

        if !self.config.keep.is_valid() {
            return Ok(MiddlewareEffect::Proceed);
        }

        // Estimate current token count
        let current_tokens = estimate_total_tokens(&messages, &context.model);
        let message_count = messages.len();
        let context_window = get_model_context_window(&context.model);

        // Check if we should trigger summarization
        if !self
            .config
            .trigger
            .should_trigger(current_tokens, message_count, context_window)
        {
            return Ok(MiddlewareEffect::Proceed);
        }

        // Split messages into keep and summarize groups
        let (keep_messages, to_summarize) = self.split_messages(&messages, context_window);

        if to_summarize.is_empty() {
            // Nothing to summarize
            return Ok(MiddlewareEffect::Proceed);
        }

        // Generate summary
        let summary = self.summarize(&to_summarize).await?;

        // Create event about summarization
        let event = StreamEvent::Token {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            content: format!(
                "{}\n[Summarized {} messages into {} characters]",
                self.config.summary_prefix,
                to_summarize.len(),
                summary.len()
            ),
        };

        // Build new message list with summary
        let mut new_messages = Vec::new();

        // Add summary as a system-like message at the beginning
        new_messages.push(ChatMessage::system(format!(
            "{}\n\nSummary of previous conversation:\n{}",
            self.config.summary_prefix, summary
        )));

        // Add the messages we wanted to keep
        new_messages.extend(keep_messages);

        Ok(MiddlewareEffect::EmitAndModify {
            event,
            messages: new_messages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::client::{ChatResponse, LlmError, StreamCallback};
    use crate::llm::LlmConfig;
    use crate::middleware::config::{KeepPolicy, TriggerCondition};
    use crate::types::ToolCall;
    use async_trait::async_trait;
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn create_test_messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage::system("You are a helpful assistant.".to_string()),
            ChatMessage::user("Hello!".to_string()),
            ChatMessage::assistant("Hi there!".to_string()),
        ]
    }

    /// Stub LLM that returns canned summaries and counts calls.
    struct StubSummaryClient {
        canned: String,
        calls: Arc<AtomicUsize>,
    }

    impl StubSummaryClient {
        fn new(canned: &str) -> (Arc<Self>, Arc<AtomicUsize>) {
            let calls = Arc::new(AtomicUsize::new(0));
            (
                Arc::new(Self {
                    canned: canned.to_string(),
                    calls: Arc::clone(&calls),
                }),
                calls,
            )
        }
    }

    #[async_trait]
    impl LlmClient for StubSummaryClient {
        fn model(&self) -> &str {
            "stub"
        }
        fn provider(&self) -> &str {
            "stub"
        }
        async fn chat(
            &self,
            _msgs: Vec<ChatMessage>,
            _tools: Option<Value>,
        ) -> Result<ChatResponse, LlmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ChatResponse {
                content: self.canned.clone(),
                tool_calls: None,
                reasoning: None,
                usage: None,
            })
        }
        async fn chat_stream(
            &self,
            _msgs: Vec<ChatMessage>,
            _tools: Option<Value>,
            _cb: StreamCallback,
        ) -> Result<ChatResponse, LlmError> {
            // Not used by summarization
            self.chat(_msgs, _tools).await
        }
    }

    fn make_middleware(
        config: SummarizationConfig,
        client: Arc<dyn LlmClient>,
    ) -> SummarizationMiddleware {
        SummarizationMiddleware::new(config, client)
    }

    fn make_ctx() -> MiddlewareContext {
        MiddlewareContext::new(
            "agent".to_string(),
            None,
            "stub".to_string(),
            "gpt-4o-mini".to_string(),
        )
    }

    fn permissive_config() -> SummarizationConfig {
        SummarizationConfig {
            enabled: true,
            trigger: TriggerCondition {
                tokens: None,
                messages: Some(2),
                fraction: None,
            },
            keep: KeepPolicy {
                messages: Some(1),
                tokens: None,
                fraction: None,
            },
            ..SummarizationConfig::default()
        }
    }

    #[test]
    fn test_split_messages() {
        let config = SummarizationConfig::default();
        let middleware = SummarizationMiddleware {
            config,
            // Would need a mock client for full testing
            summary_client: Arc::new(
                OpenAiClient::new(LlmConfig {
                    provider_id: "test".to_string(),
                    api_key: "test".to_string(),
                    base_url: "https://test.com".to_string(),
                    model: "gpt-4o-mini".to_string(),
                    temperature: 0.3,
                    max_tokens: 1000,
                    thinking_enabled: false,
                })
                .unwrap(),
            ),
        };

        let messages = create_test_messages();
        let (keep, summarize) = middleware.split_messages(&messages, 128_000);

        // All messages should be kept (3 total, default keep is 10)
        assert_eq!(keep.len(), 3); // all messages kept
        assert_eq!(summarize.len(), 0); // nothing to summarize
    }

    #[test]
    fn name_and_clone_box() {
        let (stub, _calls) = StubSummaryClient::new("");
        let mw = make_middleware(SummarizationConfig::default(), stub);
        assert_eq!(mw.name(), "summarization");
        let cloned = mw.clone_box();
        assert_eq!(cloned.name(), "summarization");
    }

    #[test]
    fn enabled_reflects_config() {
        let (stub, _) = StubSummaryClient::new("");
        let cfg = SummarizationConfig {
            enabled: false,
            ..SummarizationConfig::default()
        };
        let mw = make_middleware(cfg, Arc::clone(&stub) as Arc<dyn LlmClient>);
        assert!(!mw.enabled());

        let cfg2 = SummarizationConfig {
            enabled: true,
            ..SummarizationConfig::default()
        };
        let mw2 = make_middleware(cfg2, stub as Arc<dyn LlmClient>);
        assert!(mw2.enabled());
    }

    #[tokio::test]
    async fn process_proceeds_when_trigger_invalid() {
        let (stub, calls) = StubSummaryClient::new("");
        let cfg = SummarizationConfig {
            enabled: true,
            trigger: TriggerCondition::default(), // no fields set => invalid
            ..SummarizationConfig::default()
        };
        let mw = make_middleware(cfg, stub);
        let result = mw
            .process(create_test_messages(), &make_ctx())
            .await
            .unwrap();
        assert!(matches!(result, MiddlewareEffect::Proceed));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn process_proceeds_when_keep_invalid() {
        let (stub, calls) = StubSummaryClient::new("");
        let cfg = SummarizationConfig {
            enabled: true,
            trigger: TriggerCondition {
                messages: Some(1),
                ..TriggerCondition::default()
            },
            keep: KeepPolicy {
                messages: None,
                tokens: None,
                fraction: None,
            },
            ..SummarizationConfig::default()
        };
        let mw = make_middleware(cfg, stub);
        let result = mw
            .process(create_test_messages(), &make_ctx())
            .await
            .unwrap();
        assert!(matches!(result, MiddlewareEffect::Proceed));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn process_proceeds_when_trigger_not_met() {
        let (stub, calls) = StubSummaryClient::new("");
        let cfg = SummarizationConfig {
            enabled: true,
            trigger: TriggerCondition {
                messages: Some(100), // way above
                ..TriggerCondition::default()
            },
            keep: KeepPolicy {
                messages: Some(1),
                tokens: None,
                fraction: None,
            },
            ..SummarizationConfig::default()
        };
        let mw = make_middleware(cfg, stub);
        let result = mw
            .process(create_test_messages(), &make_ctx())
            .await
            .unwrap();
        assert!(matches!(result, MiddlewareEffect::Proceed));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn process_emits_and_modifies_when_triggered() {
        let (stub, calls) = StubSummaryClient::new("brief recap");
        let cfg = permissive_config();
        let mw = make_middleware(cfg, stub);

        // 4 messages: 1 system + 3 non-system. keep=1 means 2 should summarize.
        let messages = vec![
            ChatMessage::system("sys".to_string()),
            ChatMessage::user("u1".to_string()),
            ChatMessage::assistant("a1".to_string()),
            ChatMessage::user("u2".to_string()),
        ];
        let result = mw.process(messages, &make_ctx()).await.unwrap();
        match result {
            MiddlewareEffect::EmitAndModify { event, messages } => {
                assert!(matches!(event, StreamEvent::Token { .. }));
                // Final messages: prepended summary system msg + kept tail
                assert!(!messages.is_empty());
                assert_eq!(messages[0].role, "system");
                assert!(messages[0].text_content().contains("brief recap"));
            }
            other => panic!("expected EmitAndModify, got {other:?}"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn process_proceeds_when_nothing_to_summarize() {
        let (stub, calls) = StubSummaryClient::new("nope");
        // trigger=1 (always), but keep is large enough to keep everything
        let cfg = SummarizationConfig {
            enabled: true,
            trigger: TriggerCondition {
                messages: Some(1),
                ..TriggerCondition::default()
            },
            keep: KeepPolicy {
                messages: Some(100),
                tokens: None,
                fraction: None,
            },
            ..SummarizationConfig::default()
        };
        let mw = make_middleware(cfg, stub);
        let result = mw
            .process(create_test_messages(), &make_ctx())
            .await
            .unwrap();
        assert!(matches!(result, MiddlewareEffect::Proceed));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn split_messages_walks_past_assistant_tool_pair() {
        let (stub, _) = StubSummaryClient::new("");
        let mw = make_middleware(SummarizationConfig::default(), stub);

        // 1 user, 1 assistant w/ tool_calls, 1 tool response, 1 user.
        let messages = vec![
            ChatMessage::user("first".to_string()),
            ChatMessage {
                role: "assistant".to_string(),
                content: vec![],
                tool_calls: Some(vec![ToolCall::new(
                    "c1".to_string(),
                    "x".to_string(),
                    Value::Null,
                )]),
                tool_call_id: None,
                is_summary: false,
            },
            ChatMessage {
                role: "tool".to_string(),
                content: vec![],
                tool_calls: None,
                tool_call_id: Some("c1".to_string()),
                is_summary: false,
            },
            ChatMessage::user("after-pair".to_string()),
        ];

        // Force keep = 1 → split target is index 3 (4 - 1). Already a user, so split=3.
        // Override config keep:
        let mw2 = SummarizationMiddleware::new(
            SummarizationConfig {
                keep: KeepPolicy {
                    messages: Some(1),
                    tokens: None,
                    fraction: None,
                },
                ..SummarizationConfig::default()
            },
            mw.summary_client.clone(),
        );
        let (keep, summarize) = mw2.split_messages(&messages, 128_000);
        assert_eq!(keep.len(), 1);
        assert_eq!(keep[0].text_content(), "after-pair");
        assert_eq!(summarize.len(), 3);
    }

    #[test]
    fn split_messages_walks_forward_through_tool_response() {
        let (stub, _) = StubSummaryClient::new("");
        let mw = SummarizationMiddleware::new(
            SummarizationConfig {
                keep: KeepPolicy {
                    messages: Some(2),
                    tokens: None,
                    fraction: None,
                },
                ..SummarizationConfig::default()
            },
            stub,
        );

        // 4 messages: a, t, t, u. With keep=2 target_split = 2. Index 2 is `tool`,
        // index 3 is `user` — should walk to 3.
        let messages = vec![
            ChatMessage {
                role: "assistant".to_string(),
                content: vec![],
                tool_calls: Some(vec![ToolCall::new(
                    "c".to_string(),
                    "t".to_string(),
                    Value::Null,
                )]),
                tool_call_id: None,
                is_summary: false,
            },
            ChatMessage {
                role: "tool".to_string(),
                content: vec![],
                tool_calls: None,
                tool_call_id: Some("c".to_string()),
                is_summary: false,
            },
            ChatMessage {
                role: "tool".to_string(),
                content: vec![],
                tool_calls: None,
                tool_call_id: Some("c".to_string()),
                is_summary: false,
            },
            ChatMessage::user("end".to_string()),
        ];
        let (keep, summarize) = mw.split_messages(&messages, 128_000);
        // Walk-forward keeps the user message and summarises everything before.
        assert_eq!(keep.len(), 1);
        assert_eq!(keep[0].text_content(), "end");
        assert_eq!(summarize.len(), 3);
    }

    #[tokio::test]
    async fn from_config_constructs_with_summary_model_override() {
        let cfg = SummarizationConfig::default();
        let mw = SummarizationMiddleware::from_config(
            cfg,
            "openai",
            Some("gpt-4o-mini".to_string()),
            "gpt-4o",
            "test-key".to_string(),
            "https://api.openai.test".to_string(),
        )
        .await
        .expect("from_config should succeed with valid params");
        assert_eq!(mw.name(), "summarization");
        // Inner config still says enabled=false (default), so middleware reports disabled.
        assert!(!mw.enabled());
    }

    #[tokio::test]
    async fn from_config_falls_back_to_agent_model_when_summary_model_none() {
        let cfg = SummarizationConfig::default();
        let mw = SummarizationMiddleware::from_config(
            cfg,
            "openai",
            None,
            "gpt-4o",
            "test-key".to_string(),
            "https://api.openai.test".to_string(),
        )
        .await
        .expect("from_config should succeed");
        assert_eq!(mw.summary_client.model(), "gpt-4o");
    }

    #[test]
    fn build_conversation_text_joins_role_and_text() {
        let (stub, _) = StubSummaryClient::new("");
        let mw = make_middleware(SummarizationConfig::default(), stub);
        let text = mw.build_conversation_text(&[
            ChatMessage::user("hi".to_string()),
            ChatMessage::assistant("there".to_string()),
        ]);
        assert!(text.contains("user: hi"));
        assert!(text.contains("assistant: there"));
    }
}
