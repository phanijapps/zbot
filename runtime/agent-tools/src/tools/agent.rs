// ============================================================================
// AGENT TOOLS
// Tools for managing and discovering AI agents
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::{Tool, ToolContext, Result};
use zero_core::FileSystemContext;

// ============================================================================
// LIST AGENTS TOOL
// ============================================================================

/// Tool for discovering available agents to delegate to.
///
/// This tool reads from a cached agent list stored in the ToolContext state.
/// The list is populated by the execution runner when creating the executor.
pub struct ListAgentsTool;

impl ListAgentsTool {
    /// Create a new list agents tool
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListAgentsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ListAgentsTool {
    fn name(&self) -> &str {
        "list_agents"
    }

    fn description(&self) -> &str {
        "List available agents you can delegate tasks to using delegate_to_agent. \
         Returns agent IDs, names, and descriptions to help you choose the right agent for a task."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {},
            "required": []
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, _args: Value) -> Result<Value> {
        // Read cached agent list from context state
        let agents: Value = match ctx.get_state("available_agents") {
            Some(v) => v.clone(),
            None => json!([]),
        };

        // Get current agent ID to exclude from list
        let current_agent_id: String = match ctx.get_state("agent_id") {
            Some(v) => v.as_str().unwrap_or("").to_string(),
            None => String::new(),
        };

        // Filter out current agent
        let mut agent_list: Vec<Value> = Vec::new();
        if let Some(arr) = agents.as_array() {
            for agent in arr {
                let agent_id = match agent.get("id") {
                    Some(v) => v.as_str().unwrap_or(""),
                    None => "",
                };
                if agent_id != current_agent_id {
                    agent_list.push(agent.clone());
                }
            }
        }

        if agent_list.is_empty() {
            return Ok(json!({
                "agents": [],
                "message": "No other agents available for delegation."
            }));
        }

        Ok(json!({
            "agents": agent_list,
            "count": agent_list.len(),
            "message": format!("Found {} agent(s) available for delegation. Use delegate_to_agent with the agent's id.", agent_list.len())
        }))
    }
}

// ============================================================================
// CREATE AGENT TOOL
// ============================================================================

/// Tool for creating new AI agents
pub struct CreateAgentTool {
    /// File system context
    fs: Arc<dyn FileSystemContext>,
}

impl CreateAgentTool {
    /// Create a new create agent tool with file system context
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Tool for CreateAgentTool {
    fn name(&self) -> &str {
        "create_agent"
    }

    fn description(&self) -> &str {
        "Create a new AI agent with the specified configuration. The agent will be saved to the agents directory."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Agent identifier (kebab-case, e.g., 'my-agent')"
                },
                "displayName": {
                    "type": "string",
                    "description": "Human-readable display name (e.g., 'My Agent')"
                },
                "description": {
                    "type": "string",
                    "description": "Brief description of what this agent does"
                },
                "providerId": {
                    "type": "string",
                    "description": "Provider ID (must exist in providers.json)"
                },
                "model": {
                    "type": "string",
                    "description": "Model name (e.g., 'gpt-4o', 'claude-3-5-sonnet-20241022')"
                },
                "temperature": {
                    "type": "number",
                    "description": "Temperature (0.0-2.0, default 0.7)",
                    "default": 0.7
                },
                "maxTokens": {
                    "type": "integer",
                    "description": "Maximum tokens for response (default 2000)",
                    "default": 2000
                },
                "thinkingEnabled": {
                    "type": "boolean",
                    "description": "Enable extended thinking (for supported models)",
                    "default": false
                },
                "instructions": {
                    "type": "string",
                    "description": "System instructions for the agent"
                },
                "skills": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of skill IDs to include",
                    "default": []
                },
                "mcps": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of MCP server IDs to include",
                    "default": []
                }
            },
            "required": ["name", "displayName", "description", "providerId", "model", "instructions"]
        }))
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let name = args.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'name' parameter".to_string()))?;

        let display_name = args.get("displayName")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'displayName' parameter".to_string()))?;

        let description = args.get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'description' parameter".to_string()))?;

        let provider_id = args.get("providerId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'providerId' parameter".to_string()))?;

        let model = args.get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'model' parameter".to_string()))?;

        let instructions = args.get("instructions")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'instructions' parameter".to_string()))?;

        let temperature = args.get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.7);

        let max_tokens = args.get("maxTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(2000) as u32;

        let thinking_enabled = args.get("thinkingEnabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let skills: Vec<String> = args.get("skills")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        let mcps: Vec<String> = args.get("mcps")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        // Get agents directory from file system context
        let agents_dir = self.fs.agents_dir()
            .ok_or_else(|| zero_core::ZeroError::Tool("Agents directory not configured".to_string()))?;

        // Create agent directory
        let agent_dir = agents_dir.join(name);
        tokio::fs::create_dir_all(&agent_dir).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to create agent directory: {}", e)))?;

        // Create config.yaml
        let config = serde_json::json!({
            "name": name,
            "displayName": display_name,
            "description": description,
            "providerId": provider_id,
            "model": model,
            "temperature": temperature,
            "maxTokens": max_tokens,
            "thinkingEnabled": thinking_enabled,
            "skills": skills,
            "mcps": mcps,
        });

        let config_yaml = serde_yaml::to_string(&config)
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to serialize config: {}", e)))?;

        tokio::fs::write(agent_dir.join("config.yaml"), config_yaml).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to write config: {}", e)))?;

        // Create AGENTS.md
        let agents_md = format!("{}\n", instructions);
        tokio::fs::write(agent_dir.join("AGENTS.md"), agents_md).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to write AGENTS.md: {}", e)))?;

        tracing::info!("Created agent '{}' at {:?}", name, agent_dir);

        Ok(json!({
            "name": name,
            "displayName": display_name,
            "description": description,
            "providerId": provider_id,
            "model": model,
            "temperature": temperature,
            "maxTokens": max_tokens,
            "thinkingEnabled": thinking_enabled,
            "skills": skills,
            "mcps": mcps,
            "location": agent_dir.to_string_lossy().to_string(),
            "message": format!("Agent '{}' created successfully!", name)
        }))
    }
}
