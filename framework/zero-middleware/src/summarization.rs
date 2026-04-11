//! # Summarization Middleware
//!
//! Compresses long conversations by summarizing old messages.

use async_trait::async_trait;
use zero_core::Content;

use super::config::SummarizationConfig;
use super::traits::{
    MiddlewareContext, MiddlewareEffect, MiddlewareEvent, MiddlewareMessage, PreProcessMiddleware,
};

/// Summarization middleware - compresses conversation history
#[derive(Clone)]
pub struct SummarizationMiddleware {
    config: SummarizationConfig,
    enabled: bool,
}

impl SummarizationMiddleware {
    /// Create a new summarization middleware
    pub fn new(config: SummarizationConfig) -> Self {
        Self {
            enabled: config.enabled,
            config,
        }
    }

    /// Create a new builder
    pub fn builder() -> SummarizationBuilder {
        SummarizationBuilder::new()
    }
}

impl Default for SummarizationMiddleware {
    fn default() -> Self {
        Self::new(SummarizationConfig::default())
    }
}

#[async_trait]
impl PreProcessMiddleware for SummarizationMiddleware {
    fn name(&self) -> &'static str {
        "summarization"
    }

    fn clone_box(&self) -> Box<dyn PreProcessMiddleware> {
        Box::new(self.clone())
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    async fn process(
        &self,
        messages: Vec<MiddlewareMessage>,
        context: &MiddlewareContext,
    ) -> Result<MiddlewareEffect, String> {
        // Check if trigger condition is met
        if !self.config.trigger.should_trigger(
            context.estimated_tokens,
            context.message_count,
            context.context_window,
        ) {
            return Ok(MiddlewareEffect::ModifiedMessages(messages));
        }

        // Calculate how many messages to keep
        let to_keep = self
            .config
            .keep
            .to_keep_count(messages.len(), context.context_window);
        let to_summarize = messages.len().saturating_sub(to_keep);

        if to_summarize == 0 {
            return Ok(MiddlewareEffect::ModifiedMessages(messages));
        }

        // Split messages
        let keep_start = messages.len() - to_keep;
        let _to_summarize_msgs = &messages[..to_summarize];
        let keep_msgs = &messages[keep_start..];

        tracing::info!(
            "SUMMARIZATION: Compressing conversation - original: {} messages, summarizing: {}, keeping: {} (estimated tokens: {}, context window: {})",
            messages.len(),
            to_summarize,
            to_keep,
            context.estimated_tokens,
            context.context_window
        );

        // Create a summary placeholder
        // In a real implementation, this would call an LLM to summarize
        let summary_text = format!(
            "{} [{} previous messages were summarized]",
            self.config.summary_prefix, to_summarize
        );

        tracing::info!("SUMMARIZATION: Summary content: {}", summary_text);

        let summary_content = Content {
            role: "system".to_string(),
            parts: vec![zero_core::Part::Text { text: summary_text }],
        };

        // Build result: summary + kept messages
        let mut result = Vec::with_capacity(1 + keep_msgs.len());
        result.push(summary_content);
        result.extend(keep_msgs.iter().cloned());

        // Emit event
        let event = MiddlewareEvent::new("summarization")
            .with_data("original_count", serde_json::json!(messages.len()))
            .with_data("summarized_count", serde_json::json!(to_summarize))
            .with_data("kept_count", serde_json::json!(to_keep));

        Ok(MiddlewareEffect::EmitAndModify {
            event,
            messages: result,
        })
    }
}

/// Builder for SummarizationMiddleware
pub struct SummarizationBuilder {
    config: SummarizationConfig,
}

impl SummarizationBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: SummarizationConfig::default(),
        }
    }

    /// Set the configuration
    pub fn config(mut self, config: SummarizationConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the middleware
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Build the middleware
    pub fn build(self) -> SummarizationMiddleware {
        SummarizationMiddleware::new(self.config)
    }
}

impl Default for SummarizationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::Part;

    fn create_test_message(text: &str) -> MiddlewareMessage {
        Content {
            role: "user".to_string(),
            parts: vec![Part::Text {
                text: text.to_string(),
            }],
        }
    }

    #[tokio::test]
    async fn test_summarization_disabled() {
        let config = SummarizationConfig {
            enabled: false,
            ..Default::default()
        };
        let middleware = SummarizationMiddleware::new(config);

        assert!(!middleware.enabled());
    }

    #[tokio::test]
    async fn test_summarization_enabled() {
        let config = SummarizationConfig {
            enabled: true,
            trigger: super::super::config::TriggerCondition {
                messages: Some(5),
                ..Default::default()
            },
            keep: super::super::config::KeepPolicy {
                messages: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let middleware = SummarizationMiddleware::new(config);

        let messages = vec![
            create_test_message("msg1"),
            create_test_message("msg2"),
            create_test_message("msg3"),
            create_test_message("msg4"),
            create_test_message("msg5"),
        ];

        let context = MiddlewareContext::new(5, 100, 128000);

        let result = middleware.process(messages, &context).await.unwrap();

        match result {
            MiddlewareEffect::EmitAndModify { messages, .. } => {
                // Should have summary + 2 kept messages
                assert_eq!(messages.len(), 3);
            }
            _ => panic!("Expected EmitAndModify"),
        }
    }

    #[test]
    fn test_builder() {
        let middleware = SummarizationMiddleware::builder().enabled(true).build();

        assert!(middleware.enabled());
    }
}
