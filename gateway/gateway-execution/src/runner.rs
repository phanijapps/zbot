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
use super::invoke::micro_recall::MicroRecallContext;
use super::invoke::working_memory_middleware;
use super::invoke::{
    broadcast_event, collect_agents_summary, collect_skills_summary, process_stream_event,
    spawn_batch_writer_with_repo, AgentLoader, ExecutorBuilder, ResponseAccumulator, StreamContext,
    ToolCallAccumulator, WorkingMemory, WorkspaceCache,
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
    /// Knowledge graph storage for the graph_query tool.
    graph_storage: Option<Arc<knowledge_graph::GraphStorage>>,
    /// KG episode repository for ward artifact indexing after distillation.
    kg_episode_repo: Option<Arc<gateway_database::KgEpisodeRepository>>,
    /// Adapter for the `ingest` agent tool. Wired via [`Self::set_ingestion_adapter`].
    ingestion_adapter: Option<Arc<dyn agent_tools::IngestionAccess>>,
    /// Adapter for the `goal` agent tool. Wired via [`Self::set_goal_adapter`].
    goal_adapter: Option<Arc<dyn agent_tools::GoalAccess>>,
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
            graph_storage: None,
            kg_episode_repo: None,
            ingestion_adapter: None,
            goal_adapter: None,
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

    /// Set the knowledge graph storage for the graph_query tool.
    pub fn set_graph_storage(&mut self, storage: Arc<knowledge_graph::GraphStorage>) {
        self.graph_storage = Some(storage);
    }

    /// Set the KG episode repository used by post-distillation ward indexing.
    pub fn set_kg_episode_repo(&mut self, repo: Arc<gateway_database::KgEpisodeRepository>) {
        self.kg_episode_repo = Some(repo);
    }

    /// Set the ingestion adapter so the `ingest` agent tool is registered.
    pub fn set_ingestion_adapter(&mut self, adapter: Arc<dyn agent_tools::IngestionAccess>) {
        self.ingestion_adapter = Some(adapter);
    }

    /// Set the goal adapter so the `goal` agent tool is registered.
    pub fn set_goal_adapter(&mut self, adapter: Arc<dyn agent_tools::GoalAccess>) {
        self.goal_adapter = Some(adapter);
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
        let graph_storage_for_delegation = self.graph_storage.clone();
        let ingestion_adapter_for_delegation = self.ingestion_adapter.clone();
        let goal_adapter_for_delegation = self.goal_adapter.clone();

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
                graph_storage: &Option<Arc<knowledge_graph::GraphStorage>>,
                ingestion_adapter: &Option<Arc<dyn agent_tools::IngestionAccess>>,
                goal_adapter: &Option<Arc<dyn agent_tools::GoalAccess>>,
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
                let graph_storage = graph_storage.clone();
                let ingestion_adapter = ingestion_adapter.clone();
                let goal_adapter = goal_adapter.clone();

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
                        graph_storage,
                        ingestion_adapter,
                        goal_adapter,
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
                                &graph_storage_for_delegation,
                                &ingestion_adapter_for_delegation,
                                &goal_adapter_for_delegation,
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
                                &graph_storage_for_delegation,
                                &ingestion_adapter_for_delegation,
                                &goal_adapter_for_delegation,
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
                                    &graph_storage_for_delegation,
                                    &ingestion_adapter_for_delegation,
                                    &goal_adapter_for_delegation,
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
        let graph_storage = self.graph_storage.clone();
        let kg_episode_repo = self.kg_episode_repo.clone();
        let ingestion_adapter = self.ingestion_adapter.clone();
        let goal_adapter = self.goal_adapter.clone();

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
                            graph_storage.clone(),
                            kg_episode_repo.clone(),
                            ingestion_adapter.clone(),
                            goal_adapter.clone(),
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
        mut config: ExecutionConfig,
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

        // If session has a persisted mode, use it (overrides invoke mode)
        if let Ok(Some(session)) = self.state_service.get_session(&session_id) {
            if let Some(ref persisted_mode) = session.mode {
                config.mode = Some(persisted_mode.clone());
            }
        }

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
        .with_chat_mode(config.is_chat_mode());
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
        // Runs in BOTH chat and research modes (Phase 7): only the pipeline depth is
        // gated on mode; memory must reach every session. Chat mode uses a smaller budget
        // to keep latency low.
        if let Some(recall) = &self.memory_recall {
            let _ = session_id; // retained for future recall-log wiring
            let top_k = if config.is_chat_mode() { 5 } else { 10 };
            match recall
                .recall_unified(
                    &config.agent_id,
                    &message,
                    setup.ward_id.as_deref(),
                    &[],
                    top_k,
                )
                .await
            {
                Ok(items) if !items.is_empty() => {
                    let formatted = crate::recall::format_scored_items(&items);
                    if !formatted.is_empty() {
                        history.insert(0, ChatMessage::system(formatted));
                    }
                    tracing::info!(
                        agent_id = %config.agent_id,
                        count = items.len(),
                        "Recalled unified context for first message"
                    );
                }
                Ok(_) => {
                    tracing::debug!(
                        "First-message unified recall returned empty — no relevant items"
                    );
                }
                Err(e) => {
                    // Surface the failure so the agent can drill manually instead
                    // of assuming memory was silently empty. Empty results (Ok case
                    // above) stay quiet — only genuine errors are reported.
                    tracing::warn!("First-message unified recall failed: {}", e);
                    history.insert(
                        0,
                        ChatMessage::system(crate::recall::format_recall_failure_message(&e)),
                    );
                }
            }
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
        mut history: Vec<ChatMessage>,
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
        let memory_repo = self.memory_repo.clone();
        let graph_storage = self.graph_storage.clone();
        let kg_episode_repo = self.kg_episode_repo.clone();

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

            // Initialize working memory and seed from recalled corrections
            let mut working_memory = WorkingMemory::new(1500);
            for msg in &history {
                if msg.role == "system" {
                    let content = msg.text_content();
                    if content.contains("Recalled") || content.contains("correction") {
                        for line in content.lines() {
                            let trimmed = line.trim().trim_start_matches("- ");
                            if trimmed.starts_with("[correction]")
                                || trimmed.starts_with("[pattern]")
                            {
                                working_memory.add_correction(trimmed);
                            }
                        }
                    }
                }
            }

            // Inject working memory into history if it has content
            if !working_memory.is_empty() {
                history.push(ChatMessage::system(working_memory.format_for_prompt()));
            }

            // Track current tool name for working memory middleware
            let mut current_tool_name = String::new();

            // Phase 6d: clones for real-time tool-result extraction (fire-and-forget).
            let kg_episode_repo_inner = kg_episode_repo.clone();
            let graph_storage_inner = graph_storage.clone();
            let agent_id_inner = agent_id.clone();

            // Collect micro-recall triggers during stream (sync closure cannot run async)
            let mut pending_recall_triggers: Vec<(
                super::invoke::micro_recall::MicroRecallTrigger,
                u32,
            )> = Vec::new();

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
                            current_tool_name = tool_name.clone();
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

                            // Update working memory from tool result
                            working_memory_middleware::process_tool_result(
                                &mut working_memory,
                                &current_tool_name,
                                result,
                                error.as_deref(),
                                handle.current_iteration(),
                            );

                            // Phase 6d: real-time graph extraction from tool output.
                            // Non-blocking — fires in a background task so the
                            // execution loop never waits.
                            if let (Some(ref ep_repo), Some(ref graph)) =
                                (&kg_episode_repo_inner, &graph_storage_inner)
                            {
                                let tool_name_cl = current_tool_name.clone();
                                let tool_id_cl = tool_id.clone();
                                let result_cl = result.clone();
                                let session_id_cl = session_id_inner.clone();
                                let agent_id_cl = agent_id_inner.clone();
                                let ep_repo_cl = ep_repo.clone();
                                let graph_cl = graph.clone();
                                tokio::spawn(async move {
                                    crate::tool_result_extractor::extract_and_persist(
                                        &tool_name_cl,
                                        &tool_id_cl,
                                        &result_cl,
                                        &session_id_cl,
                                        &agent_id_cl,
                                        ep_repo_cl.as_ref(),
                                        &graph_cl,
                                    )
                                    .await;
                                });
                            }

                            // Detect micro-recall triggers (sync) — executed after stream completes
                            let triggers = working_memory_middleware::detect_recall_triggers(
                                &working_memory,
                                &current_tool_name,
                                result,
                                error.as_deref(),
                            );
                            let iter = handle.current_iteration();
                            for trigger in triggers {
                                pending_recall_triggers.push((trigger, iter));
                            }
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

            // Execute micro-recall triggers collected during the stream
            if !pending_recall_triggers.is_empty() {
                let recall_ctx = MicroRecallContext {
                    memory_repo: memory_repo.clone(),
                    graph_storage: graph_storage.clone(),
                    agent_id: agent_id.clone(),
                };
                for (trigger, iter) in &pending_recall_triggers {
                    working_memory_middleware::execute_micro_recall_triggers(
                        &mut working_memory,
                        std::slice::from_ref(trigger),
                        &recall_ctx,
                        *iter,
                    )
                    .await;
                }
            }

            let accumulated_response = response_acc.into_response();

            tracing::info!(
                execution_id = %execution_id,
                response_len = accumulated_response.len(),
                tool_calls_count = tool_acc.len(),
                "Execution stream completed"
            );

            // Emit any remaining text that wasn't flushed as part of a tool-call turn.
            // If turn_text is empty, the response was already written when the last
            // ToolResult (e.g., from the respond tool) flushed it. Don't write again.
            if !turn_text.is_empty() {
                batch_writer.session_message(
                    &session_id,
                    &execution_id,
                    "assistant",
                    &turn_text,
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

                    // Ward AGENTS.md and memory-bank/ are curated manually by agents;
                    // the runtime no longer rewrites them post-execution.
                    let session_ward = state_service
                        .get_session(&session_id)
                        .ok()
                        .flatten()
                        .and_then(|s| s.ward_id);

                    // Fire-and-forget session distillation, followed by ward artifact indexing.
                    if let Some(distiller) = distiller.as_ref() {
                        let distiller = distiller.clone();
                        let sid = session_id.clone();
                        let aid = agent_id.clone();
                        let ward_id_for_indexer = session_ward.clone();
                        let kg_episode_repo_for_indexer = kg_episode_repo.clone();
                        let graph_storage_for_indexer = graph_storage.clone();
                        let paths_for_indexer = paths.clone();
                        tokio::spawn(async move {
                            if let Err(e) = distiller.distill(&sid, &aid).await {
                                tracing::warn!("Session distillation failed: {}", e);
                            }
                            run_ward_artifact_indexer(
                                &ward_id_for_indexer,
                                &sid,
                                &aid,
                                kg_episode_repo_for_indexer.as_ref(),
                                graph_storage_for_indexer.as_ref(),
                                &paths_for_indexer,
                            )
                            .await;
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
            self.graph_storage.clone(),
            self.ingestion_adapter.clone(),
            self.goal_adapter.clone(),
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
            .with_chat_mode(config.is_chat_mode());
        if let Some(ref registry) = self.model_registry {
            builder = builder.with_model_registry(registry.clone());
        }
        if let Some(fs) = fact_store {
            builder = builder.with_fact_store(fs);
        }
        if let Some(cp) = connector_provider {
            builder = builder.with_connector_provider(cp);
        }
        if let Some(ref gs) = self.graph_storage {
            builder = builder.with_graph_storage(gs.clone());
        }
        if let Some(ref a) = self.ingestion_adapter {
            builder = builder.with_ingestion_adapter(a.clone());
        }
        if let Some(ref a) = self.goal_adapter {
            builder = builder.with_goal_adapter(a.clone());
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
        let is_chat_mode = config.is_chat_mode();
        if is_root && already_analyzed && !is_chat_mode {
            // Notify UI that intent analysis was skipped (continuation turn)
            self.event_bus
                .publish(gateway_events::GatewayEvent::IntentAnalysisSkipped {
                    session_id: session_id.to_string(),
                    execution_id: execution_id.to_string(),
                })
                .await;
            tracing::debug!("Intent analysis skipped (already analyzed for this execution)");
        }
        if is_root && !already_analyzed && !is_chat_mode {
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

                            let system_prompt =
                                crate::middleware::intent_analysis::load_intent_analysis_prompt(
                                    &self.paths,
                                );
                            match analyze_intent(
                                &retrying,
                                msg,
                                fs.as_ref(),
                                self.memory_recall.as_ref().map(|r| r.as_ref()),
                                &system_prompt,
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

                                    // Phase 2b: populate session ctx with the intent-analyzer's
                                    // decision + verbatim user prompt. Subagents spawned later
                                    // can fetch these via memory(get_fact, key="ctx.<sid>.intent")
                                    // without re-reading the original message.
                                    if let Some(ref fs) = fact_store_for_indexing {
                                        let ward = analysis.ward_recommendation.ward_name.as_str();
                                        let intent_json = serde_json::to_value(&analysis)
                                            .unwrap_or(serde_json::Value::Null);
                                        crate::session_ctx::writer::intent_snapshot(
                                            fs,
                                            session_id,
                                            ward,
                                            &intent_json,
                                            msg,
                                        )
                                        .await;
                                    }

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
    graph_storage: Option<Arc<knowledge_graph::GraphStorage>>,
    kg_episode_repo: Option<Arc<gateway_database::KgEpisodeRepository>>,
    ingestion_adapter: Option<Arc<dyn agent_tools::IngestionAccess>>,
    goal_adapter: Option<Arc<dyn agent_tools::GoalAccess>>,
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
            .recall_unified(
                root_agent_id,
                &continuation_recall_query,
                session_ward_id.as_deref(),
                &[],
                10,
            )
            .await
        {
            Ok(items) if !items.is_empty() => {
                let formatted = crate::recall::format_scored_items(&items);
                if !formatted.is_empty() {
                    history.insert(0, ChatMessage::system(formatted));
                }
                tracing::info!(
                    item_count = items.len(),
                    "Recalled unified context for continuation"
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

    // Ward AGENTS.md and memory-bank/ are curated manually by agents;
    // the runtime no longer rewrites them before continuation.

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
    // Clone for session-ctx plan_snapshot below — the builder moves the
    // primary Arc, so we keep a separate handle to write plan text to
    // ctx.<sid>.plan on continuations that load a plan.md.
    let fact_store_for_ctx = fact_store.clone();
    if let Some(fs) = fact_store {
        builder = builder.with_fact_store(fs);
    }
    let graph_storage_for_indexer = graph_storage.clone();
    if let Some(gs) = graph_storage {
        builder = builder.with_graph_storage(gs);
    }
    if let Some(a) = ingestion_adapter {
        builder = builder.with_ingestion_adapter(a);
    }
    if let Some(a) = goal_adapter {
        builder = builder.with_goal_adapter(a);
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
            // Phase 2b: also populate session ctx with the plan so
            // subagents can fetch it via memory(get_fact, key="ctx.<sid>.plan")
            // instead of re-reading the specs file each turn.
            if let (Some(fs), Some(ward)) = (fact_store_for_ctx.as_ref(), session_ward_id.as_ref())
            {
                crate::session_ctx::writer::plan_snapshot(fs, session_id, ward, &plan).await;
            }
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

        // Phase 6d: clones for real-time tool-result extraction (fire-and-forget).
        let kg_episode_repo_inner = kg_episode_repo.clone();
        let graph_storage_inner = graph_storage_for_indexer.clone();
        let agent_id_inner = agent_id_clone.clone();
        // Track current tool name so the extractor can dispatch by name.
        let mut current_tool_name = String::new();

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
                        current_tool_name = tool_name.clone();
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

                        // Phase 6d: real-time graph extraction from tool output.
                        // Non-blocking — fires in a background task so the
                        // execution loop never waits.
                        if let (Some(ref ep_repo), Some(ref graph)) =
                            (&kg_episode_repo_inner, &graph_storage_inner)
                        {
                            let tool_name_cl = current_tool_name.clone();
                            let tool_id_cl = tool_id.clone();
                            let result_cl = result.clone();
                            let session_id_cl = session_id_inner.clone();
                            let agent_id_cl = agent_id_inner.clone();
                            let ep_repo_cl = ep_repo.clone();
                            let graph_cl = graph.clone();
                            tokio::spawn(async move {
                                crate::tool_result_extractor::extract_and_persist(
                                    &tool_name_cl,
                                    &tool_id_cl,
                                    &result_cl,
                                    &session_id_cl,
                                    &agent_id_cl,
                                    ep_repo_cl.as_ref(),
                                    &graph_cl,
                                )
                                .await;
                            });
                        }
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

        // Emit any remaining text that wasn't flushed as part of a tool-call turn.
        if !turn_text.is_empty() {
            batch_writer.session_message(
                &session_id_clone,
                &execution_id,
                "assistant",
                &turn_text,
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

                // Fire-and-forget session distillation, followed by ward artifact indexing.
                if let Some(distiller) = distiller {
                    let sid = session_id_clone.clone();
                    let aid = agent_id_clone.clone();
                    let ward_id_for_indexer = state_service
                        .get_session(&sid)
                        .ok()
                        .flatten()
                        .and_then(|s| s.ward_id);
                    let kg_episode_repo_for_indexer = kg_episode_repo.clone();
                    let graph_storage_for_indexer = graph_storage_for_indexer.clone();
                    let paths_for_indexer = paths.clone();
                    tokio::spawn(async move {
                        if let Err(e) = distiller.distill(&sid, &aid).await {
                            tracing::warn!("Continuation distillation failed: {}", e);
                        }
                        run_ward_artifact_indexer(
                            &ward_id_for_indexer,
                            &sid,
                            &aid,
                            kg_episode_repo_for_indexer.as_ref(),
                            graph_storage_for_indexer.as_ref(),
                            &paths_for_indexer,
                        )
                        .await;
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

/// Phase 6a: index structured ward artifacts into the knowledge graph after distillation.
///
/// Skips when the session has no ward (scratch), the KG episode repo is not wired,
/// graph storage is unavailable, or the ward path does not exist on disk. All errors
/// from the indexer are logged and never propagate — this must not crash the pipeline.
async fn run_ward_artifact_indexer(
    ward_id: &Option<String>,
    session_id: &str,
    agent_id: &str,
    kg_episode_repo: Option<&Arc<gateway_database::KgEpisodeRepository>>,
    graph_storage: Option<&Arc<knowledge_graph::GraphStorage>>,
    paths: &SharedVaultPaths,
) {
    let (Some(wid), Some(ep_repo), Some(graph)) = (ward_id, kg_episode_repo, graph_storage) else {
        return;
    };
    let ward_path = paths.vault_dir().join("wards").join(wid);
    if !ward_path.exists() {
        return;
    }
    let n = crate::ward_artifact_indexer::index_ward(
        &ward_path,
        session_id,
        agent_id,
        ep_repo.as_ref(),
        graph,
    )
    .await;
    tracing::info!(
        ward = %wid,
        indexed_entities = n,
        session = %session_id,
        "Ward artifact indexing complete"
    );
}
