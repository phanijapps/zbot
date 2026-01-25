// ============================================================================
// CONFIG ADAPTER
// Converts YAML agent configurations to zero-app Agent instances
// ============================================================================

use std::sync::Arc;
use serde_yaml::Value as YamlValue;

use zero_app::prelude::*;
use crate::domains::agent_runtime::filesystem::TauriFileSystemContext;
use agent_tools::builtin_tools_with_fs;

// Type alias for Result with String error type (for Tauri compatibility)
type TResult<T> = std::result::Result<T, String>;

// ============================================================================
// DEFAULT FUNCTIONS
// ============================================================================

/// Default value for voiceRecordingEnabled (true = enabled by default)
fn default_voice_recording_enabled() -> Option<bool> {
    Some(true)
}

// ============================================================================
// AGENT CONFIG STRUCTURES (from YAML)
// ============================================================================

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AgentYamlConfig {
    pub name: String,
    #[serde(rename = "agentType")]
    pub agent_type: Option<String>,
    #[serde(rename = "providerId")]
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<u32>,
    #[serde(rename = "thinkingEnabled")]
    pub thinking_enabled: Option<bool>,
    #[serde(rename = "voiceRecordingEnabled", default = "default_voice_recording_enabled")]
    pub voice_recording_enabled: Option<bool>,
    #[serde(default)]
    pub mcps: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(rename = "subAgents", default)]
    pub sub_agents: Vec<AgentYamlConfig>,
    #[serde(rename = "sequentialConfig")]
    pub sequential_config: Option<SequentialConfig>,
    #[serde(rename = "parallelConfig")]
    pub parallel_config: Option<ParallelConfig>,
    #[serde(rename = "loopConfig")]
    pub loop_config: Option<LoopConfig>,
    #[serde(rename = "conditionalConfig")]
    pub conditional_config: Option<ConditionalConfig>,
    #[serde(rename = "llmConditionalConfig")]
    pub llm_conditional_config: Option<LlmConditionalConfig>,
    #[serde(rename = "customConfig")]
    pub custom_config: Option<CustomConfig>,
    #[serde(rename = "systemInstruction")]
    pub system_instruction: Option<String>,
    #[serde(default)]
    pub middleware: Option<MiddlewareYamlConfig>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SequentialConfig {
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ParallelConfig {
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LoopConfig {
    #[serde(rename = "maxIterations")]
    pub max_iterations: Option<u32>,
    #[serde(rename = "untilEscalation")]
    pub until_escalation: Option<bool>,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConditionalConfig {
    #[serde(default)]
    pub description: String,
    pub condition: String,
    #[serde(rename = "ifAgent")]
    pub if_agent: String,
    #[serde(rename = "elseAgent")]
    pub else_agent: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LlmConditionalConfig {
    #[serde(default)]
    pub description: String,
    pub instruction: String,
    pub routes: std::collections::HashMap<String, String>,
    #[serde(rename = "defaultRoute")]
    pub default_route: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CustomConfig {
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MiddlewareYamlConfig {
    pub summarization: Option<SummarizationYamlConfig>,
    #[serde(rename = "contextEditing")]
    pub context_editing: Option<ContextEditingYamlConfig>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SummarizationYamlConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<u32>,
    #[serde(rename = "triggerAt")]
    pub trigger_at: Option<u32>,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ContextEditingYamlConfig {
    pub enabled: Option<bool>,
    #[serde(rename = "keepLastN")]
    pub keep_last_n: Option<usize>,
    #[serde(rename = "keepPolicy")]
    pub keep_policy: Option<String>,
}

// ============================================================================
// CONFIG ADAPTER
// ============================================================================

pub struct ConfigAdapter {
    llm: Arc<dyn Llm>,
    tool_registry: Arc<ToolRegistry>,
}

impl ConfigAdapter {
    pub fn new(llm: Arc<dyn Llm>, tool_registry: Arc<ToolRegistry>) -> Self {
        Self { llm, tool_registry }
    }

    /// Parse agent config from YAML string
    pub fn parse_config(yaml: &str) -> TResult<AgentYamlConfig> {
        serde_yaml::from_str(yaml)
            .map_err(|e| format!("Failed to parse agent config: {}", e))
    }

    /// Build an agent from YAML config
    pub fn build_agent(&self, config: &AgentYamlConfig) -> TResult<Arc<dyn Agent>> {
        let agent_type = config.agent_type.as_deref().unwrap_or("llm");

        match agent_type {
            "llm" => self.build_llm_agent(config),
            "sequential" => self.build_sequential_agent(config),
            "parallel" => self.build_parallel_agent(config),
            "loop" => self.build_loop_agent(config),
            "conditional" => self.build_conditional_agent(config),
            "llm_conditional" => self.build_llm_conditional_agent(config),
            "custom" => self.build_custom_agent(config),
            _ => Err(format!("Unknown agent type: {}", agent_type)),
        }
    }

    /// Build a basic LLM agent
    fn build_llm_agent(&self, config: &AgentYamlConfig) -> TResult<Arc<dyn Agent>> {
        let mut builder = LlmAgentBuilder::new(&config.name, &config.name)
            .with_llm(self.llm.clone())
            .with_tools(self.tool_registry.clone());

        if let Some(instruction) = &config.system_instruction {
            builder = builder.with_system_instruction(instruction);
        }

        let agent = builder.build()
            .map_err(|e| format!("Failed to build LLM agent: {}", e.to_string()))?;

        Ok(Arc::new(agent))
    }

    /// Build a sequential agent (executes sub-agents in order)
    fn build_sequential_agent(&self, config: &AgentYamlConfig) -> TResult<Arc<dyn Agent>> {
        let sub_agents = config.sub_agents
            .iter()
            .map(|cfg| self.build_agent(cfg))
            .collect::<std::result::Result<Vec<_>, String>>()?;

        let mut agent = SequentialAgent::new(&config.name, sub_agents);

        if let Some(seq_config) = &config.sequential_config {
            if !seq_config.description.is_empty() {
                agent = agent.with_description(&seq_config.description);
            }
        }

        Ok(Arc::new(agent))
    }

    /// Build a parallel agent (executes sub-agents concurrently)
    fn build_parallel_agent(&self, config: &AgentYamlConfig) -> TResult<Arc<dyn Agent>> {
        let sub_agents = config.sub_agents
            .iter()
            .map(|cfg| self.build_agent(cfg))
            .collect::<std::result::Result<Vec<_>, String>>()?;

        let mut agent = ParallelAgent::new(&config.name, sub_agents);

        if let Some(par_config) = &config.parallel_config {
            if !par_config.description.is_empty() {
                agent = agent.with_description(&par_config.description);
            }
        }

        Ok(Arc::new(agent))
    }

    /// Build a loop agent (iterates sub-agents with exit conditions)
    fn build_loop_agent(&self, config: &AgentYamlConfig) -> TResult<Arc<dyn Agent>> {
        let sub_agents = config.sub_agents
            .iter()
            .map(|cfg| self.build_agent(cfg))
            .collect::<std::result::Result<Vec<_>, String>>()?;

        let mut agent = LoopAgent::new(&config.name, sub_agents);

        if let Some(loop_config) = &config.loop_config {
            if let Some(max_iterations) = loop_config.max_iterations {
                agent = agent.with_max_iterations(max_iterations);
            }

            if !loop_config.description.is_empty() {
                agent = agent.with_description(&loop_config.description);
            }
        }

        Ok(Arc::new(agent))
    }

    /// Build a conditional agent (rule-based routing)
    fn build_conditional_agent(&self, config: &AgentYamlConfig) -> TResult<Arc<dyn Agent>> {
        let cond_config = config.conditional_config.as_ref()
            .ok_or_else(|| "Conditional agent missing conditionalConfig")?;

        // Parse condition expression and build condition function
        let condition_expr = cond_config.condition.clone();
        let condition_fn = move |ctx: &dyn CallbackContext| -> bool {
            // Simple state-based condition evaluation
            // Supports: state.key == "value" or just state.key (truthy check)
            Self::evaluate_condition(&condition_expr, ctx)
        };

        // Build if_agent
        let if_agent = self.find_sub_agent_by_name(config, &cond_config.if_agent)?;

        // Build else_agent if specified
        let else_agent = if let Some(else_name) = &cond_config.else_agent {
            Some(self.find_sub_agent_by_name(config, else_name)?)
        } else {
            None
        };

        let mut agent = ConditionalAgent::new(&config.name, condition_fn, if_agent);

        if let Some(else_agt) = else_agent {
            agent = agent.with_else(else_agt);
        }

        if !cond_config.description.is_empty() {
            agent = agent.with_description(&cond_config.description);
        }

        Ok(Arc::new(agent))
    }

    /// Build an LLM conditional agent (LLM-based classification routing)
    fn build_llm_conditional_agent(&self, config: &AgentYamlConfig) -> TResult<Arc<dyn Agent>> {
        let llm_cond_config = config.llm_conditional_config.as_ref()
            .ok_or_else(|| "LLM conditional agent missing llmConditionalConfig")?;

        let mut builder = LlmConditionalAgent::builder(&config.name, self.llm.clone())
            .instruction(&llm_cond_config.instruction);

        // Add routes
        for (label, agent_name) in &llm_cond_config.routes {
            let agent = self.find_sub_agent_by_name(config, agent_name)?;
            builder = builder.route(label, agent);
        }

        // Add default route if specified
        if let Some(default_route) = &llm_cond_config.default_route {
            let agent = self.find_sub_agent_by_name(config, default_route)?;
            builder = builder.default_route(agent);
        }

        if !llm_cond_config.description.is_empty() {
            builder = builder.description(&llm_cond_config.description);
        }

        let agent = builder.build()
            .map_err(|e| format!("Failed to build LLM conditional agent: {}", e.to_string()))?;

        Ok(Arc::new(agent))
    }

    /// Build a custom agent (custom async logic)
    fn build_custom_agent(&self, config: &AgentYamlConfig) -> TResult<Arc<dyn Agent>> {
        // For custom agents, we'll create a basic LlmAgent as a placeholder
        // Real custom logic would need to be defined in code
        let mut builder = LlmAgentBuilder::new(&config.name, &config.name)
            .with_llm(self.llm.clone())
            .with_tools(self.tool_registry.clone());

        if let Some(instruction) = &config.system_instruction {
            builder = builder.with_system_instruction(instruction);
        }

        let agent = builder.build()
            .map_err(|e| format!("Failed to build custom agent: {}", e.to_string()))?;

        Ok(Arc::new(agent))
    }

    /// Find a sub-agent by name from the config's sub_agents list
    fn find_sub_agent_by_name(&self, config: &AgentYamlConfig, name: &str) -> TResult<Arc<dyn Agent>> {
        config.sub_agents
            .iter()
            .find(|sub_cfg| sub_cfg.name == name)
            .map(|sub_cfg| self.build_agent(sub_cfg))
            .unwrap_or_else(|| Err(format!("Sub-agent not found: {}", name)))
    }

    /// Evaluate a condition expression against callback context state
    fn evaluate_condition(expr: &str, ctx: &dyn CallbackContext) -> bool {
        // Simple condition evaluation
        // Supports: "state.key" (truthy check) or "state.key == 'value'"
        if expr.starts_with("state.") {
            let key = &expr[6..]; // Skip "state."

            if let Some(value) = ctx.get_state(key) {
                if let Some(bool_val) = value.as_bool() {
                    return bool_val;
                }
                if let Some(str_val) = value.as_str() {
                    return !str_val.is_empty();
                }
                if let Some(num_val) = value.as_i64() {
                    return num_val != 0;
                }
                return true; // Value exists, so condition is true
            }
            false
        } else {
            // For more complex expressions, we'd need a proper expression parser
            // For now, treat any non-empty expression as true
            !expr.is_empty()
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_llm_config() {
        let yaml = r#"
name: "test-agent"
agentType: "llm"
systemInstruction: "You are a helpful assistant"
"#;

        let config = ConfigAdapter::parse_config(yaml).unwrap();
        assert_eq!(config.name, "test-agent");
        assert_eq!(config.agent_type, Some("llm".to_string()));
        assert_eq!(config.system_instruction, Some("You are a helpful assistant".to_string()));
    }

    #[test]
    fn test_parse_sequential_config() {
        let yaml = r#"
name: "pipeline"
agentType: "sequential"
subAgents:
  - name: "step1"
    agentType: "llm"
  - name: "step2"
    agentType: "llm"
"#;

        let config = ConfigAdapter::parse_config(yaml).unwrap();
        assert_eq!(config.name, "pipeline");
        assert_eq!(config.agent_type, Some("sequential".to_string()));
        assert_eq!(config.sub_agents.len(), 2);
    }

    #[test]
    fn test_parse_conditional_config() {
        let yaml = r#"
name: "router"
agentType: "conditional"
conditionalConfig:
  condition: "state.is_premium"
  ifAgent: "premium_agent"
  elseAgent: "basic_agent"
"#;

        let config = ConfigAdapter::parse_config(yaml).unwrap();
        assert_eq!(config.name, "router");
        assert_eq!(config.agent_type, Some("conditional".to_string()));
        assert!(config.conditional_config.is_some());
    }
}
