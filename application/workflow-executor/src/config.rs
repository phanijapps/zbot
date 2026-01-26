//! Configuration types for workflow definitions
//!
//! These types represent the configuration loaded from the file system:
//! - OrchestratorConfig: The main orchestrating agent
//! - SubagentConfig: Worker agents that can be delegated to
//! - WorkflowDefinition: Complete workflow with all components

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::graph::WorkflowGraph;

/// Complete workflow definition loaded from file system
#[derive(Debug, Clone)]
pub struct WorkflowDefinition {
    /// Unique identifier for this workflow (folder name)
    pub id: String,

    /// Display name
    pub name: String,

    /// Orchestrator agent configuration
    pub orchestrator: OrchestratorConfig,

    /// Subagent configurations
    pub subagents: Vec<SubagentConfig>,

    /// Workflow graph defining execution flow
    pub graph: WorkflowGraph,

    /// Path to the workflow directory
    pub path: std::path::PathBuf,
}

/// Orchestrator agent configuration
///
/// The orchestrator is the main agent that coordinates the workflow.
/// It can delegate tasks to subagents based on the workflow graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorConfig {
    /// Display name for the orchestrator
    pub display_name: String,

    /// Description of what this orchestrator does
    #[serde(default)]
    pub description: Option<String>,

    /// LLM provider ID (e.g., "openai", "anthropic")
    pub provider_id: String,

    /// Model identifier (e.g., "gpt-4o-mini", "claude-3-sonnet")
    pub model: String,

    /// Temperature for LLM responses (0.0 - 2.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Maximum tokens for LLM responses
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    /// System instructions (loaded from AGENTS.md)
    #[serde(default)]
    pub system_instructions: String,

    /// MCP server IDs to enable
    #[serde(default)]
    pub mcps: Vec<String>,

    /// Skill IDs to enable
    #[serde(default)]
    pub skills: Vec<String>,

    /// Middleware configuration (YAML string)
    #[serde(default)]
    pub middleware: Option<String>,
}

/// Subagent configuration
///
/// Subagents are specialized workers that the orchestrator can delegate to.
/// Each subagent has its own LLM configuration and capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentConfig {
    /// Unique identifier (folder name in .subagents/) - set by loader, not from YAML
    #[serde(default, alias = "name")]
    pub id: String,

    /// Display name
    #[serde(default)]
    pub display_name: String,

    /// Description of what this subagent does
    #[serde(default)]
    pub description: String,

    /// LLM provider ID
    #[serde(default = "default_provider")]
    pub provider_id: String,

    /// Model identifier
    #[serde(default = "default_model")]
    pub model: String,

    /// Temperature for LLM responses
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Maximum tokens for LLM responses
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    /// System prompt (loaded from AGENTS.md)
    #[serde(default)]
    pub system_prompt: String,

    /// MCP server IDs to enable
    #[serde(default)]
    pub mcps: Vec<String>,

    /// Skill IDs to enable
    #[serde(default)]
    pub skills: Vec<String>,

    /// Built-in tool IDs to enable
    #[serde(default)]
    pub tools: Vec<String>,

    /// Middleware configuration (YAML string)
    #[serde(default)]
    pub middleware: Option<String>,
}

/// Provider configuration for LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    /// Provider type (openai, anthropic, ollama, etc.)
    pub provider_type: String,

    /// API key (can be env var reference like "${OPENAI_API_KEY}")
    #[serde(default)]
    pub api_key: Option<String>,

    /// Base URL for the API (for OpenAI-compatible providers)
    #[serde(default)]
    pub base_url: Option<String>,

    /// Additional provider-specific settings
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,
}

fn default_temperature() -> f32 {
    0.7
}

fn default_max_tokens() -> u32 {
    4096
}

fn default_provider() -> String {
    "openai".to_string()
}

fn default_model() -> String {
    "gpt-4o-mini".to_string()
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            display_name: "Orchestrator".to_string(),
            description: None,
            provider_id: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            system_instructions: String::new(),
            mcps: Vec::new(),
            skills: Vec::new(),
            middleware: None,
        }
    }
}

impl Default for SubagentConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: "Subagent".to_string(),
            description: String::new(),
            provider_id: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            system_prompt: String::new(),
            mcps: Vec::new(),
            skills: Vec::new(),
            tools: Vec::new(),
            middleware: None,
        }
    }
}
