//! # Middleware Traits
//!
//! Core traits for middleware implementation.

use async_trait::async_trait;
use serde_json::Value;
use zero_core::Content;

/// Message type compatible with zero-core
pub type MiddlewareMessage = Content;

/// Event type for middleware
#[derive(Clone, Debug)]
pub struct MiddlewareEvent {
    /// Event type
    pub event_type: String,
    /// Event data
    pub data: Value,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl MiddlewareEvent {
    /// Create a new middleware event
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            data: Value::Object(Default::default()),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Add data to the event
    pub fn with_data(mut self, key: impl Into<String>, value: Value) -> Self {
        if let Value::Object(ref mut map) = self.data {
            map.insert(key.into(), value);
        }
        self
    }
}

/// Context passed to middleware during execution
///
/// This contains only the essential information needed for middleware
/// to make decisions - message counts and model capabilities.
#[derive(Clone, Debug)]
pub struct MiddlewareContext {
    /// Current message count in conversation
    pub message_count: usize,

    /// Estimated token count
    pub estimated_tokens: usize,

    /// Model context window size (in tokens)
    pub context_window: usize,

    /// Additional metadata (for extensibility without breaking changes)
    pub metadata: Value,
}

impl MiddlewareContext {
    /// Create a new middleware context
    pub fn new(message_count: usize, estimated_tokens: usize, context_window: usize) -> Self {
        Self {
            message_count,
            estimated_tokens,
            context_window,
            metadata: Value::Object(Default::default()),
        }
    }

    /// Set metadata on the context
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Calculate the fraction of context window used
    pub fn context_fraction(&self) -> f64 {
        if self.context_window == 0 {
            0.0
        } else {
            self.estimated_tokens as f64 / self.context_window as f64
        }
    }
}

/// Effect returned by preprocess middleware
#[derive(Debug)]
pub enum MiddlewareEffect {
    /// Continue with modified messages
    ModifiedMessages(Vec<MiddlewareMessage>),
    /// Continue without modification
    Proceed,
    /// Emit an event and continue
    EmitEvent(MiddlewareEvent),
    /// Emit event AND modify messages
    EmitAndModify {
        /// Event to emit
        event: MiddlewareEvent,
        /// Modified messages
        messages: Vec<MiddlewareMessage>,
    },
}

/// Trait for middleware that pre-processes messages before LLM execution
///
/// Implement this trait for middleware that:
/// - Summarizes conversation history
/// - Edits context (removes old tool outputs)
/// - Filters/transforms messages
/// - Validates/limits input
#[async_trait]
pub trait PreProcessMiddleware: Send + Sync {
    /// Get the unique name of this middleware
    fn name(&self) -> &'static str;

    /// Clone the middleware (needed for enum wrapper)
    fn clone_box(&self) -> Box<dyn PreProcessMiddleware>;

    /// Process messages before they are sent to the LLM
    ///
    /// # Arguments
    /// * `messages` - Current conversation messages
    /// * `context` - Execution context with metadata
    ///
    /// # Returns
    /// * `MiddlewareEffect` - The effect to apply (modify messages, emit event, etc.)
    async fn process(
        &self,
        messages: Vec<MiddlewareMessage>,
        context: &MiddlewareContext,
    ) -> Result<MiddlewareEffect, String>;

    /// Whether this middleware is enabled
    fn enabled(&self) -> bool {
        true
    }
}

/// Trait for middleware that reacts to events during execution
///
/// Implement this trait for middleware that:
/// - Logs/traces execution
/// - Collects metrics
/// - Implements rate limiting
/// - Detects PII
/// - Builds todo lists
#[async_trait]
pub trait EventMiddleware: Send + Sync {
    /// Get the unique name of this middleware
    fn name(&self) -> &'static str;

    /// Clone the middleware (needed for enum wrapper)
    fn clone_box(&self) -> Box<dyn EventMiddleware>;

    /// Called when any stream event is emitted
    async fn on_event(
        &self,
        event: &MiddlewareEvent,
        context: &MiddlewareContext,
    ) -> Result<(), String>;

    /// Whether this middleware is enabled
    fn enabled(&self) -> bool {
        true
    }
}

/// Helper trait for middleware that needs state
pub trait StatefulMiddleware {
    /// Get the current state as JSON
    fn get_state(&self) -> Result<Value, String>;

    /// Reset state (e.g., clear counters)
    fn reset(&mut self) -> Result<(), String>;
}

// Implement Clone for Box<dyn PreProcessMiddleware>
impl Clone for Box<dyn PreProcessMiddleware> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// Implement Clone for Box<dyn EventMiddleware>
impl Clone for Box<dyn EventMiddleware> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_middleware_event() {
        let event = MiddlewareEvent::new("test_event")
            .with_data("key", Value::String("value".to_string()));

        assert_eq!(event.event_type, "test_event");
    }

    #[test]
    fn test_middleware_context() {
        let ctx = MiddlewareContext::new(10, 1000, 128000);

        assert_eq!(ctx.message_count, 10);
        assert_eq!(ctx.estimated_tokens, 1000);
        assert_eq!(ctx.context_window, 128000);
        assert!((ctx.context_fraction() - (1000.0 / 128000.0)).abs() < 0.001);
    }
}
