//! # Agent Capability System
//!
//! Defines capabilities that agents declare and the orchestrator uses for routing.
//!
//! ## Overview
//!
//! Capabilities describe what an agent can do, enabling:
//! - Task routing based on required capabilities
//! - Agent discovery by capability
//! - Permission checking before delegation
//!
//! ## Example
//!
//! ```rust
//! use zero_core::capability::{Capability, AgentCapabilities};
//!
//! let capabilities = AgentCapabilities::builder("code-reviewer")
//!     .add_capability(Capability::new("code_review")
//!         .with_description("Reviews code for quality and bugs")
//!         .with_input_types(vec!["code", "diff", "pull_request"])
//!         .with_output_types(vec!["review_comments", "suggestions"]))
//!     .add_capability(Capability::new("explain_code")
//!         .with_description("Explains code functionality"))
//!     .build();
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::policy::CapabilityCategory;

// ============================================================================
// CAPABILITY
// ============================================================================

/// A specific capability that an agent can perform.
///
/// Capabilities are more granular than categories - they describe
/// specific tasks like "code_review", "web_search", "image_generation".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Unique identifier for this capability (e.g., "code_review")
    pub id: String,

    /// Human-readable description of what this capability does
    #[serde(default)]
    pub description: String,

    /// Category this capability belongs to
    #[serde(default)]
    pub category: Option<CapabilityCategory>,

    /// Types of input this capability accepts (e.g., ["code", "diff"])
    #[serde(default)]
    pub input_types: Vec<String>,

    /// Types of output this capability produces (e.g., ["review", "suggestions"])
    #[serde(default)]
    pub output_types: Vec<String>,

    /// Keywords for semantic matching
    #[serde(default)]
    pub keywords: Vec<String>,

    /// Whether this capability is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Estimated cost/complexity (0.0 - 1.0, higher = more expensive)
    #[serde(default)]
    pub cost_weight: f32,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

fn default_true() -> bool {
    true
}

impl Capability {
    /// Create a new capability with the given ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: String::new(),
            category: None,
            input_types: Vec::new(),
            output_types: Vec::new(),
            keywords: Vec::new(),
            enabled: true,
            cost_weight: 0.5,
            metadata: HashMap::new(),
        }
    }

    /// Set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the category.
    pub fn with_category(mut self, category: CapabilityCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Set accepted input types.
    pub fn with_input_types(mut self, types: Vec<impl Into<String>>) -> Self {
        self.input_types = types.into_iter().map(Into::into).collect();
        self
    }

    /// Set produced output types.
    pub fn with_output_types(mut self, types: Vec<impl Into<String>>) -> Self {
        self.output_types = types.into_iter().map(Into::into).collect();
        self
    }

    /// Add keywords for matching.
    pub fn with_keywords(mut self, keywords: Vec<impl Into<String>>) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    /// Set the cost weight.
    pub fn with_cost_weight(mut self, weight: f32) -> Self {
        self.cost_weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Check if this capability matches the given query keywords.
    pub fn matches_keywords(&self, query: &[String]) -> bool {
        if query.is_empty() {
            return true;
        }

        let query_lower: HashSet<_> = query.iter().map(|s| s.to_lowercase()).collect();

        // Check ID
        if query_lower.contains(&self.id.to_lowercase()) {
            return true;
        }

        // Check keywords
        for keyword in &self.keywords {
            if query_lower.contains(&keyword.to_lowercase()) {
                return true;
            }
        }

        // Check description words
        let desc_words: HashSet<_> = self
            .description
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        query_lower.intersection(&desc_words).next().is_some()
    }

    /// Check if this capability can accept the given input type.
    pub fn accepts_input(&self, input_type: &str) -> bool {
        self.input_types.is_empty() || self.input_types.iter().any(|t| t == input_type)
    }

    /// Check if this capability produces the given output type.
    pub fn produces_output(&self, output_type: &str) -> bool {
        self.output_types.is_empty() || self.output_types.iter().any(|t| t == output_type)
    }
}

// ============================================================================
// AGENT CAPABILITIES
// ============================================================================

/// Collection of capabilities for an agent.
///
/// This is the primary way to describe what an agent can do.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentCapabilities {
    /// Agent identifier
    pub agent_id: String,

    /// Agent display name
    #[serde(default)]
    pub agent_name: String,

    /// List of capabilities this agent has
    #[serde(default)]
    pub capabilities: Vec<Capability>,

    /// Maximum concurrent tasks this agent can handle
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_tasks: usize,

    /// Whether this agent is currently available
    #[serde(default = "default_true")]
    pub available: bool,

    /// Priority for routing (higher = preferred)
    #[serde(default = "default_priority")]
    pub priority: i32,
}

fn default_max_concurrent() -> usize {
    1
}

fn default_priority() -> i32 {
    0
}

impl AgentCapabilities {
    /// Create a new builder for agent capabilities.
    pub fn builder(agent_id: impl Into<String>) -> AgentCapabilitiesBuilder {
        AgentCapabilitiesBuilder::new(agent_id)
    }

    /// Get all capability IDs.
    pub fn capability_ids(&self) -> Vec<&str> {
        self.capabilities.iter().map(|c| c.id.as_str()).collect()
    }

    /// Check if this agent has a capability with the given ID.
    pub fn has_capability(&self, capability_id: &str) -> bool {
        self.capabilities
            .iter()
            .any(|c| c.id == capability_id && c.enabled)
    }

    /// Get a capability by ID.
    pub fn get_capability(&self, capability_id: &str) -> Option<&Capability> {
        self.capabilities.iter().find(|c| c.id == capability_id)
    }

    /// Find capabilities matching the given query.
    pub fn find_matching(&self, query: &CapabilityQuery) -> Vec<&Capability> {
        self.capabilities
            .iter()
            .filter(|c| c.enabled && query.matches(c))
            .collect()
    }

    /// Check if this agent is available for new tasks.
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Calculate a score for how well this agent matches a query.
    /// Higher score = better match.
    pub fn match_score(&self, query: &CapabilityQuery) -> f32 {
        if !self.available {
            return 0.0;
        }

        let matching = self.find_matching(query);
        if matching.is_empty() {
            return 0.0;
        }

        // Base score from number of matching capabilities
        let mut score = matching.len() as f32;

        // Boost for priority
        score += self.priority as f32 * 0.1;

        // Adjust for cost (prefer lower cost)
        let avg_cost: f32 =
            matching.iter().map(|c| c.cost_weight).sum::<f32>() / matching.len() as f32;
        score *= 1.0 - (avg_cost * 0.5);

        score
    }
}

// ============================================================================
// BUILDER
// ============================================================================

/// Builder for AgentCapabilities.
pub struct AgentCapabilitiesBuilder {
    agent_id: String,
    agent_name: String,
    capabilities: Vec<Capability>,
    max_concurrent_tasks: usize,
    available: bool,
    priority: i32,
}

impl AgentCapabilitiesBuilder {
    /// Create a new builder.
    pub fn new(agent_id: impl Into<String>) -> Self {
        let id = agent_id.into();
        Self {
            agent_name: id.clone(),
            agent_id: id,
            capabilities: Vec::new(),
            max_concurrent_tasks: 1,
            available: true,
            priority: 0,
        }
    }

    /// Set the agent name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.agent_name = name.into();
        self
    }

    /// Add a capability.
    pub fn add_capability(mut self, capability: Capability) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Add multiple capabilities.
    pub fn add_capabilities(mut self, capabilities: Vec<Capability>) -> Self {
        self.capabilities.extend(capabilities);
        self
    }

    /// Set max concurrent tasks.
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent_tasks = max;
        self
    }

    /// Set availability.
    pub fn with_availability(mut self, available: bool) -> Self {
        self.available = available;
        self
    }

    /// Set priority.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Build the AgentCapabilities.
    pub fn build(self) -> AgentCapabilities {
        AgentCapabilities {
            agent_id: self.agent_id,
            agent_name: self.agent_name,
            capabilities: self.capabilities,
            max_concurrent_tasks: self.max_concurrent_tasks,
            available: self.available,
            priority: self.priority,
        }
    }
}

// ============================================================================
// CAPABILITY QUERY
// ============================================================================

/// Query for finding agents with matching capabilities.
#[derive(Debug, Clone, Default)]
pub struct CapabilityQuery {
    /// Required capability IDs (any match)
    pub capability_ids: Vec<String>,

    /// Required category (any match)
    pub categories: Vec<CapabilityCategory>,

    /// Required input type
    pub input_type: Option<String>,

    /// Required output type
    pub output_type: Option<String>,

    /// Keywords for semantic matching
    pub keywords: Vec<String>,

    /// Maximum cost weight allowed
    pub max_cost: Option<f32>,
}

impl CapabilityQuery {
    /// Create a new empty query (matches everything).
    pub fn new() -> Self {
        Self::default()
    }

    /// Require specific capability IDs.
    pub fn with_capability_ids(mut self, ids: Vec<impl Into<String>>) -> Self {
        self.capability_ids = ids.into_iter().map(Into::into).collect();
        self
    }

    /// Require specific categories.
    pub fn with_categories(mut self, categories: Vec<CapabilityCategory>) -> Self {
        self.categories = categories;
        self
    }

    /// Require specific input type.
    pub fn with_input_type(mut self, input_type: impl Into<String>) -> Self {
        self.input_type = Some(input_type.into());
        self
    }

    /// Require specific output type.
    pub fn with_output_type(mut self, output_type: impl Into<String>) -> Self {
        self.output_type = Some(output_type.into());
        self
    }

    /// Add keywords for matching.
    pub fn with_keywords(mut self, keywords: Vec<impl Into<String>>) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    /// Set maximum cost.
    pub fn with_max_cost(mut self, cost: f32) -> Self {
        self.max_cost = Some(cost);
        self
    }

    /// Check if a capability matches this query.
    pub fn matches(&self, capability: &Capability) -> bool {
        // Check capability IDs
        if !self.capability_ids.is_empty()
            && !self.capability_ids.contains(&capability.id)
        {
            return false;
        }

        // Check categories
        if !self.categories.is_empty() {
            if let Some(cat) = &capability.category {
                if !self.categories.contains(cat) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check input type
        if let Some(input) = &self.input_type {
            if !capability.accepts_input(input) {
                return false;
            }
        }

        // Check output type
        if let Some(output) = &self.output_type {
            if !capability.produces_output(output) {
                return false;
            }
        }

        // Check keywords
        if !self.keywords.is_empty() && !capability.matches_keywords(&self.keywords) {
            return false;
        }

        // Check cost
        if let Some(max_cost) = self.max_cost {
            if capability.cost_weight > max_cost {
                return false;
            }
        }

        true
    }
}

// ============================================================================
// COMMON CAPABILITIES
// ============================================================================

/// Common capability definitions for reuse.
pub mod common {
    use super::*;

    /// Code review capability.
    pub fn code_review() -> Capability {
        Capability::new("code_review")
            .with_description("Reviews code for quality, bugs, and best practices")
            .with_category(CapabilityCategory::CodeExecution)
            .with_input_types(vec!["code", "diff", "pull_request"])
            .with_output_types(vec!["review_comments", "suggestions", "approval"])
            .with_keywords(vec!["review", "pr", "quality", "bugs"])
    }

    /// Code generation capability.
    pub fn code_generation() -> Capability {
        Capability::new("code_generation")
            .with_description("Generates code from requirements or specifications")
            .with_category(CapabilityCategory::CodeExecution)
            .with_input_types(vec!["requirements", "specification", "prompt"])
            .with_output_types(vec!["code", "files"])
            .with_keywords(vec!["write", "generate", "create", "implement"])
    }

    /// Web search capability.
    pub fn web_search() -> Capability {
        Capability::new("web_search")
            .with_description("Searches the web for information")
            .with_category(CapabilityCategory::NetworkHttp)
            .with_input_types(vec!["query", "question"])
            .with_output_types(vec!["search_results", "summary"])
            .with_keywords(vec!["search", "find", "lookup", "google"])
    }

    /// File operations capability.
    pub fn file_operations() -> Capability {
        Capability::new("file_operations")
            .with_description("Reads, writes, and manages files")
            .with_category(CapabilityCategory::FileWrite)
            .with_input_types(vec!["path", "content"])
            .with_output_types(vec!["file_content", "file_list"])
            .with_keywords(vec!["file", "read", "write", "edit", "create"])
    }

    /// Data analysis capability.
    pub fn data_analysis() -> Capability {
        Capability::new("data_analysis")
            .with_description("Analyzes data and generates insights")
            .with_category(CapabilityCategory::DataTransform)
            .with_input_types(vec!["data", "csv", "json", "table"])
            .with_output_types(vec!["analysis", "charts", "summary"])
            .with_keywords(vec!["analyze", "data", "statistics", "insights"])
    }

    /// Shell execution capability.
    pub fn shell_execution() -> Capability {
        Capability::new("shell_execution")
            .with_description("Executes shell commands")
            .with_category(CapabilityCategory::ShellExecution)
            .with_input_types(vec!["command"])
            .with_output_types(vec!["output", "exit_code"])
            .with_keywords(vec!["shell", "terminal", "command", "bash"])
            .with_cost_weight(0.8)
    }

    /// Research capability.
    pub fn research() -> Capability {
        Capability::new("research")
            .with_description("Conducts research and gathers information")
            .with_category(CapabilityCategory::KnowledgeRead)
            .with_input_types(vec!["topic", "question"])
            .with_output_types(vec!["report", "summary", "facts"])
            .with_keywords(vec!["research", "investigate", "learn", "explore"])
    }
}

// ============================================================================
// CAPABILITY KIND
// ============================================================================

/// The kind of resource that provides a capability.
///
/// The orchestrator uses this to determine how to invoke a capability:
/// - **Tool**: Direct invocation via the Tool trait
/// - **Skill**: Load instructions that guide agent behavior
/// - **McpServer**: External tool provided by an MCP server
/// - **SubAgent**: Delegate to another agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    /// A tool that performs a single action (e.g., read_file, shell)
    Tool,

    /// A skill that provides instructions/context for extended behavior
    Skill,

    /// A tool provided by an external MCP server
    McpServer,

    /// A sub-agent that can handle delegated tasks
    SubAgent,
}

impl CapabilityKind {
    /// Returns a human-readable label for this kind.
    pub fn label(&self) -> &'static str {
        match self {
            CapabilityKind::Tool => "Tool",
            CapabilityKind::Skill => "Skill",
            CapabilityKind::McpServer => "MCP Server",
            CapabilityKind::SubAgent => "Sub-Agent",
        }
    }

    /// Returns whether this kind is directly invocable (vs providing context).
    pub fn is_invocable(&self) -> bool {
        matches!(
            self,
            CapabilityKind::Tool | CapabilityKind::McpServer | CapabilityKind::SubAgent
        )
    }

    /// Returns whether this kind provides instructions/context.
    pub fn is_contextual(&self) -> bool {
        matches!(self, CapabilityKind::Skill)
    }
}

impl std::fmt::Display for CapabilityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ============================================================================
// CAPABILITY DESCRIPTOR
// ============================================================================

/// A capability descriptor that combines metadata with invocation information.
///
/// This is the primary type used by the orchestrator to discover and invoke
/// capabilities regardless of their underlying implementation (Tool, Skill, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    /// The capability metadata (id, description, input/output types, etc.)
    pub capability: Capability,

    /// What kind of resource provides this capability
    pub kind: CapabilityKind,

    /// Resource identifier used for invocation:
    /// - Tool: tool name
    /// - Skill: skill id (e.g., "rust-development")
    /// - McpServer: "server_id::tool_name"
    /// - SubAgent: agent id
    pub resource_id: String,

    /// Optional display name for the resource
    #[serde(default)]
    pub resource_name: Option<String>,

    /// Whether this capability is currently available
    #[serde(default = "default_true")]
    pub available: bool,

    /// Estimated latency in milliseconds (for routing decisions)
    #[serde(default)]
    pub latency_ms: Option<u32>,

    /// Additional invocation metadata
    #[serde(default)]
    pub invocation_hints: HashMap<String, serde_json::Value>,
}

impl CapabilityDescriptor {
    /// Create a new capability descriptor.
    pub fn new(
        capability: Capability,
        kind: CapabilityKind,
        resource_id: impl Into<String>,
    ) -> Self {
        Self {
            capability,
            kind,
            resource_id: resource_id.into(),
            resource_name: None,
            available: true,
            latency_ms: None,
            invocation_hints: HashMap::new(),
        }
    }

    /// Create a descriptor for a tool.
    pub fn tool(capability: Capability, tool_name: impl Into<String>) -> Self {
        Self::new(capability, CapabilityKind::Tool, tool_name)
    }

    /// Create a descriptor for a skill.
    pub fn skill(capability: Capability, skill_id: impl Into<String>) -> Self {
        Self::new(capability, CapabilityKind::Skill, skill_id)
    }

    /// Create a descriptor for an MCP server tool.
    pub fn mcp(
        capability: Capability,
        server_id: impl Into<String>,
        tool_name: impl Into<String>,
    ) -> Self {
        let resource_id = format!("{}::{}", server_id.into(), tool_name.into());
        Self::new(capability, CapabilityKind::McpServer, resource_id)
    }

    /// Create a descriptor for a sub-agent.
    pub fn sub_agent(capability: Capability, agent_id: impl Into<String>) -> Self {
        Self::new(capability, CapabilityKind::SubAgent, agent_id)
    }

    /// Set the resource name.
    pub fn with_resource_name(mut self, name: impl Into<String>) -> Self {
        self.resource_name = Some(name.into());
        self
    }

    /// Set availability.
    pub fn with_availability(mut self, available: bool) -> Self {
        self.available = available;
        self
    }

    /// Set estimated latency.
    pub fn with_latency(mut self, latency_ms: u32) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    /// Add an invocation hint.
    pub fn with_hint(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.invocation_hints.insert(key.into(), value);
        self
    }

    /// Get the capability ID.
    pub fn id(&self) -> &str {
        &self.capability.id
    }

    /// Check if this descriptor matches a query.
    pub fn matches(&self, query: &CapabilityQuery) -> bool {
        self.available && query.matches(&self.capability)
    }

    /// Calculate a routing score for this descriptor.
    ///
    /// Higher score = better match. Considers:
    /// - Capability match quality
    /// - Cost weight
    /// - Latency (if known)
    /// - Kind preference (tools preferred over sub-agents for simple tasks)
    pub fn routing_score(&self, query: &CapabilityQuery) -> f32 {
        if !self.matches(query) {
            return 0.0;
        }

        let mut score = 1.0;

        // Adjust for cost
        score *= 1.0 - (self.capability.cost_weight * 0.5);

        // Adjust for latency (prefer lower latency)
        if let Some(latency) = self.latency_ms {
            // Normalize: 100ms = no penalty, 1000ms = 0.5 penalty
            let latency_penalty = (latency as f32 / 1000.0).min(1.0) * 0.5;
            score *= 1.0 - latency_penalty;
        }

        // Adjust for kind (prefer simpler invocation)
        score *= match self.kind {
            CapabilityKind::Tool => 1.0,
            CapabilityKind::McpServer => 0.95,
            CapabilityKind::Skill => 0.9,
            CapabilityKind::SubAgent => 0.8,
        };

        score
    }
}

// ============================================================================
// CAPABILITY PROVIDER TRAIT
// ============================================================================

/// Trait for types that can provide capability descriptors.
///
/// Implemented by tools, skills, MCP servers, and agents to expose
/// their capabilities to the orchestrator.
pub trait CapabilityProvider: Send + Sync {
    /// Returns the capability descriptors this provider offers.
    fn capabilities(&self) -> Vec<CapabilityDescriptor>;

    /// Returns a specific capability by ID, if available.
    fn get_capability(&self, id: &str) -> Option<CapabilityDescriptor> {
        self.capabilities().into_iter().find(|c| c.id() == id)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_kind() {
        assert!(CapabilityKind::Tool.is_invocable());
        assert!(CapabilityKind::McpServer.is_invocable());
        assert!(CapabilityKind::SubAgent.is_invocable());
        assert!(!CapabilityKind::Skill.is_invocable());

        assert!(CapabilityKind::Skill.is_contextual());
        assert!(!CapabilityKind::Tool.is_contextual());
    }

    #[test]
    fn test_capability_descriptor() {
        let cap = common::file_operations();
        let desc = CapabilityDescriptor::tool(cap.clone(), "read_file")
            .with_latency(50)
            .with_availability(true);

        assert_eq!(desc.kind, CapabilityKind::Tool);
        assert_eq!(desc.resource_id, "read_file");
        assert_eq!(desc.latency_ms, Some(50));
        assert!(desc.available);
    }

    #[test]
    fn test_mcp_descriptor() {
        let cap = common::web_search();
        let desc = CapabilityDescriptor::mcp(cap, "brave-search", "search");

        assert_eq!(desc.kind, CapabilityKind::McpServer);
        assert_eq!(desc.resource_id, "brave-search::search");
    }

    #[test]
    fn test_routing_score() {
        let cap = common::code_generation();

        // Tool should have higher score than sub-agent
        let tool_desc = CapabilityDescriptor::tool(cap.clone(), "generate_code");
        let agent_desc = CapabilityDescriptor::sub_agent(cap.clone(), "code-agent");

        let query = CapabilityQuery::new().with_capability_ids(vec!["code_generation"]);

        assert!(tool_desc.routing_score(&query) > agent_desc.routing_score(&query));
    }

    #[test]
    fn test_capability_creation() {
        let cap = Capability::new("test")
            .with_description("Test capability")
            .with_keywords(vec!["test", "example"]);

        assert_eq!(cap.id, "test");
        assert_eq!(cap.description, "Test capability");
        assert!(cap.enabled);
    }

    #[test]
    fn test_capability_matching() {
        let cap = common::code_review();

        // Should match by ID
        assert!(cap.matches_keywords(&["code_review".into()]));

        // Should match by keyword
        assert!(cap.matches_keywords(&["review".into()]));
        assert!(cap.matches_keywords(&["bugs".into()]));

        // Should not match unrelated
        assert!(!cap.matches_keywords(&["unrelated".into()]));
    }

    #[test]
    fn test_agent_capabilities_builder() {
        let caps = AgentCapabilities::builder("reviewer")
            .with_name("Code Reviewer")
            .add_capability(common::code_review())
            .with_priority(10)
            .build();

        assert_eq!(caps.agent_id, "reviewer");
        assert_eq!(caps.agent_name, "Code Reviewer");
        assert!(caps.has_capability("code_review"));
        assert!(!caps.has_capability("web_search"));
    }

    #[test]
    fn test_capability_query() {
        let caps = AgentCapabilities::builder("multi")
            .add_capability(common::code_review())
            .add_capability(common::web_search())
            .build();

        // Query by ID
        let query = CapabilityQuery::new().with_capability_ids(vec!["code_review"]);
        let matching = caps.find_matching(&query);
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].id, "code_review");

        // Query by category
        let query = CapabilityQuery::new().with_categories(vec![CapabilityCategory::NetworkHttp]);
        let matching = caps.find_matching(&query);
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].id, "web_search");
    }

    #[test]
    fn test_match_score() {
        let caps = AgentCapabilities::builder("agent")
            .add_capability(common::code_review())
            .with_priority(5)
            .build();

        let query = CapabilityQuery::new().with_capability_ids(vec!["code_review"]);
        let score = caps.match_score(&query);
        assert!(score > 0.0);

        // Unavailable agent should have 0 score
        let unavailable = AgentCapabilities::builder("agent")
            .add_capability(common::code_review())
            .with_availability(false)
            .build();
        assert_eq!(unavailable.match_score(&query), 0.0);
    }
}
