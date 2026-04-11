//! # Execution Runner
//!
//! High-level API for agent execution and event streaming.
//!
//! The `ExecutionRunner` is the main entry point for invoking agents. It provides:
//! - Agent invocation with streaming events
//! - Execution control (stop, pause, resume, cancel)
//! - Agent delegation handling
//! - Session and execution lifecycle management

use agent_runtime::{AgentExecutor, ChatMessage};
use api_logs::LogService;
use execution_state::StateService;
use gateway_database::{ConversationRepository, DatabaseManager};
use gateway_events::{EventBus, GatewayEvent};
use gateway_services::{AgentService, McpService, ProviderService, SharedVaultPaths};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock, Semaphore};

/// Callback invoked after session creation but before any events are emitted.
/// Receives the session_id so the caller can set up subscriptions before events fire.
pub type OnSessionReady =
    Box<dyn FnOnce(String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

use crate::middleware::intent_analysis::{
    analyze_intent, format_intent_injection, index_resources,
};

// Import types from sibling modules
pub use super::config::ExecutionConfig;
use super::delegation::{spawn_delegated_agent, DelegationRegistry, DelegationRequest};
pub use super::handle::ExecutionHandle;
use super::invoke::{
    broadcast_event, collect_agents_summary, collect_skills_summary, process_stream_event,
    spawn_batch_writer_with_repo, AgentLoader, ExecutorBuilder, ResponseAccumulator, StreamContext,
    ToolCallAccumulator, WorkspaceCache,
};
use super::lifecycle::{
    complete_execution, crash_execution, emit_agent_started, get_or_create_session,
    start_execution, stop_execution,
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
    /// Semaphore to limit concurrent delegation spawns (prevents resource exhaustion)
    delegation_semaphore: Arc<Semaphore>,
    /// Embedding client for generating vector embeddings (semantic search in memory)
    embedding_client: Option<Arc<dyn agent_runtime::llm::embedding::EmbeddingClient>>,
    /// Model capabilities registry for context window and capability lookups
    model_registry: Option<Arc<gateway_services::models::ModelRegistry>>,
    /// Per-provider rate limiters — shared across all executors using the same provider.
    rate_limiters: std::sync::Arc<
        std::sync::RwLock<
            std::collections::HashMap<String, std::sync::Arc<agent_runtime::ProviderRateLimiter>>,
        >,
    >,
}

impl ExecutionRunner {
    /// Create a new execution runner.
    ///
    /// This initializes the runner and spawns a background task for
    /// processing delegation requests from running agents.
    #[allow(clippy::too_many_arguments)]
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
            None,
            2, // default max_parallel_agents
        )
    }

    #[allow(clippy::too_many_arguments)]
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
        embedding_client: Option<Arc<dyn agent_runtime::llm::embedding::EmbeddingClient>>,
        max_parallel_agents: u32,
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
            delegation_semaphore: Arc::new(Semaphore::new(max_parallel_agents as usize)),
            embedding_client,
            model_registry: None,
            rate_limiters: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        };

        // Spawn delegation handler task
        runner.spawn_delegation_handler(delegation_rx);

        // Spawn continuation handler task
        runner.spawn_continuation_handler();

        runner
    }

    /// Set the model capabilities registry.
    pub fn set_model_registry(&mut self, registry: Arc<gateway_services::models::ModelRegistry>) {
        self.model_registry = Some(registry);
    }

    /// Get or create a shared rate limiter for a provider.
    ///
    /// Rate limiters are created once per provider and shared across all executors
    /// (root and subagents) so they share the same concurrent-request and RPM buckets.
    fn get_rate_limiter(
        &self,
        provider: &gateway_services::providers::Provider,
    ) -> std::sync::Arc<agent_runtime::ProviderRateLimiter> {
        let provider_id = provider.id.clone().unwrap_or_else(|| provider.name.clone());
        let rate_limits = provider.effective_rate_limits();

        // Check if exists (fast path — read lock)
        if let Ok(guard) = self.rate_limiters.read() {
            if let Some(limiter) = guard.get(&provider_id) {
                return limiter.clone();
            }
        }

        // Create new limiter and insert (write lock)
        let limiter = std::sync::Arc::new(agent_runtime::ProviderRateLimiter::new(
            rate_limits.concurrent_requests,
            rate_limits.requests_per_minute,
        ));

        if let Ok(mut guard) = self.rate_limiters.write() {
            // Use entry API to avoid overwriting if another thread raced us
            guard.entry(provider_id).or_insert_with(|| limiter.clone());
        }

        limiter
    }

    /// Spawn a background task that processes delegation requests.
    ///
    /// Maintains a per-session sequential queue: only one delegation per session
    /// runs at a time. Additional requests for the same session are queued and
    /// dispatched in order once the active delegation completes.
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
        let delegation_semaphore = self.delegation_semaphore.clone();
        let memory_repo = self.memory_repo.clone();
        let embedding_client = self.embedding_client.clone();
        let memory_recall = self.memory_recall.clone();
        let rate_limiters = self.rate_limiters.clone();

        tokio::spawn(async move {
            // Per-session tracking: only one delegation active per session at a time
            let mut active_sessions: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            let mut queued: std::collections::HashMap<
                String,
                std::collections::VecDeque<DelegationRequest>,
            > = std::collections::HashMap::new();

            // Completion notification channel
            let (done_tx, mut done_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

            /// Spawn a delegation task with a completion notification.
            ///
            /// Acquires the global semaphore permit, runs `spawn_delegated_agent`,
            /// then signals the handler loop via `done_tx` so the next queued
            /// request for the same session can be dispatched.
            #[allow(clippy::too_many_arguments)]
            fn spawn_with_notification(
                request: DelegationRequest,
                event_bus: &Arc<EventBus>,
                agent_service: &Arc<AgentService>,
                provider_service: &Arc<ProviderService>,
                mcp_service: &Arc<McpService>,
                skill_service: &Arc<gateway_services::SkillService>,
                paths: &SharedVaultPaths,
                conversation_repo: &Arc<ConversationRepository>,
                handles: &Arc<RwLock<HashMap<String, ExecutionHandle>>>,
                delegation_registry: &Arc<DelegationRegistry>,
                delegation_tx: &mpsc::UnboundedSender<DelegationRequest>,
                log_service: &Arc<LogService<DatabaseManager>>,
                state_service: &Arc<StateService<DatabaseManager>>,
                workspace_cache: &WorkspaceCache,
                delegation_semaphore: &Arc<Semaphore>,
                memory_repo: &Option<Arc<gateway_database::MemoryRepository>>,
                embedding_client: &Option<Arc<dyn agent_runtime::llm::embedding::EmbeddingClient>>,
                memory_recall: &Option<Arc<super::recall::MemoryRecall>>,
                rate_limiters: &Arc<
                    std::sync::RwLock<
                        std::collections::HashMap<String, Arc<agent_runtime::ProviderRateLimiter>>,
                    >,
                >,
                done_tx: mpsc::UnboundedSender<String>,
            ) {
                let session_id = request.session_id.clone();

                // Clone all Arcs for the spawned task
                let event_bus = event_bus.clone();
                let agent_service = agent_service.clone();
                let provider_service = provider_service.clone();
                let mcp_service = mcp_service.clone();
                let skill_service = skill_service.clone();
                let paths = paths.clone();
                let conversation_repo = conversation_repo.clone();
                let handles = handles.clone();
                let delegation_registry = delegation_registry.clone();
                let delegation_tx = delegation_tx.clone();
                let log_service = log_service.clone();
                let state_service = state_service.clone();
                let workspace_cache = workspace_cache.clone();
                let delegation_semaphore = delegation_semaphore.clone();
                let memory_repo = memory_repo.clone();
                let embedding_client = embedding_client.clone();
                let memory_recall = memory_recall.clone();
                let rate_limiters = rate_limiters.clone();

                tokio::spawn(async move {
                    let semaphore = delegation_semaphore.clone();
                    let permit = semaphore.acquire_owned().await.ok();

                    let result = spawn_delegated_agent(
                        &request,
                        event_bus,
                        agent_service,
                        provider_service,
                        mcp_service,
                        skill_service,
                        paths,
                        conversation_repo,
                        handles,
                        delegation_registry,
                        delegation_tx,
                        log_service,
                        state_service,
                        workspace_cache,
                        permit,
                        memory_repo,
                        embedding_client,
                        memory_recall,
                        rate_limiters,
                    )
                    .await;

                    if let Err(e) = &result {
                        tracing::error!(
                            session_id = %session_id,
                            agent = %request.child_agent_id,
                            error = %e,
                            "Delegation failed"
                        );
                    }

                    // Notify handler that this delegation is done
                    let _ = done_tx.send(session_id);
                });
            }

            loop {
                tokio::select! {
                    Some(request) = rx.recv() => {
                        let session_id = request.session_id.clone();

                        if request.parallel {
                            // Parallel: skip per-session queue, go straight to global semaphore
                            tracing::info!(
                                session_id = %session_id,
                                child_agent = %request.child_agent_id,
                                "Parallel delegation — bypassing per-session queue"
                            );
                            spawn_with_notification(
                                request,
                                &event_bus, &agent_service, &provider_service,
                                &mcp_service, &skill_service, &paths,
                                &conversation_repo, &handles, &delegation_registry,
                                &delegation_tx, &log_service, &state_service,
                                &workspace_cache, &delegation_semaphore,
                                &memory_repo, &embedding_client,
                                &memory_recall, &rate_limiters,
                                done_tx.clone(),
                            );
                        } else if active_sessions.contains(&session_id) {
                            // Sequential: queue behind active delegation
                            tracing::info!(
                                session_id = %session_id,
                                agent = %request.child_agent_id,
                                queued = queued.get(&session_id).map(|q| q.len()).unwrap_or(0),
                                "Queuing delegation (active delegation in progress)"
                            );
                            queued.entry(session_id).or_default().push_back(request);
                        } else {
                            // Sequential: no active delegation, spawn immediately
                            tracing::info!(
                                session_id = %session_id,
                                parent_agent = %request.parent_agent_id,
                                child_agent = %request.child_agent_id,
                                "Processing delegation request"
                            );
                            active_sessions.insert(session_id.clone());

                            spawn_with_notification(
                                request,
                                &event_bus, &agent_service, &provider_service,
                                &mcp_service, &skill_service, &paths,
                                &conversation_repo, &handles, &delegation_registry,
                                &delegation_tx, &log_service, &state_service,
                                &workspace_cache, &delegation_semaphore,
                                &memory_repo, &embedding_client,
                                &memory_recall, &rate_limiters,
                                done_tx.clone(),
                            );
                        }
                    }
                    Some(completed_session) = done_rx.recv() => {
                        active_sessions.remove(&completed_session);

                        // Pop next queued request for this session
                        if let Some(queue) = queued.get_mut(&completed_session) {
                            if let Some(next) = queue.pop_front() {
                                tracing::info!(
                                    session_id = %completed_session,
                                    agent = %next.child_agent_id,
                                    remaining = queue.len(),
                                    "Dequeuing next delegation"
                                );
                                active_sessions.insert(completed_session.clone());

                                spawn_with_notification(
                                    next,
                                    &event_bus, &agent_service, &provider_service,
                                    &mcp_service, &skill_service, &paths,
                                    &conversation_repo, &handles, &delegation_registry,
                                    &delegation_tx, &log_service, &state_service,
                                    &workspace_cache, &delegation_semaphore,
                                    &memory_repo, &embedding_client,
                                    &memory_recall, &rate_limiters,
                                    done_tx.clone(),
                                );
                            }
                            if queued.get(&completed_session).map(|q| q.is_empty()).unwrap_or(true) {
                                queued.remove(&completed_session);
                            }
                        }
                    }
                    else => break,
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
        let memory_repo = self.memory_repo.clone();
        let embedding_client = self.embedding_client.clone();
        let distiller = self.distiller.clone();
        let memory_recall = self.memory_recall.clone();
        let model_registry = self.model_registry.clone();

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
                            memory_repo.clone(),
                            embedding_client.clone(),
                            distiller.clone(),
                            memory_recall.clone(),
                            model_registry.clone(),
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
        self.invoke_with_callback(config, message, None).await
    }

    /// Invoke an agent with an optional session-ready callback.
    ///
    /// The callback fires after session creation but before any events are
    /// emitted, allowing the caller to set up subscriptions before intent
    /// analysis events fire.
    pub async fn invoke_with_callback(
        &self,
        config: ExecutionConfig,
        message: String,
        on_session_ready: Option<OnSessionReady>,
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
        if config.thread_id.is_some()
            || config.connector_id.is_some()
            || config.respond_to.is_some()
        {
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

        // Notify caller so it can subscribe before events fire
        if let Some(callback) = on_session_ready {
            callback(session_id.clone()).await;
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
        let settings_for_loader = gateway_services::SettingsService::new(self.paths.clone());
        let agent_loader = AgentLoader::new(
            &self.agent_service,
            &self.provider_service,
            self.paths.clone(),
        )
        .with_settings(&settings_for_loader)
        .with_fast_mode(config.is_fast_mode());
        let (agent, provider) = match agent_loader.load_or_create_root(&config.agent_id).await {
            Ok(result) => result,
            Err(e) => {
                self.emit_error(&config.conversation_id, &config.agent_id, &e)
                    .await;
                return Err(e);
            }
        };

        // Load full session conversation (all messages including tool calls/results)
        let mut history: Vec<ChatMessage> = self
            .conversation_repo
            .get_session_conversation(&session_id, 200)
            .map(|messages| {
                self.conversation_repo
                    .session_messages_to_chat_format(&messages)
            })
            .unwrap_or_default();

        // Graph-powered recall for first message — inject remembered facts, episodes, and
        // entity context before the agent sees the user's message.
        // Skipped in fast mode for speed — chat_protocol instructs the agent to skip recall.
        if !config.is_fast_mode() {
            if let Some(recall) = &self.memory_recall {
                match recall
                    .recall_with_graph(
                        &config.agent_id,
                        &message,
                        5,
                        setup.ward_id.as_deref(),
                        Some(&session_id),
                    )
                    .await
                {
                    Ok(result) if !result.facts.is_empty() || !result.episodes.is_empty() => {
                        history.insert(0, ChatMessage::system(result.formatted));
                        tracing::info!(
                            facts = result.facts.len(),
                            episodes = result.episodes.len(),
                            "Recalled memory context for first message"
                        );
                    }
                    Ok(_) => {
                        tracing::debug!(
                            "First-message recall returned empty — no relevant facts/episodes"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "First-message graph recall failed: {}, falling back to basic recall",
                            e
                        );
                        // Fallback: try basic recall without graph
                        match recall
                            .recall(&config.agent_id, &message, 5, setup.ward_id.as_deref())
                            .await
                        {
                            Ok(facts) if !facts.is_empty() => {
                                let formatted: Vec<String> = facts
                                    .iter()
                                    .map(|f| format!("- [{}] {}", f.fact.category, f.fact.content))
                                    .collect();
                                history.insert(
                                    0,
                                    ChatMessage::system(format!(
                                        "## Recalled Context\n{}",
                                        formatted.join("\n")
                                    )),
                                );
                                tracing::info!(
                                    facts = facts.len(),
                                    "Fallback recall injected facts"
                                );
                            }
                            Ok(_) => {}
                            Err(e2) => tracing::warn!("Fallback recall also failed: {}", e2),
                        }
                    }
                }
            }
        } // end !is_fast_mode recall gate

        // Nudge the agent to use memory.recall tool at session start (visible, agent-driven)
        // Skipped in fast mode — fast chat starts working immediately.
        if !config.is_fast_mode() && history.is_empty() {
            history.push(ChatMessage::system(
                "Before starting this task, use the memory tool to recall relevant knowledge \
                 — corrections, past strategies, and domain context."
                    .to_string(),
            ));
        }

        // Create executor (restore ward_id from existing session if available)
        let (executor, recommended_skills) = match self
            .create_executor(
                &agent,
                &provider,
                &config,
                &session_id,
                setup.ward_id.as_deref(),
                true,
                Some(&message),
                &execution_id,
            )
            .await
        {
            Ok(result) => result,
            Err(e) => {
                self.emit_error(&config.conversation_id, &config.agent_id, &e)
                    .await;
                return Err(e);
            }
        };

        // Inject mandatory first action for graph tasks with placeholder specs
        if let Some(ref ward_id) = setup.ward_id {
            let specs_dir = self
                .paths
                .vault_dir()
                .join("wards")
                .join(ward_id)
                .join("specs");
            if specs_dir.exists() {
                let has_placeholders = std::fs::read_dir(&specs_dir)
                    .ok()
                    .map(|entries| {
                        entries
                            .filter_map(|e| e.ok())
                            .filter(|e| e.path().is_dir())
                            .any(|topic_dir| {
                                std::fs::read_dir(topic_dir.path())
                                    .ok()
                                    .map(|files| {
                                        files.filter_map(|f| f.ok()).any(|f| {
                                            std::fs::read_to_string(f.path())
                                                .ok()
                                                .map(|c| c.contains("Status: placeholder"))
                                                .unwrap_or(false)
                                        })
                                    })
                                    .unwrap_or(false)
                            })
                    })
                    .unwrap_or(false);

                if has_placeholders {
                    history.push(ChatMessage::system(
                        "[MANDATORY FIRST ACTION] Placeholder specs found in the ward's specs/ folder. \
                         You MUST delegate to a planning subagent as your first action. \
                         Follow the pipeline in your planning shard: delegate to data-analyst with max_iterations=40 \
                         to fill the specs and analyze core/. Do NOT load skills, create plans, or write code yourself.".to_string()
                    ));
                    tracing::info!(ward = %ward_id, "Injected mandatory planning action for graph task");
                }
            }
        }

        // Spawn execution task
        self.spawn_execution_task(
            executor,
            handle_clone,
            config,
            message,
            session_id.clone(),
            execution_id,
            history,
            recommended_skills,
        );

        Ok((handle, session_id))
    }

    /// Spawn the async execution task.
    #[allow(clippy::too_many_arguments)]
    fn spawn_execution_task(
        &self,
        executor: AgentExecutor,
        handle: ExecutionHandle,
        config: ExecutionConfig,
        message: String,
        session_id: String,
        execution_id: String,
        history: Vec<ChatMessage>,
        recommended_skills: Vec<String>,
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
        let paths = self.paths.clone();
        let delegation_registry = self.delegation_registry.clone();
        let handles = self.handles.clone();
        let _skill_service = self.skill_service.clone();

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
                paths.vault_dir().clone(),
            )
            .with_batch_writer(batch_writer.clone())
            .with_recommended_skills(recommended_skills.clone());

            let mut response_acc = ResponseAccumulator::new();
            let mut tool_acc = ToolCallAccumulator::new();

            // Append user message to session stream BEFORE execution
            batch_writer.session_message(&session_id, &execution_id, "user", &message, None, None);

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
                        agent_runtime::StreamEvent::ToolCallStart {
                            tool_id,
                            tool_name,
                            args,
                            ..
                        } => {
                            tool_acc.start_call(tool_id.clone(), tool_name.clone(), args.clone());
                            // Accumulate tool call for the current assistant message
                            turn_tool_calls.push(serde_json::json!({
                                "tool_id": tool_id,
                                "tool_name": tool_name,
                                "args": args,
                            }));
                        }
                        agent_runtime::StreamEvent::ToolResult {
                            tool_id,
                            result,
                            error,
                            ..
                        } => {
                            tool_acc.complete_call(tool_id, result.clone(), error.clone());

                            // Emit the assistant message for this turn (with accumulated tool_calls)
                            if !turn_tool_calls.is_empty() {
                                let tc_json =
                                    serde_json::to_string(&turn_tool_calls).unwrap_or_default();
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

                // Log the response for session replay
                let response_log = api_logs::ExecutionLog::new(
                    &execution_id,
                    &session_id,
                    &agent_id,
                    api_logs::LogLevel::Info,
                    api_logs::LogCategory::Response,
                    &accumulated_response,
                );
                batch_writer.log(response_log);
            }

            // Handle completion
            match result {
                Ok(()) => {
                    // Check if this execution spawned delegations that are still active.
                    // Use session.pending_delegations (set synchronously in handle_delegation)
                    // rather than delegation_registry (populated asynchronously by spawn).
                    let has_active_delegations = state_service
                        .get_session(&session_id)
                        .ok()
                        .flatten()
                        .map(|s| s.has_pending_delegations())
                        .unwrap_or(false);

                    if has_active_delegations {
                        // Root paused for delegation — do NOT complete execution.
                        // The continuation callback will handle completion.
                        tracing::info!(
                            session_id = %session_id,
                            "Root paused for delegation — skipping execution completion"
                        );

                        // Request continuation so the session resumes when delegations complete
                        if let Err(e) = state_service.request_continuation(&session_id) {
                            tracing::warn!("Failed to request continuation: {}", e);
                        }

                        // Aggregate tokens so UI shows progress
                        if let Err(e) = state_service.aggregate_session_tokens(&session_id) {
                            tracing::warn!("Failed to aggregate session tokens: {}", e);
                        }
                    } else {
                        // Normal completion — no active delegations
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
                    }

                    // Auto-update ward AGENTS.md after root execution completes
                    // (scaffolding now happens at ward creation time in the WardChanged handler)
                    let session_ward = state_service
                        .get_session(&session_id)
                        .ok()
                        .flatten()
                        .and_then(|s| s.ward_id);
                    if let Some(ref ward_id) = session_ward {
                        auto_update_agents_md(paths.vault_dir(), ward_id);
                        auto_update_memory_bank(paths.vault_dir(), ward_id);
                    }

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

                    // Cancel any orphaned delegations for this session
                    cancel_session_delegations(
                        &session_id,
                        &delegation_registry,
                        &handles,
                        &state_service,
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
            Err(format!(
                "No active execution for conversation: {}",
                conversation_id
            ))
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
            Err(format!(
                "No active execution for conversation: {}",
                conversation_id
            ))
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

    /// Resume a paused or crashed execution by session ID.
    ///
    /// For crashed sessions with a crashed subagent: re-spawns only the crashed
    /// subagent using its child session's message history, avoiding root re-evaluation.
    /// For paused sessions or root-only crashes: falls through to current behavior.
    pub async fn resume(&self, session_id: &str) -> Result<(), String> {
        // Check for crashed subagent first
        if let Ok(Some(crashed_exec)) = self.state_service.get_last_crashed_subagent(session_id) {
            if crashed_exec.child_session_id.is_some() {
                tracing::info!(
                    session_id = %session_id,
                    crashed_agent = %crashed_exec.agent_id,
                    child_session = ?crashed_exec.child_session_id,
                    "Smart resume: re-spawning crashed subagent instead of root"
                );
                return self
                    .resume_crashed_subagent(session_id, &crashed_exec)
                    .await;
            }
        }

        // Fallback: standard resume (paused sessions or root-only crashes)
        self.state_service.resume_session(session_id)?;

        let handles = self.handles.read().await;
        for handle in handles.values() {
            handle.resume();
        }

        Ok(())
    }

    /// Re-spawn a crashed subagent without re-running the root agent.
    async fn resume_crashed_subagent(
        &self,
        session_id: &str,
        crashed_exec: &execution_state::AgentExecution,
    ) -> Result<(), String> {
        let child_session_id = crashed_exec
            .child_session_id
            .as_ref()
            .ok_or("No child_session_id on crashed execution")?;

        // 1. Reactivate root session and execution
        self.state_service.reactivate_session(session_id)?;
        if let Ok(Some(root_exec)) = self.state_service.get_root_execution(session_id) {
            self.state_service.reactivate_execution(&root_exec.id)?;
        }

        // 2. Cancel the old crashed execution
        self.state_service.cancel_execution(&crashed_exec.id)?;

        // 3. Reactivate the child session
        self.state_service.reactivate_session(child_session_id)?;

        // 4. Ensure pending_delegations is at least 1
        self.state_service.register_delegation(session_id)?;

        // 5. Request continuation so root agent processes the callback when subagent finishes
        self.state_service.request_continuation(session_id)?;

        // 6. Build DelegationRequest from crashed execution's data
        let parent_execution_id = crashed_exec
            .parent_execution_id
            .as_ref()
            .ok_or("No parent_execution_id on crashed execution")?;

        let task = crashed_exec
            .task
            .as_ref()
            .ok_or("No task on crashed execution")?;

        // Get root agent ID for parent_agent_id
        let root_agent_id = self
            .state_service
            .get_root_execution(session_id)?
            .map(|e| e.agent_id)
            .unwrap_or_else(|| "root".to_string());

        // Create new child execution
        let new_exec = execution_state::AgentExecution::new_delegated(
            session_id,
            &crashed_exec.agent_id,
            parent_execution_id,
            crashed_exec.delegation_type,
            task,
        );
        self.state_service.create_execution(&new_exec)?;
        self.state_service
            .set_child_session_id(&new_exec.id, child_session_id)?;

        let request = DelegationRequest {
            parent_agent_id: root_agent_id,
            session_id: session_id.to_string(),
            parent_execution_id: parent_execution_id.clone(),
            child_agent_id: crashed_exec.agent_id.clone(),
            child_execution_id: new_exec.id.clone(),
            task: task.clone(),
            context: None,
            max_iterations: None,
            output_schema: None,
            skills: vec![],
            complexity: None,
            parallel: false,
        };

        // 7. Re-spawn the subagent
        spawn_delegated_agent(
            &request,
            self.event_bus.clone(),
            self.agent_service.clone(),
            self.provider_service.clone(),
            self.mcp_service.clone(),
            self.skill_service.clone(),
            self.paths.clone(),
            self.conversation_repo.clone(),
            self.handles.clone(),
            self.delegation_registry.clone(),
            self.delegation_tx.clone(),
            self.log_service.clone(),
            self.state_service.clone(),
            self.workspace_cache.clone(),
            None, // No delegation permit needed for resume
            self.memory_repo.clone(),
            self.embedding_client.clone(),
            self.memory_recall.clone(),
            self.rate_limiters.clone(),
        )
        .await?;

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
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("0")
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
        self.delegation_registry
            .register(&child_conversation_id, delegation_context);

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
    #[allow(clippy::too_many_arguments)]
    ///
    /// Returns the executor and any recommended skill IDs from intent analysis
    /// (empty when analysis is skipped or fails).
    async fn create_executor(
        &self,
        agent: &gateway_services::agents::Agent,
        provider: &gateway_services::providers::Provider,
        config: &ExecutionConfig,
        session_id: &str,
        ward_id: Option<&str>,
        is_root: bool,
        user_message: Option<&str>,
        execution_id: &str,
    ) -> Result<(AgentExecutor, Vec<String>), String> {
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
        let fact_store: Option<Arc<dyn zero_core::MemoryFactStore>> =
            self.memory_repo.as_ref().map(|repo| {
                Arc::new(gateway_database::GatewayMemoryFactStore::new(
                    repo.clone(),
                    self.embedding_client.clone(),
                )) as Arc<dyn zero_core::MemoryFactStore>
            });
        // Clone for resource indexing (before fact_store is moved into builder)
        let fact_store_for_indexing = fact_store.clone();

        // Build connector resource provider (HTTP + bridge composite)
        let http_provider: Option<Arc<dyn zero_core::ConnectorResourceProvider>> =
            self.connector_registry.as_ref().map(|registry| {
                Arc::new(super::resource_provider::GatewayResourceProvider::new(
                    registry.clone(),
                )) as Arc<dyn zero_core::ConnectorResourceProvider>
            });
        let bridge_provider: Option<Arc<dyn zero_core::ConnectorResourceProvider>> = self
            .bridge_registry
            .as_ref()
            .zip(self.bridge_outbox.as_ref())
            .map(|(reg, outbox)| {
                Arc::new(gateway_bridge::BridgeResourceProvider::new(
                    reg.clone(),
                    outbox.clone(),
                )) as Arc<dyn zero_core::ConnectorResourceProvider>
            });
        let connector_provider: Option<Arc<dyn zero_core::ConnectorResourceProvider>> =
            if http_provider.is_some() || bridge_provider.is_some() {
                Some(
                    Arc::new(super::composite_provider::CompositeResourceProvider::new(
                        http_provider,
                        bridge_provider,
                    )) as Arc<dyn zero_core::ConnectorResourceProvider>,
                )
            } else {
                None
            };

        // Get or create shared rate limiter for this provider
        let rate_limiter = self.get_rate_limiter(provider);
        tracing::debug!(provider = %provider.name, "Using shared rate limiter for provider");

        // Use ExecutorBuilder to create the executor
        let mut builder = ExecutorBuilder::new(self.paths.vault_dir().clone(), tool_settings)
            .with_workspace_cache(self.workspace_cache.clone())
            .with_rate_limiter(rate_limiter)
            .with_fast_mode(config.is_fast_mode());
        if let Some(ref registry) = self.model_registry {
            builder = builder.with_model_registry(registry.clone());
        }
        if let Some(fs) = fact_store {
            builder = builder.with_fact_store(fs);
        }
        if let Some(cp) = connector_provider {
            builder = builder.with_connector_provider(cp);
        }

        // Intent analysis for root agent first turns only.
        // Note: execution_logs stores execution_id in the session_id column,
        // so we query by execution_id to find prior intent logs.
        let mut agent_for_build = agent.clone();
        let mut recommended_skills: Vec<String> = Vec::new();
        let already_analyzed = if is_root {
            self.log_service.has_intent_log(execution_id)
        } else {
            false
        };
        let is_fast_mode = config.is_fast_mode();
        if is_root && !already_analyzed && !is_fast_mode {
            if let Some(ref fs) = fact_store_for_indexing {
                // Index resources (fast DB upsert — no LLM call)
                index_resources(
                    fs.as_ref(),
                    &self.skill_service,
                    &self.agent_service,
                    &self.paths,
                )
                .await;
                tracing::info!("Resource indexing complete (skills, agents, wards)");

                // Run intent analysis if user message is present
                if let Some(msg) = user_message {
                    // Emit started event so UI can show "Analyzing..."
                    self.event_bus
                        .publish(gateway_events::GatewayEvent::IntentAnalysisStarted {
                            session_id: session_id.to_string(),
                            execution_id: execution_id.to_string(),
                        })
                        .await;

                    // Build temporary LLM client for analysis
                    let llm_config = agent_runtime::LlmConfig::new(
                        provider.base_url.clone(),
                        provider.api_key.clone(),
                        agent.model.clone(),
                        provider.id.clone().unwrap_or_else(|| provider.name.clone()),
                    )
                    .with_max_tokens(2048); // Intent analysis JSON is 1-2KB — keep max_tokens low for speed
                    match agent_runtime::OpenAiClient::new(llm_config) {
                        Ok(raw_client) => {
                            let retrying = agent_runtime::RetryingLlmClient::new(
                                std::sync::Arc::new(raw_client),
                                agent_runtime::RetryPolicy::default(),
                            );

                            match analyze_intent(
                                &retrying,
                                msg,
                                fs.as_ref(),
                                self.memory_recall.as_ref().map(|r| r.as_ref()),
                            )
                            .await
                            {
                                Ok(analysis) => {
                                    tracing::info!(
                                        primary_intent = %analysis.primary_intent,
                                        approach = %analysis.execution_strategy.approach,
                                        "Intent analysis succeeded"
                                    );

                                    // Emit IntentAnalysisComplete event
                                    self.event_bus
                                        .publish(GatewayEvent::IntentAnalysisComplete {
                                            session_id: session_id.to_string(),
                                            execution_id: execution_id.to_string(),
                                            primary_intent: analysis.primary_intent.clone(),
                                            hidden_intents: analysis.hidden_intents.clone(),
                                            recommended_skills: analysis.recommended_skills.clone(),
                                            recommended_agents: analysis.recommended_agents.clone(),
                                            ward_recommendation: serde_json::to_value(
                                                &analysis.ward_recommendation,
                                            )
                                            .unwrap_or_default(),
                                            execution_strategy: serde_json::to_value(
                                                &analysis.execution_strategy,
                                            )
                                            .unwrap_or_default(),
                                        })
                                        .await;

                                    // Log for session replay
                                    if let Ok(meta) = serde_json::to_value(&analysis) {
                                        let log_entry = api_logs::ExecutionLog::new(
                                            execution_id,
                                            session_id,
                                            &config.agent_id,
                                            api_logs::LogLevel::Info,
                                            api_logs::LogCategory::Intent,
                                            format!("Intent: {}", analysis.primary_intent),
                                        )
                                        .with_metadata(meta);
                                        let _ = self.log_service.log(log_entry);
                                    }

                                    // Capture recommended skills for post-execution scaffolding
                                    recommended_skills = analysis.recommended_skills.clone();

                                    // Collect spec guidance from recommended skills' ward_setup
                                    let spec_guidance = {
                                        let mut guidances = Vec::new();
                                        for skill_name in &analysis.recommended_skills {
                                            if let Ok(Some(ws)) =
                                                self.skill_service.get_ward_setup(skill_name).await
                                            {
                                                if let Some(ref g) = ws.spec_guidance {
                                                    guidances.push(g.clone());
                                                }
                                            }
                                        }
                                        if guidances.is_empty() {
                                            None
                                        } else {
                                            Some(guidances.join("\n\n"))
                                        }
                                    };

                                    // Inject intent analysis into agent instructions
                                    // so the agent can follow ward/skill/strategy recommendations
                                    agent_for_build.instructions.push_str(
                                        &format_intent_injection(
                                            &analysis,
                                            spec_guidance.as_deref(),
                                            user_message,
                                        ),
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!("Intent analysis failed (non-fatal): {}", e);
                                    // Fallback: emit minimal analysis so UI gets a block
                                    // and agent receives ward naming guidance
                                    self.event_bus
                                        .publish(GatewayEvent::IntentAnalysisComplete {
                                            session_id: session_id.to_string(),
                                            execution_id: execution_id.to_string(),
                                            primary_intent: "general".to_string(),
                                            hidden_intents: vec![],
                                            recommended_skills: vec![],
                                            recommended_agents: vec![],
                                            ward_recommendation: serde_json::json!({
                                                "action": "create_new",
                                                "ward_name": "scratch",
                                                "subdirectory": null,
                                                "reason": "Intent analysis failed — using scratch ward"
                                            }),
                                            execution_strategy: serde_json::json!({
                                                "approach": "simple",
                                                "explanation": "Intent analysis unavailable"
                                            }),
                                        })
                                        .await;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to create LLM client for intent analysis: {}",
                                e
                            );
                            // Fallback: emit minimal analysis event
                            self.event_bus
                                .publish(GatewayEvent::IntentAnalysisComplete {
                                    session_id: session_id.to_string(),
                                    execution_id: execution_id.to_string(),
                                    primary_intent: "general".to_string(),
                                    hidden_intents: vec![],
                                    recommended_skills: vec![],
                                    recommended_agents: vec![],
                                    ward_recommendation: serde_json::json!({
                                        "action": "create_new",
                                        "ward_name": "scratch",
                                        "subdirectory": null,
                                        "reason": "LLM client creation failed — using scratch ward"
                                    }),
                                    execution_strategy: serde_json::json!({
                                        "approach": "simple",
                                        "explanation": "Intent analysis unavailable (no LLM client)"
                                    }),
                                })
                                .await;
                        }
                    }
                }
            }
        }

        // Flag if placeholder specs exist — delegate tool uses this to block ad-hoc delegations
        if is_root {
            if let Some(wid) = ward_id {
                let specs_dir = self.paths.vault_dir().join("wards").join(wid).join("specs");
                if specs_dir.exists() {
                    let has_placeholders = std::fs::read_dir(&specs_dir)
                        .ok()
                        .map(|entries| entries.filter_map(|e| e.ok()).any(|e| e.path().is_dir()))
                        .unwrap_or(false);
                    if has_placeholders {
                        builder = builder.with_initial_state(
                            "app:has_placeholder_specs",
                            serde_json::Value::Bool(true),
                        );
                    }
                }
            }
        }

        let mut executor = builder
            .build(
                &agent_for_build,
                provider,
                &config.conversation_id,
                session_id,
                &available_agents,
                &available_skills,
                hook_context.as_ref(),
                &self.mcp_service,
                ward_id,
            )
            .await?;

        // Wire mid-session recall hook so the executor refreshes memory every N turns.
        if let Some(recall) = &self.memory_recall {
            let mid_cfg = &recall.config().mid_session_recall;
            if mid_cfg.enabled {
                let recall = Arc::clone(recall);
                let agent_id = agent.id.clone();
                let ward = ward_id.map(String::from);
                let min_novelty = mid_cfg.min_novelty_score;
                let every_n = mid_cfg.every_n_turns as u32;

                executor.set_recall_hook(
                    Box::new(
                        move |query: &str, already_injected: &std::collections::HashSet<String>| {
                            let recall = Arc::clone(&recall);
                            let agent_id = agent_id.clone();
                            let ward = ward.clone();
                            let query = query.to_string();
                            let already_injected = already_injected.clone();
                            Box::pin(async move {
                                let facts =
                                    recall.recall(&agent_id, &query, 5, ward.as_deref()).await?;
                                // Filter out already-injected facts and low-novelty results
                                let novel: Vec<_> = facts
                                    .into_iter()
                                    .filter(|f| !already_injected.contains(&f.fact.key))
                                    .filter(|f| f.score >= min_novelty)
                                    .collect();
                                if novel.is_empty() {
                                    return Ok(agent_runtime::RecallHookResult {
                                        system_message: String::new(),
                                        fact_keys: Vec::new(),
                                    });
                                }
                                let keys: Vec<String> =
                                    novel.iter().map(|f| f.fact.key.clone()).collect();
                                let lines: Vec<String> = novel
                                    .iter()
                                    .map(|f| format!("- [{}] {}", f.fact.category, f.fact.content))
                                    .collect();
                                Ok(agent_runtime::RecallHookResult {
                                    system_message: format!(
                                        "[Memory Refresh] Relevant facts for current context:\n{}",
                                        lines.join("\n")
                                    ),
                                    fact_keys: keys,
                                })
                            })
                        },
                    ),
                    every_n,
                    std::collections::HashSet::new(),
                );
                tracing::debug!(every_n_turns = every_n, "Mid-session recall hook wired");
            }
        }

        Ok((executor, recommended_skills))
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
    memory_repo: Option<Arc<gateway_database::MemoryRepository>>,
    embedding_client: Option<Arc<dyn agent_runtime::llm::embedding::EmbeddingClient>>,
    distiller: Option<Arc<super::distillation::SessionDistiller>>,
    memory_recall: Option<Arc<super::recall::MemoryRecall>>,
    model_registry: Option<Arc<gateway_services::models::ModelRegistry>>,
) -> Result<(), String> {
    // Generate a new conversation ID for this continuation turn
    let conversation_id = format!(
        "{}-cont-{}",
        session_id,
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("0")
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
    let _ = log_service.log_session_start(&execution_id, &conversation_id, root_agent_id, None);

    // Create execution handle
    let handle = ExecutionHandle::new(50); // Default max iterations for continuation
    {
        let mut handles_guard = handles.write().await;
        handles_guard.insert(conversation_id.clone(), handle.clone());
    }

    // Emit agent started event
    emit_agent_started(
        &event_bus,
        root_agent_id,
        &conversation_id,
        session_id,
        &execution_id,
    )
    .await;

    // Load agent and provider (with orchestrator config from settings)
    let settings_for_loader = gateway_services::SettingsService::new(paths.clone());
    let agent_loader = AgentLoader::new(&agent_service, &provider_service, paths.clone())
        .with_settings(&settings_for_loader);
    let (agent, provider) = agent_loader.load_or_create_root(root_agent_id).await?;

    // Load full session conversation (includes tool calls, results, and callbacks)
    let mut history: Vec<ChatMessage> = conversation_repo
        .get_session_conversation(session_id, 200)
        .map(|messages| conversation_repo.session_messages_to_chat_format(&messages))
        .unwrap_or_default();

    // Look up active ward from session (needed for recall ward affinity)
    let session_ward_id = state_service
        .get_session(session_id)
        .ok()
        .flatten()
        .and_then(|s| s.ward_id);

    // Recall domain-relevant facts for continuation context.
    // Use the last user message from history as the recall query (instead of a
    // hardcoded placeholder) so the recalled facts are relevant to the actual task.
    let continuation_recall_query = history
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.text_content())
        .unwrap_or_else(|| "continuation recall".to_string());

    if let Some(recall) = &memory_recall {
        match recall
            .recall_with_graph(
                root_agent_id,
                &continuation_recall_query,
                5,
                session_ward_id.as_deref(),
                Some(session_id),
            )
            .await
        {
            Ok(result) if !result.facts.is_empty() || !result.episodes.is_empty() => {
                history.insert(0, ChatMessage::system(result.formatted));
                tracing::info!(
                    fact_count = result.facts.len(),
                    episode_count = result.episodes.len(),
                    "Recalled facts and episodes for continuation"
                );
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("Continuation recall failed: {}", e),
        }
    }

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

    // Auto-update ward AGENTS.md before continuation
    if let Some(ref ward_id) = session_ward_id {
        auto_update_agents_md(paths.vault_dir(), ward_id);
        auto_update_memory_bank(paths.vault_dir(), ward_id);
    }

    // Build executor
    let mut builder = ExecutorBuilder::new(paths.vault_dir().clone(), tool_settings)
        .with_workspace_cache(workspace_cache);
    if let Some(registry) = model_registry {
        builder = builder.with_model_registry(registry);
    }

    // Build fact store for continuation (so save_fact uses DB, not file fallback)
    let fact_store: Option<Arc<dyn zero_core::MemoryFactStore>> =
        memory_repo.as_ref().map(|repo| {
            Arc::new(gateway_database::GatewayMemoryFactStore::new(
                repo.clone(),
                embedding_client.clone(),
            )) as Arc<dyn zero_core::MemoryFactStore>
        });
    if let Some(fs) = fact_store {
        builder = builder.with_fact_store(fs);
    }

    let mut executor = builder
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

    // Wire mid-session recall hook for continuation executor.
    if let Some(recall) = &memory_recall {
        let mid_cfg = &recall.config().mid_session_recall;
        if mid_cfg.enabled {
            let recall = Arc::clone(recall);
            let agent_id = root_agent_id.to_string();
            let ward = session_ward_id.clone();
            let min_novelty = mid_cfg.min_novelty_score;
            let every_n = mid_cfg.every_n_turns as u32;

            executor.set_recall_hook(
                Box::new(
                    move |query: &str, already_injected: &std::collections::HashSet<String>| {
                        let recall = Arc::clone(&recall);
                        let agent_id = agent_id.clone();
                        let ward = ward.clone();
                        let query = query.to_string();
                        let already_injected = already_injected.clone();
                        Box::pin(async move {
                            let facts =
                                recall.recall(&agent_id, &query, 5, ward.as_deref()).await?;
                            let novel: Vec<_> = facts
                                .into_iter()
                                .filter(|f| !already_injected.contains(&f.fact.key))
                                .filter(|f| f.score >= min_novelty)
                                .collect();
                            if novel.is_empty() {
                                return Ok(agent_runtime::RecallHookResult {
                                    system_message: String::new(),
                                    fact_keys: Vec::new(),
                                });
                            }
                            let keys: Vec<String> =
                                novel.iter().map(|f| f.fact.key.clone()).collect();
                            let lines: Vec<String> = novel
                                .iter()
                                .map(|f| format!("- [{}] {}", f.fact.category, f.fact.content))
                                .collect();
                            Ok(agent_runtime::RecallHookResult {
                                system_message: format!(
                                    "[Memory Refresh] Relevant facts for current context:\n{}",
                                    lines.join("\n")
                                ),
                                fact_keys: keys,
                            })
                        })
                    },
                ),
                every_n,
                std::collections::HashSet::new(),
            );
            tracing::debug!(
                every_n_turns = every_n,
                "Mid-session recall hook wired (continuation)"
            );
        }
    }

    // Build a focused continuation message with the plan injected.
    // Search specs/**/plan.md (planner saves to specs/{domain_task}/plan.md).
    let continuation_message = {
        let plan_hint = session_ward_id.as_ref().and_then(|ward_id| {
            let specs_dir = paths.vault_dir().join("wards").join(ward_id).join("specs");
            find_latest_plan(&specs_dir)
        });

        if let Some(plan) = plan_hint {
            format!(
                "[DELEGATION COMPLETED. YOUR PLAN IS BELOW.\n\
                 DO NOT read files. DO NOT analyze. DO NOT use shell.\n\
                 Just find the next step that hasn't been done and delegate it NOW.\n\
                 One action only: delegate_to_agent.]\n\n{}",
                plan
            )
        } else {
            "[Delegation completed. Delegate the next step in your plan immediately. \
             Do NOT read files or analyze — just delegate.]"
                .to_string()
        }
    };

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
            paths.vault_dir().clone(),
        )
        .with_batch_writer(batch_writer.clone());

        let mut response_acc = ResponseAccumulator::new();
        let mut tool_acc = ToolCallAccumulator::new();

        // Append continuation system message to session stream
        batch_writer.session_message(
            &session_id_clone,
            &execution_id,
            "system",
            &continuation_message,
            None,
            None,
        );

        let session_id_inner = session_id_clone.clone();
        let execution_id_inner = execution_id.clone();
        let batch_writer_inner = batch_writer.clone();
        let mut turn_tool_calls: Vec<serde_json::Value> = Vec::new();
        let mut turn_text = String::new();

        let result = executor
            .execute_stream(&continuation_message, &history, |event| {
                if handle.is_stop_requested() {
                    return;
                }

                handle.increment();

                // Stream messages to session as they happen
                match &event {
                    agent_runtime::StreamEvent::ToolCallStart {
                        tool_id,
                        tool_name,
                        args,
                        ..
                    } => {
                        tool_acc.start_call(tool_id.clone(), tool_name.clone(), args.clone());
                        turn_tool_calls.push(serde_json::json!({
                            "tool_id": tool_id,
                            "tool_name": tool_name,
                            "args": args,
                        }));
                    }
                    agent_runtime::StreamEvent::ToolResult {
                        tool_id,
                        result,
                        error,
                        ..
                    } => {
                        tool_acc.complete_call(tool_id, result.clone(), error.clone());

                        // Emit assistant message for this turn
                        if !turn_tool_calls.is_empty() {
                            let tc_json =
                                serde_json::to_string(&turn_tool_calls).unwrap_or_default();
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
                // Check if this continuation spawned new delegations
                let has_active_delegations = state_service
                    .get_session(&session_id_clone)
                    .ok()
                    .flatten()
                    .map(|s| s.has_pending_delegations())
                    .unwrap_or(false);

                if has_active_delegations {
                    // Root delegated again — wait for subagent, don't complete
                    tracing::info!(
                        session_id = %session_id_clone,
                        "Continuation paused for delegation — skipping execution completion"
                    );
                    if let Err(e) = state_service.request_continuation(&session_id_clone) {
                        tracing::warn!("Failed to request continuation: {}", e);
                    }
                    if let Err(e) = state_service.aggregate_session_tokens(&session_id_clone) {
                        tracing::warn!("Failed to aggregate session tokens: {}", e);
                    }
                } else {
                    // No more delegations — complete normally
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
                        None,
                        None,
                        None,
                    )
                    .await;
                }

                // Fire-and-forget session distillation
                if let Some(distiller) = distiller {
                    let sid = session_id_clone.clone();
                    let aid = agent_id_clone.clone();
                    tokio::spawn(async move {
                        if let Err(e) = distiller.distill(&sid, &aid).await {
                            tracing::warn!("Continuation distillation failed: {}", e);
                        }
                    });
                }
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

// ============================================================================
// ORPHAN DELEGATION CLEANUP
// ============================================================================

/// Cancel all in-flight delegations for a session.
/// Called when root execution completes or crashes to prevent orphaned subagents.
async fn cancel_session_delegations(
    session_id: &str,
    delegation_registry: &crate::delegation::DelegationRegistry,
    handles: &tokio::sync::RwLock<
        std::collections::HashMap<String, crate::handle::ExecutionHandle>,
    >,
    state_service: &execution_state::StateService<gateway_database::DatabaseManager>,
) {
    let active = delegation_registry.get_by_session_id(session_id);

    if active.is_empty() {
        return;
    }

    tracing::info!(
        session_id = %session_id,
        count = active.len(),
        "Cancelling orphaned delegations"
    );

    for (child_conv_id, _ctx) in &active {
        // Stop the execution handle
        {
            let handles_guard = handles.read().await;
            if let Some(handle) = handles_guard.get(child_conv_id) {
                handle.stop();
            }
        }

        // Remove from registry
        delegation_registry.remove(child_conv_id);

        // Decrement pending_delegations so session can complete
        if let Err(e) = state_service.complete_delegation(session_id) {
            tracing::debug!("Failed to decrement pending_delegations: {}", e);
        }
    }
}

// ============================================================================
// WARD AGENTS.MD AUTO-UPDATE
// ============================================================================

/// Auto-update ward AGENTS.md by scanning the directory structure.
/// Find the most recent plan.md under a specs/ directory.
/// Planner saves to specs/{domain_task}/plan.md — we glob for it.
fn find_latest_plan(specs_dir: &std::path::Path) -> Option<String> {
    if !specs_dir.exists() {
        return None;
    }

    let mut newest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;

    // Search specs/*/plan.md and specs/plan.md
    if let Ok(entries) = std::fs::read_dir(specs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Direct specs/plan.md
            if path.is_file() && path.file_name().map(|f| f == "plan.md").unwrap_or(false) {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if newest.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
                            newest = Some((modified, path));
                        }
                    }
                }
            } else if path.is_dir() {
                // specs/{subdir}/plan.md
                let plan_path = path.join("plan.md");
                if plan_path.exists() {
                    if let Ok(meta) = plan_path.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if newest.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
                                newest = Some((modified, plan_path));
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some((_, path)) = newest {
        let content = std::fs::read_to_string(&path).ok()?;
        if content.trim().is_empty() {
            return None;
        }
        tracing::info!(path = %path.display(), "Injecting plan into continuation message");
        Some(content)
    } else {
        None
    }
}

/// Called by the system after delegations complete, before continuation.
/// Extract Python function signatures from a .py file.
/// Returns lines like `def fetch_ohlcv(ticker: str, period: str = "1y") -> pd.DataFrame`
/// stripped of the trailing colon.
fn extract_function_signatures(file_path: &std::path::Path) -> Vec<String> {
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut signatures = Vec::new();
    let mut in_def = false;
    let mut current_def = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("def ") {
            // Start of a function definition
            in_def = true;
            current_def = trimmed.to_string();
            if current_def.contains(')') {
                // Single-line def — strip trailing `:` or ` -> ...:` keeping the return annotation
                if let Some(pos) = current_def.rfind("):") {
                    // e.g. `def foo(x: int) -> str:` — keep up to `)` then check for return annotation
                    let after_paren = &current_def[pos + 1..];
                    if after_paren.contains("->") {
                        // Include the return annotation, strip the final `:`
                        let full = format!(
                            "{}{}",
                            &current_def[..pos + 1],
                            after_paren[..after_paren.len()]
                                .trim_end_matches(':')
                                .trim_end()
                        );
                        signatures.push(full);
                    } else {
                        signatures.push(current_def[..pos + 1].trim().to_string());
                    }
                } else if let Some(pos) = current_def.find(':') {
                    signatures.push(current_def[..pos].trim().to_string());
                } else {
                    signatures.push(current_def.clone());
                }
                in_def = false;
                current_def.clear();
            }
        } else if in_def {
            current_def.push(' ');
            current_def.push_str(trimmed);
            if current_def.contains(')') {
                if let Some(pos) = current_def.rfind("):") {
                    let after_paren = &current_def[pos + 1..];
                    if after_paren.contains("->") {
                        let full = format!(
                            "{}{}",
                            &current_def[..pos + 1],
                            after_paren.trim_end_matches(':').trim_end()
                        );
                        signatures.push(full);
                    } else {
                        signatures.push(current_def[..pos + 1].trim().to_string());
                    }
                } else if let Some(pos) = current_def.find(':') {
                    signatures.push(current_def[..pos].trim().to_string());
                } else {
                    signatures.push(current_def.clone());
                }
                in_def = false;
                current_def.clear();
            }
        }
    }

    signatures
}

/// Extract the first-line docstring from a Python file.
fn extract_first_docstring(file_path: &std::path::Path) -> String {
    std::fs::read_to_string(file_path)
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|l| l.starts_with("\"\"\"") || l.starts_with("'''"))
                .map(|l| {
                    l.trim_start_matches("\"\"\"")
                        .trim_start_matches("'''")
                        .trim_end_matches("\"\"\"")
                        .trim_end_matches("'''")
                        .trim()
                        .to_string()
                })
        })
        .unwrap_or_default()
}

/// Preserve the `## Purpose` section from an existing AGENTS.md, falling back to a default.
fn extract_purpose_section(agents_md_path: &std::path::Path, ward_id: &str) -> String {
    if let Ok(existing) = std::fs::read_to_string(agents_md_path) {
        let mut in_purpose = false;
        let mut purpose_lines = Vec::new();
        for line in existing.lines() {
            if line.starts_with("## Purpose") {
                in_purpose = true;
                continue;
            }
            if in_purpose {
                if line.starts_with("## ") {
                    break;
                }
                purpose_lines.push(line.to_string());
            }
        }
        // Trim leading/trailing blank lines
        let text: String = purpose_lines.join("\n");
        let text = text.trim().to_string();
        if !text.is_empty() {
            return text;
        }
    }
    format!("Domain workspace for {} projects.", ward_id)
}

fn extract_conventions_section(agents_md_path: &std::path::Path) -> Option<Vec<String>> {
    let content = std::fs::read_to_string(agents_md_path).ok()?;
    let mut in_conventions = false;
    let mut conventions = Vec::new();
    for line in content.lines() {
        if line.starts_with("## Conventions") {
            in_conventions = true;
            continue;
        }
        if in_conventions {
            if line.starts_with("## ") {
                break;
            }
            let trimmed = line.trim();
            if trimmed.starts_with("- ") {
                conventions.push(trimmed.to_string());
            }
        }
    }
    if conventions.is_empty() {
        None
    } else {
        Some(conventions)
    }
}

/// Format a byte count as a human-readable size string (e.g. "125 KB", "8 KB", "1.2 MB").
fn format_file_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} B", bytes)
    }
}

/// Collect data files (.csv, .json, .txt, .html, .parquet) recursively under a directory.
/// Returns `(relative_path, size_in_bytes)` pairs, relative to `base_dir`.
fn collect_data_files(dir: &std::path::Path, base_dir: &std::path::Path) -> Vec<(String, u64)> {
    let data_extensions = ["csv", "json", "txt", "html", "parquet", "xlsx", "pkl"];
    let mut result = Vec::new();
    collect_data_files_recursive(dir, base_dir, &data_extensions, &mut result);
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

fn collect_data_files_recursive(
    dir: &std::path::Path,
    base_dir: &std::path::Path,
    extensions: &[&str],
    result: &mut Vec<(String, u64)>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "__pycache__" {
            continue;
        }
        if path.is_dir() {
            collect_data_files_recursive(&path, base_dir, extensions, result);
        } else if path.is_file() {
            let matches_ext = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| extensions.contains(&ext))
                .unwrap_or(false);
            if matches_ext {
                let rel = path
                    .strip_prefix(base_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                result.push((rel, size));
            }
        }
    }
}

/// Auto-update AGENTS.md using language configs for core module indexing.
///
/// This is the primary implementation. It accepts a `lang_configs_dir` path so that
/// callers (and integration tests) can supply a custom config directory. Language
/// configs are loaded from that directory; files whose extension matches a config use
/// the config's `extract_signatures` / `extract_first_docstring` methods. Files with
/// no matching config fall back to the hardcoded Python extraction helpers.
pub fn auto_update_agents_md_with_lang_configs(
    vault_dir: &std::path::Path,
    ward_id: &str,
    lang_configs_dir: &std::path::Path,
) {
    let ward_dir = vault_dir.join("wards").join(ward_id);
    let agents_md_path = ward_dir.join("AGENTS.md");

    if !ward_dir.exists() || ward_id == "scratch" {
        return;
    }

    let lang_configs = {
        let raw = gateway_services::lang_config::load_all_lang_configs(lang_configs_dir)
            .unwrap_or_default();
        gateway_services::lang_config::compile_all(&raw)
    };

    let mut sections = Vec::new();

    // ── Title ──
    sections.push(format!("# {}\n", ward_id));

    // ── Purpose (preserved from existing AGENTS.md) ──
    let purpose = extract_purpose_section(&agents_md_path, ward_id);
    sections.push(format!("\n## Purpose\n{}\n", purpose));

    // ── Read These First ──
    let memory_bank_exists = ward_dir.join("memory-bank").exists();
    if memory_bank_exists {
        sections.push("## Read These First\n".to_string());
        sections.push(
            "Before writing any code, read these files to understand the ward:\n".to_string(),
        );

        sections.push("- [memory-bank/ward.md](memory-bank/ward.md) — Domain knowledge, patterns, and session learnings\n".to_string());

        if ward_dir.join("memory-bank").join("structure.md").exists() {
            sections.push("- [memory-bank/structure.md](memory-bank/structure.md) — Directory layout and tech stack\n".to_string());
        }

        if ward_dir.join("memory-bank").join("core_docs.md").exists() {
            sections.push("- [memory-bank/core_docs.md](memory-bank/core_docs.md) — Core module functions and usage\n".to_string());
        }

        // List any other docs in memory-bank/
        if let Ok(entries) = std::fs::read_dir(ward_dir.join("memory-bank")) {
            let mut docs: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    e.path().is_file()
                        && name.ends_with(".md")
                        && name != "ward.md"
                        && name != "structure.md"
                        && name != "core_docs.md"
                })
                .collect();
            docs.sort_by_key(|e| e.file_name());
            for entry in &docs {
                let name = entry.file_name().to_string_lossy().to_string();
                sections.push(format!("- [memory-bank/{}](memory-bank/{}) \n", name, name));
            }
        }
        sections.push("\n".to_string());
    }

    // ── Core Modules with function signatures ──
    let core_dir = ward_dir.join("core");
    if core_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&core_dir) {
            let mut modules: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let path = e.path();
                    if !path.is_file() {
                        return false;
                    }
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || name == "__init__.py" {
                        return false;
                    }
                    let ext = path.extension().and_then(|ex| ex.to_str()).unwrap_or("");
                    // Accept if any lang config matches, or if it's .py (hardcoded fallback)
                    gateway_services::lang_config::CompiledLangConfig::find_for_extension(
                        &lang_configs,
                        ext,
                    )
                    .is_some()
                        || ext == "py"
                })
                .collect();
            modules.sort_by_key(|e| e.file_name());

            if !modules.is_empty() {
                sections.push("\n## Core Modules\n".to_string());
                for entry in &modules {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    let ext = path.extension().and_then(|ex| ex.to_str()).unwrap_or("");

                    sections.push(format!("### core/{}\n", name));

                    if let Some(config) =
                        gateway_services::lang_config::CompiledLangConfig::find_for_extension(
                            &lang_configs,
                            ext,
                        )
                    {
                        // Language config path: use config's extraction methods
                        let desc = config.extract_first_docstring(&path).unwrap_or_default();
                        if !desc.is_empty() {
                            sections.push(format!("{}\n", desc));
                        }
                        for sig in config.extract_signatures(&path) {
                            sections.push(format!("- `{}`\n", sig));
                        }
                    } else {
                        // Fallback: hardcoded Python extraction (only reached for .py without a lang config)
                        let desc = extract_first_docstring(&path);
                        if !desc.is_empty() {
                            sections.push(format!("{}\n", desc));
                        }
                        for sig in extract_function_signatures(&path) {
                            let display = sig.strip_prefix("def ").unwrap_or(&sig).to_string();
                            sections.push(format!("- `{}`\n", display));
                        }
                    }
                    sections.push("\n".to_string());
                }
            }
        }
    }

    // ── Conventions (preserved from existing AGENTS.md) ──
    if let Some(conventions) = extract_conventions_section(&agents_md_path) {
        sections.push("\n## Conventions\n".to_string());
        for item in &conventions {
            sections.push(format!("{}\n", item));
        }
    }

    // ── Available Data (scan task dirs + output for data files) ──
    let mut data_files = Vec::new();

    // Scan task directories for data files
    if let Ok(entries) = std::fs::read_dir(&ward_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir()
                && !["core", "output", "__pycache__", ".git"].contains(&name.as_str())
                && !name.starts_with('.')
            {
                let mut found = collect_data_files(&path, &ward_dir);
                data_files.append(&mut found);
            }
        }
    }

    // Also scan output/ for data files
    let output_dir = ward_dir.join("output");
    if output_dir.exists() {
        let mut found = collect_data_files(&output_dir, &ward_dir);
        data_files.append(&mut found);
    }

    data_files.sort_by(|a, b| a.0.cmp(&b.0));
    data_files.dedup_by(|a, b| a.0 == b.0);

    if !data_files.is_empty() {
        sections.push("## Available Data\n".to_string());
        for (rel_path, size) in &data_files {
            sections.push(format!("- `{}` ({})\n", rel_path, format_file_size(*size)));
        }
        sections.push("\n".to_string());
    }

    // ── Task Directories ──
    let mut task_dirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&ward_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir()
                && !["core", "output", "__pycache__", ".git"].contains(&name.as_str())
                && !name.starts_with('.')
            {
                // Recurse one level to find task subdirs
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub in sub_entries.filter_map(|e| e.ok()) {
                        if sub.path().is_dir() {
                            let sub_name = sub.file_name().to_string_lossy().to_string();
                            if !sub_name.starts_with('.') && sub_name != "__pycache__" {
                                task_dirs.push(format!("{}/{}", name, sub_name));
                            }
                        }
                    }
                }
                // Also include the first-level dir itself if it has files
                let has_files = std::fs::read_dir(&path)
                    .ok()
                    .map(|rd| rd.filter_map(|e| e.ok()).any(|e| e.path().is_file()))
                    .unwrap_or(false);
                if has_files {
                    task_dirs.push(name);
                }
            }
        }
    }
    if !task_dirs.is_empty() {
        task_dirs.sort();
        sections.push("## Task Directories\n".to_string());
        for dir in &task_dirs {
            sections.push(format!("- `{}/`\n", dir));
        }
        sections.push("\n".to_string());
    }

    // ── Output ──
    if output_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&output_dir) {
            let mut files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .collect();
            files.sort_by_key(|e| e.file_name());
            if !files.is_empty() {
                sections.push("## Output\n".to_string());
                for entry in &files {
                    let name = entry.file_name().to_string_lossy().to_string();
                    sections.push(format!("- `output/{}`\n", name));
                }
                sections.push("\n".to_string());
            }
        }
    }

    // ── Specs & Plans ──
    let has_specs = ward_dir.join("specs").exists();
    let has_plans = ward_dir.join("plans").exists();
    if has_specs || has_plans {
        sections.push("## Specs & Plans\n".to_string());
        // List spec directories
        if has_specs {
            if let Ok(entries) = std::fs::read_dir(ward_dir.join("specs")) {
                let mut spec_topics: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                spec_topics.sort();
                for topic in &spec_topics {
                    let spec_count = std::fs::read_dir(ward_dir.join("specs").join(topic))
                        .ok()
                        .map(|entries| {
                            entries
                                .filter_map(|e| e.ok())
                                .filter(|e| e.path().is_file())
                                .count()
                        })
                        .unwrap_or(0);
                    if spec_count > 0 {
                        sections.push(format!("- `specs/{}/` — {} spec(s)\n", topic, spec_count));
                    }
                }
            }
        }
        if has_plans {
            if let Ok(entries) = std::fs::read_dir(ward_dir.join("plans")) {
                let mut plan_topics: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                plan_topics.sort();
                for topic in &plan_topics {
                    let plan_count = std::fs::read_dir(ward_dir.join("plans").join(topic))
                        .ok()
                        .map(|entries| {
                            entries
                                .filter_map(|e| e.ok())
                                .filter(|e| e.path().is_file())
                                .count()
                        })
                        .unwrap_or(0);
                    if plan_count > 0 {
                        sections.push(format!("- `plans/{}/` — {} plan(s)\n", topic, plan_count));
                    }
                }
            }
        }
        sections.push("\n".to_string());
    }

    // ── How to Code ──
    // Determine an example module name for the import example
    let example_import = std::fs::read_dir(&core_dir).ok().and_then(|mut entries| {
        entries.find_map(|e| {
            let e = e.ok()?;
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".py") && name != "__init__.py" {
                let module = name.trim_end_matches(".py").to_string();
                let first_fn = extract_function_signatures(&e.path())
                    .first()
                    .and_then(|sig| {
                        // Extract just the function name from `def func_name(...)`
                        sig.strip_prefix("def ")
                            .and_then(|rest| rest.split('(').next())
                            .map(|s| s.to_string())
                    });
                Some((module, first_fn))
            } else {
                None
            }
        })
    });

    let _import_example = match example_import {
        Some((module, Some(func))) => format!("`from core.{} import {}`", module, func),
        Some((module, None)) => format!("`from core.{} import ...`", module),
        None => "`from core.<module> import <function>`".to_string(),
    };

    // Determine an example task dir prefix for the coding guide
    let _task_dir_hint = task_dirs
        .first()
        .map(|d| {
            // Use the top-level portion, e.g. "stocks/spy" -> "stocks/{ticker}"
            if let Some(slash) = d.find('/') {
                format!("{}/{{name}}", &d[..slash])
            } else {
                format!("{}/", d)
            }
        })
        .unwrap_or_else(|| "tasks/{name}/".to_string());

    // ── Task Runner ──
    let ralph_exists = ward_dir.join("ralph.py").exists();
    if ralph_exists {
        sections.push("## Task Runner (ralph.py)\n".to_string());
        sections.push("Use `ralph.py` to process `tasks.json` files in specs/:\n".to_string());
        sections.push("```\n".to_string());
        sections
            .push("python3 ralph.py next <tasks.json>       # Get next pending task\n".to_string());
        sections
            .push("python3 ralph.py complete <tasks.json> N  # Mark task N complete\n".to_string());
        sections
            .push("python3 ralph.py fail <tasks.json> N msg  # Mark task N failed\n".to_string());
        sections.push(
            "python3 ralph.py status <tasks.json>      # Show progress summary\n".to_string(),
        );
        sections.push("```\n\n".to_string());
    }

    // ── How to Code ──
    sections.push("## How to Code\n".to_string());
    sections
        .push("1. Reusable functions → core/. Task scripts → task subdirectories.\n".to_string());
    sections.push("2. Import from core/ — never duplicate existing modules.\n".to_string());
    sections.push(
        "3. Use write_file to create files, edit_file for changes. Keep files under 3KB.\n"
            .to_string(),
    );
    sections.push("4. Update memory-bank/core_docs.md with full function signatures after creating core modules.\n".to_string());

    // ── Timestamp ──
    sections.push(format!(
        "\n*Auto-updated: {}*\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));

    let content = sections.join("");
    if let Err(e) = std::fs::write(&agents_md_path, &content) {
        tracing::warn!(ward = %ward_id, error = %e, "Failed to auto-update AGENTS.md");
    } else {
        tracing::info!(ward = %ward_id, "Auto-updated AGENTS.md");
    }
}

fn auto_update_agents_md(vault_dir: &std::path::Path, ward_id: &str) {
    let lang_configs_dir = vault_dir.join("config").join("wards");
    auto_update_agents_md_with_lang_configs(vault_dir, ward_id, &lang_configs_dir);
}

/// Auto-generate memory-bank/structure.md and core_docs.md for a ward.
pub fn auto_update_memory_bank(vault_dir: &std::path::Path, ward_id: &str) {
    let ward_dir = vault_dir.join("wards").join(ward_id);
    let memory_bank_dir = ward_dir.join("memory-bank");

    if !ward_dir.exists() || ward_id == "scratch" {
        return;
    }

    let _ = std::fs::create_dir_all(&memory_bank_dir);
    generate_structure_md(&ward_dir, &memory_bank_dir.join("structure.md"));

    let lang_configs_dir = vault_dir.join("config").join("wards");
    generate_core_docs_md(
        &ward_dir,
        &memory_bank_dir.join("core_docs.md"),
        &lang_configs_dir,
    );
}

fn generate_structure_md(ward_dir: &std::path::Path, output_path: &std::path::Path) {
    let mut content = String::from("# Ward Structure\n\n## Directory Layout\n\n```\n");
    generate_tree(ward_dir, ward_dir, 0, 3, &mut content);
    content.push_str("```\n");

    // Tech stack detection
    let mut tech = Vec::new();
    if ward_dir.join("requirements.txt").exists() {
        tech.push("Python (requirements.txt)");
    }
    if ward_dir.join("package.json").exists() {
        tech.push("Node.js (package.json)");
    }
    if ward_dir.join("Cargo.toml").exists() {
        tech.push("Rust (Cargo.toml)");
    }

    let core_dir = ward_dir.join("core");
    if core_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&core_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.path().extension().and_then(|e| e.to_str()) == Some("py") {
                    if let Ok(src) = std::fs::read_to_string(entry.path()) {
                        if src.contains("import yfinance") && !tech.contains(&"yfinance") {
                            tech.push("yfinance");
                        }
                        if src.contains("import pandas") && !tech.contains(&"pandas") {
                            tech.push("pandas");
                        }
                        if src.contains("import numpy") && !tech.contains(&"numpy") {
                            tech.push("numpy");
                        }
                    }
                }
            }
        }
    }
    if !tech.is_empty() {
        content.push_str(&format!("\n## Tech Stack\n\n{}\n", tech.join(", ")));
    }

    if let Err(e) = std::fs::write(output_path, &content) {
        tracing::warn!("Failed to write structure.md: {}", e);
    }
}

#[allow(clippy::only_used_in_recursion)]
fn generate_tree(
    dir: &std::path::Path,
    base: &std::path::Path,
    depth: usize,
    max_depth: usize,
    output: &mut String,
) {
    if depth > max_depth {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut items: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    items.sort_by_key(|e| e.file_name());

    for entry in &items {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || name == "__pycache__"
            || name == "node_modules"
            || name == ".venv"
            || name == "data"
        {
            continue;
        }
        let indent = "  ".repeat(depth);
        let path = entry.path();
        if path.is_dir() {
            output.push_str(&format!("{}{}/ \n", indent, name));
            generate_tree(&path, base, depth + 1, max_depth, output);
        } else if depth < 2
            || name.ends_with(".py")
            || name.ends_with(".md")
            || name.ends_with(".json")
            || name.ends_with(".yaml")
        {
            output.push_str(&format!("{}{}\n", indent, name));
        }
    }
}

fn generate_core_docs_md(
    ward_dir: &std::path::Path,
    output_path: &std::path::Path,
    lang_configs_dir: &std::path::Path,
) {
    let lang_configs = {
        let raw = gateway_services::lang_config::load_all_lang_configs(lang_configs_dir)
            .unwrap_or_default();
        gateway_services::lang_config::compile_all(&raw)
    };

    // Scan ALL code files in the ward (not just core/) — recursively
    let code_extensions = ["py", "js", "ts", "rs", "go", "rb", "sh"];
    let skip_dirs = [
        "node_modules",
        ".venv",
        "__pycache__",
        ".git",
        "memory-bank",
        "specs",
    ];

    let mut all_files: Vec<std::path::PathBuf> = Vec::new();
    fn walk_dir(
        dir: &std::path::Path,
        files: &mut Vec<std::path::PathBuf>,
        exts: &[&str],
        skip: &[&str],
    ) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                if !skip.contains(&name.as_str()) {
                    walk_dir(&path, files, exts, skip);
                }
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if exts.contains(&ext) && name != "__init__.py" {
                        files.push(path);
                    }
                }
            }
        }
    }
    walk_dir(ward_dir, &mut all_files, &code_extensions, &skip_dirs);
    all_files.sort();

    if all_files.is_empty() {
        return;
    }

    let mut content = String::from(
        "# Code Inventory\n\n*Auto-generated. Lists all code files with function signatures.*\n\n",
    );

    for path in &all_files {
        let relative = path.strip_prefix(ward_dir).unwrap_or(path);
        let rel_str = relative.to_string_lossy();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // File size
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let size_str = if size > 1024 {
            format!("{:.1}KB", size as f64 / 1024.0)
        } else {
            format!("{}B", size)
        };

        content.push_str(&format!("## {} ({})\n\n", rel_str, size_str));

        // Extract signatures
        if let Some(config) = gateway_services::lang_config::CompiledLangConfig::find_for_extension(
            &lang_configs,
            ext,
        ) {
            let desc = config.extract_first_docstring(path).unwrap_or_default();
            if !desc.is_empty() {
                content.push_str(&format!("{}\n\n", desc));
            }
            let sigs = config.extract_signatures(path);
            if !sigs.is_empty() {
                content.push_str("**Functions:**\n");
                for sig in &sigs {
                    content.push_str(&format!("- `{}`\n", sig));
                }
                content.push('\n');
            }
        } else {
            let desc = extract_first_docstring(path);
            if !desc.is_empty() {
                content.push_str(&format!("{}\n\n", desc));
            }
            let sigs = extract_function_signatures(path);
            if !sigs.is_empty() {
                content.push_str("**Functions:**\n");
                for sig in &sigs {
                    let display = sig.strip_prefix("def ").unwrap_or(sig);
                    content.push_str(&format!("- `{}`\n", display));
                }
                content.push('\n');
            }
        }
    }

    if let Err(e) = std::fs::write(output_path, &content) {
        tracing::warn!("Failed to write core_docs.md: {}", e);
    }
}
