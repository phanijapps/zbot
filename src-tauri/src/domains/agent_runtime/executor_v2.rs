// ============================================================================
// ZERO-APP INTEGRATED EXECUTOR
// New executor using the zero-app framework
// ============================================================================

//! Tauri-specific executor that integrates zero-app framework with the existing codebase

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use futures::StreamExt;
use async_trait::async_trait;

use zero_app::prelude::*;
use zero_app::{Tool, ToolContext, Result as ZeroResult, ZeroError, Toolset, MutexSession};
use crate::settings::AppDirs;
use crate::domains::agent_runtime::{
    config_adapter::{AgentYamlConfig, ConfigAdapter},
    middleware_integration::{MiddlewareFactory, MiddlewareExecutor},
    filesystem::TauriFileSystemContext,
    McpManager,
    state_keys,
};
use crate::commands::agent_channels::SqliteSessionRepository;
use agent_tools::builtin_tools_with_fs;
use serde_json::json;

// Type alias for Result with String error type (for Tauri compatibility)
type TResult<T> = std::result::Result<T, String>;

// ============================================================================
// SYSTEM PROMPT TEMPLATE
// ============================================================================

/// Base system prompt template with tool usage guidelines
/// This is prepended to all agent instructions to ensure consistent behavior
const SYSTEM_PROMPT_TEMPLATE: &str = include_str!("../../../templates/system_prompt.md");

/// List of built-in tools (static, known at compile time)
const BUILTIN_TOOLS: &[&str] = &[
    "read", "write", "edit", "glob", "grep",
    "shell", "python", "load_skill",
    "request_input", "show_content", "create_agent",
    "list_entities", "search_entities", "get_entity_relationships",
    "add_entity", "add_relationship",
    "todos",
];

/// Build a complete system prompt by combining the template with agent-specific instructions
///
/// # Arguments
/// * `base_instructions` - The agent's AGENTS.md content
/// * `agent_name` - The agent's name/id for file paths
/// * `vault_path` - Path to the vault directory
/// * `mcp_tool_names` - Names of configured MCP tools
fn build_full_system_prompt(
    base_instructions: &str,
    agent_name: &str,
    vault_path: &str,
    mcp_tool_names: &[String],
) -> String {
    let mut prompt = SYSTEM_PROMPT_TEMPLATE.to_string();

    // Replace base instructions placeholder
    prompt = prompt.replace("{BASE_INSTRUCTIONS}", base_instructions);

    // Generate built-in tools XML list
    let tools_xml = BUILTIN_TOOLS
        .iter()
        .map(|t| format!("- {}", t))
        .collect::<Vec<_>>()
        .join("\n");
    prompt = prompt.replace("{AVAILABLE_TOOLS_XML}", &tools_xml);

    // Generate MCP tools XML list
    let mcp_tools_xml = if mcp_tool_names.is_empty() {
        "No MCP tools configured for this agent.".to_string()
    } else {
        mcp_tool_names
            .iter()
            .map(|t| format!("- {}", t))
            .collect::<Vec<_>>()
            .join("\n")
    };
    prompt = prompt.replace("{AVAILABLE_MCP_TOOLS_XML}", &mcp_tools_xml);

    // Replace vault path (and fix typo in template)
    prompt = prompt.replace("{vault}", vault_path);
    prompt = prompt.replace("{valut}", vault_path); // Fix typo in template

    // Replace agent name placeholder
    prompt = prompt.replace("<agent_name>", agent_name);

    prompt
}

// ============================================================================
// EXECUTOR CONFIG
// ============================================================================

/// Configuration for creating a ZeroAppExecutor
#[derive(Debug, Clone)]
pub struct ZeroExecutorConfig {
    pub agent_id: String,
    pub agent_config: AgentYamlConfig,
    pub provider_id: String,
    pub llm_config: LlmConfig,
    pub conversation_id: Option<String>,
    /// Skip loading conversation history from database (for subagents with fresh sessions)
    pub skip_history_load: bool,
    /// Root agent ID for data directory (subagents use parent's ID)
    /// If None, defaults to agent_id
    pub root_agent_id: Option<String>,
}

// ============================================================================
// MCP TOOL BRIDGE
// ============================================================================

/// Bridge between agent-runtime MCP tools and zero_core::Tool
///
/// This wraps MCP tools so they can be registered in the zero_app ToolRegistry.
struct McpToolBridge {
    /// Leaked string for 'static lifetime (required by Tool trait)
    qualified_name: &'static str,
    /// Leaked string for 'static lifetime (required by Tool trait)
    description: &'static str,
    /// Original server_id for tool execution
    server_id: String,
    /// Original tool name for tool execution
    tool_name: String,
    /// Parameters schema
    parameters_schema: Option<serde_json::Value>,
    /// MCP manager for executing tools
    mcp_manager: Arc<McpManager>,
}

impl McpToolBridge {
    fn new(
        server_id: String,
        tool: agent_runtime::mcp::McpTool,
        mcp_manager: Arc<McpManager>,
    ) -> Self {
        // Create qualified name and leak it for 'static lifetime
        let qualified_name = format!("{}__{}", server_id.replace('-', "_"), tool.name);
        let qualified_name = Box::leak(qualified_name.into_boxed_str());

        // Leak description for 'static lifetime
        let description = Box::leak(tool.description.into_boxed_str());

        Self {
            qualified_name,
            description,
            server_id,
            tool_name: tool.name,
            parameters_schema: tool.parameters,
            mcp_manager,
        }
    }
}

#[async_trait]
impl Tool for McpToolBridge {
    fn name(&self) -> &str {
        self.qualified_name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn parameters_schema(&self) -> Option<serde_json::Value> {
        self.parameters_schema.clone()
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: serde_json::Value) -> ZeroResult<serde_json::Value> {
        self.mcp_manager
            .execute_tool(&self.server_id, &self.tool_name, args)
            .await
            .map_err(|e| ZeroError::Tool(format!("MCP tool error: {}", e)))
    }
}

// ============================================================================
// EXECUTOR TOOL CONTEXT
// ============================================================================

/// Adapter that converts executor context to ToolContext for tool execution.
struct ExecutorToolContext {
    agent: Arc<dyn Agent>,
    session: Arc<MutexSession>,
    function_call_id: String,
    actions: std::sync::Mutex<EventActions>,
}

impl ExecutorToolContext {
    fn new(
        agent: Arc<dyn Agent>,
        session: Arc<MutexSession>,
        function_call_id: String,
    ) -> Self {
        Self {
            agent,
            session,
            function_call_id,
            actions: std::sync::Mutex::new(EventActions::default()),
        }
    }
}

impl ReadonlyContext for ExecutorToolContext {
    fn invocation_id(&self) -> &str {
        "executor_invocation"
    }

    fn agent_name(&self) -> &str {
        self.agent.name()
    }

    fn user_id(&self) -> &str {
        "user"
    }

    fn app_name(&self) -> &str {
        "agentzero"
    }

    fn session_id(&self) -> &str {
        static DEFAULT_ID: once_cell::sync::Lazy<&'static str> = once_cell::sync::Lazy::new(|| "unknown");
        &DEFAULT_ID
    }

    fn branch(&self) -> &str {
        "main"
    }

    fn user_content(&self) -> &zero_app::Content {
        static EMPTY_CONTENT: once_cell::sync::Lazy<Content> = once_cell::sync::Lazy::new(|| Content {
            role: String::new(),
            parts: Vec::new(),
        });
        &EMPTY_CONTENT
    }
}

impl CallbackContext for ExecutorToolContext {
    fn get_state(&self, key: &str) -> Option<serde_json::Value> {
        if let Ok(s) = self.session.lock() {
            s.state().get(key).cloned()
        } else {
            None
        }
    }

    fn set_state(&self, key: String, value: serde_json::Value) {
        if let Ok(mut s) = self.session.lock() {
            let _ = s.state_mut().set(key, value);
        }
    }
}

impl ToolContext for ExecutorToolContext {
    fn function_call_id(&self) -> &str {
        &self.function_call_id
    }

    fn actions(&self) -> EventActions {
        self.actions.lock().unwrap().clone()
    }

    fn set_actions(&self, actions: EventActions) {
        *self.actions.lock().unwrap() = actions;
    }
}

// ============================================================================
// ZERO APP EXECUTOR
// ============================================================================

/// Executor that uses zero-app framework
pub struct ZeroAppExecutor {
    config: ZeroExecutorConfig,
    agent: Arc<dyn Agent>,
    session: Arc<MutexSession>,
    llm: Arc<dyn Llm>,
    tool_registry: Arc<ToolRegistry>,
    mcp_manager: Arc<McpManager>,
    middleware_executor: Arc<MiddlewareExecutor>,
}

impl ZeroAppExecutor {
    /// Create a new executor from configuration
    pub async fn new(
        config: ZeroExecutorConfig,
        dirs: Arc<AppDirs>,
    ) -> TResult<Self> {
        // Create LLM client
        let llm = Self::create_llm(&config.llm_config)?;

        // Create tool registry with builtin tools (mutable)
        let mut tool_registry = Self::create_tool_registry(dirs.clone())?;

        // Create MCP manager
        let mcp_manager = Arc::new(McpManager::default());

        // Load and start MCP servers if configured
        let agent_mcps = config.agent_config.mcps.clone();
        if !agent_mcps.is_empty() {
            tracing::info!("Loading MCP servers for agent: {:?}", agent_mcps);
            if let Err(e) = load_and_start_mcp_servers(&mcp_manager, &agent_mcps, &dirs).await {
                tracing::warn!("Failed to load MCP servers: {}", e);
            } else {
                tracing::info!("Successfully loaded MCP servers");
            }
        }

        // Register MCP tools in the tool registry (before wrapping in Arc)
        Self::register_mcp_tools(&mut tool_registry, &mcp_manager, &agent_mcps).await;

        // Register subagent tools from .subagents/ folder
        Self::register_subagent_tools(&mut tool_registry, &config.agent_id, &dirs).await;

        // Now wrap in Arc
        let tool_registry = Arc::new(tool_registry);

        // Create config adapter
        let adapter = ConfigAdapter::new(llm.clone(), tool_registry.clone());

        // Build the agent
        let agent = adapter.build_agent(&config.agent_config)?;

        // Create session using MutexSession for shared access
        let conversation_id = config.conversation_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let session = Arc::new(MutexSession::with_params(
            conversation_id.clone(),
            "agentzero".to_string(),
            "user".to_string(),
        ));

        // Load conversation history and session state from SQLite
        let db_path = dirs.agent_channels_db_path();
        let repo = SqliteSessionRepository::new(db_path.clone())
            .map_err(|e| format!("Failed to create session repository: {}", e))?;

        // Load session state first (this contains things the agent learned)
        if let Ok(Some(saved_state)) = repo.load_session_state(&config.agent_id).await {
            tracing::info!("Loading saved session state for agent: {} ({} keys)",
                config.agent_id, saved_state.len());

            if let Ok(mut session) = session.lock().map_err(|e| format!("Session lock error: {}", e)) {
                // Merge saved state into session state
                for (key, value) in saved_state {
                    session.state_mut().set(key, value);
                }
            }
        } else {
            tracing::info!("No saved session state found for agent: {}", config.agent_id);
        }

        // Load conversation history from today's session (skip for subagents with fresh sessions)
        if !config.skip_history_load {
            match repo.load_conversation_history_into_session(&config.agent_id, &session).await {
                Ok(count) => {
                    tracing::info!("Loaded {} messages from history for agent: {}", count, config.agent_id);
                }
                Err(e) => {
                    tracing::warn!("Failed to load conversation history: {}", e);
                }
            }
        } else {
            tracing::info!("Skipping history load for subagent: {}", config.agent_id);
        }

        // Set conversation_id in session state so tools can access it
        {
            let mut session = session.lock().map_err(|e| format!("Session lock error: {}", e))?;
            // Only set if not already in state (don't override loaded state)
            if session.state().get(state_keys::state_keys::CONVERSATION_ID).is_none() {
                session.state_mut().set(
                    state_keys::state_keys::CONVERSATION_ID.to_string(),
                    json!(conversation_id),
                );
            }
            // Only set if not already in state (don't override loaded state)
            if session.state().get(state_keys::state_keys::AGENT_ID).is_none() {
                session.state_mut().set(
                    state_keys::state_keys::AGENT_ID.to_string(),
                    json!(config.agent_id),
                );
            }
            // Set root agent ID (determines data directory)
            // For subagents, this is the parent orchestrator's ID
            if session.state().get(state_keys::state_keys::ROOT_AGENT_ID).is_none() {
                let root_id = config.root_agent_id.as_ref().unwrap_or(&config.agent_id);
                session.state_mut().set(
                    state_keys::state_keys::ROOT_AGENT_ID.to_string(),
                    json!(root_id),
                );
            }
            // Only set if not already in state (don't override loaded state)
            if session.state().get(state_keys::state_keys::PROVIDER_ID).is_none() {
                session.state_mut().set(
                    state_keys::state_keys::PROVIDER_ID.to_string(),
                    json!(config.provider_id),
                );
            }
            // Set db_path for knowledge graph tools (always set, not in saved state)
            // Note: This is the path to the knowledge graph database, not the agent channels database
            let kg_db_path = dirs.db_dir.join("knowledge-graph.db");
            let kg_db_path_str = kg_db_path.to_string_lossy().to_string();
            session.state_mut().set(
                state_keys::state_keys::DB_PATH.to_string(),
                json!(kg_db_path_str),
            );
            tracing::info!("Session ready: conversation_id={}, agent_id={}, provider_id={}, kg_db_path={}",
                conversation_id, config.agent_id, config.provider_id, kg_db_path_str);
        }

        // Create middleware executor from config
        let factory = MiddlewareFactory::new(llm.clone(), config.provider_id.clone());
        let middleware_executor = factory.create_executor(config.agent_config.middleware.as_ref())
            .await
            .unwrap_or_else(|e| {
                // If middleware creation fails, use minimal pipeline
                tracing::warn!("Failed to create middleware from config: {}, using minimal pipeline", e);
                Arc::new(MiddlewareExecutor::new(Arc::new(MiddlewarePipeline::new())))
            });

        Ok(Self {
            config,
            agent,
            session,
            llm,
            tool_registry,
            mcp_manager,
            middleware_executor,
        })
    }

    /// Run the agent with a user message
    pub async fn run(&self, user_message: String) -> TResult<Vec<Event>> {
        // Add user message to session (history is already there from previous turns)
        let user_content = Content::user(&user_message);
        {
            let mut session = self.session.lock().map_err(|e| format!("Session lock error: {}", e))?;
            session.add_content(user_content.clone());
        }

        // Create invocation context with user content
        let ctx = self.create_invocation_context(user_content)?;

        // Run the agent
        let mut events = Vec::new();
        let mut stream = self.agent.run(ctx).await
            .map_err(|e| format!("Agent execution error: {}", e.to_string()))?;

        while let Some(event_result) = stream.next().await {
            let event = event_result.map_err(|e| format!("Event error: {}", e.to_string()))?;
            // Note: Don't add assistant content here - llm_agent.rs already does this
            // via ctx_clone.add_content() in the agent loop
            events.push(event);
        }

        Ok(events)
    }

    /// Stream the agent execution
    pub async fn run_stream(
        &self,
        user_message: String,
        mut callback: impl FnMut(ZeroAppStreamEvent),
    ) -> TResult<()> {
        tracing::info!("run_stream called with user_message: {}", user_message);

        // Add user message to session (synchronous)
        let user_content = Content::user(&user_message);
        {
            let mut session = self.session.lock().map_err(|e| format!("Session lock error: {}", e))?;
            session.add_content(user_content.clone());
        }

        // Create invocation context with user content
        let ctx = self.create_invocation_context(user_content)?;

        // Run the agent with streaming
        tracing::info!("Starting agent.run()");
        let mut stream = self.agent.run(ctx).await
            .map_err(|e| format!("Agent execution error: {}", e.to_string()))?;

        let mut event_count = 0;
        while let Some(event_result) = stream.next().await {
            event_count += 1;
            tracing::info!("Received event #{}", event_count);

            match event_result {
                Ok(event) => {
                    tracing::info!("Event: turn_complete={}, has_content={}", event.turn_complete, event.content.is_some());

                    // Convert event to stream event and emit it
                    let stream_event = ZeroAppStreamEvent::from_event(&event);
                    tracing::info!("Stream event: {:?}", std::mem::discriminant(&stream_event));

                    // Check if we need to emit an additional Complete event after the Content event
                    let needs_complete_event = event.turn_complete && matches!(stream_event, ZeroAppStreamEvent::Content { .. });

                    callback(stream_event);

                    // If the event has turn_complete=true and we emitted a Content event,
                    // also emit a Complete event so the frontend knows the turn is done
                    if needs_complete_event {
                        tracing::info!("Event has turn_complete=true, emitting additional Complete event");
                        callback(ZeroAppStreamEvent::Complete {
                            turn_complete: true,
                        });
                    }
                }
                Err(e) => {
                    tracing::error!("Event error: {}", e);
                    callback(ZeroAppStreamEvent::Error {
                        message: e.to_string(),
                    });
                    return Err(e.to_string());
                }
            }
        }

        tracing::info!("Agent run completed with {} events", event_count);

        // Check if execution was stopped by user request
        if self.is_stop_requested() {
            let iteration = self.get_iteration();
            tracing::info!("Execution was stopped at iteration {}", iteration);
            callback(ZeroAppStreamEvent::Stopped {
                iteration,
                reason: "User requested stop".to_string(),
            });
            // Clear the stop flag for next execution
            let _ = self.clear_stop();
        }

        Ok(())
    }

    /// Execute a tool by name with arguments
    async fn execute_tool(
        &self,
        ctx: &Arc<dyn ToolContext>,
        tool_name: &str,
        args: serde_json::Value,
    ) -> TResult<String> {
        // Get all tools from the registry
        let tools = self.tool_registry.tools().await
            .map_err(|e| format!("Failed to get tools: {}", e))?;

        // Find the tool by name
        let tool = tools.iter()
            .find(|t| t.name() == tool_name)
            .ok_or_else(|| format!("Tool not found: {}", tool_name))?;

        // Execute the tool
        let result = tool.execute(ctx.clone(), args).await
            .map_err(|e| format!("Tool execution error: {}", e))?;

        // Convert result to string
        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize tool result: {}", e))
    }

    /// Get the session
    pub fn session(&self) -> Arc<MutexSession> {
        self.session.clone()
    }

    /// Get the agent
    pub fn agent(&self) -> Arc<dyn Agent> {
        self.agent.clone()
    }

    /// Get the tool registry
    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }

    /// Get the LLM
    pub fn llm(&self) -> Arc<dyn Llm> {
        self.llm.clone()
    }

    /// Request the agent to stop execution
    ///
    /// This sets the stop flag in session state, which will be checked
    /// by the agent loop at the start of each iteration.
    pub fn request_stop(&self) -> TResult<()> {
        let mut session = self.session.lock()
            .map_err(|e| format!("Session lock error: {}", e))?;

        session.state_mut().set(
            state_keys::state_keys::EXECUTION_STOP.to_string(),
            json!(true),
        );

        tracing::info!("Stop requested for agent: {}", self.config.agent_id);
        Ok(())
    }

    /// Clear the stop flag (for resumption after continuation prompt)
    pub fn clear_stop(&self) -> TResult<()> {
        let mut session = self.session.lock()
            .map_err(|e| format!("Session lock error: {}", e))?;

        session.state_mut().set(
            state_keys::state_keys::EXECUTION_STOP.to_string(),
            json!(false),
        );

        Ok(())
    }

    /// Check if a stop has been requested
    pub fn is_stop_requested(&self) -> bool {
        if let Ok(session) = self.session.lock() {
            if let Some(stop_value) = session.state().get(state_keys::state_keys::EXECUTION_STOP) {
                return stop_value.as_bool().unwrap_or(false);
            }
        }
        false
    }

    /// Get the current iteration count
    pub fn get_iteration(&self) -> u32 {
        if let Ok(session) = self.session.lock() {
            if let Some(iter_value) = session.state().get(state_keys::state_keys::EXECUTION_ITERATION) {
                return iter_value.as_u64().unwrap_or(0) as u32;
            }
        }
        0
    }

    /// Get the agent ID
    pub fn agent_id(&self) -> &str {
        &self.config.agent_id
    }

    // ============================================================================
    // PRIVATE HELPERS
    // ============================================================================

    fn create_llm(config: &LlmConfig) -> TResult<Arc<dyn Llm>> {
        let openai_llm = OpenAiLlm::new(config.clone())
            .map_err(|e| format!("Failed to create LLM: {}", e.to_string()))?;
        Ok(Arc::new(openai_llm))
    }

    /// Create tool registry with builtin tools (returns non-Arc for mutation)
    fn create_tool_registry(_dirs: Arc<AppDirs>) -> TResult<ToolRegistry> {
        // Get a fresh AppDirs instance since we can't clone from Arc
        let app_dirs = AppDirs::get().map_err(|e| e.to_string())?;

        // Create file system context (no conversation_id needed - tools read from state)
        let fs_context = TauriFileSystemContext::new(app_dirs);

        // Get tools from agent-tools (conversation_id now read from session state by tools)
        let tools = builtin_tools_with_fs(Arc::new(fs_context));

        // Register tools
        let mut tool_registry = ToolRegistry::new();
        for tool in tools {
            tool_registry.register(tool);
        }

        Ok(tool_registry)
    }

    /// Register MCP tools from the MCP manager into the tool registry
    async fn register_mcp_tools(
        tool_registry: &mut ToolRegistry,
        mcp_manager: &Arc<McpManager>,
        agent_mcps: &[String],
    ) {
        if agent_mcps.is_empty() {
            return;
        }

        // List all tools from connected MCP servers
        let mcp_tools = match mcp_manager.list_all_tools().await {
            Ok(tools) => tools,
            Err(e) => {
                tracing::warn!("Failed to list MCP tools: {}", e);
                return;
            }
        };

        tracing::info!("Found {} MCP tools to register", mcp_tools.len());

        // Register each MCP tool
        for mcp_tool in mcp_tools {
            // Determine which server this tool belongs to
            // We need to find the server_id by checking which MCP server has this tool
            let server_id = Self::find_server_for_tool(mcp_manager, agent_mcps, &mcp_tool.name).await;

            if let Some(server_id) = server_id {
                let bridge = McpToolBridge::new(
                    server_id.clone(),
                    mcp_tool.clone(),
                    mcp_manager.clone(), // Clone the Arc
                );

                tracing::info!("Registering MCP tool: {} (from server: {})", bridge.qualified_name, server_id);

                // Register the tool
                tool_registry.register(Arc::new(bridge));
            }
        }
    }

    /// Find which MCP server provides a given tool
    async fn find_server_for_tool(
        mcp_manager: &Arc<McpManager>,
        agent_mcps: &[String],
        tool_name: &str,
    ) -> Option<String> {
        for mcp_id in agent_mcps {
            if let Some(client) = mcp_manager.get_client(mcp_id).await {
                if let Ok(tools) = client.list_tools().await {
                    if tools.iter().any(|t| t.name == tool_name) {
                        return Some(mcp_id.clone());
                    }
                }
            }
        }
        None
    }

    /// Register subagent tools from the .subagents folder
    ///
    /// Scans the .subagents directory for subagent configurations and
    /// registers each as a tool that the orchestrator can call.
    async fn register_subagent_tools(
        tool_registry: &mut ToolRegistry,
        agent_id: &str,
        dirs: &Arc<AppDirs>,
    ) {
        use super::subagent_tool::SubagentTool;

        let subagents_dir = dirs.config_dir.join("agents").join(agent_id).join(".subagents");

        if !subagents_dir.exists() {
            tracing::info!("No .subagents directory found for agent: {}", agent_id);
            return;
        }

        tracing::info!("Scanning for subagents in: {:?}", subagents_dir);

        let entries = match std::fs::read_dir(&subagents_dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("Failed to read .subagents directory: {}", e);
                return;
            }
        };

        let mut count = 0;
        for entry in entries.flatten() {
            let path = entry.path();

            // Skip if not a directory
            if !path.is_dir() {
                continue;
            }

            let config_path = path.join("config.yaml");
            if !config_path.exists() {
                continue;
            }

            // Parse config for description
            let (subagent_id, description) = match std::fs::read_to_string(&config_path) {
                Ok(config_content) => {
                    match ConfigAdapter::parse_config(&config_content) {
                        Ok(_agent_config) => {
                            let id = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            // AgentYamlConfig doesn't have description field,
                            // generate a default description from the name
                            let desc = format!("Subagent: {}", id);
                            (id, desc)
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse subagent config {:?}: {}", path, e);
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read subagent config {:?}: {}", path, e);
                    continue;
                }
            };

            // Create and register SubagentTool
            let tool = SubagentTool::new(
                agent_id.to_string(),
                subagent_id.clone(),
                description,
            );

            tracing::info!("Registering subagent tool: {}", subagent_id);
            tool_registry.register(Arc::new(tool));
            count += 1;
        }

        tracing::info!("Registered {} subagent tools for agent: {}", count, agent_id);
    }

    fn create_invocation_context(&self, user_content: Content) -> TResult<Arc<dyn InvocationContext>> {
        Ok(Arc::new(ZeroInvocationContext::new(
            self.agent.clone(),
            self.session.clone(),
            self.llm.clone(),
            self.tool_registry.clone(),
            user_content,
        )))
    }
}

// ============================================================================
// STREAM EVENTS
// ============================================================================

/// Stream event for zero-app executor
#[derive(Debug, Clone)]
pub enum ZeroAppStreamEvent {
    Content {
        delta: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    ToolResponse {
        id: String,
        response: String,
    },
    Complete {
        turn_complete: bool,
    },
    Error {
        message: String,
    },
    /// Iteration update event for tracking execution progress
    IterationUpdate {
        current: u32,
        max: u32,
    },
    /// Continuation prompt when max iterations reached (for main agent)
    ContinuationPrompt {
        iteration: u32,
        message: String,
    },
    /// Execution was stopped by user request
    Stopped {
        iteration: u32,
        reason: String,
    },
}

impl ZeroAppStreamEvent {
    fn from_event(event: &Event) -> Self {
        if let Some(content) = &event.content {
            tracing::info!("from_event: content.role={}, parts.len()={}, turn_complete={}", content.role, content.parts.len(), event.turn_complete);

            for (idx, part) in content.parts.iter().enumerate() {
                tracing::info!("from_event: part #{} = {:?}", idx, std::mem::discriminant(part));

                match part {
                    Part::Text { text } => {
                        // Truncate text in logs to avoid spamming console with full prompts/responses
                        let preview = if text.len() > 100 {
                            format!("{}... ({} chars total)", &text[..100], text.len())
                        } else {
                            format!("{} ({} chars)", text, text.len())
                        };
                        tracing::info!("from_event: Text part with text='{}'", preview);
                        return Self::Content {
                            delta: text.clone(),
                        };
                    }
                    Part::FunctionCall { id, name, args } => {
                        if let Ok(args_str) = serde_json::to_string(args) {
                            tracing::info!("from_event: FunctionCall id={}, name={}", id.as_ref().unwrap_or(&"?".to_string()), name);
                            return Self::ToolCall {
                                id: id.clone().unwrap_or_default(),
                                name: name.clone(),
                                arguments: args_str,
                            };
                        }
                    }
                    Part::FunctionResponse { id, response } => {
                        // response is already a JSON string from the tool, no need to serialize again
                        tracing::info!("from_event: FunctionResponse id={}, response.len={}", id, response.len());

                        return Self::ToolResponse {
                            id: id.clone(),
                            response: response.clone(),
                        };
                    }
                    // Binary parts are not yet supported in stream events
                    Part::Binary { .. } => {
                        tracing::info!("from_event: Binary part (not supported)");
                    }
                }
            }

            tracing::warn!("from_event: content has {} parts but no text/function data extracted", content.parts.len());
        } else {
            tracing::info!("from_event: event has no content");
        }

        Self::Complete {
            turn_complete: event.turn_complete,
        }
    }
}

// ============================================================================
// INVOCATION CONTEXT
// ============================================================================

/// Custom invocation context for zero-app executor
pub struct ZeroInvocationContext {
    agent: Arc<dyn Agent>,
    session: Arc<MutexSession>,
    llm: Arc<dyn Llm>,
    tool_registry: Arc<ToolRegistry>,
    user_content: Content, // Store user_content here to avoid reference issues
    actions: std::sync::Mutex<EventActions>,
    /// Flag to indicate the invocation should stop (thread-safe)
    ended: AtomicBool,
    /// Current iteration count (thread-safe for tracking)
    current_iteration: AtomicU32,
    run_config: RunConfig,
}

impl ZeroInvocationContext {
    fn new(
        agent: Arc<dyn Agent>,
        session: Arc<MutexSession>,
        llm: Arc<dyn Llm>,
        tool_registry: Arc<ToolRegistry>,
        user_content: Content, // Pass user_content during creation
    ) -> Self {
        Self {
            agent,
            session,
            llm,
            tool_registry,
            user_content,
            actions: std::sync::Mutex::new(EventActions::default()),
            ended: AtomicBool::new(false),
            current_iteration: AtomicU32::new(0),
            run_config: RunConfig::default(),
        }
    }

    /// Check if a stop has been requested via session state
    pub fn check_stop_requested(&self) -> bool {
        // First check the atomic ended flag
        if self.ended.load(Ordering::SeqCst) {
            return true;
        }

        // Also check the session state for stop signal
        if let Ok(s) = self.session.lock() {
            if let Some(stop_value) = s.state().get(state_keys::state_keys::EXECUTION_STOP) {
                if stop_value.as_bool().unwrap_or(false) {
                    // Set the atomic flag so subsequent checks are faster
                    self.ended.store(true, Ordering::SeqCst);
                    return true;
                }
            }
        }

        false
    }

    /// Get the current iteration count
    pub fn get_iteration(&self) -> u32 {
        self.current_iteration.load(Ordering::SeqCst)
    }

    /// Increment and return the new iteration count
    pub fn increment_iteration(&self) -> u32 {
        let new_val = self.current_iteration.fetch_add(1, Ordering::SeqCst) + 1;

        // Also update session state so frontend can track
        if let Ok(mut s) = self.session.lock() {
            let _ = s.state_mut().set(
                state_keys::state_keys::EXECUTION_ITERATION.to_string(),
                json!(new_val),
            );
        }

        new_val
    }

    /// Reset iteration count (for continuation after max iterations)
    pub fn reset_iteration(&self) {
        self.current_iteration.store(0, Ordering::SeqCst);
        if let Ok(mut s) = self.session.lock() {
            let _ = s.state_mut().set(
                state_keys::state_keys::EXECUTION_ITERATION.to_string(),
                json!(0),
            );
        }
    }
}

impl ReadonlyContext for ZeroInvocationContext {
    fn invocation_id(&self) -> &str {
        "invocation"
    }

    fn agent_name(&self) -> &str {
        self.agent.name()
    }

    fn user_id(&self) -> &str {
        "user"
    }

    fn app_name(&self) -> &str {
        "agentzero"
    }

    fn session_id(&self) -> &str {
        // Get session ID from the locked session
        // Since we need to return a reference, we need to clone the ID
        static DEFAULT_ID: once_cell::sync::Lazy<&'static str> = once_cell::sync::Lazy::new(|| "unknown");
        if let Ok(s) = self.session.lock() {
            // We need to return a static reference, so we use a workaround
            // In production, this would need architectural changes
            &DEFAULT_ID
        } else {
            &DEFAULT_ID
        }
    }

    fn branch(&self) -> &str {
        "main"
    }

    fn user_content(&self) -> &Content {
        &self.user_content
    }
}

impl CallbackContext for ZeroInvocationContext {
    fn get_state(&self, key: &str) -> Option<serde_json::Value> {
        if let Ok(s) = self.session.lock() {
            s.state().get(key).cloned()
        } else {
            None
        }
    }

    fn set_state(&self, key: String, value: serde_json::Value) {
        if let Ok(mut s) = self.session.lock() {
            let _ = s.state_mut().set(key, value);
        }
    }
}

impl InvocationContext for ZeroInvocationContext {
    fn agent(&self) -> Arc<dyn Agent> {
        self.agent.clone()
    }

    fn session(&self) -> Arc<dyn Session> {
        // Clone the Arc and coerce to trait object
        let session_arc: Arc<MutexSession> = Arc::clone(&self.session);
        session_arc as Arc<dyn Session>
    }

    fn run_config(&self) -> &RunConfig {
        &self.run_config
    }

    fn actions(&self) -> EventActions {
        self.actions.lock().unwrap().clone()
    }

    fn set_actions(&self, actions: EventActions) {
        *self.actions.lock().unwrap() = actions;
    }

    fn end_invocation(&self) {
        // Mark the invocation as ended using atomic store
        self.ended.store(true, Ordering::SeqCst);

        // Also set the state flag so other parts of the system can see it
        if let Ok(mut s) = self.session.lock() {
            let _ = s.state_mut().set(
                state_keys::state_keys::EXECUTION_STOP.to_string(),
                json!(true),
            );
        }
    }

    fn ended(&self) -> bool {
        // Check both the atomic flag and the state-based stop signal
        self.check_stop_requested()
    }

    fn add_content(&self, content: zero_app::Content) {
        // Use the MutexSession's add_content method
        self.session.add_content(content);
    }
}

// ============================================================================
// FACTORY FUNCTION
// ============================================================================

/// Create a zero-app executor from an agent ID and conversation ID
///
/// This is the main entry point for creating executors in the Tauri app
///
/// # Arguments
/// * `agent_id` - The ID of the agent to load
/// * `conversation_id` - Optional conversation/session ID
/// * `provider_id_override` - Optional override for the provider ID (instead of config)
/// * `model_override` - Optional override for the model (instead of config)
pub async fn create_zero_executor(
    agent_id: &str,
    conversation_id: Option<String>,
    provider_id_override: Option<&str>,
    model_override: Option<&str>,
) -> TResult<ZeroAppExecutor> {
    let dirs = Arc::new(AppDirs::get().map_err(|e| e.to_string())?);

    // Load agent configuration
    let agent_dir = dirs.config_dir.join("agents").join(agent_id);
    let config_file = agent_dir.join("config.yaml");

    if !config_file.exists() {
        return Err(format!("Agent config not found: {}", config_file.display()));
    }

    let config_content = std::fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read agent config: {}", e))?;

    // Parse config - read entire file, then parse
    let agent_config = ConfigAdapter::parse_config(&config_content)?;

    // Read AGENTS.md for system instruction
    let agents_md_file = agent_dir.join("AGENTS.md");
    let mut system_instruction = if agents_md_file.exists() {
        std::fs::read_to_string(&agents_md_file)
            .map_err(|e| format!("Failed to read AGENTS.md: {}", e))?
            .trim()
            .to_string()
    } else {
        String::new()
    };

    // Load skills and append their name/description to the system instruction
    // Full skill content is lazy-loaded via load_skill tool when needed
    let skills_dir = dirs.skills_dir.clone();
    for skill_id in &agent_config.skills {
        let skill_dir = skills_dir.join(skill_id);
        let skill_md_file = skill_dir.join("SKILL.md");

        if skill_md_file.exists() {
            match std::fs::read_to_string(&skill_md_file) {
                Ok(skill_content) => {
                    // Parse the YAML frontmatter to extract name and description
                    if let Some(pos) = skill_content.find("---") {
                        let frontmatter = &skill_content[0..pos].trim();
                        // Just add skill name/description - full content is lazy-loaded
                        system_instruction.push_str(&format!("\n\n## Available Skill: {}\nYAML: {}", skill_id, frontmatter.trim()));
                    } else {
                        // No frontmatter, just mention the skill exists
                        system_instruction.push_str(&format!("\n\n## Available Skill: {}\n(Use load_skill to load this skill)", skill_id));
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read skill {}: {}", skill_id, e);
                }
            }
        } else {
            tracing::warn!("Skill not found: {}", skill_id);
        }
    }

    // For agent-creator, inject available providers, skills, and MCPs into system instruction
    if agent_id == "agent-creator" {
        let available_context = load_available_context(&dirs).await.unwrap_or_default();
        system_instruction.push_str(&available_context);
    }

    // Build the full system prompt using the template
    // This wraps the agent's instructions with tool usage guidelines
    let vault_path = dirs.config_dir.to_string_lossy().to_string();
    let mcp_tool_names: Vec<String> = agent_config.mcps.iter().cloned().collect();
    let full_system_prompt = build_full_system_prompt(
        &system_instruction,
        agent_id,
        &vault_path,
        &mcp_tool_names,
    );

    // Override system_instruction with full template + AGENTS.md content + skills
    let agent_config = AgentYamlConfig {
        system_instruction: if full_system_prompt.is_empty() { None } else { Some(full_system_prompt) },
        ..agent_config
    };

    // Use provider override if provided, otherwise use config
    let provider_id = provider_id_override
        .map(|p| p.to_string())
        .or_else(|| agent_config.provider_id.clone())
        .ok_or_else(|| format!("Agent missing providerId"))?;

    let (api_key, base_url) = load_provider_credentials(&provider_id).await?;

    // Use model override if provided, otherwise use config
    let model = model_override
        .map(|m| m.to_string())
        .or_else(|| agent_config.model.clone())
        .unwrap_or_else(|| "gpt-4".to_string());

    // Create LLM config
    let llm_config = LlmConfig {
        api_key,
        base_url: Some(base_url),
        model,
        organization_id: None, // Optional: can be added to config if needed
        temperature: agent_config.temperature.map(|t| t as f32),
        max_tokens: agent_config.max_tokens,
    };

    // Create executor config
    let executor_config = ZeroExecutorConfig {
        agent_id: agent_id.to_string(),
        agent_config,
        provider_id: provider_id.clone(),
        llm_config,
        conversation_id,
        skip_history_load: false,
        root_agent_id: None,  // Regular agents use their own agent_id for data directory
    };

    ZeroAppExecutor::new(executor_config, dirs).await
}

/// Create a zero-app executor for a subagent with isolated context
///
/// This function creates a fresh executor for a subagent with:
/// - A NEW session (isolated from parent orchestrator)
/// - Injected context+task+goal in the system prompt
/// - No access to parent's conversation history
///
/// # Arguments
/// * `parent_agent_id` - The parent/orchestrator agent ID
/// * `subagent_id` - The subagent ID (folder name in .subagents/)
/// * `context` - Summary of relevant information from orchestrator
/// * `task` - Specific task for the subagent
/// * `goal` - Overall goal for context
pub async fn create_subagent_executor(
    parent_agent_id: &str,
    subagent_id: &str,
    context: String,
    task: String,
    goal: String,
) -> TResult<ZeroAppExecutor> {
    let dirs = Arc::new(AppDirs::get().map_err(|e| e.to_string())?);

    // Load subagent config from .subagents/{subagent_id}/
    let agent_dir = dirs.config_dir.join("agents").join(parent_agent_id).join(".subagents").join(subagent_id);
    let config_file = agent_dir.join("config.yaml");

    if !config_file.exists() {
        return Err(format!("Subagent config not found: {}", config_file.display()));
    }

    let config_content = std::fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read subagent config: {}", e))?;

    // Parse config
    let agent_config = ConfigAdapter::parse_config(&config_content)?;

    // Read AGENTS.md for system instruction
    let agents_md_file = agent_dir.join("AGENTS.md");
    let base_instruction = if agents_md_file.exists() {
        std::fs::read_to_string(&agents_md_file)
            .map_err(|e| format!("Failed to read AGENTS.md: {}", e))?
            .trim()
            .to_string()
    } else {
        String::new()
    };

    // Build the full system prompt using the template (includes tool usage guidelines)
    let vault_path = dirs.config_dir.to_string_lossy().to_string();
    let mcp_tool_names: Vec<String> = agent_config.mcps.iter().cloned().collect();
    let subagent_full_id = format!("{}.{}", parent_agent_id, subagent_id);
    let full_system_prompt = build_full_system_prompt(
        &base_instruction,
        &subagent_full_id,
        &vault_path,
        &mcp_tool_names,
    );

    // Inject context+task+goal into subagent's system instruction
    let enhanced_instruction = format!(
        "{}\n\n---\n\n## Context from Orchestrator\n{}\n\n## Your Task\n{}\n\n## Overall Goal\n{}",
        full_system_prompt, context, task, goal
    );

    // Override system_instruction with enhanced version
    let agent_config = AgentYamlConfig {
        system_instruction: if enhanced_instruction.is_empty() { None } else { Some(enhanced_instruction) },
        ..agent_config
    };

    // Use provider_id from subagent config
    let provider_id = agent_config.provider_id.clone()
        .ok_or_else(|| format!("Subagent missing providerId"))?;

    let (api_key, base_url) = load_provider_credentials(&provider_id).await?;

    // Use model from subagent config
    let model = agent_config.model.clone()
        .unwrap_or_else(|| "gpt-4".to_string());

    // Create LLM config
    let llm_config = LlmConfig {
        api_key,
        base_url: Some(base_url),
        model,
        organization_id: None,
        temperature: agent_config.temperature.map(|t| t as f32),
        max_tokens: agent_config.max_tokens,
    };

    // IMPORTANT: Create FRESH session (new conversation_id, no history from parent)
    // This ensures isolation - the subagent has no access to orchestrator's conversation
    let conversation_id = format!("subagent-{}-{}", parent_agent_id, subagent_id);

    // Create executor config with skip_history_load=true to ensure fresh session
    let executor_config = ZeroExecutorConfig {
        agent_id: format!("{}.{}", parent_agent_id, subagent_id),
        agent_config,
        provider_id: provider_id.clone(),
        llm_config,
        conversation_id: Some(conversation_id),
        skip_history_load: true,  // Subagents always start fresh
        root_agent_id: Some(parent_agent_id.to_string()),  // Use parent's data directory
    };

    ZeroAppExecutor::new(executor_config, dirs).await
}

async fn load_provider_credentials(provider_id: &str) -> TResult<(String, String)> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    let providers_file = dirs.config_dir.join("providers.json");

    let content = std::fs::read_to_string(&providers_file)
        .map_err(|e| format!("Failed to read providers file: {}", e))?;

    let providers: Vec<serde_json::Value> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse providers: {}", e))?;

    let provider = providers
        .into_iter()
        .find(|p| p.get("id").and_then(|i| i.as_str()) == Some(provider_id))
        .ok_or_else(|| format!("Provider not found: {}", provider_id))?;

    let api_key = provider.get("apiKey")
        .and_then(|k| k.as_str())
        .ok_or_else(|| format!("Provider missing apiKey"))?
        .to_string();

    let base_url = provider.get("baseUrl")
        .and_then(|u| u.as_str())
        .ok_or_else(|| format!("Provider missing baseUrl"))?
        .to_string();

    Ok((api_key, base_url))
}

/// Load available context (providers, skills, MCPs) for agent-creator
async fn load_available_context(dirs: &AppDirs) -> TResult<String> {
    let mut context = String::from("\n\n# Available Options\n\n");

    // Load providers
    let providers_file = dirs.config_dir.join("providers.json");
    if providers_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&providers_file) {
            if let Ok(providers) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                context.push_str("## Available Providers\n\n");
                for provider in providers {
                    let id = provider.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let name = provider.get("name").and_then(|v| v.as_str()).unwrap_or(id);
                    context.push_str(&format!("- **{}** (id: `{}`)\n", name, id));
                }
                context.push_str("\n");
            }
        }
    }

    // Load skills
    let skills_dir = dirs.skills_dir.clone();
    if skills_dir.exists() {
        context.push_str("## Available Skills\n\n");
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let skill_file = path.join("SKILL.md");
                    if skill_file.exists() {
                        if let Ok(content) = std::fs::read_to_string(&skill_file) {
                            // Extract name from frontmatter
                            let name = content.lines()
                                .skip_while(|line| !line.starts_with("name:"))
                                .next()
                                .and_then(|line| line.split(':').nth(1))
                                .map(|s| s.trim().trim_matches('\'').trim_matches('"').to_string())
                                .unwrap_or_else(|| path.file_name().unwrap_or_default().to_string_lossy().to_string());

                            let id = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                            context.push_str(&format!("- **{}** (id: `{}`)\n", name, id));
                        }
                    }
                }
            }
            context.push_str("\n");
        }
    }

    // Load MCP servers
    let mcps_file = dirs.config_dir.join("mcps.json");
    if mcps_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&mcps_file) {
            let mcps: Vec<serde_json::Value> = if content.trim().starts_with('[') {
                serde_json::from_str(&content).unwrap_or_default()
            } else {
                vec![serde_json::from_str(&content).unwrap_or_default()]
            };

            if !mcps.is_empty() {
                context.push_str("## Available MCP Servers\n\n");
                for mcp in mcps {
                    let id = mcp.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let name = mcp.get("name").and_then(|v| v.as_str()).unwrap_or(id);
                    context.push_str(&format!("- **{}** (id: `{}`)\n", name, id));
                }
                context.push_str("\n");
            }
        }
    }

    Ok(context)
}

/// Load and start MCP servers from the config file
pub async fn load_and_start_mcp_servers(
    mcp_manager: &McpManager,
    agent_mcps: &[String],
    dirs: &Arc<AppDirs>,
) -> TResult<()> {
    use agent_runtime::McpServerConfig;

    let mcp_file = dirs.config_dir.join("mcps.json");

    tracing::info!("Loading MCP servers from: {:?}", mcp_file);
    tracing::info!("Agent MCPs: {:?}", agent_mcps);

    if !mcp_file.exists() {
        tracing::info!("MCP servers file does not exist");
        return Ok(()); // No MCP servers configured
    }

    let content = std::fs::read_to_string(&mcp_file)
        .map_err(|e| format!("Failed to read MCP servers file: {}", e))?;

    // Support both array format and single object format
    let servers: Vec<McpServerConfig> = if content.trim().starts_with('[') {
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse MCP servers array: {}", e))?
    } else {
        // Single object - wrap in array
        let server: McpServerConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse MCP server: {}", e))?;
        vec![server]
    };

    tracing::info!("Found {} MCP servers in config", servers.len());

    for server_config in servers {
        let id = server_config.id();
        let name = server_config.name().to_string();
        let enabled = server_config.enabled();

        tracing::info!("Checking server: id={}, name={}, enabled={}", id, name, enabled);

        if agent_mcps.contains(&id) {
            // Start this MCP server since the agent explicitly uses it
            tracing::info!("Starting MCP server: {} (required by agent)", id);
            mcp_manager.start_server(server_config).await
                .map_err(|e| format!("Failed to start MCP server {}: {}", id, e))?;
        } else if enabled {
            // Also start if it's globally enabled
            tracing::info!("Starting MCP server: {} (globally enabled)", id);
            mcp_manager.start_server(server_config).await
                .map_err(|e| format!("Failed to start MCP server {}: {}", id, e))?;
        } else {
            tracing::info!("Skipping MCP server {} (not used by agent and not enabled)", id);
        }
    }

    Ok(())
}
