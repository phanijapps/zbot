//! # Delegation Module
//!
//! Handles agent-to-agent delegation with fire-and-forget pattern
//! and callback completion notifications.

use crate::events::{EventBus, GatewayEvent};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

// ============================================================================
// DELEGATION REQUEST
// ============================================================================

/// Request to spawn a delegated subagent.
///
/// This is sent from the parent agent's execution to spawn a child agent
/// that will handle a delegated task.
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

/// Handle subagent completion and send callback to parent.
///
/// This is called when a delegated subagent completes its task.
/// If callback_on_complete is true, it sends a message to the parent
/// conversation with the result.
pub async fn handle_subagent_completion(
    event_bus: Arc<EventBus>,
    delegation: &DelegationContext,
    child_agent_id: &str,
    child_conversation_id: &str,
    result: Option<String>,
) {
    // Emit delegation completed event
    event_bus
        .publish(GatewayEvent::DelegationCompleted {
            parent_agent_id: delegation.parent_agent_id.clone(),
            parent_conversation_id: delegation.parent_conversation_id.clone(),
            child_agent_id: child_agent_id.to_string(),
            child_conversation_id: child_conversation_id.to_string(),
            result,
        })
        .await;
}

/// Registry for tracking active delegations.
///
/// Maps child conversation IDs to their delegation contexts.
#[derive(Debug, Default)]
pub struct DelegationRegistry {
    /// Active delegations indexed by child conversation ID.
    delegations: std::sync::RwLock<std::collections::HashMap<String, DelegationContext>>,
}

impl DelegationRegistry {
    /// Create a new delegation registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new delegation.
    pub fn register(&self, child_conversation_id: &str, context: DelegationContext) {
        let mut delegations = self.delegations.write().unwrap();
        delegations.insert(child_conversation_id.to_string(), context);
    }

    /// Get delegation context for a child conversation.
    pub fn get(&self, child_conversation_id: &str) -> Option<DelegationContext> {
        let delegations = self.delegations.read().unwrap();
        delegations.get(child_conversation_id).cloned()
    }

    /// Remove a delegation (called on completion).
    pub fn remove(&self, child_conversation_id: &str) -> Option<DelegationContext> {
        let mut delegations = self.delegations.write().unwrap();
        delegations.remove(child_conversation_id)
    }

    /// Get all active delegations for a parent.
    pub fn get_children(&self, parent_conversation_id: &str) -> Vec<String> {
        let delegations = self.delegations.read().unwrap();
        delegations
            .iter()
            .filter(|(_, ctx)| ctx.parent_conversation_id == parent_conversation_id)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Check if a conversation is a delegated subagent.
    pub fn is_delegated(&self, conversation_id: &str) -> bool {
        let delegations = self.delegations.read().unwrap();
        delegations.contains_key(conversation_id)
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

    #[test]
    fn test_delegation_registry() {
        let registry = DelegationRegistry::new();

        let ctx = DelegationContext::new("parent", "parent-conv");
        registry.register("child-conv", ctx);

        assert!(registry.is_delegated("child-conv"));
        assert!(!registry.is_delegated("other-conv"));

        let retrieved = registry.get("child-conv");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().parent_agent_id, "parent");

        let children = registry.get_children("parent-conv");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], "child-conv");

        let removed = registry.remove("child-conv");
        assert!(removed.is_some());
        assert!(!registry.is_delegated("child-conv"));
    }
}
