//! # Context Editing Middleware
//!
//! Clears old tool results to reduce context size.

use async_trait::async_trait;
use zero_core::Part;

use super::traits::{PreProcessMiddleware, MiddlewareContext, MiddlewareEffect, MiddlewareMessage, MiddlewareEvent};
use super::config::ContextEditingConfig;

/// Context editing middleware - clears old tool outputs
#[derive(Clone)]
pub struct ContextEditingMiddleware {
    config: ContextEditingConfig,
    enabled: bool,
}

impl ContextEditingMiddleware {
    /// Create a new context editing middleware
    pub fn new(config: ContextEditingConfig) -> Self {
        Self {
            enabled: config.enabled,
            config,
        }
    }

    /// Create a new builder
    pub fn builder() -> ContextEditingBuilder {
        ContextEditingBuilder::new()
    }

    /// Check if a message contains a tool result that can be cleared
    fn is_clearable_tool_result(&self, message: &MiddlewareMessage) -> bool {
        for part in &message.parts {
            if let Part::FunctionResponse { .. } = part {
                return true;
            }
        }
        false
    }

    /// Clear a tool result message
    fn clear_tool_result(&self, message: &MiddlewareMessage) -> MiddlewareMessage {
        let mut cleared = message.clone();
        for part in &mut cleared.parts {
            if let Part::FunctionResponse { response, .. } = part {
                *response = self.config.placeholder.clone();
            }
        }
        cleared
    }
}

impl Default for ContextEditingMiddleware {
    fn default() -> Self {
        Self::new(ContextEditingConfig::default())
    }
}

#[async_trait]
impl PreProcessMiddleware for ContextEditingMiddleware {
    fn name(&self) -> &'static str {
        "context_editing"
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
        if context.estimated_tokens < self.config.trigger_tokens {
            return Ok(MiddlewareEffect::ModifiedMessages(messages));
        }

        // Find all tool results (excluding recent ones)
        let mut tool_result_indices = Vec::new();
        for (i, msg) in messages.iter().enumerate() {
            if self.is_clearable_tool_result(msg) {
                tool_result_indices.push(i);
            }
        }

        // Keep the most recent N tool results
        let keep_count = self.config.keep_tool_results;
        let clear_count = tool_result_indices.len().saturating_sub(keep_count);

        if clear_count == 0 {
            return Ok(MiddlewareEffect::ModifiedMessages(messages));
        }

        // Clear old tool results
        let mut result = messages.clone();
        let mut cleared = 0;
        let mut tokens_reclaimed = 0;

        for &idx in &tool_result_indices[..clear_count] {
            if let Some(msg) = result.get(idx) {
                // Estimate tokens before clearing
                let before_tokens = crate::token_counter::estimate_tokens(msg);
                result[idx] = self.clear_tool_result(msg);
                let after_tokens = crate::token_counter::estimate_tokens(&result[idx]);
                tokens_reclaimed += before_tokens.saturating_sub(after_tokens);
                cleared += 1;
            }
        }

        tracing::info!(
            "CONTEXT EDITING: Cleared {} old tool results, reclaimed ~{} tokens (estimated tokens: {}, trigger: {})",
            cleared,
            tokens_reclaimed,
            context.estimated_tokens,
            self.config.trigger_tokens
        );

        // Check if we reclaimed enough tokens
        if tokens_reclaimed < self.config.min_reclaim {
            return Ok(MiddlewareEffect::ModifiedMessages(messages));
        }

        // Emit event
        let event = MiddlewareEvent::new("context_editing")
            .with_data("cleared_count", serde_json::json!(cleared))
            .with_data("tokens_reclaimed", serde_json::json!(tokens_reclaimed));

        Ok(MiddlewareEffect::EmitAndModify {
            event,
            messages: result,
        })
    }
}

/// Builder for ContextEditingMiddleware
pub struct ContextEditingBuilder {
    config: ContextEditingConfig,
}

impl ContextEditingBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: ContextEditingConfig::default(),
        }
    }

    /// Set the configuration
    pub fn config(mut self, config: ContextEditingConfig) -> Self {
        self.config = config;
        self
    }

    /// Enable the middleware
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set the trigger token threshold
    pub fn trigger_tokens(mut self, tokens: usize) -> Self {
        self.config.trigger_tokens = tokens;
        self
    }

    /// Set how many recent tool results to keep
    pub fn keep_tool_results(mut self, count: usize) -> Self {
        self.config.keep_tool_results = count;
        self
    }

    /// Build the middleware
    pub fn build(self) -> ContextEditingMiddleware {
        ContextEditingMiddleware::new(self.config)
    }
}

impl Default for ContextEditingBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_tool_result(id: &str, response: &str) -> MiddlewareMessage {
        Content {
            role: "tool".to_string(),
            parts: vec![Part::FunctionResponse {
                id: id.to_string(),
                response: response.to_string(),
            }],
        }
    }

    #[tokio::test]
    async fn test_context_editing_disabled() {
        let config = ContextEditingConfig {
            enabled: false,
            ..Default::default()
        };
        let middleware = ContextEditingMiddleware::new(config);

        assert!(!middleware.enabled());
    }

    #[tokio::test]
    async fn test_is_clearable_tool_result() {
        let config = ContextEditingConfig::default();
        let middleware = ContextEditingMiddleware::new(config);

        let msg = create_tool_result("call_123", "results here");
        assert!(middleware.is_clearable_tool_result(&msg));
    }

    #[test]
    fn test_builder() {
        let middleware = ContextEditingMiddleware::builder()
            .enabled(true)
            .trigger_tokens(5000)
            .keep_tool_results(2)
            .build();

        assert!(middleware.enabled());
    }
}
