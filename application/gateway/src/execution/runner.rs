//! # Execution Runner
//!
//! Manages agent execution and event streaming for the gateway.

use crate::database::ConversationRepository;
use crate::events::{EventBus, GatewayEvent};
use crate::services::{AgentService, ProviderService};
use crate::services::providers::Provider;
use agent_runtime::{
    AgentExecutor, ExecutorConfig, LlmConfig, McpManager, MiddlewarePipeline,
    OpenAiClient, StreamEvent, ToolRegistry, ChatMessage,
};
use agent_tools::builtin_tools_with_fs;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use zero_core::FileSystemContext;

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
}

impl ExecutionConfig {
    /// Create a new execution config.
    pub fn new(agent_id: String, conversation_id: String, config_dir: PathBuf) -> Self {
        Self {
            agent_id,
            conversation_id,
            config_dir,
            max_iterations: 25,
        }
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

/// Execution runner that manages agent invocations.
pub struct ExecutionRunner {
    /// Event bus for broadcasting events
    event_bus: Arc<EventBus>,
    /// Agent service for loading agent configs
    agent_service: Arc<AgentService>,
    /// Provider service for loading provider configs
    provider_service: Arc<ProviderService>,
    /// Configuration directory
    config_dir: PathBuf,
    /// Active execution handles
    handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    /// Conversation repository for SQLite persistence
    conversation_repo: Arc<ConversationRepository>,
}

impl ExecutionRunner {
    /// Create a new execution runner.
    pub fn new(
        event_bus: Arc<EventBus>,
        agent_service: Arc<AgentService>,
        provider_service: Arc<ProviderService>,
        config_dir: PathBuf,
        conversation_repo: Arc<ConversationRepository>,
    ) -> Self {
        Self {
            event_bus,
            agent_service,
            provider_service,
            config_dir,
            handles: Arc::new(RwLock::new(HashMap::new())),
            conversation_repo,
        }
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
        let executor_config = ExecutorConfig::new(
            agent.id.clone(),
            provider.id.clone().unwrap_or_else(|| provider.name.clone()),
            agent.model.clone(),
        );

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

        // Load built-in tools
        let mut tool_registry = ToolRegistry::new();
        let builtin_tools = builtin_tools_with_fs(fs_context);
        tool_registry.register_all(builtin_tools);
        let tool_registry = Arc::new(tool_registry);

        // MCP manager - servers are loaded lazily when needed
        let mcp_manager = Arc::new(McpManager::new());

        // Create empty middleware pipeline
        let middleware_pipeline = Arc::new(MiddlewarePipeline::new());

        // Build executor config with system instruction
        let mut final_config = executor_config;
        final_config.system_instruction = Some(agent.instructions.clone());
        final_config.conversation_id = Some(config.conversation_id.clone());
        final_config.temperature = agent.temperature;
        final_config.max_tokens = agent.max_tokens;

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
        // Handle other event types (ToolCallEnd, ShowContent, RequestInput)
        _ => GatewayEvent::AgentStarted {
            agent_id: agent_id.to_string(),
            conversation_id: conversation_id.to_string(),
        },
    }
}
