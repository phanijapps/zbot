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
use gateway_events::{EventBus, GatewayEvent};
use gateway_services::{AgentService, McpService, ProviderService, SharedVaultPaths};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock, Semaphore};
use zero_stores_sqlite::{ConversationRepository, DatabaseManager};

/// Callback invoked after session creation but before any events are emitted.
/// Receives the session_id so the caller can set up subscriptions before events fire.
pub type OnSessionReady =
    Box<dyn FnOnce(String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

// Import types from sibling modules
pub use crate::config::ExecutionConfig;
use crate::delegation::{spawn_delegated_agent, DelegationRegistry, DelegationRequest};
pub use crate::handle::ExecutionHandle;
use crate::invoke::{
    broadcast_event, collect_agents_summary, collect_skills_summary, process_stream_event,
    spawn_batch_writer_with_repo, AgentLoader, ExecutorBuilder, ResponseAccumulator, StreamContext,
    ToolCallAccumulator, WorkspaceCache,
};
use crate::lifecycle::{
    complete_execution, crash_execution, emit_agent_started, stop_execution, CompleteExecution,
    CrashExecution, StopExecution,
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
    memory_repo: Option<Arc<zero_stores_sqlite::MemoryRepository>>,
    /// Trait-routed memory store. Preferred over `memory_repo` for new
    /// fact_store wiring — wired in both SQLite and SurrealDB modes
    /// while `memory_repo` is `None` in SurrealDB mode.
    memory_store: Option<Arc<dyn zero_stores::MemoryFactStore>>,
    /// Session distiller for automatic fact extraction after sessions
    distiller: Option<Arc<crate::distillation::SessionDistiller>>,
    /// Memory recall for automatic fact retrieval at session start
    memory_recall: Option<Arc<crate::recall::MemoryRecall>>,
    /// Semaphore to limit concurrent delegation spawns (prevents resource exhaustion)
    delegation_semaphore: Arc<Semaphore>,
    /// Embedding client for generating vector embeddings (semantic search in memory)
    embedding_client: Option<Arc<dyn agent_runtime::llm::embedding::EmbeddingClient>>,
    /// Model capabilities registry for context window and capability lookups.
    ///
    /// Stored in an `ArcSwapOption` so the `RunnerContinuationInvoker`
    /// pre-captured by `ContinuationWatcher` (constructed before
    /// [`Self::set_model_registry`] is called from `runtime.rs:145`) can
    /// still observe the registry once it's installed. A plain
    /// `Option<Arc<ModelRegistry>>` would freeze as `None` in any
    /// pre-spawned task's captured clone — the original cause of the
    /// `context_window_tokens = 8192` fallback on the continuation path
    /// (see `invoke/executor.rs:419-424`).
    model_registry: Arc<arc_swap::ArcSwapOption<gateway_services::models::ModelRegistry>>,
    /// Per-provider rate limiters — shared across all executors using the same provider.
    rate_limiters: std::sync::Arc<
        std::sync::RwLock<
            std::collections::HashMap<String, std::sync::Arc<agent_runtime::ProviderRateLimiter>>,
        >,
    >,
    /// Knowledge graph storage for the graph_query tool.
    graph_storage: Option<Arc<zero_stores_sqlite::kg::storage::GraphStorage>>,
    /// Trait-routed kg store. Phase E5b — preferred over `graph_storage`
    /// for the graph_query tool wiring; wired in both backends.
    kg_store: Option<Arc<dyn zero_stores::KnowledgeGraphStore>>,
    /// KG episode repository for ward artifact indexing after distillation.
    kg_episode_repo: Option<Arc<zero_stores_sqlite::KgEpisodeRepository>>,
    /// Adapter for the `ingest` agent tool. Wired via [`Self::set_ingestion_adapter`].
    ingestion_adapter: Option<Arc<dyn agent_tools::IngestionAccess>>,
    /// Adapter for the `goal` agent tool. Wired via [`Self::set_goal_adapter`].
    goal_adapter: Option<Arc<dyn agent_tools::GoalAccess>>,
    /// Pre-session setup delegate. Holds the dependency set needed by
    /// `invoke_with_callback`'s bootstrap phase, extracted here so
    /// `setup()` can be tested and read independently of the full runner.
    bootstrap: super::invoke_bootstrap::InvokeBootstrap,
}

/// All inputs needed to construct an [`ExecutionRunner`].
///
/// Replaces the previous 18-positional-argument `with_connector_registry`
/// constructor. Using a struct literal at the call site means:
///
/// - Adding a new dependency is one line here + one line at every caller,
///   no positional reshuffling.
/// - Same-type `Option<Arc<...>>` fields (connector_registry vs bridge_registry
///   vs memory_repo) can't be silently swapped — the field name is checked at
///   compile time.
/// - Callers that only want the minimum can lean on `Default::default()` for
///   the optional integrations.
pub struct ExecutionRunnerConfig {
    // --- Required services ---
    pub event_bus: Arc<EventBus>,
    pub agent_service: Arc<AgentService>,
    pub provider_service: Arc<ProviderService>,
    pub paths: SharedVaultPaths,
    pub conversation_repo: Arc<ConversationRepository>,
    pub mcp_service: Arc<McpService>,
    pub skill_service: Arc<gateway_services::SkillService>,
    pub log_service: Arc<LogService<DatabaseManager>>,
    pub state_service: Arc<StateService<DatabaseManager>>,

    // --- Optional integrations ---
    pub connector_registry: Option<Arc<gateway_connectors::ConnectorRegistry>>,
    pub workspace_cache: WorkspaceCache,
    pub memory_repo: Option<Arc<zero_stores_sqlite::MemoryRepository>>,
    /// Trait-routed memory store (preferred over `memory_repo` for fact_store
    /// wiring; wired in both SQLite and SurrealDB modes).
    pub memory_store: Option<Arc<dyn zero_stores::MemoryFactStore>>,
    pub distiller: Option<Arc<crate::distillation::SessionDistiller>>,
    pub memory_recall: Option<Arc<crate::recall::MemoryRecall>>,
    pub bridge_registry: Option<Arc<gateway_bridge::BridgeRegistry>>,
    pub bridge_outbox: Option<Arc<gateway_bridge::OutboxRepository>>,
    pub embedding_client: Option<Arc<dyn agent_runtime::llm::embedding::EmbeddingClient>>,

    // --- Resource control ---
    pub max_parallel_agents: u32,
}

/// Inputs for [`invoke_continuation`]. Previously 24 positional arguments,
/// with eight same-type `Option<Arc<…>>` dependencies in a row
/// (memory_repo, embedding_client, distiller, memory_recall,
/// model_registry, graph_storage, kg_episode_repo, ingestion_adapter,
/// goal_adapter) — the densest silent-swap cluster in the file. A
/// psychopath adding a 25th dependency to the old signature had an even
/// chance of scrambling which optional dep routed where.
pub(super) struct ContinuationArgs<'a> {
    pub(super) session_id: &'a str,
    pub(super) root_agent_id: &'a str,
    pub(super) event_bus: Arc<EventBus>,
    pub(super) agent_service: Arc<AgentService>,
    pub(super) provider_service: Arc<ProviderService>,
    pub(super) mcp_service: Arc<McpService>,
    pub(super) skill_service: Arc<gateway_services::SkillService>,
    pub(super) paths: SharedVaultPaths,
    pub(super) conversation_repo: Arc<ConversationRepository>,
    pub(super) handles: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
    pub(super) delegation_registry: Arc<DelegationRegistry>,
    pub(super) delegation_tx: mpsc::UnboundedSender<DelegationRequest>,
    pub(super) log_service: Arc<LogService<DatabaseManager>>,
    pub(super) state_service: Arc<StateService<DatabaseManager>>,
    pub(super) workspace_cache: WorkspaceCache,
    pub(super) memory_repo: Option<Arc<zero_stores_sqlite::MemoryRepository>>,
    pub(super) memory_store: Option<Arc<dyn zero_stores::MemoryFactStore>>,
    pub(super) embedding_client: Option<Arc<dyn agent_runtime::llm::embedding::EmbeddingClient>>,
    pub(super) distiller: Option<Arc<crate::distillation::SessionDistiller>>,
    pub(super) memory_recall: Option<Arc<crate::recall::MemoryRecall>>,
    pub(super) model_registry: Option<Arc<gateway_services::models::ModelRegistry>>,
    pub(super) graph_storage: Option<Arc<zero_stores_sqlite::kg::storage::GraphStorage>>,
    pub(super) kg_store: Option<Arc<dyn zero_stores::KnowledgeGraphStore>>,
    pub(super) kg_episode_repo: Option<Arc<zero_stores_sqlite::KgEpisodeRepository>>,
    pub(super) ingestion_adapter: Option<Arc<dyn agent_tools::IngestionAccess>>,
    pub(super) goal_adapter: Option<Arc<dyn agent_tools::GoalAccess>>,
}

/// Prepend recalled facts to `history` as a system message at position 0.
///
/// Uses the most recent user message in `history` as the recall query so the
/// recalled facts are relevant to the task at hand (vs. a hardcoded placeholder).
/// No-op when `memory_recall` is `None`, the recall call errors, or it returns
/// no items.
async fn prepend_continuation_recall(
    history: &mut Vec<ChatMessage>,
    memory_recall: Option<&Arc<crate::recall::MemoryRecall>>,
    agent_id: &str,
    ward_id: Option<&str>,
) {
    let Some(recall) = memory_recall else {
        return;
    };

    // Use the last user message as the recall query.
    let query = history
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.text_content())
        .unwrap_or_else(|| "continuation recall".to_string());

    match recall
        .recall_unified(agent_id, &query, ward_id, &[], 10)
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

/// Build the system-message prompt that seeds a continuation turn.
///
/// If the session has a ward and `specs/{topic}/plan.md` exists, inject the
/// plan's full text with a "just find-next-step + delegate" directive so the
/// continuation agent doesn't redo analysis. Otherwise emit the terse "delegate
/// the next step immediately" nudge.
///
/// Side effect: when a plan is found and a fact store is available, the plan
/// text is written to `ctx.<session_id>.plan` so subagents can fetch it via
/// `memory(get_fact, …)` without re-reading the file.
async fn build_continuation_message(
    paths: &SharedVaultPaths,
    session_id: &str,
    ward_id: Option<&str>,
    fact_store: Option<&Arc<dyn zero_stores::MemoryFactStore>>,
) -> String {
    let plan_hint = ward_id.and_then(|wid| {
        let specs_dir = paths.vault_dir().join("wards").join(wid).join("specs");
        find_latest_plan(&specs_dir)
    });

    let Some(plan) = plan_hint else {
        return "[Delegation completed. Delegate the next step in your plan immediately. \
                 Do NOT read files or analyze — just delegate.]"
            .to_string();
    };

    // Populate session ctx with the plan so subagents can fetch it via
    // memory(get_fact, key="ctx.<sid>.plan") instead of re-reading the specs
    // file each turn.
    if let (Some(fs), Some(ward)) = (fact_store, ward_id) {
        crate::session_ctx::writer::plan_snapshot(fs, session_id, ward, &plan).await;
    }

    format!(
        "[DELEGATION COMPLETED. YOUR PLAN IS BELOW.\n\
         DO NOT read files. DO NOT analyze. DO NOT use shell.\n\
         Just find the next step that hasn't been done and delegate it NOW.\n\
         One action only: delegate_to_agent.]\n\n{}",
        plan
    )
}

/// Wire the mid-session recall hook onto an [`AgentExecutor`] if the owning
/// runner has a [`MemoryRecall`] configured with `mid_session_recall.enabled`.
///
/// Same closure body is wired at two points — after a root executor is built
/// in `create_executor`, and after a continuation executor is built in
/// `invoke_continuation`. Extracted here so the ~55-line `set_recall_hook`
/// invocation lives in exactly one place; either call site that forgets it
/// must explicitly opt out rather than silently diverge.
pub(super) fn attach_mid_session_recall_hook(
    executor: &mut AgentExecutor,
    memory_recall: Option<&Arc<crate::recall::MemoryRecall>>,
    agent_id: &str,
    ward_id: Option<&str>,
) {
    let Some(recall) = memory_recall else {
        return;
    };
    let mid_cfg = &recall.config().mid_session_recall;
    if !mid_cfg.enabled {
        return;
    }

    let recall = Arc::clone(recall);
    let agent_id = agent_id.to_string();
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
                    let facts = recall.recall(&agent_id, &query, 5, ward.as_deref()).await?;
                    // Filter out already-injected facts and low-novelty results.
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
                    let keys: Vec<String> = novel.iter().map(|f| f.fact.key.clone()).collect();
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

impl ExecutionRunner {
    /// Create a new execution runner from a [`ExecutionRunnerConfig`].
    ///
    /// Initializes the runner and spawns background tasks for processing
    /// delegation + continuation requests.
    pub fn with_config(config: ExecutionRunnerConfig) -> Self {
        let ExecutionRunnerConfig {
            event_bus,
            agent_service,
            provider_service,
            paths,
            conversation_repo,
            mcp_service,
            skill_service,
            log_service,
            state_service,
            connector_registry,
            workspace_cache,
            memory_repo,
            memory_store,
            distiller,
            memory_recall,
            bridge_registry,
            bridge_outbox,
            embedding_client,
            max_parallel_agents,
        } = config;

        // Create channel for delegation requests
        let (delegation_tx, delegation_rx) = mpsc::unbounded_channel::<DelegationRequest>();

        // Shared data structures — constructed once and Arc-cloned into both the
        // runner fields and the bootstrap.
        let handles: Arc<RwLock<HashMap<String, ExecutionHandle>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let delegation_registry = Arc::new(DelegationRegistry::new());
        let delegation_semaphore = Arc::new(Semaphore::new(max_parallel_agents as usize));
        let model_registry: Arc<arc_swap::ArcSwapOption<gateway_services::models::ModelRegistry>> =
            Arc::new(arc_swap::ArcSwapOption::from(None));
        let rate_limiters: std::sync::Arc<
            std::sync::RwLock<
                std::collections::HashMap<
                    String,
                    std::sync::Arc<agent_runtime::ProviderRateLimiter>,
                >,
            >,
        > = std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));

        let bootstrap = super::invoke_bootstrap::InvokeBootstrap {
            agent_service: agent_service.clone(),
            provider_service: provider_service.clone(),
            mcp_service: mcp_service.clone(),
            skill_service: skill_service.clone(),
            state_service: state_service.clone(),
            log_service: log_service.clone(),
            conversation_repo: conversation_repo.clone(),
            paths: paths.clone(),
            memory_store: memory_store.clone(),
            memory_recall: memory_recall.clone(),
            model_registry: model_registry.clone(),
            rate_limiters: rate_limiters.clone(),
            connector_registry: connector_registry.clone(),
            bridge_registry: bridge_registry.clone(),
            bridge_outbox: bridge_outbox.clone(),
            graph_storage: None,
            kg_store: None,
            ingestion_adapter: None,
            goal_adapter: None,
            event_bus: event_bus.clone(),
            handles: handles.clone(),
            workspace_cache: workspace_cache.clone(),
        };

        let runner = Self {
            event_bus,
            agent_service,
            provider_service,
            mcp_service,
            skill_service,
            paths,
            handles,
            conversation_repo,
            delegation_registry,
            delegation_tx,
            log_service,
            state_service,
            connector_registry,
            bridge_registry,
            bridge_outbox,
            workspace_cache,
            memory_repo,
            memory_store,
            distiller,
            memory_recall,
            delegation_semaphore,
            embedding_client,
            model_registry,
            rate_limiters,
            graph_storage: None,
            kg_store: None,
            kg_episode_repo: None,
            ingestion_adapter: None,
            goal_adapter: None,
            bootstrap,
        };

        // Spawn delegation handler task — extracted into DelegationDispatcher.
        super::delegation_dispatcher::DelegationDispatcher {
            delegation_rx,
            delegation_semaphore: runner.delegation_semaphore.clone(),
            invoker: std::sync::Arc::new(runner.make_delegation_invoker()),
        }
        .spawn();

        // Spawn continuation watcher — extracted from the old inline
        // `spawn_continuation_handler` closure so the event-loop logic
        // is testable independently.
        super::continuation_watcher::ContinuationWatcher {
            event_bus: runner.event_bus.clone(),
            invoker: Arc::new(runner.make_continuation_invoker()),
        }
        .spawn();

        runner
    }

    /// Set the model capabilities registry.
    ///
    /// Takes `&self` (not `&mut self`) because the field is now an
    /// `Arc<ArcSwapOption<...>>` shared with the continuation handler
    /// task spawned during [`Self::new`]. The store is lock-free and
    /// becomes visible to subsequent `.load_full()` reads — which is
    /// what the continuation path does at fire time.
    pub fn set_model_registry(&self, registry: Arc<gateway_services::models::ModelRegistry>) {
        self.model_registry.store(Some(registry));
    }

    /// Late-wired setter. Mirrored to `self.bootstrap.graph_storage` because
    /// `InvokeBootstrap::finish_setup` reads its own clone at session-setup time.
    pub fn set_graph_storage(
        &mut self,
        storage: Arc<zero_stores_sqlite::kg::storage::GraphStorage>,
    ) {
        self.bootstrap.graph_storage = Some(storage.clone());
        self.graph_storage = Some(storage);
    }

    /// Set the KG episode repository used by post-distillation ward indexing.
    pub fn set_kg_episode_repo(&mut self, repo: Arc<zero_stores_sqlite::KgEpisodeRepository>) {
        self.kg_episode_repo = Some(repo);
    }

    /// Late-wired setter for the trait-routed kg store. Mirrored to the
    /// bootstrap so `InvokeBootstrap::finish_setup` reads its own clone
    /// at session-setup time. Phase E5b — wired in both backends so the
    /// `graph_query` tool registers regardless of SQLite vs SurrealDB.
    pub fn set_kg_store(&mut self, store: Arc<dyn zero_stores::KnowledgeGraphStore>) {
        self.bootstrap.kg_store = Some(store.clone());
        self.kg_store = Some(store);
    }

    /// Late-wired setter. Mirrored to `self.bootstrap.ingestion_adapter` because
    /// `InvokeBootstrap::finish_setup` reads its own clone at session-setup time.
    pub fn set_ingestion_adapter(&mut self, adapter: Arc<dyn agent_tools::IngestionAccess>) {
        self.bootstrap.ingestion_adapter = Some(adapter.clone());
        self.ingestion_adapter = Some(adapter);
    }

    /// Late-wired setter. Mirrored to `self.bootstrap.goal_adapter` because
    /// `InvokeBootstrap::finish_setup` reads its own clone at session-setup time.
    pub fn set_goal_adapter(&mut self, adapter: Arc<dyn agent_tools::GoalAccess>) {
        self.bootstrap.goal_adapter = Some(adapter.clone());
        self.goal_adapter = Some(adapter);
    }

    /// Build a [`RunnerContinuationInvoker`] from this runner's fields.
    ///
    /// Called from `with_config` to wire the `ContinuationWatcher` before
    /// the runner is wrapped in `Arc`. Each field is cloned — the
    /// `model_registry` ArcSwap handle is cloned (not its inner value)
    /// so late-stored registries are visible at fire time.
    pub(super) fn make_continuation_invoker(
        &self,
    ) -> super::continuation_watcher::RunnerContinuationInvoker {
        super::continuation_watcher::RunnerContinuationInvoker {
            event_bus: self.event_bus.clone(),
            agent_service: self.agent_service.clone(),
            provider_service: self.provider_service.clone(),
            mcp_service: self.mcp_service.clone(),
            skill_service: self.skill_service.clone(),
            paths: self.paths.clone(),
            handles: self.handles.clone(),
            conversation_repo: self.conversation_repo.clone(),
            delegation_registry: self.delegation_registry.clone(),
            delegation_tx: self.delegation_tx.clone(),
            log_service: self.log_service.clone(),
            state_service: self.state_service.clone(),
            workspace_cache: self.workspace_cache.clone(),
            memory_repo: self.memory_repo.clone(),
            memory_store: self.memory_store.clone(),
            embedding_client: self.embedding_client.clone(),
            distiller: self.distiller.clone(),
            memory_recall: self.memory_recall.clone(),
            model_registry: self.model_registry.clone(),
            graph_storage: self.graph_storage.clone(),
            kg_store: self.kg_store.clone(),
            kg_episode_repo: self.kg_episode_repo.clone(),
            ingestion_adapter: self.ingestion_adapter.clone(),
            goal_adapter: self.goal_adapter.clone(),
        }
    }

    /// Build a [`RunnerDelegationInvoker`] from this runner's fields.
    ///
    /// Called from `with_config` to wire the `DelegationDispatcher` before
    /// the runner is wrapped in `Arc`. Each field is cloned so the invoker
    /// holds live Arc handles rather than stale captured values.
    pub(super) fn make_delegation_invoker(
        &self,
    ) -> super::delegation_dispatcher::RunnerDelegationInvoker {
        super::delegation_dispatcher::RunnerDelegationInvoker {
            event_bus: self.event_bus.clone(),
            agent_service: self.agent_service.clone(),
            provider_service: self.provider_service.clone(),
            mcp_service: self.mcp_service.clone(),
            skill_service: self.skill_service.clone(),
            paths: self.paths.clone(),
            conversation_repo: self.conversation_repo.clone(),
            handles: self.handles.clone(),
            delegation_registry: self.delegation_registry.clone(),
            delegation_tx: self.delegation_tx.clone(),
            log_service: self.log_service.clone(),
            state_service: self.state_service.clone(),
            workspace_cache: self.workspace_cache.clone(),
            memory_repo: self.memory_repo.clone(),
            memory_store: self.memory_store.clone(),
            embedding_client: self.embedding_client.clone(),
            memory_recall: self.memory_recall.clone(),
            rate_limiters: self.rate_limiters.clone(),
            graph_storage: self.graph_storage.clone(),
            kg_store: self.kg_store.clone(),
            ingestion_adapter: self.ingestion_adapter.clone(),
            goal_adapter: self.goal_adapter.clone(),
        }
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
    /// The callback fires after session creation but BEFORE any agent or intent
    /// events are emitted, so the caller's subscriber sees every event from
    /// `AgentStarted` onward.
    ///
    /// # Event ordering
    ///
    /// ```text
    /// begin_setup  [get_or_create_session, persist_routing,
    ///               start_execution, store_handle]
    /// → on_session_ready CALLBACK  ← subscriber registers HERE
    /// → finish_setup [emit_agent_started, load_agent, run_intent_analysis,
    ///                 inject_placeholder, build executor]
    /// → tokio::spawn
    /// ```
    pub async fn invoke_with_callback(
        &self,
        mut config: ExecutionConfig,
        message: String,
        on_session_ready: Option<OnSessionReady>,
    ) -> Result<(ExecutionHandle, String), String> {
        // Phase 1: create session + handle, BEFORE any events fire.
        let partial = self.bootstrap.begin_setup(&mut config).await?;

        // Subscriber registers HERE — captures every event from AgentStarted onward.
        if let Some(cb) = on_session_ready {
            cb(partial.session_id.clone()).await;
        }

        // Phase 2: emit AgentStarted, load agent, intent analysis, build executor.
        let setup = self
            .bootstrap
            .finish_setup(&config, &message, partial)
            .await?;

        // Assemble the per-execution stream + context exactly as the old call site did.
        let stream = super::execution_stream::ExecutionStream {
            event_bus: self.event_bus.clone(),
            state_service: self.state_service.clone(),
            log_service: self.log_service.clone(),
            conversation_repo: self.conversation_repo.clone(),
            delegation_tx: self.delegation_tx.clone(),
            delegation_registry: self.delegation_registry.clone(),
            handles: self.handles.clone(),
            distiller: self.distiller.clone(),
            kg_episode_repo: self.kg_episode_repo.clone(),
            graph_storage: self.graph_storage.clone(),
            paths: self.paths.clone(),
            memory_store: self.memory_store.clone(),
            connector_registry: self.connector_registry.clone(),
            bridge_registry: self.bridge_registry.clone(),
            bridge_outbox: self.bridge_outbox.clone(),
        };
        let ctx = super::execution_stream::ExecutionContext {
            execution_id: setup.execution_id,
            session_id: setup.session_id.clone(),
            agent_id: config.agent_id.clone(),
            conversation_id: config.conversation_id.clone(),
            handle: setup.handle.clone(),
            respond_to: config.respond_to.clone(),
            thread_id: config.thread_id.clone(),
            message,
            history: setup.history,
            recommended_skills: setup.recommended_skills,
        };
        tokio::spawn(async move {
            let _ = stream.run(ctx, setup.executor).await;
        });

        Ok((setup.handle, setup.session_id))
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
            // Resume-from-crash: parent's conversation_id is not separately tracked
            // here. The root agent's conversation_id equals session_id by convention,
            // so use session_id as a best-effort fallback. This is consistent with
            // the legacy emit at runner/core.rs spawn_delegation.
            parent_conversation_id: session_id.to_string(),
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
            self.memory_store.clone(),
            self.embedding_client.clone(),
            self.memory_recall.clone(),
            self.rate_limiters.clone(),
            self.graph_storage.clone(),
            self.kg_store.clone(),
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
        let delegation_context = crate::delegation::DelegationContext::new(
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
pub(super) async fn invoke_continuation(args: ContinuationArgs<'_>) -> Result<(), String> {
    let ContinuationArgs {
        session_id,
        root_agent_id,
        event_bus,
        agent_service,
        provider_service,
        mcp_service,
        skill_service,
        paths,
        conversation_repo,
        handles,
        delegation_registry: _delegation_registry,
        delegation_tx,
        log_service,
        state_service,
        workspace_cache,
        memory_repo,
        memory_store,
        embedding_client,
        distiller,
        memory_recall,
        model_registry,
        graph_storage,
        kg_store,
        kg_episode_repo,
        ingestion_adapter,
        goal_adapter,
    } = args;
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

    // Prepend recalled facts (if any) to history as a system message at
    // position 0 — formatted by `format_scored_items`. No-op when
    // memory_recall is None, recall fails, or returns nothing.
    prepend_continuation_recall(
        &mut history,
        memory_recall.as_ref(),
        root_agent_id,
        session_ward_id.as_deref(),
    )
    .await;

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

    // Build fact store for continuation (so save_fact uses DB, not file fallback).
    // Phase E5: prefer the trait-routed `memory_store` (wired in both
    // SQLite and SurrealDB modes) over re-wrapping the concrete
    // `memory_repo` (None on Surreal). Falls back to the legacy wrap
    // only when memory_store is unavailable AND memory_repo is Some —
    // a state that should never occur in production but the fallback
    // keeps test paths and any future minimal callers working.
    let fact_store: Option<Arc<dyn zero_stores::MemoryFactStore>> =
        memory_store.clone().or_else(|| {
            memory_repo.as_ref().map(|repo| {
                Arc::new(zero_stores_sqlite::GatewayMemoryFactStore::new(
                    repo.clone(),
                    embedding_client.clone(),
                )) as Arc<dyn zero_stores::MemoryFactStore>
            })
        });
    // Clone for session-ctx plan_snapshot below — the builder moves the
    // primary Arc, so we keep a separate handle to write plan text to
    // ctx.<sid>.plan on continuations that load a plan.md.
    let fact_store_for_ctx = fact_store.clone();
    if let Some(fs) = fact_store {
        builder = builder.with_fact_store(fs);
    }
    let graph_storage_for_indexer = graph_storage.clone();
    if let Some(ks) = kg_store.clone() {
        builder = builder.with_kg_store(ks);
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

    attach_mid_session_recall_hook(
        &mut executor,
        memory_recall.as_ref(),
        root_agent_id,
        session_ward_id.as_deref(),
    );

    // Build a focused continuation message with the plan injected if one exists.
    let continuation_message = build_continuation_message(
        &paths,
        session_id,
        session_ward_id.as_deref(),
        fact_store_for_ctx.as_ref(),
    )
    .await;

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
                    complete_execution(CompleteExecution {
                        state_service: &state_service,
                        log_service: &log_service,
                        event_bus: &event_bus,
                        execution_id: &execution_id,
                        session_id: &session_id_clone,
                        agent_id: &agent_id_clone,
                        conversation_id: &conversation_id,
                        response: Some(accumulated_response),
                        connector_registry: None,
                        respond_to: None,
                        thread_id: None,
                        bridge_registry: None,
                        bridge_outbox: None,
                    })
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
                    // Phase C: indexer is trait-routed. Build a
                    // KgEpisodeStore wrapper from the SQLite repo (or
                    // re-use whatever trait impl is wired). graph_storage
                    // -> kg_store is already on the runner via
                    // set_kg_store, so we pass that directly.
                    let kg_episode_store_for_indexer: Option<Arc<dyn zero_stores_traits::KgEpisodeStore>> =
                        kg_episode_repo.as_ref().map(|r| {
                            Arc::new(zero_stores_sqlite::GatewayKgEpisodeStore::new(r.clone()))
                                as Arc<dyn zero_stores_traits::KgEpisodeStore>
                        });
                    let kg_store_for_indexer = kg_store.clone();
                    let paths_for_indexer = paths.clone();
                    tokio::spawn(async move {
                        if let Err(e) = distiller.distill(&sid, &aid).await {
                            tracing::warn!("Continuation distillation failed: {}", e);
                        }
                        run_ward_artifact_indexer(
                            &ward_id_for_indexer,
                            &sid,
                            &aid,
                            kg_episode_store_for_indexer.as_ref(),
                            kg_store_for_indexer.as_ref(),
                            &paths_for_indexer,
                        )
                        .await;
                    });
                }
            }
            Err(e) => {
                crash_execution(CrashExecution {
                    state_service: &state_service,
                    log_service: &log_service,
                    event_bus: &event_bus,
                    execution_id: &execution_id,
                    session_id: &session_id_clone,
                    agent_id: &agent_id_clone,
                    conversation_id: &conversation_id,
                    error: &e.to_string(),
                    crash_session: true,
                })
                .await;
            }
        }

        if handle.is_stop_requested() {
            stop_execution(StopExecution {
                state_service: &state_service,
                log_service: &log_service,
                event_bus: &event_bus,
                execution_id: &execution_id,
                session_id: &session_id_clone,
                agent_id: &agent_id_clone,
                conversation_id: &conversation_id,
                iteration: handle.current_iteration(),
            })
            .await;
        }
    });

    Ok(())
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
/// Phase C: trait-routed. Skips when the session has no ward (scratch),
/// either trait store is unwired, or the ward path does not exist on disk.
/// All errors from the indexer are logged and never propagate.
pub(super) async fn run_ward_artifact_indexer(
    ward_id: &Option<String>,
    session_id: &str,
    agent_id: &str,
    kg_episode_store: Option<&Arc<dyn zero_stores_traits::KgEpisodeStore>>,
    kg_store: Option<&Arc<dyn zero_stores::KnowledgeGraphStore>>,
    paths: &SharedVaultPaths,
) {
    let (Some(wid), Some(ep_store), Some(kg)) = (ward_id, kg_episode_store, kg_store) else {
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
        ep_store,
        kg,
    )
    .await;
    tracing::info!(
        ward = %wid,
        indexed_entities = n,
        session = %session_id,
        "Ward artifact indexing complete"
    );
}

#[cfg(test)]
mod model_registry_late_binding_tests {
    //! Regression tests for the capture-before-init bug that caused
    //! `context_window_tokens = 8192` on the continuation path.
    //!
    //! The failure mode: `RunnerContinuationInvoker` clones
    //! `self.model_registry` (the ArcSwap handle) inside `with_config`
    //! BEFORE `set_model_registry` runs. When the field was a plain
    //! `Option<Arc<_>>`, the captured clone froze as `None` and every
    //! continuation-path executor fell back to the 8192 default at
    //! `invoke/executor.rs:423`. After the fix the field is an
    //! `Arc<ArcSwapOption<_>>`, so pre-captured handles read the live
    //! value at fire time.
    //!
    //! These tests target the ArcSwap-based late-binding contract
    //! without needing the full `ExecutionRunner` construction graph.
    use arc_swap::ArcSwapOption;
    use gateway_services::models::ModelRegistry;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn load_user_registry() -> Arc<ModelRegistry> {
        let bundled = gateway_templates::Templates::get("models_registry.json")
            .map(|f| f.data.to_vec())
            .unwrap_or_default();
        let vault = PathBuf::from("/tmp/agentzero-test-vault");
        Arc::new(ModelRegistry::load(&bundled, &vault))
    }

    /// The core contract: a clone of the `Arc<ArcSwapOption<T>>` captured
    /// before `store(...)` must see `Some(...)` on a subsequent
    /// `load_full()`. This is what pre-spawned async tasks rely on.
    #[test]
    fn pre_captured_clone_sees_late_store() {
        // Step 1: field initialized empty (mirrors `ExecutionRunner::new`).
        let field: Arc<ArcSwapOption<ModelRegistry>> = Arc::new(ArcSwapOption::from(None));

        // Step 2: `RunnerContinuationInvoker` clones the handle inside
        // `with_config` BEFORE the setter runs.
        let captured = field.clone();
        assert!(captured.load_full().is_none(), "field starts empty");

        // Step 3: `runtime.rs:145` calls `set_model_registry(...)`.
        field.store(Some(load_user_registry()));

        // Step 4: the pre-captured clone reads the live value at fire time.
        let reg = captured
            .load_full()
            .expect("late store must be visible to pre-captured clone");

        // And the registry returns the real context window, not 8192.
        let ctx = reg.context_window("glm-5-turbo");
        assert_eq!(
            ctx.input, 200_000,
            "registry lookup must return glm-5-turbo's real 200k input \
             window, not the 8192 fallback"
        );
    }

    /// Multiple pre-captured clones (e.g. multiple background tasks)
    /// each see the latest stored value. Mirrors the real topology:
    /// spawn_delegation_handler + ContinuationWatcher + others.
    #[test]
    fn multiple_captures_all_observe_late_store() {
        let field: Arc<ArcSwapOption<ModelRegistry>> = Arc::new(ArcSwapOption::from(None));

        let cap_a = field.clone();
        let cap_b = field.clone();
        let cap_c = field.clone();

        field.store(Some(load_user_registry()));

        for (name, cap) in [("a", cap_a), ("b", cap_b), ("c", cap_c)] {
            assert!(
                cap.load_full().is_some(),
                "capture '{name}' must observe the stored registry"
            );
        }
    }

    /// Sanity: an unknown model falls back to the registry's internal
    /// `input: 200_000`, NOT the executor's `8192`. That proves the fix
    /// also helps the degenerate case (unknown model) as long as the
    /// registry itself is installed.
    #[test]
    fn unknown_model_uses_registry_fallback_not_executor_fallback() {
        let field: Arc<ArcSwapOption<ModelRegistry>> = Arc::new(ArcSwapOption::from(None));
        let captured = field.clone();
        field.store(Some(load_user_registry()));

        let reg = captured.load_full().expect("installed");
        let ctx = reg.context_window("some-unknown-model-xyz");
        assert_eq!(
            ctx.input, 200_000,
            "registry's internal fallback for unknown models is 200k, \
             not the 8192 emergency default"
        );
    }
}
