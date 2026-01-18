// ============================================================================
// ZERO-APP INTEGRATED EXECUTOR
// New executor using the zero-app framework
// ============================================================================

//! Tauri-specific executor that integrates zero-app framework with the existing codebase

use std::sync::Arc;
use std::sync::Mutex;
use futures::StreamExt;
use async_trait::async_trait;

use zero_app::prelude::*;
use zero_app::{Tool, ToolContext, Result as ZeroResult, ZeroError, Toolset, MutexSession};
use crate::settings::AppDirs;
use crate::domains::agent_runtime::{
    config_adapter::{AgentYamlConfig, ConfigAdapter},
    middleware_integration::{MiddlewareFactory, MiddlewareExecutor, convert_middleware_config},
    filesystem::TauriFileSystemContext,
    McpManager,
};
use agent_tools::builtin_tools_with_fs;

// Type alias for Result with String error type (for Tauri compatibility)
type TResult<T> = std::result::Result<T, String>;

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
    pub middleware_config: Option<MiddlewareConfig>,
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
        let mut tool_registry = Self::create_tool_registry(dirs.clone(), &config.conversation_id)?;

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

        // Now wrap in Arc
        let tool_registry = Arc::new(tool_registry);

        // Create config adapter
        let adapter = ConfigAdapter::new(llm.clone(), tool_registry.clone());

        // Build the agent
        let agent = adapter.build_agent(&config.agent_config)?;

        // Create session using MutexSession for shared access
        let session = Arc::new(MutexSession::with_params(
            config.conversation_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            "agentzero".to_string(),
            "user".to_string(),
        ));

        // Create middleware executor with minimal pipeline
        let middleware_executor = Arc::new(MiddlewareExecutor::new(
            Arc::new(MiddlewarePipeline::new())
        ));

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
        // Apply middleware preprocessing if needed
        let user_content = Content::user(&user_message);

        // Add user content to session (synchronous)
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

            // Add assistant content to session (synchronous)
            if let Some(content) = &event.content {
                if content.role == "assistant" {
                    let mut session = self.session.lock().map_err(|e| format!("Session lock error: {}", e))?;
                    session.add_content(content.clone());
                }
            }

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
                    callback(stream_event);
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

    // ============================================================================
    // PRIVATE HELPERS
    // ============================================================================

    fn create_llm(config: &LlmConfig) -> TResult<Arc<dyn Llm>> {
        let openai_llm = OpenAiLlm::new(config.clone())
            .map_err(|e| format!("Failed to create LLM: {}", e.to_string()))?;
        Ok(Arc::new(openai_llm))
    }

    /// Create tool registry with builtin tools (returns non-Arc for mutation)
    fn create_tool_registry(_dirs: Arc<AppDirs>, conversation_id: &Option<String>) -> TResult<ToolRegistry> {
        // Get a fresh AppDirs instance since we can't clone from Arc
        let app_dirs = AppDirs::get().map_err(|e| e.to_string())?;

        // Create file system context
        let fs_context = if let Some(conv_id) = conversation_id {
            TauriFileSystemContext::new(app_dirs).with_conversation(conv_id.clone())
        } else {
            TauriFileSystemContext::new(app_dirs)
        };

        // Get tools from zerotools (now using zero_core::Tool)
        let tools = builtin_tools_with_fs(Arc::new(fs_context), conversation_id.clone());

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
}

impl ZeroAppStreamEvent {
    fn from_event(event: &Event) -> Self {
        if let Some(content) = &event.content {
            tracing::info!("from_event: content.role={}, parts.len()={}", content.role, content.parts.len());

            for (idx, part) in content.parts.iter().enumerate() {
                tracing::info!("from_event: part #{} = {:?}", idx, std::mem::discriminant(part));

                match part {
                    Part::Text { text } => {
                        tracing::info!("from_event: Text part with text.len()={}, text='{}'", text.len(), text);
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
                        if let Ok(response_str) = serde_json::to_string(response) {
                            tracing::info!("from_event: FunctionResponse id={}", id);
                            return Self::ToolResponse {
                                id: id.clone(),
                                response: response_str,
                            };
                        }
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
    actions: EventActions,
    ended: bool,
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
            actions: EventActions::default(),
            ended: false,
            run_config: RunConfig::default(),
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
        self.actions.clone()
    }

    fn set_actions(&self, actions: EventActions) {
        // In a real implementation, this would update the actions
        // For now, we'll just ignore it since actions is a clone
    }

    fn end_invocation(&self) {
        // Mark the invocation as ended
        // In a real implementation, this would set a flag
    }

    fn ended(&self) -> bool {
        self.ended
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
pub async fn create_zero_executor(
    agent_id: &str,
    conversation_id: Option<String>,
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
    let system_instruction = if agents_md_file.exists() {
        std::fs::read_to_string(&agents_md_file)
            .map_err(|e| format!("Failed to read AGENTS.md: {}", e))?
            .trim()
            .to_string()
    } else {
        String::new()
    };

    // Override system_instruction with AGENTS.md content
    let agent_config = AgentYamlConfig {
        system_instruction: if system_instruction.is_empty() { None } else { Some(system_instruction) },
        ..agent_config
    };

    // Load provider credentials
    let provider_id = agent_config.provider_id.as_ref()
        .ok_or_else(|| format!("Agent missing providerId"))?
        .clone();

    let (api_key, base_url) = load_provider_credentials(&provider_id).await?;

    // Create LLM config
    let llm_config = LlmConfig {
        api_key,
        base_url: Some(base_url),
        model: agent_config.model.clone().unwrap_or_else(|| "gpt-4".to_string()),
        organization_id: None, // Optional: can be added to config if needed
        temperature: agent_config.temperature.map(|t| t as f32),
        max_tokens: agent_config.max_tokens,
    };

    // Create middleware config
    let middleware_config = agent_config.middleware.as_ref()
        .and_then(|m| convert_middleware_config(Some(m)));

    // Create executor config
    let executor_config = ZeroExecutorConfig {
        agent_id: agent_id.to_string(),
        agent_config,
        provider_id: provider_id.clone(),
        llm_config,
        conversation_id,
        middleware_config,
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
