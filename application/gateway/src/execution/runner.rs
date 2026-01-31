//! # Execution Runner
//! # Execution Runner
//!
//! Manages agent execution and event streaming for the gateway.

use crate::database::ConversationRepository;
use crate::events::{EventBus, GatewayEvent};
use crate::hooks::HookContext;
use crate::services::providers::Provider;
use crate::services::{AgentService, McpService, ProviderService, SettingsService};
use agent_runtime::{
    AgentExecutor, ChatMessage, DelegateTool, ExecutorConfig, LlmConfig, McpManager,
    MiddlewarePipeline, OpenAiClient, RespondTool, StreamEvent, ToolRegistry,
};
use agent_tools::{core_tools, optional_tools, ListAgentsTool, ToolSettings};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use zero_core::FileSystemContext;

/// Request to spawn a delegated subagent
#[derive(Debug, Clone)]
pub struct DelegationRequest {
    pub parent_agent_id: String,
    pub parent_conversation_id: String,
    pub child_agent_id: String,
    pub task: String,
    pub context: Option<Value>,
}

// ============================================================================
// FILE SYSTEM CONTEXT FOR GATEWAY
// ============================================================================

/// File system context for gateway execution.
///
/// Provides paths to the agent tools based on the vault directory structure.
#[derive(Debug, Clone)]
struct GatewayFileSystem {
    /// Base vault/config directory
    vault_dir: PathBuf,
}

impl GatewayFileSystem {
    fn new(vault_dir: PathBuf) -> Self {
        Self { vault_dir }
    }
}

impl FileSystemContext for GatewayFileSystem {
    fn conversation_dir(&self, conversation_id: &str) -> Option<PathBuf> {
        Some(self.vault_dir.join("conversations").join(conversation_id))
    }

    fn outputs_dir(&self) -> Option<PathBuf> {
        Some(self.vault_dir.join("outputs"))
    }

    fn skills_dir(&self) -> Option<PathBuf> {
        Some(self.vault_dir.join("skills"))
    }

    fn agents_dir(&self) -> Option<PathBuf> {
        Some(self.vault_dir.join("agents"))
    }

    fn agent_data_dir(&self, agent_id: &str) -> Option<PathBuf> {
        Some(self.vault_dir.join("agents_data").join(agent_id))
    }

    fn python_executable(&self) -> Option<PathBuf> {
        // Use system Python - could be made configurable
        None
    }

    fn vault_path(&self) -> Option<PathBuf> {
        Some(self.vault_dir.clone())
    }
}

/// Configuration for agent execution.
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Agent ID to execute
    pub agent_id: String,
    /// Conversation ID for tracking
    pub conversation_id: String,
    /// Configuration directory (vault path)
    pub config_dir: PathBuf,
    /// Maximum iterations before prompting for continuation
    pub max_iterations: u32,
    /// Optional hook context for routing responses
    pub hook_context: Option<HookContext>,
}

impl ExecutionConfig {
    /// Create a new execution config.
    pub fn new(agent_id: String, conversation_id: String, config_dir: PathBuf) -> Self {
        Self {
            agent_id,
            conversation_id,
            config_dir,
            max_iterations: 25,
            hook_context: None,
        }
    }

    /// Set the hook context for routing responses.
    #[must_use]
    pub fn with_hook_context(mut self, hook_context: HookContext) -> Self {
        self.hook_context = Some(hook_context);
        self
    }
}

/// Handle to a running execution, allowing control operations.
#[derive(Clone)]
pub struct ExecutionHandle {
    /// Flag to signal stop
    stop_flag: Arc<AtomicBool>,
    /// Current iteration counter
    iteration: Arc<AtomicU32>,
    /// Maximum iterations
    max_iterations: Arc<AtomicU32>,
}

impl ExecutionHandle {
    fn new(max_iterations: u32) -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
            iteration: Arc::new(AtomicU32::new(0)),
            max_iterations: Arc::new(AtomicU32::new(max_iterations)),
        }
    }

    /// Request the execution to stop.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Check if stop was requested.
    pub fn is_stop_requested(&self) -> bool {
        self.stop_flag.load(Ordering::SeqCst)
    }

    /// Get current iteration.
    pub fn current_iteration(&self) -> u32 {
        self.iteration.load(Ordering::SeqCst)
    }

    /// Increment iteration counter.
    fn increment(&self) {
        self.iteration.fetch_add(1, Ordering::SeqCst);
    }

    /// Reset iteration counter.
    fn reset(&self) {
        self.iteration.store(0, Ordering::SeqCst);
    }

    /// Add more iterations for continuation.
    pub fn add_iterations(&self, additional: u32) {
        self.max_iterations.fetch_add(additional, Ordering::SeqCst);
        self.stop_flag.store(false, Ordering::SeqCst);
    }

    /// Get max iterations.
    pub fn max_iterations(&self) -> u32 {
        self.max_iterations.load(Ordering::SeqCst)
    }
}



// ============================================================================
// EXECUTION RUNNER
// ============================================================================

/// Execution runner that manages agent invocations.
pub struct ExecutionRunner {
    /// Event bus for broadcasting events
    event_bus: Arc<EventBus>,
    /// Agent service for loading agent configs
    agent_service: Arc<AgentService>,
    /// Provider service for loading provider configs
    provider_service: Arc<ProviderService>,
    /// MCP service for loading MCP server configs
    mcp_service: Arc<McpService>,
    /// Configuration directory
    config_dir: PathBuf,
    /// Active execution handles
    handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    /// Conversation repository for SQLite persistence
    conversation_repo: Arc<ConversationRepository>,
    /// Delegation registry for tracking parent-child relationships
    delegation_registry: Arc<super::DelegationRegistry>,
    /// Channel for delegation requests
    delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
}

impl ExecutionRunner {
    /// Create a new execution runner.
    pub fn new(
        event_bus: Arc<EventBus>,
        agent_service: Arc<AgentService>,
        provider_service: Arc<ProviderService>,
        config_dir: PathBuf,
        conversation_repo: Arc<ConversationRepository>,
        mcp_service: Arc<McpService>,
    ) -> Self {
        // Create channel for delegation requests
        let (delegation_tx, delegation_rx) = mpsc::unbounded_channel::<DelegationRequest>();

        let runner = Self {
            event_bus,
            agent_service,
            provider_service,
            mcp_service,
            config_dir,
            handles: Arc::new(RwLock::new(HashMap::new())),
            conversation_repo,
            delegation_registry: Arc::new(super::DelegationRegistry::new()),
            delegation_tx,
        };

        // Spawn delegation handler task
        runner.spawn_delegation_handler(delegation_rx);

        runner
    }

    /// Spawn a background task that processes delegation requests.
    fn spawn_delegation_handler(&self, mut rx: mpsc::UnboundedReceiver<DelegationRequest>) {
        let event_bus = self.event_bus.clone();
        let agent_service = self.agent_service.clone();
        let provider_service = self.provider_service.clone();
        let mcp_service = self.mcp_service.clone();
        let config_dir = self.config_dir.clone();
        let conversation_repo = self.conversation_repo.clone();
        let handles = self.handles.clone();
        let delegation_registry = self.delegation_registry.clone();
        let delegation_tx = self.delegation_tx.clone();

        tokio::spawn(async move {
            while let Some(request) = rx.recv().await {
                tracing::info!(
                    parent_agent = %request.parent_agent_id,
                    child_agent = %request.child_agent_id,
                    "Processing delegation request"
                );

                // Spawn the delegated agent using the standalone function
                if let Err(e) = spawn_delegated_agent(
                    &request,
                    event_bus.clone(),
                    agent_service.clone(),
                    provider_service.clone(),
                    mcp_service.clone(),
                    config_dir.clone(),
                    conversation_repo.clone(),
                    handles.clone(),
                    delegation_registry.clone(),
                    delegation_tx.clone(),
                )
                .await
                {
                    tracing::error!(
                        parent_agent = %request.parent_agent_id,
                        child_agent = %request.child_agent_id,
                        error = %e,
                        "Failed to spawn delegated agent"
                    );
                }
            }
        });
    }

    /// Invoke an agent with a message.
    ///
    /// Returns an execution handle for controlling the execution.
    pub async fn invoke(
        &self,
        config: ExecutionConfig,
        message: String,
    ) -> Result<ExecutionHandle, String> {
        let handle = ExecutionHandle::new(config.max_iterations);
        let handle_clone = handle.clone();

        // Store handle
        {
            let mut handles = self.handles.write().await;
            handles.insert(config.conversation_id.clone(), handle.clone());
        }

        // Emit start event
        self.event_bus
            .publish(GatewayEvent::AgentStarted {
                agent_id: config.agent_id.clone(),
                conversation_id: config.conversation_id.clone(),
            })
            .await;

        // Load agent configuration (or create default for "root" agent)
        let agent = match self.agent_service.get(&config.agent_id).await {
            Ok(a) => a,
            Err(_) if config.agent_id == "root" => {
                // Create a default root agent using the default provider
                let provider = match self.get_default_provider() {
                    Ok(p) => p,
                    Err(e) => {
                        self.emit_error(&config.conversation_id, &config.agent_id, &e).await;
                        return Err(e);
                    }
                };

                // Use first model from provider or default
                let model = provider.models.first()
                    .cloned()
                    .unwrap_or_else(|| "gpt-4o".to_string());

                crate::services::agents::Agent {
                    id: "root".to_string(),
                    name: "root".to_string(),
                    display_name: "Root Agent".to_string(),
                    description: "System root agent that handles all conversations".to_string(),
                    agent_type: Some("orchestrator".to_string()),
                    provider_id: provider.id.clone().unwrap_or_default(),
                    model,
                    temperature: 0.7,
                    max_tokens: 4096,
                    thinking_enabled: false,
                    voice_recording_enabled: false,
                    system_instruction: None,
                    instructions: crate::templates::default_system_prompt(),
                    mcps: vec![],
                    skills: vec![],
                    middleware: None,
                    created_at: None,
                }
            }
            Err(e) => {
                self.emit_error(&config.conversation_id, &config.agent_id, &e)
                    .await;
                return Err(e);
            }
        };

        // Get or create conversation in database
        let _ = self.conversation_repo.get_or_create_conversation(
            &config.conversation_id,
            &config.agent_id,
        );

        // Load conversation history from database
        let history: Vec<ChatMessage> = self
            .conversation_repo
            .get_recent_messages(&config.conversation_id, 50)
            .map(|messages| self.conversation_repo.messages_to_chat_format(&messages))
            .unwrap_or_default();

        // Create executor
        let executor = match self.create_executor(&agent, &config).await {
            Ok(e) => e,
            Err(e) => {
                self.emit_error(&config.conversation_id, &config.agent_id, &e)
                    .await;
                return Err(e);
            }
        };

        // Spawn execution task
        let event_bus = self.event_bus.clone();
        let agent_id = config.agent_id.clone();
        let conversation_id = config.conversation_id.clone();
        let conversation_repo = self.conversation_repo.clone();
        let delegation_tx = self.delegation_tx.clone();

        tokio::spawn(async move {
            let mut accumulated_response = String::new();

            // Execute with streaming
            let result = executor
                .execute_stream(&message, &history, |event| {
                    // Check for stop request
                    if handle_clone.is_stop_requested() {
                        return;
                    }

                    handle_clone.increment();

                    // Check for ActionDelegate events and send delegation requests
                    if let StreamEvent::ActionDelegate {
                        agent_id: delegate_agent,
                        task: delegate_task,
                        context: delegate_context,
                        ..
                    } = &event
                    {
                        let _ = delegation_tx.send(DelegationRequest {
                            parent_agent_id: agent_id.clone(),
                            parent_conversation_id: conversation_id.clone(),
                            child_agent_id: delegate_agent.clone(),
                            task: delegate_task.clone(),
                            context: delegate_context.clone(),
                        });
                    }

                    // Convert and broadcast event
                    let gateway_event =
                        convert_stream_event(event, &agent_id, &conversation_id);

                    // Accumulate response text
                    if let GatewayEvent::Token { delta, .. } = &gateway_event {
                        accumulated_response.push_str(delta);
                    }

                    // Broadcast event (fire and forget in sync context)
                    let event_bus = event_bus.clone();
                    let event = gateway_event.clone();
                    tokio::spawn(async move {
                        event_bus.publish(event).await;
                    });
                })
                .await;

            // Handle completion
            match result {
                Ok(()) => {
                    // Persist messages to SQLite
                    if let Err(e) = conversation_repo.add_message(
                        &conversation_id,
                        "user",
                        &message,
                        None,
                        None,
                    ) {
                        tracing::error!("Failed to save user message: {}", e);
                    }

                    if !accumulated_response.is_empty() {
                        if let Err(e) = conversation_repo.add_message(
                            &conversation_id,
                            "assistant",
                            &accumulated_response,
                            None,
                            None,
                        ) {
                            tracing::error!("Failed to save assistant message: {}", e);
                        }
                    }

                    // Emit completion
                    event_bus
                        .publish(GatewayEvent::AgentCompleted {
                            agent_id: agent_id.clone(),
                            conversation_id: conversation_id.clone(),
                            result: Some(accumulated_response),
                        })
                        .await;
                }
                Err(e) => {
                    event_bus
                        .publish(GatewayEvent::Error {
                            agent_id: Some(agent_id.clone()),
                            conversation_id: Some(conversation_id.clone()),
                            message: e.to_string(),
                        })
                        .await;
                }
            }

            // Check if stopped
            if handle_clone.is_stop_requested() {
                event_bus
                    .publish(GatewayEvent::AgentStopped {
                        agent_id,
                        conversation_id,
                        iteration: handle_clone.current_iteration(),
                    })
                    .await;
            }
        });

        Ok(handle)
    }

    /// Stop an execution by conversation ID.
    pub async fn stop(&self, conversation_id: &str) -> Result<(), String> {
        let handles = self.handles.read().await;
        if let Some(handle) = handles.get(conversation_id) {
            handle.stop();
            Ok(())
        } else {
            Err(format!("No active execution for conversation: {}", conversation_id))
        }
    }

    /// Continue an execution after max iterations.
    pub async fn continue_execution(
        &self,
        conversation_id: &str,
        additional_iterations: u32,
    ) -> Result<(), String> {
        let handles = self.handles.read().await;
        if let Some(handle) = handles.get(conversation_id) {
            handle.add_iterations(additional_iterations);
            Ok(())
        } else {
            Err(format!("No active execution for conversation: {}", conversation_id))
        }
    }

    /// Get execution handle for a conversation.
    pub async fn get_handle(&self, conversation_id: &str) -> Option<ExecutionHandle> {
        let handles = self.handles.read().await;
        handles.get(conversation_id).cloned()
    }

    /// Get the delegation registry.
    pub fn delegation_registry(&self) -> Arc<super::DelegationRegistry> {
        self.delegation_registry.clone()
    }

    /// Spawn a delegated subagent.
    ///
    /// This is called when an agent uses the delegate_to_agent tool.
    /// The subagent runs in a separate task with its own conversation.
    pub async fn spawn_delegation(
        &self,
        parent_agent_id: &str,
        parent_conversation_id: &str,
        child_agent_id: &str,
        task: &str,
        context: Option<Value>,
    ) -> Result<String, String> {
        // Generate child conversation ID
        let child_conversation_id = format!(
            "{}-sub-{}",
            parent_conversation_id,
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0")
        );

        // Register the delegation
        let delegation_context = super::DelegationContext::new(parent_agent_id, parent_conversation_id);
        let delegation_context = if let Some(ctx) = context {
            delegation_context.with_context(ctx)
        } else {
            delegation_context
        };
        self.delegation_registry.register(&child_conversation_id, delegation_context);

        // Create config for the child agent
        let config = ExecutionConfig::new(
            child_agent_id.to_string(),
            child_conversation_id.clone(),
            self.config_dir.clone(),
        );

        // Emit delegation started event
        self.event_bus
            .publish(GatewayEvent::DelegationStarted {
                parent_agent_id: parent_agent_id.to_string(),
                parent_conversation_id: parent_conversation_id.to_string(),
                child_agent_id: child_agent_id.to_string(),
                child_conversation_id: child_conversation_id.clone(),
                task: task.to_string(),
            })
            .await;

        // Spawn the child agent
        // Note: We pass the task as the message
        match self.invoke(config, task.to_string()).await {
            Ok(_handle) => {
                tracing::info!(
                    parent_agent = %parent_agent_id,
                    child_agent = %child_agent_id,
                    child_conversation = %child_conversation_id,
                    "Spawned delegated subagent"
                );
                Ok(child_conversation_id)
            }
            Err(e) => {
                // Remove from registry on failure
                self.delegation_registry.remove(&child_conversation_id);
                Err(e)
            }
        }
    }

    /// Get the default provider (marked as is_default) or fall back to first.
    fn get_default_provider(&self) -> Result<Provider, String> {
        let providers = self.provider_service.list()
            .map_err(|e| format!("Failed to list providers: {}", e))?;

        // First try to find the provider marked as default
        if let Some(default_provider) = providers.iter().find(|p| p.is_default).cloned() {
            return Ok(default_provider);
        }

        // Fall back to first provider
        providers.into_iter().next()
            .ok_or_else(|| "No providers configured. Add a provider in Integrations.".to_string())
    }

    /// Create an executor for the agent.
    async fn create_executor(
        &self,
        agent: &crate::services::agents::Agent,
        config: &ExecutionConfig,
    ) -> Result<AgentExecutor, String> {
        // Get the provider - use agent's provider_id or fall back to default
        let provider = if !agent.provider_id.is_empty() {
            self.provider_service.get(&agent.provider_id)
                .unwrap_or_else(|_| self.get_default_provider().unwrap())
        } else {
            self.get_default_provider()?
        };

        // Build executor config
        let mut executor_config = ExecutorConfig::new(
            agent.id.clone(),
            provider.id.clone().unwrap_or_else(|| provider.name.clone()),
            agent.model.clone(),
        );

        // Add hook context to initial state if present
        if let Some(hook_ctx) = &config.hook_context {
            if let Ok(hook_json) = serde_json::to_value(hook_ctx) {
                executor_config = executor_config.with_initial_state("hook_context", hook_json);
            }
        }

        // Cache available agents for list_agents tool
        if let Ok(all_agents) = self.agent_service.list().await {
            let agents_summary: Vec<serde_json::Value> = all_agents
                .iter()
                .map(|a| serde_json::json!({
                    "id": a.id,
                    "name": a.display_name,
                    "description": a.description
                }))
                .collect();
            executor_config = executor_config.with_initial_state(
                "available_agents",
                serde_json::Value::Array(agents_summary),
            );
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

        let llm_client = Arc::new(
            OpenAiClient::new(llm_config)
                .map_err(|e| format!("Failed to create LLM client: {}", e))?,
        );

        // Create file system context for tools
        let fs_context: Arc<dyn FileSystemContext> = Arc::new(GatewayFileSystem::new(
            config.config_dir.clone()
        ));

        // Load tool settings
        let settings_service = SettingsService::new(config.config_dir.clone());
        let tool_settings = settings_service.get_tool_settings().unwrap_or_default();

        // Load core tools (always enabled)
        let mut tool_registry = ToolRegistry::new();
        tool_registry.register_all(core_tools(fs_context.clone()));

        // Load optional tools based on settings
        tool_registry.register_all(optional_tools(fs_context, &tool_settings));

        // Register action tools (respond, delegate, list_agents)
        tool_registry.register(Arc::new(RespondTool::new()));
        tool_registry.register(Arc::new(DelegateTool::new()));
        tool_registry.register(Arc::new(ListAgentsTool::new()));

        let tool_registry = Arc::new(tool_registry);

        // MCP manager - start servers for this agent
        let mcp_manager = Arc::new(McpManager::new());

        // Load and start MCP servers configured for this agent
        if !agent.mcps.is_empty() {
            let mcp_configs = self.mcp_service.get_multiple(&agent.mcps);
            for mcp_config in mcp_configs {
                let server_id = mcp_config.id();
                tracing::info!("Starting MCP server: {}", server_id);
                if let Err(e) = mcp_manager.start_server(mcp_config).await {
                    tracing::warn!("Failed to start MCP server {}: {}", server_id, e);
                }
            }
        }

        // Create empty middleware pipeline
        let middleware_pipeline = Arc::new(MiddlewarePipeline::new());

        // Build executor config with system instruction
        let mut final_config = executor_config;
        final_config.system_instruction = Some(agent.instructions.clone());
        final_config.conversation_id = Some(config.conversation_id.clone());
        final_config.temperature = agent.temperature;
        final_config.max_tokens = agent.max_tokens;
        final_config.mcps = agent.mcps.clone(); // Set MCP IDs so executor can gather tools

        // Configure tool result offload settings
        final_config.offload_large_results = tool_settings.offload_large_results;
        final_config.offload_threshold_chars = tool_settings.offload_threshold_tokens * 4; // tokens to chars
        final_config.offload_dir = Some(config.config_dir.join("temp"));

        AgentExecutor::new(
            final_config,
            llm_client,
            tool_registry,
            mcp_manager,
            middleware_pipeline,
        )
        .map_err(|e| format!("Failed to create executor: {}", e))
    }

    /// Emit an error event.
    async fn emit_error(&self, conversation_id: &str, agent_id: &str, message: &str) {
        self.event_bus
            .publish(GatewayEvent::Error {
                agent_id: Some(agent_id.to_string()),
                conversation_id: Some(conversation_id.to_string()),
                message: message.to_string(),
            })
            .await;
    }
}

// ============================================================================
// STANDALONE DELEGATION SPAWNER
// ============================================================================

/// Spawn a delegated agent.
///
/// This is a standalone function that creates and runs a delegated agent
/// without needing a reference to `ExecutionRunner`.
async fn spawn_delegated_agent(
    request: &DelegationRequest,
    event_bus: Arc<EventBus>,
    agent_service: Arc<AgentService>,
    provider_service: Arc<ProviderService>,
    mcp_service: Arc<McpService>,
    config_dir: PathBuf,
    conversation_repo: Arc<ConversationRepository>,
    handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    delegation_registry: Arc<super::DelegationRegistry>,
    delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
) -> Result<String, String> {
    // Generate child conversation ID
    let child_conversation_id = format!(
        "{}-sub-{}",
        request.parent_conversation_id,
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0")
    );

    // Register the delegation
    let delegation_context = super::DelegationContext::new(
        &request.parent_agent_id,
        &request.parent_conversation_id,
    );
    let delegation_context = if let Some(ctx) = request.context.clone() {
        delegation_context.with_context(ctx)
    } else {
        delegation_context
    };
    delegation_registry.register(&child_conversation_id, delegation_context);

    // Emit delegation started event
    event_bus
        .publish(GatewayEvent::DelegationStarted {
            parent_agent_id: request.parent_agent_id.clone(),
            parent_conversation_id: request.parent_conversation_id.clone(),
            child_agent_id: request.child_agent_id.clone(),
            child_conversation_id: child_conversation_id.clone(),
            task: request.task.clone(),
        })
        .await;

    // Load agent configuration
    let agent = match agent_service.get(&request.child_agent_id).await {
        Ok(a) => a,
        Err(e) => {
            delegation_registry.remove(&child_conversation_id);
            return Err(format!("Failed to load agent {}: {}", request.child_agent_id, e));
        }
    };

    // Get provider
    let provider = if !agent.provider_id.is_empty() {
        provider_service.get(&agent.provider_id)
            .map_err(|e| format!("Failed to get provider: {}", e))?
    } else {
        // Get default provider
        let providers = provider_service.list()
            .map_err(|e| format!("Failed to list providers: {}", e))?;
        providers.into_iter()
            .find(|p| p.is_default)
            .or_else(|| provider_service.list().ok()?.into_iter().next())
            .ok_or_else(|| "No provider configured".to_string())?
    };

    // Build executor config
    let mut executor_config = ExecutorConfig::new(
        agent.id.clone(),
        provider.id.clone().unwrap_or_else(|| provider.name.clone()),
        agent.model.clone(),
    );

    // Cache available agents for list_agents tool
    if let Ok(all_agents) = agent_service.list().await {
        let agents_summary: Vec<serde_json::Value> = all_agents
            .iter()
            .map(|a| serde_json::json!({
                "id": a.id,
                "name": a.display_name,
                "description": a.description
            }))
            .collect();
        executor_config = executor_config.with_initial_state(
            "available_agents",
            serde_json::Value::Array(agents_summary),
        );
    }

    // Create LLM client
    let llm_config = LlmConfig::new(
        provider.base_url.clone(),
        provider.api_key.clone(),
        agent.model.clone(),
        provider.id.clone().unwrap_or_else(|| provider.name.clone()),
    )
    .with_temperature(agent.temperature)
    .with_max_tokens(agent.max_tokens)
    .with_thinking(agent.thinking_enabled);

    let llm_client = Arc::new(
        OpenAiClient::new(llm_config)
            .map_err(|e| format!("Failed to create LLM client: {}", e))?,
    );

    // Create file system context for tools
    let fs_context: Arc<dyn FileSystemContext> = Arc::new(GatewayFileSystem::new(config_dir.clone()));

    // Load tool settings
    let settings_service = SettingsService::new(config_dir.clone());
    let tool_settings = settings_service.get_tool_settings().unwrap_or_default();

    // Load core tools (always enabled)
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register_all(core_tools(fs_context.clone()));

    // Load optional tools based on settings
    tool_registry.register_all(optional_tools(fs_context, &tool_settings));

    // Register action tools (respond, delegate, list_agents)
    tool_registry.register(Arc::new(RespondTool::new()));
    tool_registry.register(Arc::new(DelegateTool::new()));
    tool_registry.register(Arc::new(ListAgentsTool::new()));

    let tool_registry = Arc::new(tool_registry);

    // MCP manager
    let mcp_manager = Arc::new(McpManager::new());

    // Load and start MCP servers configured for this agent
    if !agent.mcps.is_empty() {
        let mcp_configs = mcp_service.get_multiple(&agent.mcps);
        for mcp_config in mcp_configs {
            let server_id = mcp_config.id();
            if let Err(e) = mcp_manager.start_server(mcp_config).await {
                tracing::warn!("Failed to start MCP server {}: {}", server_id, e);
            }
        }
    }

    // Create middleware pipeline
    let middleware_pipeline = Arc::new(MiddlewarePipeline::new());

    // Build final config
    executor_config.system_instruction = Some(agent.instructions.clone());
    executor_config.conversation_id = Some(child_conversation_id.clone());
    executor_config.temperature = agent.temperature;
    executor_config.max_tokens = agent.max_tokens;
    executor_config.mcps = agent.mcps.clone(); // Set MCP IDs so executor can gather tools

    // Configure tool result offload settings
    executor_config.offload_large_results = tool_settings.offload_large_results;
    executor_config.offload_threshold_chars = tool_settings.offload_threshold_tokens * 4; // tokens to chars
    executor_config.offload_dir = Some(config_dir.join("temp"));

    // Create executor
    let executor = AgentExecutor::new(
        executor_config,
        llm_client,
        tool_registry,
        mcp_manager,
        middleware_pipeline,
    )
    .map_err(|e| format!("Failed to create executor: {}", e))?;

    // Create execution handle
    let handle = ExecutionHandle::new(20);
    let handle_clone = handle.clone();

    // Store handle
    {
        let mut handles_guard = handles.write().await;
        handles_guard.insert(child_conversation_id.clone(), handle.clone());
    }

    // Create conversation in database for the subagent
    let _ = conversation_repo.get_or_create_conversation(
        &child_conversation_id,
        &request.child_agent_id,
    );

    // Spawn the child agent execution
    let agent_id = request.child_agent_id.clone();
    let conv_id = child_conversation_id.clone();
    let task_msg = request.task.clone();
    let parent_agent = request.parent_agent_id.clone();
    let parent_conv = request.parent_conversation_id.clone();

    tokio::spawn(async move {
        let mut accumulated_response = String::new();

        let result = executor
            .execute_stream(&task_msg, &[], |event| {
                if handle_clone.is_stop_requested() {
                    return;
                }

                handle_clone.increment();

                // Check for ActionDelegate events and send delegation requests
                if let StreamEvent::ActionDelegate {
                    agent_id: delegate_agent,
                    task: delegate_task,
                    context: delegate_context,
                    ..
                } = &event
                {
                    let _ = delegation_tx.send(DelegationRequest {
                        parent_agent_id: agent_id.clone(),
                        parent_conversation_id: conv_id.clone(),
                        child_agent_id: delegate_agent.clone(),
                        task: delegate_task.clone(),
                        context: delegate_context.clone(),
                    });
                }

                let gateway_event = convert_stream_event(event, &agent_id, &conv_id);

                if let GatewayEvent::Token { delta, .. } = &gateway_event {
                    accumulated_response.push_str(delta);
                }

                let event_bus = event_bus.clone();
                let event = gateway_event.clone();
                tokio::spawn(async move {
                    event_bus.publish(event).await;
                });
            })
            .await;

        match result {
            Ok(()) => {
                // Save messages
                if let Err(e) = conversation_repo.add_message(
                    &conv_id,
                    "user",
                    &task_msg,
                    None,
                    None,
                ) {
                    tracing::error!("Failed to save task message: {}", e);
                }

                if !accumulated_response.is_empty() {
                    if let Err(e) = conversation_repo.add_message(
                        &conv_id,
                        "assistant",
                        &accumulated_response,
                        None,
                        None,
                    ) {
                        tracing::error!("Failed to save assistant message: {}", e);
                    }
                }

                // Emit completion
                event_bus
                    .publish(GatewayEvent::AgentCompleted {
                        agent_id: agent_id.clone(),
                        conversation_id: conv_id.clone(),
                        result: Some(accumulated_response.clone()),
                    })
                    .await;

                // Get delegation context before removing (for callback check)
                let delegation_ctx = delegation_registry.get(&conv_id);

                // Emit delegation completed
                event_bus
                    .publish(GatewayEvent::DelegationCompleted {
                        parent_agent_id: parent_agent.clone(),
                        parent_conversation_id: parent_conv.clone(),
                        child_agent_id: agent_id.clone(),
                        child_conversation_id: conv_id.clone(),
                        result: Some(accumulated_response.clone()),
                    })
                    .await;

                // Send callback message to parent conversation if enabled
                if let Some(ctx) = delegation_ctx {
                    if ctx.callback_on_complete {
                        // Format the callback message as JSON
                        let callback_data = serde_json::json!({
                            "convid": conv_id,
                            "agent": agent_id,
                            "response": if accumulated_response.is_empty() {
                                "(no response)".to_string()
                            } else {
                                accumulated_response.clone()
                            }
                        });
                        let callback_msg = format!(
                            "[Delegation Result]\n{}",
                            serde_json::to_string_pretty(&callback_data).unwrap_or_default()
                        );

                        // Add callback message to parent's conversation
                        if let Err(e) = conversation_repo.add_message(
                            &parent_conv,
                            "system",
                            &callback_msg,
                            None,
                            None,
                        ) {
                            tracing::error!(
                                parent_conv = %parent_conv,
                                "Failed to add callback message: {}", e
                            );
                        } else {
                            tracing::info!(
                                parent_agent = %parent_agent,
                                parent_conv = %parent_conv,
                                child_agent = %agent_id,
                                "Sent callback to parent conversation"
                            );
                        }
                    }
                }

                // Remove from delegation registry
                delegation_registry.remove(&conv_id);
            }
            Err(e) => {
                event_bus
                    .publish(GatewayEvent::Error {
                        agent_id: Some(agent_id),
                        conversation_id: Some(conv_id.clone()),
                        message: e.to_string(),
                    })
                    .await;

                delegation_registry.remove(&conv_id);
            }
        }
    });

    tracing::info!(
        parent_agent = %request.parent_agent_id,
        child_agent = %request.child_agent_id,
        child_conversation = %child_conversation_id,
        "Spawned delegated subagent"
    );

    Ok(child_conversation_id)
}

/// Convert a StreamEvent to a GatewayEvent.
fn convert_stream_event(
    event: StreamEvent,
    agent_id: &str,
    conversation_id: &str,
) -> GatewayEvent {
    match event {
        StreamEvent::Metadata { .. } => GatewayEvent::AgentStarted {
            agent_id: agent_id.to_string(),
            conversation_id: conversation_id.to_string(),
        },
        StreamEvent::Token { content, .. } => GatewayEvent::Token {
            agent_id: agent_id.to_string(),
            conversation_id: conversation_id.to_string(),
            delta: content,
        },
        StreamEvent::Reasoning { content, .. } => GatewayEvent::Thinking {
            agent_id: agent_id.to_string(),
            conversation_id: conversation_id.to_string(),
            content,
        },
        StreamEvent::ToolCallStart {
            tool_id, tool_name, args, ..
        } => GatewayEvent::ToolCall {
            agent_id: agent_id.to_string(),
            conversation_id: conversation_id.to_string(),
            tool_id,
            tool_name,
            args,
        },
        StreamEvent::ToolResult {
            tool_id, result, error, ..
        } => GatewayEvent::ToolResult {
            agent_id: agent_id.to_string(),
            conversation_id: conversation_id.to_string(),
            tool_id,
            result,
            error,
        },
        StreamEvent::Done { final_message, .. } => GatewayEvent::TurnComplete {
            agent_id: agent_id.to_string(),
            conversation_id: conversation_id.to_string(),
            message: final_message,
        },
        StreamEvent::Error { error, .. } => GatewayEvent::Error {
            agent_id: Some(agent_id.to_string()),
            conversation_id: Some(conversation_id.to_string()),
            message: error,
        },
        // Action events from tools
        StreamEvent::ActionRespond {
            message,
            session_id,
            ..
        } => GatewayEvent::Respond {
            conversation_id: conversation_id.to_string(),
            message,
            session_id,
        },
        StreamEvent::ActionDelegate {
            agent_id: child_agent_id,
            task,
            ..
        } => GatewayEvent::DelegationStarted {
            parent_agent_id: agent_id.to_string(),
            parent_conversation_id: conversation_id.to_string(),
            child_agent_id,
            child_conversation_id: format!("{}-sub", conversation_id),
            task,
        },
        // Handle other event types (ToolCallEnd, ShowContent, RequestInput)
        _ => GatewayEvent::AgentStarted {
            agent_id: agent_id.to_string(),
            conversation_id: conversation_id.to_string(),
        },
    }
}
