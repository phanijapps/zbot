//! # Execution Runner
//!
//! High-level API for agent execution and event streaming.
//!
//! The `ExecutionRunner` is the main entry point for invoking agents. It provides:
//! - Agent invocation with streaming events
//! - Execution control (stop, pause, resume, cancel)
//! - Agent delegation handling
//! - Session and execution lifecycle management

use api_logs::LogService;
use execution_state::StateService;
use gateway_database::{ConversationRepository, DatabaseManager};
use gateway_events::{EventBus, GatewayEvent};
use gateway_services::{AgentService, McpService, ProviderService, SharedVaultPaths};
use agent_runtime::{AgentExecutor, ChatMessage};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::middleware::intent_analysis::{analyze_intent, inject_intent_context};

// Import types from sibling modules
pub use super::config::ExecutionConfig;
use super::delegation::{
    spawn_delegated_agent, DelegationRegistry, DelegationRequest,
};
pub use super::handle::ExecutionHandle;
use super::invoke::{
    broadcast_event, collect_agents_summary, collect_skills_summary, process_stream_event,
    spawn_batch_writer_with_repo, AgentLoader,
    ExecutorBuilder, ResponseAccumulator, StreamContext, ToolCallAccumulator, WorkspaceCache,
};
use super::lifecycle::{
    complete_execution, crash_execution, emit_agent_started,
    get_or_create_session, start_execution,
    stop_execution,
};

// ============================================================================
// EXECUTION RUNNER
// ============================================================================

/// Execution runner that manages agent invocations.
///
/// The runner is responsible for:
/// - Creating and managing agent executors
/// - Processing delegation requests from running agents
/// - Tracking execution handles for control operations
/// - Broadcasting events to connected clients
pub struct ExecutionRunner {
    /// Event bus for broadcasting events
    event_bus: Arc<EventBus>,
    /// Agent service for loading agent configs
    agent_service: Arc<AgentService>,
    /// Provider service for loading provider configs
    provider_service: Arc<ProviderService>,
    /// MCP service for loading MCP server configs
    mcp_service: Arc<McpService>,
    /// Skill service for loading skill configs
    skill_service: Arc<gateway_services::SkillService>,
    /// Vault paths for accessing configuration and data directories
    paths: SharedVaultPaths,
    /// Active execution handles
    handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    /// Conversation repository for SQLite persistence
    conversation_repo: Arc<ConversationRepository>,
    /// Delegation registry for tracking parent-child relationships
    delegation_registry: Arc<DelegationRegistry>,
    /// Channel for delegation requests
    delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    /// Log service for execution tracing
    log_service: Arc<LogService<DatabaseManager>>,
    /// State service for execution state management
    state_service: Arc<StateService<DatabaseManager>>,
    /// Connector registry for response routing to external connectors
    connector_registry: Option<Arc<gateway_connectors::ConnectorRegistry>>,
    /// Bridge registry for WebSocket worker connections
    bridge_registry: Option<Arc<gateway_bridge::BridgeRegistry>>,
    /// Bridge outbox for reliable message delivery
    bridge_outbox: Option<Arc<gateway_bridge::OutboxRepository>>,
    /// Cached workspace context (avoids reading workspace.json per execution)
    workspace_cache: WorkspaceCache,
    /// Memory repository for structured fact storage
    memory_repo: Option<Arc<gateway_database::MemoryRepository>>,
    /// Session distiller for automatic fact extraction after sessions
    distiller: Option<Arc<super::distillation::SessionDistiller>>,
    /// Memory recall for automatic fact retrieval at session start
    memory_recall: Option<Arc<super::recall::MemoryRecall>>,
}

impl ExecutionRunner {
    /// Create a new execution runner.
    ///
    /// This initializes the runner and spawns a background task for
    /// processing delegation requests from running agents.
    pub fn new(
        event_bus: Arc<EventBus>,
        agent_service: Arc<AgentService>,
        provider_service: Arc<ProviderService>,
        paths: SharedVaultPaths,
        conversation_repo: Arc<ConversationRepository>,
        mcp_service: Arc<McpService>,
        skill_service: Arc<gateway_services::SkillService>,
        log_service: Arc<LogService<DatabaseManager>>,
        state_service: Arc<StateService<DatabaseManager>>,
    ) -> Self {
        Self::with_connector_registry(
            event_bus,
            agent_service,
            provider_service,
            paths,
            conversation_repo,
            mcp_service,
            skill_service,
            log_service,
            state_service,
            None,
            Arc::new(tokio::sync::RwLock::new(None)),
            None,
            None,
            None,
            None,
            None,
        )
    }

    /// Create a new execution runner with connector registry for response routing.
    pub fn with_connector_registry(
        event_bus: Arc<EventBus>,
        agent_service: Arc<AgentService>,
        provider_service: Arc<ProviderService>,
        paths: SharedVaultPaths,
        conversation_repo: Arc<ConversationRepository>,
        mcp_service: Arc<McpService>,
        skill_service: Arc<gateway_services::SkillService>,
        log_service: Arc<LogService<DatabaseManager>>,
        state_service: Arc<StateService<DatabaseManager>>,
        connector_registry: Option<Arc<gateway_connectors::ConnectorRegistry>>,
        workspace_cache: WorkspaceCache,
        memory_repo: Option<Arc<gateway_database::MemoryRepository>>,
        distiller: Option<Arc<super::distillation::SessionDistiller>>,
        memory_recall: Option<Arc<super::recall::MemoryRecall>>,
        bridge_registry: Option<Arc<gateway_bridge::BridgeRegistry>>,
        bridge_outbox: Option<Arc<gateway_bridge::OutboxRepository>>,
    ) -> Self {
        // Create channel for delegation requests
        let (delegation_tx, delegation_rx) = mpsc::unbounded_channel::<DelegationRequest>();

        let runner = Self {
            event_bus,
            agent_service,
            provider_service,
            mcp_service,
            skill_service,
            paths,
            handles: Arc::new(RwLock::new(HashMap::new())),
            conversation_repo,
            delegation_registry: Arc::new(DelegationRegistry::new()),
            delegation_tx,
            log_service,
            state_service,
            connector_registry,
            bridge_registry,
            bridge_outbox,
            workspace_cache,
            memory_repo,
            distiller,
            memory_recall,
        };

        // Spawn delegation handler task
        runner.spawn_delegation_handler(delegation_rx);

        // Spawn continuation handler task
        runner.spawn_continuation_handler();

        runner
    }

    /// Spawn a background task that processes delegation requests.
    fn spawn_delegation_handler(&self, mut rx: mpsc::UnboundedReceiver<DelegationRequest>) {
        let event_bus = self.event_bus.clone();
        let agent_service = self.agent_service.clone();
        let provider_service = self.provider_service.clone();
        let mcp_service = self.mcp_service.clone();
        let skill_service = self.skill_service.clone();
        let paths = self.paths.clone();
        let conversation_repo = self.conversation_repo.clone();
        let handles = self.handles.clone();
        let delegation_registry = self.delegation_registry.clone();
        let delegation_tx = self.delegation_tx.clone();
        let log_service = self.log_service.clone();
        let state_service = self.state_service.clone();
        let workspace_cache = self.workspace_cache.clone();

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
                    skill_service.clone(),
                    paths.clone(),
                    conversation_repo.clone(),
                    handles.clone(),
                    delegation_registry.clone(),
                    delegation_tx.clone(),
                    log_service.clone(),
                    state_service.clone(),
                    workspace_cache.clone(),
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

    /// Spawn a background task that handles continuation after delegations complete.
    ///
    /// When all delegations for a session complete, this handler invokes the root
    /// agent to continue processing with the accumulated context (including callbacks).
    fn spawn_continuation_handler(&self) {
        let event_bus = self.event_bus.clone();
        let agent_service = self.agent_service.clone();
        let provider_service = self.provider_service.clone();
        let mcp_service = self.mcp_service.clone();
        let skill_service = self.skill_service.clone();
        let paths = self.paths.clone();
        let conversation_repo = self.conversation_repo.clone();
        let handles = self.handles.clone();
        let delegation_registry = self.delegation_registry.clone();
        let delegation_tx = self.delegation_tx.clone();
        let log_service = self.log_service.clone();
        let state_service = self.state_service.clone();
        let workspace_cache = self.workspace_cache.clone();

        // Subscribe to all events to catch SessionContinuationReady
        let mut event_rx = event_bus.subscribe_all();

        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(GatewayEvent::SessionContinuationReady {
                        session_id,
                        root_agent_id,
                        root_execution_id,
                    }) => {
                        tracing::info!(
                            session_id = %session_id,
                            root_agent_id = %root_agent_id,
                            root_execution_id = %root_execution_id,
                            "Continuation triggered - invoking root agent"
                        );

                        // Clear continuation flag to prevent double-trigger
                        if let Err(e) = state_service.clear_continuation(&session_id) {
                            tracing::warn!("Failed to clear continuation flag: {}", e);
                        }

                        // Invoke the root agent to continue
                        // The agent will see full session context including callbacks
                        if let Err(e) = invoke_continuation(
                            &session_id,
                            &root_agent_id,
                            event_bus.clone(),
                            agent_service.clone(),
                            provider_service.clone(),
                            mcp_service.clone(),
                            skill_service.clone(),
                            paths.clone(),
                            conversation_repo.clone(),
                            handles.clone(),
                            delegation_registry.clone(),
                            delegation_tx.clone(),
                            log_service.clone(),
                            state_service.clone(),
                            workspace_cache.clone(),
                        )
                        .await
                        {
                            tracing::error!(
                                session_id = %session_id,
                                error = %e,
                                "Failed to invoke continuation"
                            );
                        }
                    }
                    Ok(_) => {
                        // Ignore other events
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Continuation handler lagged by {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("Event bus closed, stopping continuation handler");
                        break;
                    }
                }
            }
        });
    }

    /// Invoke an agent with a message.
    ///
    /// Returns an execution handle for controlling the execution and the session ID.
    ///
    /// # Session Behavior
    ///
    /// - If `config.session_id` is Some: continues that session with a new execution
    /// - If `config.session_id` is None: creates a new session
    ///
    /// # Errors
    ///
    /// Returns an error if the agent or provider cannot be loaded.
    pub async fn invoke(
        &self,
        config: ExecutionConfig,
        message: String,
    ) -> Result<(ExecutionHandle, String), String> {
        let handle = ExecutionHandle::new(config.max_iterations);
        let handle_clone = handle.clone();

        // Get or create session and execution
        let setup = get_or_create_session(
            &self.state_service,
            &config.agent_id,
            config.session_id.as_deref(),
            config.source,
        );
        let session_id = setup.session_id;
        let execution_id = setup.execution_id;

        // Persist routing fields on the session (thread_id, connector_id, respond_to)
        if config.thread_id.is_some() || config.connector_id.is_some() || config.respond_to.is_some() {
            if let Err(e) = self.state_service.update_session_routing(
                &session_id,
                config.thread_id.as_deref(),
                config.connector_id.as_deref(),
                config.respond_to.as_ref(),
            ) {
                tracing::warn!("Failed to persist session routing: {}", e);
            }
        }

        // Start execution and log
        start_execution(
            &self.state_service,
            &self.log_service,
            &execution_id,
            &session_id,
            &config.agent_id,
            None,
        );

        // Store handle
        {
            let mut handles = self.handles.write().await;
            handles.insert(config.conversation_id.clone(), handle.clone());
        }

        // Emit start event
        emit_agent_started(
            &self.event_bus,
            &config.agent_id,
            &config.conversation_id,
            &session_id,
            &execution_id,
        )
        .await;

        // Load agent configuration (or create default for "root" agent)
        let agent_loader = AgentLoader::new(&self.agent_service, &self.provider_service, self.paths.clone());
        let (agent, provider) = match agent_loader.load_or_create_root(&config.agent_id).await {
            Ok(result) => result,
            Err(e) => {
                self.emit_error(&config.conversation_id, &config.agent_id, &e).await;
                return Err(e);
            }
        };

        // Load full session conversation (all messages including tool calls/results)
        let mut history: Vec<ChatMessage> = self
            .conversation_repo
            .get_session_conversation(&session_id, 200)
            .map(|messages| self.conversation_repo.session_messages_to_chat_format(&messages))
            .unwrap_or_default();

        // Smart recall: inject relevant facts at session start (only for fresh sessions)
        if history.is_empty() {
            if let Some(recall) = &self.memory_recall {
                match recall.recall(&config.agent_id, &message, 10).await {
                    Ok(facts) if !facts.is_empty() => {
                        let context = super::recall::format_recalled_facts(&facts);
                        history.insert(0, ChatMessage::system(context));
                        tracing::info!(
                            agent_id = %config.agent_id,
                            fact_count = facts.len(),
                            "Injected recalled memory facts"
                        );
                    }
                    Ok(_) => {} // No facts recalled
                    Err(e) => {
                        tracing::warn!("Memory recall failed: {}", e);
                    }
                }
            }
        }

        // Create executor (restore ward_id from existing session if available)
        let executor = match self.create_executor(&agent, &provider, &config, &session_id, setup.ward_id.as_deref(), true, Some(&message)).await {
            Ok(e) => e,
            Err(e) => {
                self.emit_error(&config.conversation_id, &config.agent_id, &e)
                    .await;
                return Err(e);
            }
        };

        // Spawn execution task
        self.spawn_execution_task(
            executor,
            handle_clone,
            config,
            message,
            session_id.clone(),
            execution_id,
            history,
        );

        Ok((handle, session_id))
    }

    /// Spawn the async execution task.
    fn spawn_execution_task(
        &self,
        executor: AgentExecutor,
        handle: ExecutionHandle,
        config: ExecutionConfig,
        message: String,
        session_id: String,
        execution_id: String,
        history: Vec<ChatMessage>,
    ) {
        let event_bus = self.event_bus.clone();
        let agent_id = config.agent_id.clone();
        let conversation_id = config.conversation_id.clone();
        let conversation_repo = self.conversation_repo.clone();
        let log_service = self.log_service.clone();
        let state_service = self.state_service.clone();
        let delegation_tx = self.delegation_tx.clone();
        let connector_registry = self.connector_registry.clone();
        let bridge_registry = self.bridge_registry.clone();
        let bridge_outbox = self.bridge_outbox.clone();
        let respond_to = config.respond_to.clone();
        let thread_id = config.thread_id.clone();
        let distiller = self.distiller.clone();

        tokio::spawn(async move {
            // Create batch writer for non-blocking DB writes (with conversation repo for session messages)
            let batch_writer = spawn_batch_writer_with_repo(
                state_service.clone(),
                log_service.clone(),
                Some(conversation_repo.clone()),
            );

            // Create stream context for event processing
            let stream_ctx = StreamContext::new(
                agent_id.clone(),
                conversation_id.clone(),
                session_id.clone(),
                execution_id.clone(),
                event_bus.clone(),
                log_service.clone(),
                state_service.clone(),
                delegation_tx,
            )
            .with_batch_writer(batch_writer.clone());

            let mut response_acc = ResponseAccumulator::new();
            let mut tool_acc = ToolCallAccumulator::new();

            // Append user message to session stream BEFORE execution
            batch_writer.session_message(
                &session_id,
                &execution_id,
                "user",
                &message,
                None,
                None,
            );

            // Track per-turn tool calls for assistant message emission
            let session_id_inner = session_id.clone();
            let execution_id_inner = execution_id.clone();
            let batch_writer_inner = batch_writer.clone();
            // Track tool calls for the current assistant turn
            let mut turn_tool_calls: Vec<serde_json::Value> = Vec::new();
            // Track accumulated text for the current assistant turn
            let mut turn_text = String::new();

            // Execute with streaming
            let result = executor
                .execute_stream(&message, &history, |event| {
                    // Check for stop request
                    if handle.is_stop_requested() {
                        return;
                    }

                    handle.increment();

                    // Stream messages to session as they happen
                    match &event {
                        agent_runtime::StreamEvent::ToolCallStart { tool_id, tool_name, args, .. } => {
                            tool_acc.start_call(tool_id.clone(), tool_name.clone(), args.clone());
                            // Accumulate tool call for the current assistant message
                            turn_tool_calls.push(serde_json::json!({
                                "tool_id": tool_id,
                                "tool_name": tool_name,
                                "args": args,
                            }));
                        }
                        agent_runtime::StreamEvent::ToolResult { tool_id, result, error, .. } => {
                            tool_acc.complete_call(tool_id, result.clone(), error.clone());

                            // Emit the assistant message for this turn (with accumulated tool_calls)
                            if !turn_tool_calls.is_empty() {
                                let tc_json = serde_json::to_string(&turn_tool_calls).unwrap_or_default();
                                let content = if turn_text.is_empty() {
                                    "[tool calls]".to_string()
                                } else {
                                    std::mem::take(&mut turn_text)
                                };
                                batch_writer_inner.session_message(
                                    &session_id_inner,
                                    &execution_id_inner,
                                    "assistant",
                                    &content,
                                    Some(&tc_json),
                                    None,
                                );
                                turn_tool_calls.clear();
                            }

                            // Emit tool result message
                            let tool_content = if let Some(err) = error {
                                format!("Error: {}", err)
                            } else {
                                result.clone()
                            };
                            batch_writer_inner.session_message(
                                &session_id_inner,
                                &execution_id_inner,
                                "tool",
                                &tool_content,
                                None,
                                Some(tool_id),
                            );
                        }
                        agent_runtime::StreamEvent::Token { content, .. } => {
                            turn_text.push_str(content);
                        }
                        _ => {}
                    }

                    // Process the event (logging, delegation, token tracking)
                    let (gateway_event, response_delta) = process_stream_event(&stream_ctx, &event);

                    // Accumulate response content
                    if let Some(delta) = response_delta {
                        response_acc.append(&delta);
                    }

                    // Broadcast the gateway event (if not an internal-only event)
                    if let Some(event) = gateway_event {
                        broadcast_event(stream_ctx.event_bus.clone(), event);
                    }
                })
                .await;

            let accumulated_response = response_acc.into_response();

            tracing::info!(
                execution_id = %execution_id,
                response_len = accumulated_response.len(),
                tool_calls_count = tool_acc.len(),
                "Execution stream completed"
            );

            // Emit final assistant response to session stream
            // (only if there's content not already emitted as part of a tool-call turn)
            if !accumulated_response.is_empty() {
                batch_writer.session_message(
                    &session_id,
                    &execution_id,
                    "assistant",
                    &accumulated_response,
                    None,
                    None,
                );
            }

            // Handle completion
            match result {
                Ok(()) => {
                    // Complete execution and emit events
                    complete_execution(
                        &state_service,
                        &log_service,
                        &event_bus,
                        &execution_id,
                        &session_id,
                        &agent_id,
                        &conversation_id,
                        Some(accumulated_response),
                        connector_registry.as_ref(),
                        respond_to.as_ref(),
                        thread_id.as_deref(),
                        bridge_registry.as_ref(),
                        bridge_outbox.as_ref(),
                    )
                    .await;

                    // Fire-and-forget session distillation
                    if let Some(distiller) = distiller.as_ref() {
                        let distiller = distiller.clone();
                        let sid = session_id.clone();
                        let aid = agent_id.clone();
                        tokio::spawn(async move {
                            if let Err(e) = distiller.distill(&sid, &aid).await {
                                tracing::warn!("Session distillation failed: {}", e);
                            }
                        });
                    }
                }
                Err(e) => {
                    // Crash execution and emit events
                    crash_execution(
                        &state_service,
                        &log_service,
                        &event_bus,
                        &execution_id,
                        &session_id,
                        &agent_id,
                        &conversation_id,
                        &e.to_string(),
                        true, // crash session for root execution
                    )
                    .await;
                }
            }

            // Check if stopped
            if handle.is_stop_requested() {
                stop_execution(
                    &state_service,
                    &log_service,
                    &event_bus,
                    &execution_id,
                    &session_id,
                    &agent_id,
                    &conversation_id,
                    handle.current_iteration(),
                )
                .await;
            }
        });
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

    /// Pause an execution by session ID.
    ///
    /// Pausing sets a flag that the executor will check. The execution
    /// will complete the current operation and then wait for resume.
    pub async fn pause(&self, session_id: &str) -> Result<(), String> {
        // First update the database state
        self.state_service.pause_session(session_id)?;

        // Then pause any running execution with matching session
        let handles = self.handles.read().await;
        for handle in handles.values() {
            handle.pause();
        }

        Ok(())
    }

    /// Resume a paused execution by session ID.
    pub async fn resume(&self, session_id: &str) -> Result<(), String> {
        // First update the database state
        self.state_service.resume_session(session_id)?;

        // Then resume any paused execution
        let handles = self.handles.read().await;
        for handle in handles.values() {
            handle.resume();
        }

        Ok(())
    }

    /// Cancel an execution by session ID.
    ///
    /// Cancellation immediately stops the execution and marks it as cancelled.
    pub async fn cancel(&self, session_id: &str) -> Result<(), String> {
        // First update the database state
        self.state_service.cancel_session(session_id)?;

        // Then cancel any running execution
        let handles = self.handles.read().await;
        for handle in handles.values() {
            handle.cancel();
        }

        Ok(())
    }

    /// End a session (mark as completed).
    ///
    /// Called when user explicitly ends a session via /end, /new, or +new button.
    /// This marks the session as completed regardless of running executions.
    pub async fn end_session(&self, session_id: &str) -> Result<(), String> {
        tracing::info!(session_id = %session_id, "User requested session end");

        // Stop any running executions gracefully
        let handles = self.handles.read().await;
        for handle in handles.values() {
            handle.stop();
        }

        // Mark session as completed
        self.state_service.complete_session(session_id)?;

        tracing::info!(session_id = %session_id, "Session ended by user request");
        Ok(())
    }

    /// Get execution handle for a conversation.
    pub async fn get_handle(&self, conversation_id: &str) -> Option<ExecutionHandle> {
        let handles = self.handles.read().await;
        handles.get(conversation_id).cloned()
    }

    /// Get the delegation registry.
    pub fn delegation_registry(&self) -> Arc<DelegationRegistry> {
        self.delegation_registry.clone()
    }

    /// Get the state service for execution state management.
    pub fn state_service(&self) -> Arc<StateService<DatabaseManager>> {
        self.state_service.clone()
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

        // Register the delegation (legacy function, using conversation_id as session for backward compat)
        let delegation_context = super::delegation::DelegationContext::new(
            parent_conversation_id, // session_id (using conv_id for legacy)
            parent_conversation_id, // parent_execution_id (using conv_id for legacy)
            parent_agent_id,
            parent_conversation_id,
        );
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
            self.paths.vault_dir().clone(),
        );

        // Emit delegation started event
        self.event_bus
            .publish(GatewayEvent::DelegationStarted {
                session_id: parent_conversation_id.to_string(), // legacy: using conv_id as session
                parent_execution_id: parent_conversation_id.to_string(),
                child_execution_id: child_conversation_id.clone(),
                parent_agent_id: parent_agent_id.to_string(),
                child_agent_id: child_agent_id.to_string(),
                task: task.to_string(),
                parent_conversation_id: Some(parent_conversation_id.to_string()),
                child_conversation_id: Some(child_conversation_id.clone()),
            })
            .await;

        // Spawn the child agent
        match self.invoke(config, task.to_string()).await {
            Ok((_handle, session_id)) => {
                tracing::info!(
                    parent_agent = %parent_agent_id,
                    child_agent = %child_agent_id,
                    child_conversation = %child_conversation_id,
                    session_id = %session_id,
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

    /// Create an executor for the agent using the ExecutorBuilder.
    async fn create_executor(
        &self,
        agent: &gateway_services::agents::Agent,
        provider: &gateway_services::providers::Provider,
        config: &ExecutionConfig,
        session_id: &str,
        ward_id: Option<&str>,
        is_root: bool,
        user_message: Option<&str>,
    ) -> Result<AgentExecutor, String> {
        // Collect available agents and skills for executor state
        let available_agents = collect_agents_summary(&self.agent_service).await;
        let available_skills = collect_skills_summary(&self.skill_service).await;

        // Get tool settings
        let settings_service = gateway_services::SettingsService::new(self.paths.clone());
        let tool_settings = settings_service.get_tool_settings().unwrap_or_default();

        // Build hook context if present
        let hook_context = config
            .hook_context
            .as_ref()
            .and_then(|ctx| serde_json::to_value(ctx).ok());

        // Build fact store from memory repo + embedding client (if available)
        let fact_store: Option<Arc<dyn zero_core::MemoryFactStore>> = self.memory_repo.as_ref().map(|repo| {
            // TODO: pass embedding client when available in runner context
            Arc::new(gateway_database::GatewayMemoryFactStore::new(repo.clone(), None))
                as Arc<dyn zero_core::MemoryFactStore>
        });

        // Build connector resource provider (HTTP + bridge composite)
        let http_provider: Option<Arc<dyn zero_core::ConnectorResourceProvider>> =
            self.connector_registry.as_ref().map(|registry| {
                Arc::new(super::resource_provider::GatewayResourceProvider::new(registry.clone()))
                    as Arc<dyn zero_core::ConnectorResourceProvider>
            });
        let bridge_provider: Option<Arc<dyn zero_core::ConnectorResourceProvider>> =
            self.bridge_registry.as_ref().zip(self.bridge_outbox.as_ref()).map(|(reg, outbox)| {
                Arc::new(gateway_bridge::BridgeResourceProvider::new(reg.clone(), outbox.clone()))
                    as Arc<dyn zero_core::ConnectorResourceProvider>
            });
        let connector_provider: Option<Arc<dyn zero_core::ConnectorResourceProvider>> =
            if http_provider.is_some() || bridge_provider.is_some() {
                Some(Arc::new(super::composite_provider::CompositeResourceProvider::new(
                    http_provider,
                    bridge_provider,
                )) as Arc<dyn zero_core::ConnectorResourceProvider>)
            } else {
                None
            };

        // Use ExecutorBuilder to create the executor
        let mut builder = ExecutorBuilder::new(self.paths.vault_dir().clone(), tool_settings)
            .with_workspace_cache(self.workspace_cache.clone());
        if let Some(fs) = fact_store {
            builder = builder.with_fact_store(fs);
        }
        if let Some(cp) = connector_provider {
            builder = builder.with_connector_provider(cp);
        }

        // Intent analysis enrichment (root agent first turn only)
        let enriched_agent = if is_root {
            if let Some(msg) = user_message {
                let llm_config = agent_runtime::LlmConfig::new(
                    provider.base_url.clone(),
                    provider.api_key.clone(),
                    agent.model.clone(),
                    provider.id.clone().unwrap_or_else(|| provider.name.clone()),
                );
                match agent_runtime::OpenAiClient::new(llm_config) {
                    Ok(raw_client) => {
                        let llm_client: std::sync::Arc<dyn agent_runtime::LlmClient> =
                            std::sync::Arc::new(raw_client);
                        match analyze_intent(
                            llm_client.as_ref(),
                            msg,
                            &available_skills,
                            &available_agents,
                        )
                        .await
                        {
                            Ok(analysis) => {
                                let mut enriched = agent.clone();
                                inject_intent_context(&mut enriched.instructions, &analysis);
                                tracing::info!(
                                    primary_intent = %analysis.primary_intent,
                                    hidden_intents = analysis.hidden_intents.len(),
                                    "Intent analysis enrichment complete"
                                );
                                Some(enriched)
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Intent analysis failed, proceeding without enrichment: {}",
                                    e
                                );
                                None
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to create LLM client for intent analysis: {}",
                            e
                        );
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        let agent_for_build = enriched_agent.as_ref().unwrap_or(agent);

        builder
            .build(
                agent_for_build,
                provider,
                &config.conversation_id,
                session_id,
                &available_agents,
                &available_skills,
                hook_context.as_ref(),
                &self.mcp_service,
                ward_id,
            )
            .await
    }

    /// Emit an error event.
    async fn emit_error(&self, conversation_id: &str, agent_id: &str, message: &str) {
        self.event_bus
            .publish(GatewayEvent::Error {
                agent_id: Some(agent_id.to_string()),
                session_id: None,
                execution_id: None,
                message: message.to_string(),
                conversation_id: Some(conversation_id.to_string()),
            })
            .await;
    }
}

// ============================================================================
// CONTINUATION HANDLER
// ============================================================================

/// Invoke the root agent to continue after all delegations have completed.
///
/// This is called when all subagents have finished and the root agent needs
/// to process their results and decide what to do next:
/// - Respond to the user with synthesized results
/// - Delegate to more subagents if needed
/// - Continue its orchestration loop
///
/// The agent sees the full session context including:
/// - Original user message
/// - Previous assistant responses
/// - Callback messages from completed subagents (as system messages)
#[allow(clippy::too_many_arguments)]
async fn invoke_continuation(
    session_id: &str,
    root_agent_id: &str,
    event_bus: Arc<EventBus>,
    agent_service: Arc<AgentService>,
    provider_service: Arc<ProviderService>,
    mcp_service: Arc<McpService>,
    skill_service: Arc<gateway_services::SkillService>,
    paths: SharedVaultPaths,
    conversation_repo: Arc<ConversationRepository>,
    handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    _delegation_registry: Arc<DelegationRegistry>,
    delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    log_service: Arc<LogService<DatabaseManager>>,
    state_service: Arc<StateService<DatabaseManager>>,
    workspace_cache: WorkspaceCache,
) -> Result<(), String> {
    // Generate a new conversation ID for this continuation turn
    let conversation_id = format!(
        "{}-cont-{}",
        session_id,
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0")
    );

    // Reuse the root execution for continuation (one continuous conversation)
    let execution_id = match state_service.get_root_execution(session_id)? {
        Some(root_exec) => root_exec.id,
        None => {
            // Fallback: create new root execution if none found
            let execution = execution_state::AgentExecution::new_root(session_id, root_agent_id);
            state_service.create_execution(&execution)?;
            execution.id
        }
    };

    // Reactivate session and execution if they were in a terminal state
    state_service.reactivate_session(session_id)?;
    state_service.reactivate_execution(&execution_id)?;
    let _ = log_service.log_session_start(
        &execution_id,
        &conversation_id,
        root_agent_id,
        None,
    );

    // Create execution handle
    let handle = ExecutionHandle::new(50); // Default max iterations for continuation
    {
        let mut handles_guard = handles.write().await;
        handles_guard.insert(conversation_id.clone(), handle.clone());
    }

    // Emit agent started event
    emit_agent_started(&event_bus, root_agent_id, &conversation_id, session_id, &execution_id).await;

    // Load agent and provider
    let agent_loader = AgentLoader::new(&agent_service, &provider_service, paths.clone());
    let (agent, provider) = agent_loader.load_or_create_root(root_agent_id).await?;

    // Load full session conversation (includes tool calls, results, and callbacks)
    let history: Vec<ChatMessage> = conversation_repo
        .get_session_conversation(session_id, 200)
        .map(|messages| conversation_repo.session_messages_to_chat_format(&messages))
        .unwrap_or_default();

    tracing::info!(
        session_id = %session_id,
        execution_id = %execution_id,
        history_count = %history.len(),
        "Loading session history for continuation"
    );

    // Get tool settings
    let settings_service = gateway_services::SettingsService::new(paths.clone());
    let tool_settings = settings_service.get_tool_settings().unwrap_or_default();

    // Collect available agents and skills
    let available_agents = collect_agents_summary(&agent_service).await;
    let available_skills = collect_skills_summary(&skill_service).await;

    // Look up active ward from session
    let session_ward_id = state_service
        .get_session(session_id)
        .ok()
        .flatten()
        .and_then(|s| s.ward_id);

    // Build executor
    let builder = ExecutorBuilder::new(paths.vault_dir().clone(), tool_settings)
        .with_workspace_cache(workspace_cache);
    let executor = builder
        .build(
            &agent,
            &provider,
            &conversation_id,
            session_id,
            &available_agents,
            &available_skills,
            None, // No hook context for continuation
            &mcp_service,
            session_ward_id.as_deref(),
        )
        .await?;

    // The continuation message prompts the agent to process subagent results
    let continuation_message =
        "[All delegated tasks have completed. Review the results above and continue your orchestration. \
         You may respond to the user, delegate to more agents, or take other actions as needed.]";

    // Spawn execution task
    let session_id_clone = session_id.to_string();
    let agent_id_clone = root_agent_id.to_string();

    tokio::spawn(async move {
        // Create batch writer for non-blocking DB writes (with conversation repo for session messages)
        let batch_writer = spawn_batch_writer_with_repo(
            state_service.clone(),
            log_service.clone(),
            Some(conversation_repo.clone()),
        );

        let stream_ctx = StreamContext::new(
            agent_id_clone.clone(),
            conversation_id.clone(),
            session_id_clone.clone(),
            execution_id.clone(),
            event_bus.clone(),
            log_service.clone(),
            state_service.clone(),
            delegation_tx,
        )
        .with_batch_writer(batch_writer.clone());

        let mut response_acc = ResponseAccumulator::new();
        let mut tool_acc = ToolCallAccumulator::new();

        // Append continuation system message to session stream
        batch_writer.session_message(
            &session_id_clone,
            &execution_id,
            "system",
            continuation_message,
            None,
            None,
        );

        let session_id_inner = session_id_clone.clone();
        let execution_id_inner = execution_id.clone();
        let batch_writer_inner = batch_writer.clone();
        let mut turn_tool_calls: Vec<serde_json::Value> = Vec::new();
        let mut turn_text = String::new();

        let result = executor
            .execute_stream(continuation_message, &history, |event| {
                if handle.is_stop_requested() {
                    return;
                }

                handle.increment();

                // Stream messages to session as they happen
                match &event {
                    agent_runtime::StreamEvent::ToolCallStart { tool_id, tool_name, args, .. } => {
                        tool_acc.start_call(tool_id.clone(), tool_name.clone(), args.clone());
                        turn_tool_calls.push(serde_json::json!({
                            "tool_id": tool_id,
                            "tool_name": tool_name,
                            "args": args,
                        }));
                    }
                    agent_runtime::StreamEvent::ToolResult { tool_id, result, error, .. } => {
                        tool_acc.complete_call(tool_id, result.clone(), error.clone());

                        // Emit assistant message for this turn
                        if !turn_tool_calls.is_empty() {
                            let tc_json = serde_json::to_string(&turn_tool_calls).unwrap_or_default();
                            let content = if turn_text.is_empty() {
                                "[tool calls]".to_string()
                            } else {
                                std::mem::take(&mut turn_text)
                            };
                            batch_writer_inner.session_message(
                                &session_id_inner,
                                &execution_id_inner,
                                "assistant",
                                &content,
                                Some(&tc_json),
                                None,
                            );
                            turn_tool_calls.clear();
                        }

                        // Emit tool result message
                        let tool_content = if let Some(err) = error {
                            format!("Error: {}", err)
                        } else {
                            result.clone()
                        };
                        batch_writer_inner.session_message(
                            &session_id_inner,
                            &execution_id_inner,
                            "tool",
                            &tool_content,
                            None,
                            Some(tool_id),
                        );
                    }
                    agent_runtime::StreamEvent::Token { content, .. } => {
                        turn_text.push_str(content);
                    }
                    _ => {}
                }

                let (gateway_event, response_delta) = process_stream_event(&stream_ctx, &event);

                if let Some(delta) = response_delta {
                    response_acc.append(&delta);
                }

                // Broadcast the gateway event (if not an internal-only event)
                if let Some(event) = gateway_event {
                    broadcast_event(stream_ctx.event_bus.clone(), event);
                }
            })
            .await;

        let accumulated_response = response_acc.into_response();

        // Emit final assistant response to session stream
        if !accumulated_response.is_empty() {
            batch_writer.session_message(
                &session_id_clone,
                &execution_id,
                "assistant",
                &accumulated_response,
                None,
                None,
            );
        }

        match result {
            Ok(()) => {
                // Continuation turns don't dispatch to connectors (they're internal)
                complete_execution(
                    &state_service,
                    &log_service,
                    &event_bus,
                    &execution_id,
                    &session_id_clone,
                    &agent_id_clone,
                    &conversation_id,
                    Some(accumulated_response),
                    None,
                    None,
                    None, // No thread_id for continuation turns
                    None, // No bridge dispatch for continuation turns
                    None,
                )
                .await;
            }
            Err(e) => {
                crash_execution(
                    &state_service,
                    &log_service,
                    &event_bus,
                    &execution_id,
                    &session_id_clone,
                    &agent_id_clone,
                    &conversation_id,
                    &e.to_string(),
                    true,
                )
                .await;
            }
        }

        if handle.is_stop_requested() {
            stop_execution(
                &state_service,
                &log_service,
                &event_bus,
                &execution_id,
                &session_id_clone,
                &agent_id_clone,
                &conversation_id,
                handle.current_iteration(),
            )
            .await;
        }
    });

    Ok(())
}
