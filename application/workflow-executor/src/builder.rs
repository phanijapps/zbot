//! Workflow builder - constructs executable agents from workflow definitions
//!
//! This module takes a `WorkflowDefinition` and builds an executable agent graph
//! using the zero-agent framework's agent types (Sequential, Parallel, Conditional, etc.)

use std::collections::HashMap;
use std::sync::Arc;

use zero_agent::{LlmAgentBuilder, SequentialAgent, ParallelAgent};
use zero_core::{Agent, Tool, Toolset, FileSystemContext};
use zero_llm::{Llm, LlmConfig, OpenAiLlm};

use crate::config::{SubagentConfig, WorkflowDefinition};
use crate::error::{Result, WorkflowError};
use crate::graph::{WorkflowGraph, WorkflowPattern, NodeType};

/// Provider factory for creating LLM instances
pub type LlmFactory = Arc<dyn Fn(&str, &str) -> Result<Arc<dyn Llm>> + Send + Sync>;

/// Toolset factory for creating toolsets based on configuration
pub type ToolsetFactory = Arc<dyn Fn(&[String], &[String], &[String]) -> Result<Arc<dyn Toolset>> + Send + Sync>;

/// Built workflow ready for execution
pub struct ExecutableWorkflow {
    /// The root agent (composed from workflow pattern)
    pub root_agent: Arc<dyn Agent>,

    /// Workflow definition (for reference)
    pub definition: WorkflowDefinition,

    /// Map of subagent ID to built agent
    pub subagent_map: HashMap<String, Arc<dyn Agent>>,
}

/// Workflow builder configuration
pub struct WorkflowBuilderConfig {
    /// Factory for creating LLM instances
    pub llm_factory: Option<LlmFactory>,

    /// Factory for creating toolsets
    pub toolset_factory: Option<ToolsetFactory>,

    /// File system context for tools
    pub fs_context: Option<Arc<dyn FileSystemContext>>,

    /// Default API key for LLMs (if not using factory)
    pub default_api_key: Option<String>,
}

impl Default for WorkflowBuilderConfig {
    fn default() -> Self {
        Self {
            llm_factory: None,
            toolset_factory: None,
            fs_context: None,
            default_api_key: None,
        }
    }
}

/// Workflow builder - constructs executable agents from workflow definitions
pub struct WorkflowBuilder {
    config: WorkflowBuilderConfig,
}

impl WorkflowBuilder {
    /// Create a new workflow builder
    pub fn new() -> Self {
        Self {
            config: WorkflowBuilderConfig::default(),
        }
    }

    /// Create with configuration
    pub fn with_config(config: WorkflowBuilderConfig) -> Self {
        Self { config }
    }

    /// Set the LLM factory
    pub fn with_llm_factory(mut self, factory: LlmFactory) -> Self {
        self.config.llm_factory = Some(factory);
        self
    }

    /// Set the toolset factory
    pub fn with_toolset_factory(mut self, factory: ToolsetFactory) -> Self {
        self.config.toolset_factory = Some(factory);
        self
    }

    /// Set the file system context
    pub fn with_fs_context(mut self, fs: Arc<dyn FileSystemContext>) -> Self {
        self.config.fs_context = Some(fs);
        self
    }

    /// Set the default API key
    pub fn with_default_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.config.default_api_key = Some(api_key.into());
        self
    }

    /// Build an executable workflow from a definition
    pub async fn build(&self, definition: WorkflowDefinition) -> Result<ExecutableWorkflow> {
        tracing::info!("Building workflow: {}", definition.name);

        // Build all subagents first
        let mut subagent_map = HashMap::new();
        for subagent_config in &definition.subagents {
            let agent = self.build_subagent(subagent_config).await?;
            subagent_map.insert(subagent_config.id.clone(), agent);
        }

        // Build the root agent based on workflow pattern
        let root_agent = self.build_root_agent(&definition, &subagent_map).await?;

        Ok(ExecutableWorkflow {
            root_agent,
            definition,
            subagent_map,
        })
    }

    /// Build a subagent from its configuration
    async fn build_subagent(&self, config: &SubagentConfig) -> Result<Arc<dyn Agent>> {
        tracing::debug!("Building subagent: {}", config.id);

        // Create LLM instance
        let llm = self.create_llm(&config.provider_id, &config.model)?;

        // Create toolset
        let tools = self.create_toolset(&config.mcps, &config.skills, &config.tools)?;

        // Build the agent
        let agent = LlmAgentBuilder::new(&config.id, &config.description)
            .with_llm(llm)
            .with_tools(tools)
            .with_system_instruction(&config.system_prompt)
            .build()
            .map_err(|e| WorkflowError::Framework(e.to_string()))?;

        Ok(Arc::new(agent))
    }

    /// Build the root agent based on workflow pattern
    async fn build_root_agent(
        &self,
        definition: &WorkflowDefinition,
        subagent_map: &HashMap<String, Arc<dyn Agent>>,
    ) -> Result<Arc<dyn Agent>> {
        let graph = &definition.graph;

        match graph.pattern {
            WorkflowPattern::Pipeline => {
                self.build_pipeline_agent(definition, subagent_map, graph).await
            }
            WorkflowPattern::Parallel => {
                self.build_parallel_agent(definition, subagent_map, graph).await
            }
            WorkflowPattern::Router => {
                self.build_router_agent(definition, subagent_map, graph).await
            }
            WorkflowPattern::Custom => {
                self.build_custom_agent(definition, subagent_map, graph).await
            }
        }
    }

    /// Build a sequential pipeline agent
    async fn build_pipeline_agent(
        &self,
        definition: &WorkflowDefinition,
        subagent_map: &HashMap<String, Arc<dyn Agent>>,
        graph: &WorkflowGraph,
    ) -> Result<Arc<dyn Agent>> {
        // Get execution order
        let order = graph.execution_order()?;

        // Collect subagents in order (skip start/end nodes)
        let mut agents: Vec<Arc<dyn Agent>> = Vec::new();

        for node_id in order {
            if let Some(node) = graph.find_node(&node_id) {
                if node.node_type == NodeType::Subagent {
                    if let Some(ref subagent_id) = node.subagent_id {
                        if let Some(agent) = subagent_map.get(subagent_id) {
                            agents.push(agent.clone());
                        } else {
                            return Err(WorkflowError::SubagentNotFound(subagent_id.clone()));
                        }
                    }
                }
            }
        }

        if agents.is_empty() {
            // If no subagents, create a simple orchestrator agent
            return self.build_orchestrator_agent(definition).await;
        }

        // Create sequential agent
        let pipeline = SequentialAgent::new(
            &format!("{}_pipeline", definition.id),
            agents,
        );

        Ok(Arc::new(pipeline))
    }

    /// Build a parallel agent
    async fn build_parallel_agent(
        &self,
        definition: &WorkflowDefinition,
        subagent_map: &HashMap<String, Arc<dyn Agent>>,
        graph: &WorkflowGraph,
    ) -> Result<Arc<dyn Agent>> {
        // Get all subagent nodes (they run in parallel)
        let mut agents: Vec<Arc<dyn Agent>> = Vec::new();

        for node in graph.subagent_nodes() {
            if let Some(ref subagent_id) = node.subagent_id {
                if let Some(agent) = subagent_map.get(subagent_id) {
                    agents.push(agent.clone());
                }
            }
        }

        if agents.is_empty() {
            return self.build_orchestrator_agent(definition).await;
        }

        let parallel = ParallelAgent::new(
            &format!("{}_parallel", definition.id),
            agents,
        );

        Ok(Arc::new(parallel))
    }

    /// Build a router agent (orchestrator that delegates to subagents)
    async fn build_router_agent(
        &self,
        definition: &WorkflowDefinition,
        subagent_map: &HashMap<String, Arc<dyn Agent>>,
        _graph: &WorkflowGraph,
    ) -> Result<Arc<dyn Agent>> {
        // For router pattern, we build the orchestrator with subagent context
        // The orchestrator's system instructions should guide it to delegate
        self.build_orchestrator_agent(definition).await
    }

    /// Build a custom agent (follows graph edges)
    async fn build_custom_agent(
        &self,
        definition: &WorkflowDefinition,
        subagent_map: &HashMap<String, Arc<dyn Agent>>,
        graph: &WorkflowGraph,
    ) -> Result<Arc<dyn Agent>> {
        // For now, treat custom as pipeline
        // TODO: Implement graph traversal logic
        self.build_pipeline_agent(definition, subagent_map, graph).await
    }

    /// Build the orchestrator agent
    async fn build_orchestrator_agent(
        &self,
        definition: &WorkflowDefinition,
    ) -> Result<Arc<dyn Agent>> {
        let config = &definition.orchestrator;

        // Create LLM instance
        let llm = self.create_llm(&config.provider_id, &config.model)?;

        // Create toolset
        let tools = self.create_toolset(&config.mcps, &config.skills, &[])?;

        // Build the agent
        let agent = LlmAgentBuilder::new(&definition.id, config.description.as_deref().unwrap_or("Orchestrator"))
            .with_llm(llm)
            .with_tools(tools)
            .with_system_instruction(&config.system_instructions)
            .build()
            .map_err(|e| WorkflowError::Framework(e.to_string()))?;

        Ok(Arc::new(agent))
    }

    /// Create an LLM instance
    fn create_llm(&self, provider_id: &str, model: &str) -> Result<Arc<dyn Llm>> {
        // Use factory if available
        if let Some(ref factory) = self.config.llm_factory {
            return factory(provider_id, model);
        }

        // Default: Create OpenAI-compatible LLM
        let api_key = self.config.default_api_key.as_ref()
            .ok_or_else(|| WorkflowError::LlmConfig(
                "No API key provided and no LLM factory configured".to_string()
            ))?;

        let llm_config = LlmConfig::new(api_key, model);
        let llm = OpenAiLlm::new(llm_config)
            .map_err(|e| WorkflowError::LlmConfig(e.to_string()))?;

        Ok(Arc::new(llm))
    }

    /// Create a toolset
    fn create_toolset(
        &self,
        mcps: &[String],
        skills: &[String],
        tools: &[String],
    ) -> Result<Arc<dyn Toolset>> {
        // Use factory if available
        if let Some(ref factory) = self.config.toolset_factory {
            return factory(mcps, skills, tools);
        }

        // Default: Return empty toolset
        Ok(Arc::new(EmptyToolset))
    }
}

impl Default for WorkflowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Empty toolset implementation
struct EmptyToolset;

#[async_trait::async_trait]
impl Toolset for EmptyToolset {
    fn name(&self) -> &str {
        "empty"
    }

    async fn tools(&self) -> zero_core::Result<Vec<Arc<dyn Tool>>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_creation() {
        let builder = WorkflowBuilder::new();
        assert!(builder.config.llm_factory.is_none());
    }
}
