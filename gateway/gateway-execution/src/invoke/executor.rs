//! # Executor Builder
//!
//! Builds agent executors with all required components.

use agent_runtime::{
    AgentExecutor, ContextEditingConfig, ContextEditingMiddleware, DelegateTool, ExecutorConfig,
    LlmConfig, McpManager, MiddlewarePipeline, OpenAiClient, PlanBlockMiddleware, RespondTool,
    RetryPolicy, RetryingLlmClient, ToolCallDecision, ToolRegistry,
};
use agent_tools::{
    EditFileTool,
    GlobTool,
    // Knowledge graph query tool
    GraphQueryTool,
    GrepTool,
    ListMcpsTool,
    ListSkillsTool,
    LoadSkillTool,
    // Root orchestrator tools
    MemoryTool,
    // Multimodal vision fallback
    MultimodalAnalyzeTool,
    QueryResourceTool,
    // Optional file reading tools
    ReadTool,
    SetSessionTitleTool,
    // Subagent tools
    ShellTool,
    ToolSettings,
    UpdatePlanTool,
    WardTool,
    WriteFileTool,
};
use gateway_services::agents::Agent;
use gateway_services::models::ModelRegistry;
use gateway_services::providers::Provider;
use gateway_services::{McpService, SettingsService, SkillService};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use zero_core::{ConnectorResourceProvider, FileSystemContext};
use zero_stores::MemoryFactStore;

use super::graph_adapter::GraphStorageAdapter;
use crate::config::GatewayFileSystem;
use knowledge_graph::GraphStorage;

/// Workspace context cache type — same pattern as SkillService/ConnectorRegistry.
pub type WorkspaceCache = Arc<tokio::sync::RwLock<Option<HashMap<String, serde_json::Value>>>>;

/// Create an empty workspace cache.
/// Resolve the effective thinking flag for an agent execution.
///
/// Previously this consulted `ModelRegistry.has_capability(model, Thinking)`
/// and silently disabled thinking on models the registry didn't know about.
/// That blocked users from using `thinkingEnabled=true` against any model
/// they typed into Settings > Advanced that wasn't in the curated registry
/// — effectively gating a user-visible setting on a local allowlist.
///
/// Current behaviour: trust the user-declared flag verbatim. If the
/// provider rejects the reasoning payload, the LLM client returns an
/// error that bubbles to the UI through the normal tool_error path.
/// The `_model` parameter is kept for future telemetry / logging without
/// changing the public signature.
pub fn resolve_thinking_flag(user_flag: bool, _model: &str) -> bool {
    user_flag
}

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
    rate_limiter: Option<Arc<agent_runtime::ProviderRateLimiter>>,
    model_registry: Option<Arc<ModelRegistry>>,
    is_delegated: bool,
    subagent_non_streaming: bool,
    graph_storage: Option<Arc<GraphStorage>>,
    ingestion_adapter: Option<Arc<dyn agent_tools::IngestionAccess>>,
    goal_adapter: Option<Arc<dyn agent_tools::GoalAccess>>,
    extra_initial_state: Option<Vec<(String, serde_json::Value)>>,
    chat_mode: bool,
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
            rate_limiter: None,
            model_registry: None,
            is_delegated: false,
            subagent_non_streaming: true,
            graph_storage: None,
            ingestion_adapter: None,
            goal_adapter: None,
            extra_initial_state: None,
            chat_mode: false,
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

    /// Set the shared rate limiter for this executor's provider.
    ///
    /// The limiter is shared across all executors using the same provider,
    /// so root and subagents respect the same concurrent-request and RPM limits.
    pub fn with_rate_limiter(mut self, limiter: Arc<agent_runtime::ProviderRateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Mark this executor as a delegated subagent (enables plan step cap).
    pub fn with_delegated(mut self, is_delegated: bool) -> Self {
        self.is_delegated = is_delegated;
        self
    }

    /// Set whether subagents use non-streaming requests.
    pub fn with_subagent_non_streaming(mut self, non_streaming: bool) -> Self {
        self.subagent_non_streaming = non_streaming;
        self
    }

    /// Set the model registry for capability lookups and context window resolution.
    pub fn with_model_registry(mut self, registry: Arc<ModelRegistry>) -> Self {
        self.model_registry = Some(registry);
        self
    }

    /// Set the knowledge graph storage for the graph_query tool.
    pub fn with_graph_storage(mut self, storage: Arc<GraphStorage>) -> Self {
        self.graph_storage = Some(storage);
        self
    }

    /// Set the ingestion access adapter for the `ingest` tool.
    pub fn with_ingestion_adapter(
        mut self,
        adapter: Arc<dyn agent_tools::IngestionAccess>,
    ) -> Self {
        self.ingestion_adapter = Some(adapter);
        self
    }

    /// Set the goal access adapter for the `goal` tool.
    pub fn with_goal_adapter(mut self, adapter: Arc<dyn agent_tools::GoalAccess>) -> Self {
        self.goal_adapter = Some(adapter);
        self
    }

    /// Enable chat mode (disables single_action_mode for multi-tool turns, larger
    /// middleware keep window, higher compaction warn threshold).
    pub fn with_chat_mode(mut self, chat_mode: bool) -> Self {
        self.chat_mode = chat_mode;
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
    #[allow(clippy::too_many_arguments)]
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
            executor_config = executor_config.with_initial_state(
                "available_agents",
                serde_json::Value::Array(available_agents.to_vec()),
            );
        }

        // Cache available skills for list_skills tool
        if !available_skills.is_empty() {
            executor_config = executor_config.with_initial_state(
                "available_skills",
                serde_json::Value::Array(available_skills.to_vec()),
            );
        }

        // Load workspace context (from cache if available, otherwise disk)
        let workspace = if let Some(cache) = &self.workspace_cache {
            cache.read().await.clone()
        } else {
            load_workspace_from_disk(&self.config_dir)
        };
        if let Some(ws) = workspace {
            tracing::debug!(
                "Loaded workspace context: {:?}",
                ws.keys().collect::<Vec<_>>()
            );
            executor_config =
                executor_config.with_initial_state("workspace", serde_json::json!(ws));
        }

        // Inject session_id so tools (e.g., shell) can scope working directories
        executor_config = executor_config.with_initial_state(
            "session_id",
            serde_json::Value::String(session_id.to_string()),
        );

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

        // Inject multimodal config for the multimodal_analyze tool
        let settings_service = SettingsService::new_legacy(self.config_dir.clone());
        if let Ok(settings) = settings_service.load() {
            let mm = &settings.execution.multimodal;
            if let (Some(provider_id), Some(model)) = (&mm.provider_id, &mm.model) {
                // Resolve the provider to get base_url and api_key
                let providers_path = self.config_dir.join("config/providers.json");
                let provider_creds = std::fs::read_to_string(&providers_path)
                    .ok()
                    .and_then(|content| {
                        serde_json::from_str::<Vec<serde_json::Value>>(&content).ok()
                    })
                    .and_then(|providers| {
                        providers
                            .into_iter()
                            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(provider_id))
                    });

                if let Some(prov) = provider_creds {
                    let base_url = prov.get("baseUrl").and_then(|v| v.as_str()).unwrap_or("");
                    let api_key = prov.get("apiKey").and_then(|v| v.as_str()).unwrap_or("");
                    executor_config = executor_config.with_initial_state(
                        "multimodal_config",
                        serde_json::json!({
                            "providerId": provider_id,
                            "model": model,
                            "temperature": mm.temperature,
                            "maxTokens": mm.max_tokens,
                            "baseUrl": base_url,
                            "apiKey": api_key,
                        }),
                    );
                }
            }
        }

        // User-driven: trust agent.thinking_enabled. If the provider
        // rejects the reasoning payload, the LLM client surfaces the error
        // through the normal tool_error path.
        let thinking_enabled = resolve_thinking_flag(agent.thinking_enabled, &agent.model);

        // Create LLM client using provider config
        let llm_config = LlmConfig::new(
            provider.base_url.clone(),
            provider.api_key.clone(),
            agent.model.clone(),
            provider.id.clone().unwrap_or_else(|| provider.name.clone()),
        )
        .with_temperature(agent.temperature)
        .with_max_tokens(agent.max_tokens)
        .with_thinking(thinking_enabled);

        let raw_client: Arc<dyn agent_runtime::LlmClient> = Arc::new(
            OpenAiClient::new(llm_config)
                .map_err(|e| format!("Failed to create LLM client: {}", e))?,
        );

        // Wrap with retry logic: 3 retries, 500ms base delay, exponential backoff with jitter
        let retrying_client: Arc<dyn agent_runtime::LlmClient> =
            Arc::new(RetryingLlmClient::new(raw_client, RetryPolicy::default()));

        // Wrap with shared rate limiter if configured (limits concurrent calls and RPM per provider)
        let llm_client: Arc<dyn agent_runtime::LlmClient> =
            if let Some(ref limiter) = self.rate_limiter {
                Arc::new(agent_runtime::RateLimitedLlmClient::new(
                    retrying_client,
                    limiter.clone(),
                ))
            } else {
                retrying_client
            };
        // Stream decode errors are handled by openai.rs fallback (stream error → retry non-streaming).
        // All agents stream — no NonStreamingLlmClient wrapper needed.

        // Create file system context for tools
        let fs_context: Arc<dyn FileSystemContext> =
            Arc::new(GatewayFileSystem::new(self.config_dir.clone()));

        // Build tool registry
        let tool_registry = self.build_tool_registry(fs_context);

        // Build MCP manager
        let mcp_manager = self.build_mcp_manager(agent, mcp_service).await;

        // Build final executor config with system instruction
        executor_config.system_instruction = Some(agent.instructions.clone());
        executor_config.conversation_id = Some(conversation_id.to_string());
        executor_config.temperature = agent.temperature;
        executor_config.max_tokens = agent.max_tokens;

        // Clamp max_tokens to model's actual output limit (prevents API errors)
        // Priority: provider model config → model registry → no clamping
        let mut clamped = false;

        // Check provider-level model config (user overrides)
        if let Some(provider_max) = provider.effective_max_output(&agent.model) {
            if provider_max > 0 && (executor_config.max_tokens as u64) > provider_max {
                tracing::warn!(
                    agent = %agent.id,
                    model = %agent.model,
                    requested = executor_config.max_tokens,
                    clamped_to = provider_max,
                    "Clamped max_tokens to provider model config limit"
                );
                executor_config.max_tokens = provider_max as u32;
                clamped = true;
            }
        }

        // Check model registry (bundled + local overrides)
        if !clamped {
            if let Some(ref registry) = self.model_registry {
                let model_output = registry.context_window(&agent.model).resolved_output();
                if model_output > 0 && (executor_config.max_tokens as u64) > model_output {
                    tracing::warn!(
                        agent = %agent.id,
                        model = %agent.model,
                        requested = executor_config.max_tokens,
                        clamped_to = model_output,
                        "Clamped max_tokens to model registry output limit"
                    );
                    executor_config.max_tokens = model_output as u32;
                }
            }
        }

        // Resolve context window: provider override > model registry > default 8192
        executor_config.context_window_tokens = provider.context_window.unwrap_or_else(|| {
            self.model_registry
                .as_ref()
                .map(|r| r.context_window(&agent.model).input)
                .unwrap_or(8192)
        });
        executor_config.mcps = agent.mcps.clone();

        // Create middleware pipeline with context editing
        // Must be after context_window_tokens is resolved (above)
        // Chat mode: trigger later (80%) but keep fewer results — conversations are long-running
        // Deep mode: trigger earlier (70%) and keep more results — tasks need full context
        let middleware_pipeline = {
            let pipeline = MiddlewarePipeline::new();
            let pipeline = if executor_config.context_window_tokens > 0 {
                let (trigger_pct, keep_results) = if self.chat_mode {
                    (80, 5) // Chat: 80% trigger, keep 5 recent tool results
                } else {
                    (70, 8) // Deep: 70% trigger, keep 8 recent tool results
                };
                pipeline.add_pre_processor(Box::new(ContextEditingMiddleware::new(
                    ContextEditingConfig {
                        enabled: true,
                        trigger_tokens: (executor_config.context_window_tokens as usize
                            * trigger_pct)
                            / 100,
                        keep_tool_results: keep_results,
                        min_reclaim: 500,
                        clear_tool_inputs: true,
                        cascade_unload: true,
                        skill_aware_placeholders: true,
                        ..Default::default()
                    },
                )))
            } else {
                pipeline
            };
            // Layer 1 (pinned plan anchor) runs AFTER context editing so
            // tool-result clearing happens first on the raw tape, then
            // the fresh plan block is re-inserted at a stable slot
            // behind the system prompt. The block's `is_summary = true`
            // flag keeps it out of any future compaction pass.
            let pipeline = pipeline.add_pre_processor(Box::new(PlanBlockMiddleware::new()));
            Arc::new(pipeline)
        };

        // Root is an orchestrator — enforce single action per turn (except chat mode)
        if !self.is_delegated && !self.chat_mode {
            executor_config.single_action_mode = true;
        }

        // Chat mode: nudge at 70% so agent saves facts before 80% middleware prune
        if self.chat_mode {
            executor_config.compaction_warn_pct = 70;
        }

        // Wire execution hooks for subagents (code-agent, research-agent, etc.)
        if self.is_delegated {
            // beforeToolCall: block shell-as-file-writer bypass
            executor_config.before_tool_call = Some(Arc::new(|tool_name, args| {
                if tool_name == "shell" {
                    let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                    // Block shell commands that create/write files — use write_file instead
                    if cmd.contains("> ")
                        || cmd.contains("cat <<")
                        || cmd.contains("heredoc")
                        || cmd.contains("echo \"") && cmd.contains("> ")
                        || cmd.contains("printf") && cmd.contains("> ")
                        || cmd.contains("tee ")
                    {
                        return ToolCallDecision::Block {
                            reason: "Use write_file to create files, not shell redirects. Shell is for running commands and reading output.".to_string()
                        };
                    }
                }
                ToolCallDecision::Allow
            }));

            // afterToolCall: inject guidance after errors to reduce fix-retry loops
            executor_config.after_tool_call = Some(Arc::new(
                |tool_name, _args, result, succeeded| {
                    if !succeeded && tool_name == "shell" {
                        Some(format!(
                        "{}\n\n[SYSTEM: Command failed. Read the error. Fix the ROOT CAUSE in your code, \
                         not the symptom. Do not retry the same command — fix the file first with edit_file.]",
                        result
                    ))
                    } else if !succeeded {
                        Some(format!(
                        "{}\n\n[SYSTEM: Tool failed. Read the error carefully before retrying.]",
                        result
                    ))
                    } else {
                        None // Pass through unchanged
                    }
                },
            ));
        }

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

        if self.is_delegated {
            // Subagents: execute + context awareness + respond.
            tool_registry.register(Arc::new(ShellTool::new()));
            {
                let mut wt = WriteFileTool::new(fs_context.clone());
                if let Some(fs) = self.fact_store.clone() {
                    wt = wt.with_fact_store(fs);
                }
                tool_registry.register(Arc::new(wt));
            }
            {
                let mut et = EditFileTool::new(fs_context.clone());
                if let Some(fs) = self.fact_store.clone() {
                    et = et.with_fact_store(fs);
                }
                tool_registry.register(Arc::new(et));
            }
            tool_registry.register(Arc::new(LoadSkillTool::new(fs_context.clone())));
            tool_registry.register(Arc::new(ListSkillsTool::new(fs_context.clone())));
            tool_registry.register(Arc::new(ListMcpsTool::new(fs_context.clone())));
            tool_registry.register(Arc::new(GrepTool));
            tool_registry.register(Arc::new(WardTool::new(
                fs_context.clone(),
                self.fact_store.clone(),
            )));
            tool_registry.register(Arc::new(MemoryTool::new(
                fs_context.clone(),
                self.fact_store.clone(),
            )));
            tool_registry.register(Arc::new(RespondTool::new()));
            tool_registry.register(Arc::new(MultimodalAnalyzeTool::new()));

            // Knowledge graph query (if storage available)
            if let Some(ref gs) = self.graph_storage {
                let adapter = Arc::new(GraphStorageAdapter::new(gs.clone()));
                tool_registry.register(Arc::new(GraphQueryTool::new(adapter)));
            }

            // Ingestion (if adapter wired)
            if let Some(ref a) = self.ingestion_adapter {
                tool_registry.register(Arc::new(agent_tools::IngestTool::new(a.clone())));
            }

            // Goal management (if adapter wired)
            if let Some(ref a) = self.goal_adapter {
                tool_registry.register(Arc::new(agent_tools::GoalTool::new(a.clone())));
            }
        } else {
            // Root agent: orchestrator tools only.
            // Root delegates — it doesn't do specialist work.
            // Excluded: load_skill, list_skills, list_agents, execution_graph

            // Orchestrator essentials
            tool_registry.register(Arc::new(ShellTool::new()));
            tool_registry.register(Arc::new(MemoryTool::new(
                fs_context.clone(),
                self.fact_store.clone(),
            )));
            tool_registry.register(Arc::new(WardTool::new(
                fs_context.clone(),
                self.fact_store.clone(),
            )));
            tool_registry.register(Arc::new(UpdatePlanTool::new()));
            tool_registry.register(Arc::new(SetSessionTitleTool::new()));
            tool_registry.register(Arc::new(GrepTool));

            // Delegation + response
            tool_registry.register(Arc::new(RespondTool::new()));
            tool_registry.register(Arc::new(DelegateTool::new()));
            tool_registry.register(Arc::new(MultimodalAnalyzeTool::new()));

            // Knowledge graph query (if storage available)
            if let Some(ref gs) = self.graph_storage {
                let adapter = Arc::new(GraphStorageAdapter::new(gs.clone()));
                tool_registry.register(Arc::new(GraphQueryTool::new(adapter)));
            }

            // Ingestion (if adapter wired)
            if let Some(ref a) = self.ingestion_adapter {
                tool_registry.register(Arc::new(agent_tools::IngestTool::new(a.clone())));
            }

            // Goal management (if adapter wired)
            if let Some(ref a) = self.goal_adapter {
                tool_registry.register(Arc::new(agent_tools::GoalTool::new(a.clone())));
            }

            // Optional file reading (root may need to review delegation results)
            if self.tool_settings.file_tools {
                tool_registry.register(Arc::new(ReadTool));
                tool_registry.register(Arc::new(GlobTool));
            }

            // Connector query (if provider available)
            if let Some(provider) = &self.connector_provider {
                tool_registry.register(Arc::new(QueryResourceTool::new(provider.clone())));
            }
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
fn load_workspace_from_disk(
    config_dir: &std::path::Path,
) -> Option<HashMap<String, serde_json::Value>> {
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
        let result = load_workspace_from_disk(dir.path());
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

        let result = load_workspace_from_disk(dir.path());
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

        let result = load_workspace_from_disk(dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_load_workspace_context_invalid_json() {
        let dir = TempDir::new().unwrap();
        let shared_dir = dir.path().join("agents_data").join("shared");
        std::fs::create_dir_all(&shared_dir).unwrap();

        std::fs::write(shared_dir.join("workspace.json"), "not valid json").unwrap();

        let result = load_workspace_from_disk(dir.path());
        assert!(result.is_none());
    }
}
