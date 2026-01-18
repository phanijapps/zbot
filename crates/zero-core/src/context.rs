//! # Context Traits
//!
//! Context types for agent and tool execution.

use crate::event::{Event, EventActions};
use crate::types::Content;
use crate::{Agent, EventStream, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// Streaming mode for agent responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingMode {
    /// Stream responses token by token
    Token,
    /// Stream responses in chunks
    Chunk,
    /// Wait for complete response
    None,
}

/// Configuration for agent runs.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Streaming mode for this run
    pub streaming_mode: StreamingMode,

    /// Maximum iterations for tool calling
    pub max_iterations: Option<usize>,

    /// Additional parameters
    pub params: HashMap<String, Value>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            streaming_mode: StreamingMode::Token,
            max_iterations: Some(50),
            params: HashMap::new(),
        }
    }
}

impl RunConfig {
    /// Create a new run config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the streaming mode.
    pub fn with_streaming_mode(mut self, mode: StreamingMode) -> Self {
        self.streaming_mode = mode;
        self
    }

    /// Set max iterations.
    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = Some(n);
        self
    }

    /// Add a parameter.
    pub fn with_param(mut self, key: impl Into<String>, value: Value) -> Self {
        self.params.insert(key.into(), value);
        self
    }
}

/// Readonly context provides read access to execution context.
pub trait ReadonlyContext: Send + Sync {
    /// Get the invocation ID.
    fn invocation_id(&self) -> &str;

    /// Get the agent name.
    fn agent_name(&self) -> &str;

    /// Get the user ID.
    fn user_id(&self) -> &str;

    /// Get the app name.
    fn app_name(&self) -> &str;

    /// Get the session ID.
    fn session_id(&self) -> &str;

    /// Get the branch.
    fn branch(&self) -> &str;

    /// Get the user's content/message.
    fn user_content(&self) -> &Content;
}

/// Callback context provides access to artifacts and other callback-specific data.
pub trait CallbackContext: ReadonlyContext {
    /// Get state value.
    fn get_state(&self, key: &str) -> Option<Value>;

    /// Set state value.
    fn set_state(&self, key: String, value: Value);
}

/// Tool context provides additional context for tool execution.
pub trait ToolContext: CallbackContext {
    /// Get the function call ID for this tool execution.
    fn function_call_id(&self) -> &str;

    /// Get the current event actions.
    fn actions(&self) -> EventActions;

    /// Set event actions (e.g., to trigger escalation).
    fn set_actions(&self, actions: EventActions);
}

/// Invocation context provides full context during agent invocation.
///
/// Note: This trait is NOT dyn-compatible due to the async `run` method
/// in the Agent trait. Use concrete types instead of `dyn InvocationContext`.
pub trait InvocationContext: CallbackContext {
    /// Get the agent being run.
    fn agent(&self) -> Arc<dyn Agent>;

    /// Get the session.
    /// Returns an Arc to allow shared ownership across async boundaries.
    fn session(&self) -> Arc<dyn Session>;

    /// Get the run config.
    fn run_config(&self) -> &RunConfig;

    /// Get the current event actions.
    fn actions(&self) -> EventActions;

    /// Set event actions (e.g., to trigger escalation).
    fn set_actions(&self, actions: EventActions);

    /// End the invocation.
    fn end_invocation(&self);

    /// Check if the invocation has ended.
    fn ended(&self) -> bool;

    /// Add content to the session (for tool responses, etc.).
    fn add_content(&self, content: Content);
}

/// Session trait for conversation session management.
pub trait Session: Send + Sync {
    /// Get the session ID.
    fn id(&self) -> &str;

    /// Get the app name.
    fn app_name(&self) -> &str;

    /// Get the user ID.
    fn user_id(&self) -> &str;

    /// Get the session state.
    fn state(&self) -> &dyn State;

    /// Get conversation history.
    fn conversation_history(&self) -> Vec<Content>;
}

/// State trait for key-value storage.
pub trait State: Send + Sync {
    /// Get a value by key.
    fn get(&self, key: &str) -> Option<Value>;

    /// Set a value.
    fn set(&mut self, key: String, value: Value);

    /// Get all values.
    fn all(&self) -> HashMap<String, Value>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_config_default() {
        let config = RunConfig::default();
        assert_eq!(config.streaming_mode, StreamingMode::Token);
        assert_eq!(config.max_iterations, Some(50));
    }

    #[test]
    fn test_run_config_builder() {
        let config = RunConfig::new()
            .with_streaming_mode(StreamingMode::None)
            .with_max_iterations(100)
            .with_param("key", serde_json::json!("value"));

        assert_eq!(config.streaming_mode, StreamingMode::None);
        assert_eq!(config.max_iterations, Some(100));
        assert_eq!(config.params.get("key"), Some(&json!("value")));
    }
}
