//! # Delegation Registry
//!
//! Registry for tracking active delegations between parent and child agents.

use super::context::DelegationContext;
use std::collections::HashMap;
use std::sync::RwLock;

/// Registry for tracking active delegations.
///
/// Maps child conversation IDs to their delegation contexts.
/// Thread-safe for concurrent access from multiple execution tasks.
#[derive(Debug, Default)]
pub struct DelegationRegistry {
    /// Active delegations indexed by child conversation ID.
    delegations: RwLock<HashMap<String, DelegationContext>>,
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

    /// Get all active delegations for a session.
    pub fn get_by_session_id(&self, session_id: &str) -> Vec<(String, DelegationContext)> {
        let delegations = self.delegations.read().unwrap();
        delegations
            .iter()
            .filter(|(_, ctx)| ctx.session_id == session_id)
            .map(|(conv_id, ctx)| (conv_id.clone(), ctx.clone()))
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
    fn test_delegation_registry() {
        let registry = DelegationRegistry::new();

        let ctx = DelegationContext::new("session-123", "exec-456", "parent", "parent-conv");
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
