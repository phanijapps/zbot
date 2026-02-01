//! # Delegation Context
//!
//! Types for tracking delegation relationships between agents.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// DELEGATION REQUEST
// ============================================================================

/// Request to spawn a delegated subagent.
///
/// This is sent from the parent agent's execution to spawn a child agent
/// that will handle a delegated task.
///
/// The `child_execution_id` is created synchronously when the delegation is
/// requested, ensuring the execution record exists before `try_complete_session()`
/// is called. This prevents a race condition where the session could be marked
/// COMPLETED before the subagent execution exists.
#[derive(Debug, Clone)]
pub struct DelegationRequest {
    /// ID of the parent agent initiating the delegation
    pub parent_agent_id: String,
    /// Session ID (shared across the entire conversation tree)
    pub session_id: String,
    /// Execution ID of the parent (for linking child to parent)
    pub parent_execution_id: String,
    /// ID of the child agent to spawn
    pub child_agent_id: String,
    /// Pre-created execution ID for the child agent.
    ///
    /// This execution is created synchronously when the delegation is requested,
    /// with status QUEUED. The spawn handler will transition it to RUNNING.
    pub child_execution_id: String,
    /// Task description for the child agent
    pub task: String,
    /// Optional context to pass to the child agent
    pub context: Option<Value>,
}

// ============================================================================
// DELEGATION CONTEXT
// ============================================================================

/// Context for delegated task execution.
///
/// When a parent agent delegates to a subagent, this context tracks
/// the relationship and enables callbacks on completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationContext {
    /// ID of the parent agent that initiated delegation.
    pub parent_agent_id: String,

    /// Conversation ID of the parent agent.
    pub parent_conversation_id: String,

    /// Task-scoped context passed from parent.
    #[serde(default)]
    pub task_context: Option<Value>,

    /// Whether to send a callback message on completion.
    #[serde(default = "default_callback")]
    pub callback_on_complete: bool,
}

fn default_callback() -> bool {
    true
}

impl DelegationContext {
    /// Create a new delegation context.
    pub fn new(parent_agent_id: impl Into<String>, parent_conversation_id: impl Into<String>) -> Self {
        Self {
            parent_agent_id: parent_agent_id.into(),
            parent_conversation_id: parent_conversation_id.into(),
            task_context: None,
            callback_on_complete: true,
        }
    }

    /// Set task-scoped context.
    pub fn with_context(mut self, context: Value) -> Self {
        self.task_context = Some(context);
        self
    }

    /// Disable callback on completion.
    pub fn without_callback(mut self) -> Self {
        self.callback_on_complete = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delegation_context() {
        let ctx = DelegationContext::new("parent-agent", "parent-conv")
            .with_context(serde_json::json!({"key": "value"}));

        assert_eq!(ctx.parent_agent_id, "parent-agent");
        assert_eq!(ctx.parent_conversation_id, "parent-conv");
        assert!(ctx.callback_on_complete);
        assert!(ctx.task_context.is_some());
    }
}
