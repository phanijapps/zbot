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

use std::sync::Arc;
use crate::types::{ChatMessage, StreamEvent};
use crate::llm::{LlmClient, LlmConfig};
use crate::llm::openai::OpenAiClient;
use super::traits::{PreProcessMiddleware, MiddlewareContext, MiddlewareEffect};
use super::config::SummarizationConfig;
use super::token_counter::{estimate_total_tokens, get_model_context_window};

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

        let summary_client = Arc::new(OpenAiClient::new(llm_config)
            .map_err(|e| format!("Failed to create LLM client: {}", e))?);

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
                 Preserve key information, decisions, and context:\n\n{}",
                conversation_text
            )
        };

        // Call LLM to generate summary
        let response = self
            .summary_client
            .chat(
                vec![ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                    tool_calls: None,
                    tool_call_id: None,
                }],
                None, // No tools needed for summarization
            )
            .await
            .map_err(|e| format!("Summarization failed: {}", e))?;

        Ok(response.content)
    }

    /// Build conversation text from messages
    fn build_conversation_text(&self, messages: &[ChatMessage]) -> String {
        messages
            .iter()
            .map(|msg| {
                format!("{}: {}", msg.role, msg.content)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Split messages into keep and summarize groups.
    ///
    /// IMPORTANT: Never splits between an assistant message with tool_calls
    /// and its tool responses. The split boundary is walked forward to find
    /// a clean break point.
    fn split_messages(
        &self,
        messages: &[ChatMessage],
        context_window: usize,
    ) -> (Vec<ChatMessage>, Vec<ChatMessage>) {
        let to_keep = self.config.keep.to_keep_count(messages.len(), context_window);

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
            for i in target_split..non_system_messages.len() {
                let msg = &non_system_messages[i];
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
        let current_tokens = estimate_total_tokens(&messages);
        let message_count = messages.len();
        let context_window = get_model_context_window(&context.model);

        // Check if we should trigger summarization
        if !self.config.trigger.should_trigger(current_tokens, message_count, context_window) {
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
        new_messages.push(ChatMessage {
            role: "system".to_string(),
            content: format!(
                "{}\n\nSummary of previous conversation:\n{}",
                self.config.summary_prefix, summary
            ),
            tool_calls: None,
            tool_call_id: None,
        });

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
    use crate::llm::LlmConfig;
    use std::sync::Arc;

    fn create_test_messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a helpful assistant.".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Hello!".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "Hi there!".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
        ]
    }

    #[test]
    fn test_split_messages() {
        let config = SummarizationConfig::default();
        let middleware = SummarizationMiddleware {
            config,
            // Would need a mock client for full testing
            summary_client: Arc::new(OpenAiClient::new(LlmConfig {
                provider_id: "test".to_string(),
                api_key: "test".to_string(),
                base_url: "https://test.com".to_string(),
                model: "gpt-4o-mini".to_string(),
                temperature: 0.3,
                max_tokens: 1000,
                thinking_enabled: false,
            }).unwrap()),
        };

        let messages = create_test_messages();
        let (keep, summarize) = middleware.split_messages(&messages, 128000);

        // All messages should be kept (3 total, default keep is 10)
        assert_eq!(keep.len(), 3); // all messages kept
        assert_eq!(summarize.len(), 0); // nothing to summarize
    }
}
