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
    /// Optional max iterations for the child agent execution loop.
    /// Defaults to 25 if not specified.
    pub max_iterations: Option<u32>,
    /// Optional JSON Schema the child agent's response must conform to.
    ///
    /// When provided, the child's system prompt is augmented with an output
    /// contract requiring a JSON response matching this schema.
    pub output_schema: Option<Value>,

    /// Skills to pre-load for the subagent.
    pub skills: Vec<String>,
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
    /// Session ID (shared across the entire conversation tree).
    pub session_id: String,

    /// Execution ID of the parent agent.
    pub parent_execution_id: String,

    /// ID of the parent agent that initiated delegation.
    pub parent_agent_id: String,

    /// Conversation ID of the parent agent (legacy, for backward compatibility).
    pub parent_conversation_id: String,

    /// Task-scoped context passed from parent.
    #[serde(default)]
    pub task_context: Option<Value>,

    /// Whether to send a callback message on completion.
    #[serde(default = "default_callback")]
    pub callback_on_complete: bool,

    /// Conversation ID of the child agent (for routing events back).
    #[serde(default)]
    pub child_conversation_id: Option<String>,

    /// Optional JSON Schema the child's response should conform to.
    ///
    /// Stored here so the callback handler can validate the child's response
    /// at completion time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
}

fn default_callback() -> bool {
    true
}

impl DelegationContext {
    /// Create a new delegation context.
    pub fn new(
        session_id: impl Into<String>,
        parent_execution_id: impl Into<String>,
        parent_agent_id: impl Into<String>,
        parent_conversation_id: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            parent_execution_id: parent_execution_id.into(),
            parent_agent_id: parent_agent_id.into(),
            parent_conversation_id: parent_conversation_id.into(),
            task_context: None,
            callback_on_complete: true,
            child_conversation_id: None,
            output_schema: None,
        }
    }

    /// Set task-scoped context.
    pub fn with_context(mut self, context: Value) -> Self {
        self.task_context = Some(context);
        self
    }

    /// Set the child conversation ID.
    pub fn with_child_conversation_id(mut self, id: String) -> Self {
        self.child_conversation_id = Some(id);
        self
    }

    /// Disable callback on completion.
    pub fn without_callback(mut self) -> Self {
        self.callback_on_complete = false;
        self
    }

    /// Set the output schema for response validation.
    pub fn with_output_schema(mut self, schema: Value) -> Self {
        self.output_schema = Some(schema);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delegation_context() {
        let ctx = DelegationContext::new("sess-123", "exec-456", "parent-agent", "parent-conv")
            .with_context(serde_json::json!({"key": "value"}));

        assert_eq!(ctx.session_id, "sess-123");
        assert_eq!(ctx.parent_execution_id, "exec-456");
        assert_eq!(ctx.parent_agent_id, "parent-agent");
        assert_eq!(ctx.parent_conversation_id, "parent-conv");
        assert!(ctx.callback_on_complete);
        assert!(ctx.task_context.is_some());
    }
}
