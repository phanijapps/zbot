//! # Capability Registry
//!
//! Registry for discovering agents by their capabilities.
//!
//! ## Overview
//!
//! The CapabilityRegistry provides:
//! - Agent registration with capabilities
//! - Discovery by capability query
//! - Best-match routing for tasks
//!
//! ## Example
//!
//! ```rust
//! use zero_core::registry::CapabilityRegistry;
//! use zero_core::capability::{AgentCapabilities, Capability, CapabilityQuery, common};
//!
//! let mut registry = CapabilityRegistry::new();
//!
//! // Register an agent
//! let caps = AgentCapabilities::builder("code-agent")
//!     .add_capability(common::code_review())
//!     .add_capability(common::code_generation())
//!     .build();
//! registry.register(caps);
//!
//! // Find agents for a task
//! let query = CapabilityQuery::new()
//!     .with_capability_ids(vec!["code_review"]);
//! let matches = registry.find_agents(&query);
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::capability::{AgentCapabilities, CapabilityQuery};
use crate::policy::CapabilityCategory;

// ============================================================================
// CAPABILITY REGISTRY
// ============================================================================

/// Registry for agent capabilities.
///
/// Thread-safe registry that indexes agents by their capabilities
/// for efficient discovery and routing.
#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    /// Agent capabilities indexed by agent ID
    agents: RwLock<HashMap<String, AgentCapabilities>>,

    /// Index: capability ID -> list of agent IDs
    capability_index: RwLock<HashMap<String, Vec<String>>>,

    /// Index: category -> list of agent IDs
    category_index: RwLock<HashMap<CapabilityCategory, Vec<String>>>,
}

impl CapabilityRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an agent with its capabilities.
    pub fn register(&self, capabilities: AgentCapabilities) {
        let agent_id = capabilities.agent_id.clone();

        // Update capability index
        {
            let mut cap_index = self.capability_index.write().unwrap();
            let mut cat_index = self.category_index.write().unwrap();

            for cap in &capabilities.capabilities {
                // Add to capability index (allow duplicates for same capability)
                cap_index
                    .entry(cap.id.clone())
                    .or_default()
                    .push(agent_id.clone());

                // Add to category index (deduplicate - only add once per agent)
                if let Some(category) = cap.category {
                    let agents = cat_index.entry(category).or_default();
                    if !agents.contains(&agent_id) {
                        agents.push(agent_id.clone());
                    }
                }
            }
        }

        // Store agent capabilities
        {
            let mut agents = self.agents.write().unwrap();
            agents.insert(agent_id, capabilities);
        }
    }

    /// Unregister an agent.
    pub fn unregister(&self, agent_id: &str) {
        // Remove from agents
        let capabilities = {
            let mut agents = self.agents.write().unwrap();
            agents.remove(agent_id)
        };

        // Remove from indexes
        if let Some(caps) = capabilities {
            let mut cap_index = self.capability_index.write().unwrap();
            let mut cat_index = self.category_index.write().unwrap();

            for cap in &caps.capabilities {
                if let Some(agents) = cap_index.get_mut(&cap.id) {
                    agents.retain(|id| id != agent_id);
                }

                if let Some(category) = cap.category {
                    if let Some(agents) = cat_index.get_mut(&category) {
                        agents.retain(|id| id != agent_id);
                    }
                }
            }
        }
    }

    /// Get capabilities for a specific agent.
    pub fn get(&self, agent_id: &str) -> Option<AgentCapabilities> {
        let agents = self.agents.read().unwrap();
        agents.get(agent_id).cloned()
    }

    /// Get all registered agent IDs.
    pub fn agent_ids(&self) -> Vec<String> {
        let agents = self.agents.read().unwrap();
        agents.keys().cloned().collect()
    }

    /// Get all registered agents.
    pub fn all_agents(&self) -> Vec<AgentCapabilities> {
        let agents = self.agents.read().unwrap();
        agents.values().cloned().collect()
    }

    /// Find agents matching a capability query.
    pub fn find_agents(&self, query: &CapabilityQuery) -> Vec<AgentCapabilities> {
        let agents = self.agents.read().unwrap();

        agents
            .values()
            .filter(|caps| !caps.find_matching(query).is_empty())
            .cloned()
            .collect()
    }

    /// Find the best agent for a capability query.
    ///
    /// Returns the agent with the highest match score, or None if no match.
    pub fn find_best_agent(&self, query: &CapabilityQuery) -> Option<AgentCapabilities> {
        let agents = self.agents.read().unwrap();

        agents
            .values()
            .filter(|caps| caps.is_available() && caps.match_score(query) > 0.0)
            .max_by(|a, b| {
                a.match_score(query)
                    .partial_cmp(&b.match_score(query))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    /// Find agents by capability ID (fast index lookup).
    pub fn find_by_capability(&self, capability_id: &str) -> Vec<AgentCapabilities> {
        let cap_index = self.capability_index.read().unwrap();
        let agents = self.agents.read().unwrap();

        cap_index
            .get(capability_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| agents.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find agents by category (fast index lookup).
    pub fn find_by_category(&self, category: CapabilityCategory) -> Vec<AgentCapabilities> {
        let cat_index = self.category_index.read().unwrap();
        let agents = self.agents.read().unwrap();

        cat_index
            .get(&category)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| agents.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get available agents (not at capacity).
    pub fn available_agents(&self) -> Vec<AgentCapabilities> {
        let agents = self.agents.read().unwrap();
        agents.values().filter(|a| a.is_available()).cloned().collect()
    }

    /// Update agent availability.
    pub fn set_availability(&self, agent_id: &str, available: bool) {
        let mut agents = self.agents.write().unwrap();
        if let Some(caps) = agents.get_mut(agent_id) {
            caps.available = available;
        }
    }

    /// Number of registered agents.
    pub fn len(&self) -> usize {
        let agents = self.agents.read().unwrap();
        agents.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all registrations.
    pub fn clear(&self) {
        let mut agents = self.agents.write().unwrap();
        let mut cap_index = self.capability_index.write().unwrap();
        let mut cat_index = self.category_index.write().unwrap();

        agents.clear();
        cap_index.clear();
        cat_index.clear();
    }
}

// ============================================================================
// THREAD-SAFE WRAPPER
// ============================================================================

/// Thread-safe shared registry.
pub type SharedCapabilityRegistry = Arc<CapabilityRegistry>;

/// Create a new shared registry.
pub fn shared_registry() -> SharedCapabilityRegistry {
    Arc::new(CapabilityRegistry::new())
}

// ============================================================================
// ROUTING RESULT
// ============================================================================

/// Result of capability-based routing.
#[derive(Debug, Clone)]
pub struct RoutingResult {
    /// Selected agent ID
    pub agent_id: String,

    /// Matching capabilities
    pub matched_capabilities: Vec<String>,

    /// Match score
    pub score: f32,

    /// Reason for selection
    pub reason: String,
}

impl RoutingResult {
    /// Create a routing result.
    pub fn new(
        agent_id: impl Into<String>,
        matched_capabilities: Vec<String>,
        score: f32,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            matched_capabilities,
            score,
            reason: reason.into(),
        }
    }
}

// ============================================================================
// ROUTER
// ============================================================================

/// Capability-based router for task assignment.
pub struct CapabilityRouter {
    registry: SharedCapabilityRegistry,
}

impl CapabilityRouter {
    /// Create a new router with the given registry.
    pub fn new(registry: SharedCapabilityRegistry) -> Self {
        Self { registry }
    }

    /// Route a task to the best matching agent.
    pub fn route(&self, query: &CapabilityQuery) -> Option<RoutingResult> {
        let best = self.registry.find_best_agent(query)?;

        let matched = best
            .find_matching(query)
            .iter()
            .map(|c| c.id.clone())
            .collect();

        let score = best.match_score(query);

        Some(RoutingResult::new(
            &best.agent_id,
            matched,
            score,
            format!("Best match with score {:.2}", score),
        ))
    }

    /// Route to multiple agents for parallel execution.
    pub fn route_parallel(&self, query: &CapabilityQuery, max_agents: usize) -> Vec<RoutingResult> {
        let mut agents = self.registry.find_agents(query);

        // Sort by score (descending)
        agents.sort_by(|a, b| {
            b.match_score(query)
                .partial_cmp(&a.match_score(query))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        agents
            .into_iter()
            .take(max_agents)
            .map(|caps| {
                let matched = caps
                    .find_matching(query)
                    .iter()
                    .map(|c| c.id.clone())
                    .collect();
                let score = caps.match_score(query);
                RoutingResult::new(
                    &caps.agent_id,
                    matched,
                    score,
                    "Parallel execution candidate",
                )
            })
            .collect()
    }
}

// ============================================================================
// UNIFIED CAPABILITY REGISTRY
// ============================================================================

use crate::capability::{CapabilityKind, CapabilityDescriptor};

/// Unified registry for all capability types (Tools, Skills, MCPs, SubAgents).
///
/// This is the primary registry used by the orchestrator to discover and route
/// to capabilities regardless of their underlying implementation.
#[derive(Debug, Default)]
pub struct UnifiedCapabilityRegistry {
    /// All capability descriptors indexed by capability ID
    descriptors: RwLock<HashMap<String, Vec<CapabilityDescriptor>>>,

    /// Index: kind -> capability IDs
    kind_index: RwLock<HashMap<CapabilityKind, Vec<String>>>,

    /// Index: resource_id -> capability IDs
    resource_index: RwLock<HashMap<String, Vec<String>>>,
}

impl UnifiedCapabilityRegistry {
    /// Create a new empty unified registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a capability descriptor.
    pub fn register(&self, descriptor: CapabilityDescriptor) {
        let cap_id = descriptor.capability.id.clone();
        let kind = descriptor.kind;
        let resource_id = descriptor.resource_id.clone();

        // Add to descriptors (multiple descriptors can provide same capability)
        {
            let mut descriptors = self.descriptors.write().unwrap();
            descriptors
                .entry(cap_id.clone())
                .or_default()
                .push(descriptor);
        }

        // Update kind index
        {
            let mut kind_index = self.kind_index.write().unwrap();
            let ids = kind_index.entry(kind).or_default();
            if !ids.contains(&cap_id) {
                ids.push(cap_id.clone());
            }
        }

        // Update resource index
        {
            let mut resource_index = self.resource_index.write().unwrap();
            let ids = resource_index.entry(resource_id).or_default();
            if !ids.contains(&cap_id) {
                ids.push(cap_id);
            }
        }
    }

    /// Register multiple descriptors at once.
    pub fn register_all(&self, descriptors: Vec<CapabilityDescriptor>) {
        for desc in descriptors {
            self.register(desc);
        }
    }

    /// Unregister all capabilities from a specific resource.
    pub fn unregister_resource(&self, resource_id: &str) {
        // Get capability IDs for this resource
        let cap_ids: Vec<String> = {
            let resource_index = self.resource_index.read().unwrap();
            resource_index.get(resource_id).cloned().unwrap_or_default()
        };

        // Remove from descriptors
        {
            let mut descriptors = self.descriptors.write().unwrap();
            for cap_id in &cap_ids {
                if let Some(descs) = descriptors.get_mut(cap_id) {
                    descs.retain(|d| d.resource_id != resource_id);
                    if descs.is_empty() {
                        descriptors.remove(cap_id);
                    }
                }
            }
        }

        // Remove from resource index
        {
            let mut resource_index = self.resource_index.write().unwrap();
            resource_index.remove(resource_id);
        }

        // Note: kind_index cleanup is deferred for simplicity
    }

    /// Find all descriptors for a capability ID.
    pub fn find_by_id(&self, capability_id: &str) -> Vec<CapabilityDescriptor> {
        let descriptors = self.descriptors.read().unwrap();
        descriptors
            .get(capability_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Find all descriptors of a specific kind.
    pub fn find_by_kind(&self, kind: CapabilityKind) -> Vec<CapabilityDescriptor> {
        let kind_index = self.kind_index.read().unwrap();
        let descriptors = self.descriptors.read().unwrap();

        kind_index
            .get(&kind)
            .map(|ids| {
                ids.iter()
                    .flat_map(|id| descriptors.get(id).cloned().unwrap_or_default())
                    .filter(|d| d.kind == kind)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find all descriptors from a specific resource.
    pub fn find_by_resource(&self, resource_id: &str) -> Vec<CapabilityDescriptor> {
        let resource_index = self.resource_index.read().unwrap();
        let descriptors = self.descriptors.read().unwrap();

        resource_index
            .get(resource_id)
            .map(|ids| {
                ids.iter()
                    .flat_map(|id| descriptors.get(id).cloned().unwrap_or_default())
                    .filter(|d| d.resource_id == resource_id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find descriptors matching a capability query.
    pub fn find_matching(&self, query: &CapabilityQuery) -> Vec<CapabilityDescriptor> {
        let descriptors = self.descriptors.read().unwrap();

        descriptors
            .values()
            .flat_map(|descs| descs.iter())
            .filter(|d| d.matches(query))
            .cloned()
            .collect()
    }

    /// Find the best descriptor for a query based on routing score.
    pub fn find_best(&self, query: &CapabilityQuery) -> Option<CapabilityDescriptor> {
        self.find_matching(query)
            .into_iter()
            .max_by(|a, b| {
                a.routing_score(query)
                    .partial_cmp(&b.routing_score(query))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Find the best descriptor of a specific kind.
    pub fn find_best_of_kind(
        &self,
        query: &CapabilityQuery,
        kind: CapabilityKind,
    ) -> Option<CapabilityDescriptor> {
        self.find_matching(query)
            .into_iter()
            .filter(|d| d.kind == kind)
            .max_by(|a, b| {
                a.routing_score(query)
                    .partial_cmp(&b.routing_score(query))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get all registered capability IDs.
    pub fn capability_ids(&self) -> Vec<String> {
        let descriptors = self.descriptors.read().unwrap();
        descriptors.keys().cloned().collect()
    }

    /// Get all available descriptors.
    pub fn all_available(&self) -> Vec<CapabilityDescriptor> {
        let descriptors = self.descriptors.read().unwrap();
        descriptors
            .values()
            .flat_map(|descs| descs.iter())
            .filter(|d| d.available)
            .cloned()
            .collect()
    }

    /// Set availability for all capabilities from a resource.
    pub fn set_resource_availability(&self, resource_id: &str, available: bool) {
        let mut descriptors = self.descriptors.write().unwrap();

        for descs in descriptors.values_mut() {
            for desc in descs.iter_mut() {
                if desc.resource_id == resource_id {
                    desc.available = available;
                }
            }
        }
    }

    /// Number of unique capability IDs registered.
    pub fn len(&self) -> usize {
        let descriptors = self.descriptors.read().unwrap();
        descriptors.len()
    }

    /// Total number of descriptors (including duplicates).
    pub fn total_descriptors(&self) -> usize {
        let descriptors = self.descriptors.read().unwrap();
        descriptors.values().map(|v| v.len()).sum()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all registrations.
    pub fn clear(&self) {
        let mut descriptors = self.descriptors.write().unwrap();
        let mut kind_index = self.kind_index.write().unwrap();
        let mut resource_index = self.resource_index.write().unwrap();

        descriptors.clear();
        kind_index.clear();
        resource_index.clear();
    }
}

/// Thread-safe shared unified registry.
pub type SharedUnifiedRegistry = Arc<UnifiedCapabilityRegistry>;

/// Create a new shared unified registry.
pub fn shared_unified_registry() -> SharedUnifiedRegistry {
    Arc::new(UnifiedCapabilityRegistry::new())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::common;

    fn setup_registry() -> CapabilityRegistry {
        let registry = CapabilityRegistry::new();

        // Register code agent
        let code_caps = AgentCapabilities::builder("code-agent")
            .with_name("Code Agent")
            .add_capability(common::code_review())
            .add_capability(common::code_generation())
            .with_priority(10)
            .build();
        registry.register(code_caps);

        // Register research agent
        let research_caps = AgentCapabilities::builder("research-agent")
            .with_name("Research Agent")
            .add_capability(common::web_search())
            .add_capability(common::research())
            .build();
        registry.register(research_caps);

        registry
    }

    #[test]
    fn test_register_and_get() {
        let registry = setup_registry();

        assert_eq!(registry.len(), 2);

        let code = registry.get("code-agent");
        assert!(code.is_some());
        assert_eq!(code.unwrap().agent_name, "Code Agent");

        let research = registry.get("research-agent");
        assert!(research.is_some());
    }

    #[test]
    fn test_find_by_capability() {
        let registry = setup_registry();

        let reviewers = registry.find_by_capability("code_review");
        assert_eq!(reviewers.len(), 1);
        assert_eq!(reviewers[0].agent_id, "code-agent");

        let searchers = registry.find_by_capability("web_search");
        assert_eq!(searchers.len(), 1);
        assert_eq!(searchers[0].agent_id, "research-agent");
    }

    #[test]
    fn test_find_by_category() {
        let registry = setup_registry();

        let code_agents = registry.find_by_category(CapabilityCategory::CodeExecution);
        assert_eq!(code_agents.len(), 1);
        assert_eq!(code_agents[0].agent_id, "code-agent");

        let network_agents = registry.find_by_category(CapabilityCategory::NetworkHttp);
        assert_eq!(network_agents.len(), 1);
        assert_eq!(network_agents[0].agent_id, "research-agent");
    }

    #[test]
    fn test_find_best_agent() {
        let registry = setup_registry();

        let query = CapabilityQuery::new().with_capability_ids(vec!["code_review"]);
        let best = registry.find_best_agent(&query);
        assert!(best.is_some());
        assert_eq!(best.unwrap().agent_id, "code-agent");
    }

    #[test]
    fn test_unregister() {
        let registry = setup_registry();
        assert_eq!(registry.len(), 2);

        registry.unregister("code-agent");
        assert_eq!(registry.len(), 1);
        assert!(registry.get("code-agent").is_none());

        // Index should be updated
        let reviewers = registry.find_by_capability("code_review");
        assert!(reviewers.is_empty());
    }

    #[test]
    fn test_availability() {
        let registry = setup_registry();

        // Initially available
        let available = registry.available_agents();
        assert_eq!(available.len(), 2);

        // Set unavailable
        registry.set_availability("code-agent", false);

        let available = registry.available_agents();
        assert_eq!(available.len(), 1);

        // Best agent should skip unavailable
        let query = CapabilityQuery::new().with_capability_ids(vec!["code_review"]);
        let best = registry.find_best_agent(&query);
        assert!(best.is_none()); // code-agent is unavailable
    }

    #[test]
    fn test_router() {
        let registry = Arc::new(setup_registry());
        let router = CapabilityRouter::new(registry);

        let query = CapabilityQuery::new().with_capability_ids(vec!["code_review"]);
        let result = router.route(&query);
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.agent_id, "code-agent");
        assert!(result.matched_capabilities.contains(&"code_review".to_string()));
    }

    #[test]
    fn test_router_parallel() {
        let registry = Arc::new(setup_registry());
        let router = CapabilityRouter::new(registry);

        // Query that matches both agents
        let query = CapabilityQuery::new(); // Empty query matches all
        let results = router.route_parallel(&query, 5);

        assert_eq!(results.len(), 2);
    }

    // ========================================================================
    // UNIFIED REGISTRY TESTS
    // ========================================================================

    #[test]
    fn test_unified_registry_basic() {
        let registry = UnifiedCapabilityRegistry::new();

        // Register a tool capability
        let tool_desc = CapabilityDescriptor::tool(common::file_operations(), "read_file");
        registry.register(tool_desc);

        // Register an MCP capability
        let mcp_desc = CapabilityDescriptor::mcp(common::web_search(), "brave", "search");
        registry.register(mcp_desc);

        // Register a sub-agent capability
        let agent_desc = CapabilityDescriptor::sub_agent(common::code_review(), "code-agent");
        registry.register(agent_desc);

        assert_eq!(registry.len(), 3);
        assert_eq!(registry.total_descriptors(), 3);
    }

    #[test]
    fn test_unified_registry_find_by_kind() {
        let registry = UnifiedCapabilityRegistry::new();

        registry.register(CapabilityDescriptor::tool(common::file_operations(), "read_file"));
        registry.register(CapabilityDescriptor::tool(common::shell_execution(), "shell"));
        registry.register(CapabilityDescriptor::mcp(common::web_search(), "brave", "search"));

        let tools = registry.find_by_kind(CapabilityKind::Tool);
        assert_eq!(tools.len(), 2);

        let mcps = registry.find_by_kind(CapabilityKind::McpServer);
        assert_eq!(mcps.len(), 1);
    }

    #[test]
    fn test_unified_registry_find_best() {
        let registry = UnifiedCapabilityRegistry::new();

        // Same capability provided by different resources
        let tool_desc = CapabilityDescriptor::tool(common::code_generation(), "generate_code")
            .with_latency(50);
        let agent_desc = CapabilityDescriptor::sub_agent(common::code_generation(), "code-agent")
            .with_latency(200);

        registry.register(tool_desc);
        registry.register(agent_desc);

        // Should prefer tool (lower latency, higher kind preference)
        let query = CapabilityQuery::new().with_capability_ids(vec!["code_generation"]);
        let best = registry.find_best(&query);

        assert!(best.is_some());
        assert_eq!(best.unwrap().kind, CapabilityKind::Tool);
    }

    #[test]
    fn test_unified_registry_unregister_resource() {
        let registry = UnifiedCapabilityRegistry::new();

        registry.register(CapabilityDescriptor::tool(common::file_operations(), "file_tool"));
        registry.register(CapabilityDescriptor::tool(common::shell_execution(), "file_tool"));
        registry.register(CapabilityDescriptor::mcp(common::web_search(), "search-mcp", "search"));

        assert_eq!(registry.total_descriptors(), 3);

        // Unregister all capabilities from file_tool
        registry.unregister_resource("file_tool");

        assert_eq!(registry.total_descriptors(), 1);
    }

    #[test]
    fn test_unified_registry_availability() {
        let registry = UnifiedCapabilityRegistry::new();

        registry.register(CapabilityDescriptor::tool(common::file_operations(), "file_tool"));
        registry.register(CapabilityDescriptor::mcp(common::web_search(), "mcp", "search"));

        // Initially all available
        assert_eq!(registry.all_available().len(), 2);

        // Set one resource unavailable
        registry.set_resource_availability("file_tool", false);

        assert_eq!(registry.all_available().len(), 1);
        assert_eq!(registry.all_available()[0].kind, CapabilityKind::McpServer);
    }
}
