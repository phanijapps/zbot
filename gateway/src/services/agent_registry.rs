//! # Agent Registry
//!
//! Registry for managing agent relationships and delegation permissions.
//!
//! The agent registry tracks:
//! - Which agents exist in the system
//! - Which agents can delegate to which subagents
//! - Agent hierarchy and relationships

use super::AgentService;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Registry for managing agent relationships.
///
/// Tracks delegation permissions between agents,
/// allowing orchestrators to delegate to specialized agents.
pub struct AgentRegistry {
    /// Agent service for accessing agent configurations.
    agent_service: Arc<AgentService>,

    /// Delegation relationships: parent_id -> set of allowed child_ids.
    /// If None, the parent can delegate to any agent.
    relationships: RwLock<HashMap<String, Option<HashSet<String>>>>,
}

impl AgentRegistry {
    /// Create a new agent registry.
    pub fn new(agent_service: Arc<AgentService>) -> Self {
        Self {
            agent_service,
            relationships: RwLock::new(HashMap::new()),
        }
    }

    /// Get the agent service.
    pub fn agent_service(&self) -> &Arc<AgentService> {
        &self.agent_service
    }

    /// Allow an agent to delegate to specific subagents.
    ///
    /// If `subagent_ids` is empty, revokes all delegation permissions.
    pub async fn set_subagents(&self, agent_id: &str, subagent_ids: Vec<String>) {
        let mut relationships = self.relationships.write().await;
        if subagent_ids.is_empty() {
            relationships.remove(agent_id);
        } else {
            relationships.insert(
                agent_id.to_string(),
                Some(subagent_ids.into_iter().collect()),
            );
        }
    }

    /// Allow an agent to delegate to any agent.
    pub async fn allow_all_delegations(&self, agent_id: &str) {
        let mut relationships = self.relationships.write().await;
        relationships.insert(agent_id.to_string(), None);
    }

    /// Revoke all delegation permissions for an agent.
    pub async fn revoke_delegations(&self, agent_id: &str) {
        let mut relationships = self.relationships.write().await;
        relationships.remove(agent_id);
    }

    /// Get the list of agents that the given agent can delegate to.
    ///
    /// Returns `None` if the agent can delegate to any agent,
    /// or an empty Vec if no delegations are allowed.
    pub async fn get_subagents(&self, agent_id: &str) -> Option<Vec<String>> {
        let relationships = self.relationships.read().await;
        match relationships.get(agent_id) {
            Some(Some(set)) => Some(set.iter().cloned().collect()),
            Some(None) => None, // Can delegate to any
            None => Some(vec![]), // Cannot delegate
        }
    }

    /// Check if an agent can delegate to another agent.
    ///
    /// By default, "root" and "orchestrator" type agents can delegate to any agent.
    /// Other agents need explicit permission.
    pub async fn can_delegate(&self, from: &str, to: &str) -> bool {
        // Check if the source agent exists
        let from_agent = match self.agent_service.get(from).await {
            Ok(agent) => agent,
            Err(_) => return false,
        };

        // Check if target agent exists
        if self.agent_service.get(to).await.is_err() {
            return false;
        }

        // Root agents and orchestrators can always delegate
        if from == "root"
            || from_agent
                .agent_type
                .as_ref()
                .map(|t| t == "orchestrator")
                .unwrap_or(false)
        {
            return true;
        }

        // Check explicit relationships
        let relationships = self.relationships.read().await;
        match relationships.get(from) {
            Some(Some(allowed)) => allowed.contains(to),
            Some(None) => true, // Can delegate to any
            None => false,      // No delegation allowed
        }
    }

    /// Get all agents that can be delegated to (for tool schema).
    pub async fn list_delegatable_agents(&self, from: &str) -> Vec<String> {
        let all_agents = match self.agent_service.list().await {
            Ok(agents) => agents,
            Err(_) => return vec![],
        };

        let mut delegatable = Vec::new();
        for agent in all_agents {
            if agent.id != from && self.can_delegate(from, &agent.id).await {
                delegatable.push(agent.id);
            }
        }
        delegatable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_registry() -> AgentRegistry {
        let agent_service = Arc::new(AgentService::new(PathBuf::from("/tmp/test-agents")));
        AgentRegistry::new(agent_service)
    }

    #[tokio::test]
    async fn test_set_subagents() {
        let registry = create_test_registry();

        registry
            .set_subagents("parent", vec!["child1".to_string(), "child2".to_string()])
            .await;

        let subagents = registry.get_subagents("parent").await;
        assert!(subagents.is_some());
        let subagents = subagents.unwrap();
        assert!(subagents.contains(&"child1".to_string()));
        assert!(subagents.contains(&"child2".to_string()));
    }

    #[tokio::test]
    async fn test_allow_all_delegations() {
        let registry = create_test_registry();

        registry.allow_all_delegations("orchestrator").await;

        let subagents = registry.get_subagents("orchestrator").await;
        assert!(subagents.is_none()); // None means can delegate to any
    }

    #[tokio::test]
    async fn test_revoke_delegations() {
        let registry = create_test_registry();

        registry
            .set_subagents("parent", vec!["child".to_string()])
            .await;
        registry.revoke_delegations("parent").await;

        let subagents = registry.get_subagents("parent").await;
        assert!(subagents.is_some());
        assert!(subagents.unwrap().is_empty());
    }
}
