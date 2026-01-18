// ============================================================================
// MIDDLEWARE TRAITS
// Core traits for middleware implementation
// ============================================================================

//! # Middleware Traits
//!
//! Core traits for middleware implementation.

use async_trait::async_trait;
use serde_json::Value;
use crate::types::{ChatMessage, StreamEvent};

/// Context passed to middleware during execution
#[derive(Clone, Debug)]
pub struct MiddlewareContext {
    /// Agent ID
    pub agent_id: String,
    /// Conversation ID (if available)
    pub conversation_id: Option<String>,
    /// Provider ID
    pub provider_id: String,
    /// Model name
    pub model: String,
    /// Current message count in conversation
    pub message_count: usize,
    /// Estimated token count
    pub estimated_tokens: usize,
    /// Additional metadata
    pub metadata: Value,
}

impl MiddlewareContext {
    pub fn new(
        agent_id: String,
        conversation_id: Option<String>,
        provider_id: String,
        model: String,
    ) -> Self {
        Self {
            agent_id,
            conversation_id,
            provider_id,
            model,
            message_count: 0,
            estimated_tokens: 0,
            metadata: Value::Object(Default::default()),
        }
    }

    pub fn with_counts(mut self, message_count: usize, estimated_tokens: usize) -> Self {
        self.message_count = message_count;
        self.estimated_tokens = estimated_tokens;
        self
    }

    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Effect returned by preprocess middleware
#[derive(Debug)]
pub enum MiddlewareEffect {
    /// Continue with modified messages
    ModifiedMessages(Vec<ChatMessage>),
    /// Continue without modification
    Proceed,
    /// Emit an event and continue
    EmitEvent(StreamEvent),
    /// Emit event AND modify messages
    EmitAndModify { event: StreamEvent, messages: Vec<ChatMessage> },
}

/// Trait for middleware that pre-processes messages before LLM execution
///
/// Implement this trait for middleware that:
/// - Summarizes conversation history
/// - Edits context (removes old tool outputs)
/// - Filters/transforms messages
/// - Validates/limits input
///
/// Note: This trait uses a different approach to avoid dyn-compatibility issues.
/// Middleware returns events in the effect rather than calling callbacks.
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
        messages: Vec<ChatMessage>,
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
///
/// Note: This trait is NOT dyn-compatible due to async trait bounds.
/// Middleware must be stored as concrete types, not trait objects.
#[async_trait]
pub trait EventMiddleware: Send + Sync {
    /// Get the unique name of this middleware
    fn name(&self) -> &'static str;

    /// Clone the middleware (needed for enum wrapper)
    fn clone_box(&self) -> Box<dyn EventMiddleware>;

    /// Called when any stream event is emitted
    async fn on_event(
        &self,
        event: &StreamEvent,
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
