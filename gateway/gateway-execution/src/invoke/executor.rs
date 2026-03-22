//! # Executor Builder
//!
//! Builds agent executors with all required components.

use gateway_services::agents::Agent;
use gateway_services::providers::Provider;
use gateway_services::{McpService, SkillService};
use agent_runtime::{
    AgentExecutor, DelegateTool, ExecutorConfig, LlmConfig, McpManager, MiddlewarePipeline,
    OpenAiClient, RespondTool, RetryPolicy, RetryingLlmClient, ToolRegistry,
};
use agent_tools::{core_tools, optional_tools, ListAgentsTool, QueryResourceTool, ToolSettings};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use zero_core::{ConnectorResourceProvider, FileSystemContext, MemoryFactStore};

use crate::config::GatewayFileSystem;

/// Workspace context cache type — same pattern as SkillService/ConnectorRegistry.
pub type WorkspaceCache = Arc<tokio::sync::RwLock<Option<HashMap<String, serde_json::Value>>>>;

/// Create an empty workspace cache.
pub fn new_workspace_cache() -> WorkspaceCache {
    Arc::new(tokio::sync::RwLock::new(None))
}

// ============================================================================
// EXECUTOR BUILDER
// ============================================================================

/// Builder for creating agent executors.
///
/// Encapsulates the complex setup process for creating an executor
/// with all required components (LLM client, tools, MCP, middleware).
pub struct ExecutorBuilder {
    config_dir: PathBuf,
    tool_settings: ToolSettings,
    workspace_cache: Option<WorkspaceCache>,
    fact_store: Option<Arc<dyn MemoryFactStore>>,
    connector_provider: Option<Arc<dyn ConnectorResourceProvider>>,
    llm_throttle: Option<Arc<tokio::sync::Semaphore>>,
    is_delegated: bool,
    extra_initial_state: Option<Vec<(String, serde_json::Value)>>,
}

impl ExecutorBuilder {
    /// Create a new executor builder.
    pub fn new(config_dir: PathBuf, tool_settings: ToolSettings) -> Self {
        Self {
            config_dir,
            tool_settings,
            workspace_cache: None,
            fact_store: None,
            connector_provider: None,
            llm_throttle: None,
            is_delegated: false,
            extra_initial_state: None,
        }
    }

    /// Set workspace cache for this builder.
    pub fn with_workspace_cache(mut self, cache: WorkspaceCache) -> Self {
        self.workspace_cache = Some(cache);
        self
    }

    /// Set the memory fact store for DB-backed save_fact/recall.
    pub fn with_fact_store(mut self, fact_store: Arc<dyn MemoryFactStore>) -> Self {
        self.fact_store = Some(fact_store);
        self
    }

    /// Set the connector resource provider for query_resource tool.
    pub fn with_connector_provider(mut self, provider: Arc<dyn ConnectorResourceProvider>) -> Self {
        self.connector_provider = Some(provider);
        self
    }

    /// Set the LLM throttle semaphore (shared per provider across all executors).
    pub fn with_llm_throttle(mut self, semaphore: Arc<tokio::sync::Semaphore>) -> Self {
        self.llm_throttle = Some(semaphore);
        self
    }

    /// Mark this executor as a delegated subagent (enables plan step cap).
    pub fn with_delegated(mut self, is_delegated: bool) -> Self {
        self.is_delegated = is_delegated;
        self
    }

    /// Add an initial state entry that will be injected into executor context.
    pub fn with_initial_state(mut self, key: &str, value: serde_json::Value) -> Self {
        self.extra_initial_state
            .get_or_insert_with(Vec::new)
            .push((key.to_string(), value));
        self
    }

    /// Build an executor for the given agent and provider.
    ///
    /// # Arguments
    /// * `agent` - The agent configuration
    /// * `provider` - The resolved provider
    /// * `conversation_id` - The conversation ID for this execution
    /// * `session_id` - The session ID for this execution
    /// * `available_agents` - List of available agents (for list_agents tool)
    /// * `available_skills` - List of available skills (for list_skills tool)
    /// * `hook_context` - Optional hook context for initial state
    /// * `mcp_service` - MCP service for starting servers
    /// * `ward_id` - Optional active ward from existing session
    pub async fn build(
        &self,
        agent: &Agent,
        provider: &Provider,
        conversation_id: &str,
        session_id: &str,
        available_agents: &[serde_json::Value],
        available_skills: &[serde_json::Value],
        hook_context: Option<&serde_json::Value>,
        mcp_service: &McpService,
        ward_id: Option<&str>,
    ) -> Result<AgentExecutor, String> {
        // Build executor config
        let mut executor_config = ExecutorConfig::new(
            agent.id.clone(),
            provider.id.clone().unwrap_or_else(|| provider.name.clone()),
            agent.model.clone(),
        );

        // Add hook context to initial state if present
        if let Some(hook_ctx) = hook_context {
            executor_config = executor_config.with_initial_state("hook_context", hook_ctx.clone());
        }

        // Cache available agents for list_agents tool
        if !available_agents.is_empty() {
            executor_config = executor_config
                .with_initial_state("available_agents", serde_json::Value::Array(available_agents.to_vec()));
        }

        // Cache available skills for list_skills tool
        if !available_skills.is_empty() {
            executor_config = executor_config
                .with_initial_state("available_skills", serde_json::Value::Array(available_skills.to_vec()));
        }

        // Load workspace context (from cache if available, otherwise disk)
        let workspace = if let Some(cache) = &self.workspace_cache {
            cache.read().await.clone()
        } else {
            load_workspace_from_disk(&self.config_dir)
        };
        if let Some(ws) = workspace {
            tracing::debug!("Loaded workspace context: {:?}", ws.keys().collect::<Vec<_>>());
            executor_config =
                executor_config.with_initial_state("workspace", serde_json::json!(ws));
        }

        // Inject session_id so tools (e.g., shell) can scope working directories
        executor_config = executor_config
            .with_initial_state("session_id", serde_json::Value::String(session_id.to_string()));

        // Restore ward_id from session so continuations keep the active ward
        if let Some(ward) = ward_id {
            executor_config = executor_config
                .with_initial_state("ward_id", serde_json::Value::String(ward.to_string()));
        }

        // Mark delegated executors so tools can enforce subagent constraints
        if self.is_delegated {
            executor_config = executor_config
                .with_initial_state("app:is_delegated", serde_json::Value::Bool(true));
        }

        // Inject extra initial state (e.g., ward_purpose, ward_structure from intent analysis)
        if let Some(entries) = &self.extra_initial_state {
            for (key, value) in entries {
                executor_config = executor_config.with_initial_state(key, value.clone());
            }
        }

        // Create LLM client using provider config
        let llm_config = LlmConfig::new(
            provider.base_url.clone(),
            provider.api_key.clone(),
            agent.model.clone(),
            provider.id.clone().unwrap_or_else(|| provider.name.clone()),
        )
        .with_temperature(agent.temperature)
        .with_max_tokens(agent.max_tokens)
        .with_thinking(agent.thinking_enabled);

        let raw_client: Arc<dyn agent_runtime::LlmClient> = Arc::new(
            OpenAiClient::new(llm_config)
                .map_err(|e| format!("Failed to create LLM client: {}", e))?,
        );

        // Wrap with retry logic: 3 retries, 500ms base delay, exponential backoff with jitter
        let retrying_client: Arc<dyn agent_runtime::LlmClient> =
            Arc::new(RetryingLlmClient::new(raw_client, RetryPolicy::default()));

        // Wrap with throttle if configured (limits concurrent calls per provider)
        let llm_client: Arc<dyn agent_runtime::LlmClient> = if let Some(ref sem) = self.llm_throttle {
            Arc::new(agent_runtime::ThrottledLlmClient::new(retrying_client, sem.clone()))
        } else {
            retrying_client
        };

        // Create file system context for tools
        let fs_context: Arc<dyn FileSystemContext> =
            Arc::new(GatewayFileSystem::new(self.config_dir.clone()));

        // Build tool registry
        let tool_registry = self.build_tool_registry(fs_context);

        // Build MCP manager
        let mcp_manager = self.build_mcp_manager(agent, mcp_service).await;

        // Create empty middleware pipeline
        let middleware_pipeline = Arc::new(MiddlewarePipeline::new());

        // Build final executor config with system instruction
        executor_config.system_instruction = Some(agent.instructions.clone());
        executor_config.conversation_id = Some(conversation_id.to_string());
        executor_config.temperature = agent.temperature;
        executor_config.max_tokens = agent.max_tokens;
        // Provider-level override takes precedence, then model lookup, then default
        executor_config.context_window_tokens = provider.context_window
            .unwrap_or_else(|| agent_runtime::middleware::token_counter::get_model_context_window(&agent.model) as u64);
        executor_config.mcps = agent.mcps.clone();

        // Configure tool result offload settings
        executor_config.offload_large_results = self.tool_settings.offload_large_results;
        executor_config.offload_threshold_chars = self.tool_settings.offload_threshold_tokens * 4;
        executor_config.offload_dir = Some(self.config_dir.join("temp"));

        AgentExecutor::new(
            executor_config,
            llm_client,
            tool_registry,
            mcp_manager,
            middleware_pipeline,
        )
        .map_err(|e| format!("Failed to create executor: {}", e))
    }

    /// Build the tool registry with core and optional tools.
    fn build_tool_registry(&self, fs_context: Arc<dyn FileSystemContext>) -> Arc<ToolRegistry> {
        let mut tool_registry = ToolRegistry::new();

        // Load core tools (always enabled, with optional DB-backed fact store)
        tool_registry.register_all(core_tools(fs_context.clone(), self.fact_store.clone()));

        // Load optional tools based on settings
        tool_registry.register_all(optional_tools(fs_context, &self.tool_settings));

        // Register action tools (respond, delegate, list_agents)
        tool_registry.register(Arc::new(RespondTool::new()));
        tool_registry.register(Arc::new(DelegateTool::new()));
        tool_registry.register(Arc::new(ListAgentsTool::new()));

        // Register connector resource query tool (if provider available)
        if let Some(provider) = &self.connector_provider {
            tool_registry.register(Arc::new(QueryResourceTool::new(provider.clone())));
        }

        Arc::new(tool_registry)
    }

    /// Build the MCP manager and start configured servers.
    async fn build_mcp_manager(&self, agent: &Agent, mcp_service: &McpService) -> Arc<McpManager> {
        let mcp_manager = Arc::new(McpManager::new());

        // Load and start MCP servers configured for this agent
        if !agent.mcps.is_empty() {
            let mcp_configs = mcp_service.get_multiple(&agent.mcps);
            for mcp_config in mcp_configs {
                let server_id = mcp_config.id();
                tracing::info!("Starting MCP server: {}", server_id);
                if let Err(e) = mcp_manager.start_server(mcp_config).await {
                    tracing::warn!("Failed to start MCP server {}: {}", server_id, e);
                }
            }
        }

        mcp_manager
    }
}

/// Helper to collect available agents summary for executor state.
pub async fn collect_agents_summary(
    agent_service: &gateway_services::AgentService,
) -> Vec<serde_json::Value> {
    match agent_service.list().await {
        Ok(all_agents) => all_agents
            .iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.id,
                    "name": a.display_name,
                    "description": a.description
                })
            })
            .collect(),
        Err(_) => vec![],
    }
}

/// Helper to collect available skills summary for executor state.
pub async fn collect_skills_summary(skill_service: &SkillService) -> Vec<serde_json::Value> {
    match skill_service.list().await {
        Ok(all_skills) => all_skills
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                })
            })
            .collect(),
        Err(_) => vec![],
    }
}

/// Load workspace context from shared memory.
///
/// Reads `agents_data/shared/workspace.json` and returns its contents
/// as a HashMap for injection into executor initial state.
fn load_workspace_from_disk(config_dir: &PathBuf) -> Option<HashMap<String, serde_json::Value>> {
    let workspace_path = config_dir
        .join("agents_data")
        .join("shared")
        .join("workspace.json");

    if !workspace_path.exists() {
        return None;
    }

    match std::fs::read_to_string(&workspace_path) {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(serde_json::Value::Object(obj)) => {
                    // Extract the "entries" field which contains the key-value pairs
                    if let Some(serde_json::Value::Object(entries)) = obj.get("entries") {
                        let workspace: HashMap<String, serde_json::Value> = entries
                            .iter()
                            .filter_map(|(k, v)| {
                                // Each entry has a "value" field with the actual data
                                v.get("value").map(|val| (k.clone(), val.clone()))
                            })
                            .collect();

                        if !workspace.is_empty() {
                            return Some(workspace);
                        }
                    }
                    None
                }
                Ok(_) => {
                    tracing::warn!("workspace.json is not a valid object");
                    None
                }
                Err(e) => {
                    tracing::warn!("Failed to parse workspace.json: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to read workspace.json: {}", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_workspace_context_missing_file() {
        let dir = TempDir::new().unwrap();
        let result = load_workspace_from_disk(&dir.path().to_path_buf());
        assert!(result.is_none());
    }

    #[test]
    fn test_load_workspace_context_with_data() {
        let dir = TempDir::new().unwrap();
        let shared_dir = dir.path().join("agents_data").join("shared");
        std::fs::create_dir_all(&shared_dir).unwrap();

        let workspace_data = serde_json::json!({
            "entries": {
                "working_dir": {
                    "value": "/home/user/projects/myproject",
                    "tags": [],
                    "created_at": "2024-02-04T10:00:00Z",
                    "updated_at": "2024-02-04T10:00:00Z"
                },
                "project_name": {
                    "value": "myproject",
                    "tags": ["active"],
                    "created_at": "2024-02-04T10:00:00Z",
                    "updated_at": "2024-02-04T10:00:00Z"
                }
            }
        });

        std::fs::write(
            shared_dir.join("workspace.json"),
            serde_json::to_string(&workspace_data).unwrap(),
        )
        .unwrap();

        let result = load_workspace_from_disk(&dir.path().to_path_buf());
        assert!(result.is_some());

        let workspace = result.unwrap();
        assert_eq!(workspace.len(), 2);
        assert_eq!(
            workspace.get("working_dir"),
            Some(&serde_json::json!("/home/user/projects/myproject"))
        );
        assert_eq!(
            workspace.get("project_name"),
            Some(&serde_json::json!("myproject"))
        );
    }

    #[test]
    fn test_load_workspace_context_empty_entries() {
        let dir = TempDir::new().unwrap();
        let shared_dir = dir.path().join("agents_data").join("shared");
        std::fs::create_dir_all(&shared_dir).unwrap();

        let workspace_data = serde_json::json!({
            "entries": {}
        });

        std::fs::write(
            shared_dir.join("workspace.json"),
            serde_json::to_string(&workspace_data).unwrap(),
        )
        .unwrap();

        let result = load_workspace_from_disk(&dir.path().to_path_buf());
        assert!(result.is_none());
    }

    #[test]
    fn test_load_workspace_context_invalid_json() {
        let dir = TempDir::new().unwrap();
        let shared_dir = dir.path().join("agents_data").join("shared");
        std::fs::create_dir_all(&shared_dir).unwrap();

        std::fs::write(shared_dir.join("workspace.json"), "not valid json").unwrap();

        let result = load_workspace_from_disk(&dir.path().to_path_buf());
        assert!(result.is_none());
    }
}
