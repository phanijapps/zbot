//! # Application State
//!
//! Shared state for the gateway application.

use crate::connectors::{ConnectorRegistry, ConnectorService};
use crate::cron::CronScheduler;
use crate::database::{ConversationRepository, DatabaseManager};
use crate::events::EventBus;
use crate::execution::{
    new_workspace_cache, DelegationRegistry, MemoryRecall, SessionArchiver, SessionDistiller,
    WorkspaceCache,
};
use crate::hooks::HookRegistry;
use crate::services::{
    AgentService, McpService, ModelRegistry, ProviderService, RuntimeService, SettingsService,
    SharedVaultPaths, SkillService, VaultPaths,
};
use agent_runtime::llm::EmbeddingClient;
use agent_tools::MemoryEntry;
use agent_tools::MemoryStore;
use api_logs::LogService;
use chrono::Utc;
use execution_state::StateService;
use gateway_database::vector_index::{SqliteVecIndex, VectorIndex};
use gateway_database::{
    DistillationRepository, EpisodeRepository, KgEpisodeRepository, KnowledgeDatabase,
    MemoryRepository, ProcedureRepository, RecallLogRepository, WardWikiRepository,
};
use gateway_services::EmbeddingService;
use knowledge_graph::{GraphService, GraphStorage};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Shared application state for the gateway.
#[derive(Clone)]
pub struct AppState {
    /// Agent service for managing agent configurations.
    pub agents: Arc<AgentService>,

    /// Skill service for managing skill configurations.
    pub skills: Arc<SkillService>,

    /// Provider service for managing LLM providers.
    pub provider_service: Arc<ProviderService>,

    /// MCP service for managing MCP server configurations.
    pub mcp_service: Arc<McpService>,

    /// Runtime service for agent execution.
    pub runtime: Arc<RuntimeService>,

    /// Event bus for broadcasting events.
    pub event_bus: Arc<EventBus>,

    /// Hook registry for managing inbound triggers.
    pub hook_registry: Option<Arc<HookRegistry>>,

    /// Delegation registry for tracking agent delegations.
    pub delegation_registry: Arc<DelegationRegistry>,

    /// Conversation repository for message persistence.
    pub conversations: Arc<ConversationRepository>,

    /// Knowledge database — memory facts, graph, vec0 indexes.
    pub knowledge_db: Arc<KnowledgeDatabase>,

    /// Settings service for application configuration.
    pub settings: Arc<SettingsService>,

    /// Log service for execution tracing.
    pub log_service: Arc<LogService<DatabaseManager>>,

    /// State service for execution state management.
    pub state_service: Arc<StateService<DatabaseManager>>,

    /// Connector registry for external bridge management.
    pub connector_registry: Arc<ConnectorRegistry>,

    /// Bridge registry for WebSocket worker connections.
    pub bridge_registry: Arc<gateway_bridge::BridgeRegistry>,

    /// Bridge outbox for reliable message delivery to workers.
    pub bridge_outbox: Arc<gateway_bridge::OutboxRepository>,

    /// Gateway bus for bridge inbound message routing (set during server start).
    pub bridge_bus: Option<Arc<dyn gateway_bus::GatewayBus>>,

    /// Memory repository for accessing agent memory facts.
    pub memory_repo: Option<Arc<MemoryRepository>>,

    /// Goal repository — active goals used for intent boost in unified recall.
    pub goal_repo: Option<Arc<gateway_database::GoalRepository>>,

    /// Distillation repository for tracking distillation run outcomes.
    pub distillation_repo: Option<Arc<DistillationRepository>>,

    /// Session distiller for triggering on-demand distillation (e.g., backfill).
    pub distiller: Option<Arc<SessionDistiller>>,

    /// Episode repository for accessing session episodes.
    pub episode_repo: Option<Arc<EpisodeRepository>>,

    /// Knowledge graph episode repository (Phase 6a+).
    pub kg_episode_repo: Option<Arc<KgEpisodeRepository>>,

    /// Graph service for knowledge graph operations.
    pub graph_service: Option<Arc<GraphService>>,

    /// Trait-based knowledge-graph store (Phase 1 extraction).
    /// Coexists with `graph_service` — consumers are migrated incrementally.
    /// `None` when `GraphStorage` fails to initialise (same condition as
    /// `graph_service`).
    pub kg_store: Option<Arc<dyn zero_stores::KnowledgeGraphStore>>,

    /// Streaming ingestion queue (Phase 2) — None when graph is unavailable.
    pub ingestion_queue: Option<Arc<gateway_execution::ingest::IngestionQueue>>,

    /// Per-source + global backpressure gate for `/api/graph/ingest`.
    pub ingestion_backpressure: Option<Arc<gateway_execution::ingest::Backpressure>>,

    /// Cron scheduler for scheduled agent triggers.
    /// Optional because it requires async initialization with GatewayBus.
    pub cron_scheduler: Option<Arc<CronScheduler>>,

    /// Plugin manager for STDIO plugin lifecycle.
    pub plugin_manager: Arc<gateway_bridge::PluginManager>,

    /// Session archiver for offloading old transcripts to compressed files.
    pub session_archiver: Option<Arc<SessionArchiver>>,

    /// Sleep-time worker — triggers graph compaction/consolidation cycles.
    /// Set by server.start() in Phase 4 Task 10; `None` until then.
    pub sleep_time_worker: Option<Arc<gateway_execution::sleep::SleepTimeWorker>>,

    /// Compaction repository — read-model for the last compaction run.
    /// Set by server.start() in Phase 4 Task 10; `None` until then.
    pub compaction_repo: Option<Arc<gateway_database::CompactionRepository>>,

    /// Model capabilities registry (bundled + local overrides).
    pub model_registry: Arc<ModelRegistry>,

    /// Embedding service — owns live EmbeddingClient, supports backend swap.
    pub embedding_service: Arc<EmbeddingService>,

    /// Cached workspace context (shared with ExecutionRunner).
    workspace_cache: WorkspaceCache,

    /// Vault paths for accessing configuration and data directories.
    pub paths: SharedVaultPaths,

    /// Configuration directory path (legacy, use paths.vault_dir() instead).
    pub config_dir: PathBuf,
}

/// Boot-time helper: synchronously reconcile vec0 tables to match the
/// `EmbeddingService` dimension. Extracted from `AppState::new` so the
/// constructor stays under the cognitive-complexity threshold.
///
/// `current_dim == 0` is the `EmbeddingBackend::Unconfigured` sentinel —
/// the user hasn't picked a backend yet, so we leave the marker-derived
/// tables in place and let the async reconciler reindex once the user
/// reconfigures.
fn sync_reconcile_vec_dim_at_boot(
    embedding_service: &Arc<EmbeddingService>,
    knowledge_db: &Arc<gateway_database::KnowledgeDatabase>,
) {
    let current_dim = embedding_service.dimensions();
    if current_dim == 0 {
        tracing::info!("Embedding backend unconfigured at boot — skipping sync vec0 reconcile");
        return;
    }
    tracing::warn!(
        dim = current_dim,
        "Embedding dim mismatch at boot — rebuilding vec0 tables synchronously"
    );
    if let Err(e) = knowledge_db.reconcile_vec_tables_dim(current_dim) {
        tracing::error!(
            "Synchronous vec0 reconcile failed ({e}); recall may be degraded until the async reindex completes"
        );
        return;
    }
    if let Err(e) = embedding_service.mark_indexed(current_dim) {
        tracing::warn!("mark_indexed failed after sync reconcile: {e}; async reindex will retry");
        return;
    }
    tracing::info!(
        dim = current_dim,
        "Synchronous vec0 reconcile complete; content repopulates at next sleep cycle"
    );
}

impl AppState {
    /// Create a new application state.
    ///
    /// This creates a fully initialized state with execution runner and SQLite database.
    pub fn new(config_dir: PathBuf) -> Self {
        // Create centralized vault paths
        let paths = Arc::new(VaultPaths::new(config_dir.clone()));

        // Ensure required directories exist
        if let Err(e) = paths.ensure_dirs_exist() {
            tracing::warn!("Failed to create vault directories: {}", e);
        }

        let agents_dir = paths.agents_dir();
        let skills_roots = paths.skills_dirs();
        let event_bus = Arc::new(EventBus::new());
        let agents = Arc::new(AgentService::new(agents_dir));
        // Load skills from the vault first, then $HOME/.agents/skills.
        // Vault wins when both roots provide a skill with the same name.
        let skills = Arc::new(SkillService::with_roots(skills_roots));
        let provider_service = Arc::new(ProviderService::new(paths.clone()));
        let mcp_service = Arc::new(McpService::new(paths.clone()));

        // Initialize model capabilities registry (bundled + local overrides)
        let bundled_models = gateway_templates::Templates::get("models_registry.json")
            .map(|f| f.data.to_vec())
            .unwrap_or_default();
        let model_registry = Arc::new(ModelRegistry::load(&bundled_models, paths.vault_dir()));

        // Initialize SQLite database for conversation persistence
        let db_manager = Arc::new(
            DatabaseManager::new(paths.clone())
                .expect("Failed to initialize conversation database"),
        );
        let conversation_repo = Arc::new(ConversationRepository::new(db_manager.clone()));

        // Initialize knowledge database (memory facts, graph, vec0 indexes)
        let knowledge_db = Arc::new(
            KnowledgeDatabase::new(paths.clone()).expect("Failed to initialize knowledge database"),
        );

        // Create log service for execution tracing
        let log_service = Arc::new(LogService::new(db_manager.clone()));

        // Create state service for execution state management
        let state_service = Arc::new(StateService::new(db_manager.clone()));

        // Create connector registry
        let connector_service = ConnectorService::new(paths.clone());
        let connector_registry = Arc::new(ConnectorRegistry::new(connector_service));

        // Create workspace cache (shared between AppState and ExecutionRunner)
        let workspace_cache = new_workspace_cache();

        // Create bridge registry and outbox for WebSocket workers
        let bridge_registry = Arc::new(gateway_bridge::BridgeRegistry::new());
        let bridge_outbox = Arc::new(gateway_bridge::OutboxRepository::new(db_manager.clone()));

        // Initialize memory evolution services — repositories that need vector
        // similarity get a SqliteVecIndex over their vec0 partner table.
        let memory_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "memory_facts_index", "fact_id")
                .expect("vec index init"),
        );
        let memory_repo = Arc::new(MemoryRepository::new(knowledge_db.clone(), memory_vec));
        let goal_repo = Arc::new(gateway_database::GoalRepository::new(knowledge_db.clone()));
        let distillation_repo = Arc::new(DistillationRepository::new(db_manager.clone()));
        let episode_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "session_episodes_index", "episode_id")
                .expect("vec index init"),
        );
        let episode_repo = Arc::new(EpisodeRepository::new(knowledge_db.clone(), episode_vec));
        let kg_episode_repo = Arc::new(KgEpisodeRepository::new(knowledge_db.clone()));

        // Initialize knowledge graph service and storage
        let (graph_service, graph_storage): (Option<Arc<GraphService>>, Option<Arc<GraphStorage>>) =
            match GraphStorage::new(knowledge_db.clone()) {
                Ok(storage) => {
                    let storage = Arc::new(storage);
                    let service = Arc::new(GraphService::new(storage.clone()));
                    tracing::info!("Knowledge graph service initialized");
                    (Some(service), Some(storage))
                }
                Err(e) => {
                    tracing::warn!("Knowledge graph initialization failed: {}", e);
                    (None, None)
                }
            };

        // EmbeddingService — owns the live EmbeddingClient and supports
        // hot-swap between internal (fastembed) and Ollama backends.
        // Phase 1 of embedding-backend-selection: boot succeeds even if the
        // configured Ollama endpoint is unreachable; consumers continue
        // holding their Arc<dyn EmbeddingClient> cloned from service.client().
        let embedding_service = match EmbeddingService::from_config(paths.clone()) {
            Ok(svc) => Arc::new(svc),
            Err(e) => {
                tracing::warn!(
                    "EmbeddingService init failed ({e}); falling back to internal/384d default"
                );
                Arc::new(
                    EmbeddingService::with_config(paths.clone(), Default::default())
                        .expect("default EmbeddingService must build"),
                )
            }
        };
        // Best-effort boot-time reindex. Non-fatal.
        if let Err(e) = embedding_service.ensure_indexed_blocking() {
            tracing::warn!("EmbeddingService ensure_indexed_blocking failed: {e}");
        }

        // Fix 2: synchronously align the vec0 tables' dim with the live
        // `EmbeddingService` BEFORE we start accepting WebSocket invokes.
        //
        // Without this, the daemon accepts a user prompt while the
        // async reconciler in `reconcile_embeddings_at_boot` is still
        // running. The first `memory.recall` then hits either:
        //   - a dim mismatch (tables at 384, client embeds at 1024), or
        //   - a brand-new `*__new` rename window left by the async path.
        //
        // Drop-and-recreate synchronously + mark_indexed(current_dim).
        // Table content is repopulated from source rows at the next sleep
        // cycle; recall returns empty in the interim instead of erroring.
        //
        // `current_dim == 0` is the `EmbeddingBackend::Unconfigured`
        // sentinel — the user hasn't picked a backend yet. Leave the
        // marker-derived tables in place (the default 384 layout from
        // `KnowledgeDatabase::new` is still usable for FTS-only recall)
        // and let the async reconciler reindex once the user reconfigures.
        if embedding_service.needs_reindex() {
            sync_reconcile_vec_dim_at_boot(&embedding_service, &knowledge_db);
        }
        // Hand downstream (distillation, recall, memory_fact_store, etc.) a
        // LiveEmbeddingClient wrapper so they follow ArcSwap backend changes
        // instead of caching the boot-time client (which would still be the
        // Noop / Unconfigured client after the user later picks Ollama).
        let embedding_client: Option<Arc<dyn EmbeddingClient>> = Some(Arc::new(
            gateway_services::LiveEmbeddingClient::new(embedding_service.clone()),
        ));
        tracing::info!(
            "Embedding client ready (lazy, {}d)",
            embedding_service.dimensions()
        );

        // Load recall configuration (compiled defaults merged with optional user overrides)
        let recall_config = Arc::new(gateway_services::RecallConfig::load_from_path(
            paths.vault_dir(),
        ));

        // Create session archiver for offloading old transcripts to compressed files
        let archive_path = paths
            .data_dir()
            .join(&recall_config.session_offload.archive_path);
        let session_archiver = Arc::new(SessionArchiver::new(db_manager.clone(), archive_path));

        // Create memory recall with optional graph enrichment and episodic recall
        let mut memory_recall_inner = match &graph_service {
            Some(gs) => MemoryRecall::with_graph(
                embedding_client.clone(),
                memory_repo.clone(),
                gs.clone(),
                recall_config.clone(),
            ),
            None => MemoryRecall::new(
                embedding_client.clone(),
                memory_repo.clone(),
                recall_config.clone(),
            ),
        };
        memory_recall_inner.set_episode_repo(episode_repo.clone());

        // Wire recall log for tracking recalled facts per session (enables predictive recall)
        let recall_log = Arc::new(RecallLogRepository::new(db_manager.clone()));
        memory_recall_inner.set_recall_log(recall_log);

        // Wire ward wiki repository for wiki-first recall
        let wiki_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "wiki_articles_index", "article_id")
                .expect("vec index init"),
        );
        let wiki_repo = Arc::new(WardWikiRepository::new(
            knowledge_db.clone(),
            wiki_vec.clone(),
        ));
        memory_recall_inner.set_wiki_repo(wiki_repo);

        // Wire procedure repository for procedure recall during intent analysis
        let procedure_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "procedures_index", "procedure_id")
                .expect("vec index init"),
        );
        let procedure_repo = Arc::new(ProcedureRepository::new(
            knowledge_db.clone(),
            procedure_vec,
        ));
        memory_recall_inner.set_procedure_repo(procedure_repo.clone());

        let memory_recall = Arc::new(memory_recall_inner);

        // Clone embedding client before it's moved into distiller — the runner
        // also needs it so the memory fact store can generate embeddings.
        let runner_embedding_client = embedding_client.clone();

        // Clone graph_storage before it's moved into the distiller — the runner
        // also needs it for the graph_query tool.
        let runner_graph_storage = graph_storage.clone();

        // Build the trait-object KG store from runner_graph_storage.
        // Coexists with graph_service/graph_storage until Phase 5 retirement.
        //
        // We use `with_embedding_client` (not `new`) so the trait method
        // `reindex_embeddings` is functional. The wired client is the
        // `LiveEmbeddingClient` constructed above, so it follows ArcSwap
        // backend changes — same client the gateway-side wrapper at
        // `gateway-execution::sleep::embedding_reindex` already uses.
        let kg_store: Option<Arc<dyn zero_stores::KnowledgeGraphStore>> =
            runner_graph_storage.as_ref().map(|gs| {
                let embedder = embedding_client
                    .clone()
                    .expect("embedding_client wired above for distillation/recall");
                Arc::new(zero_stores_sqlite::SqliteKgStore::with_embedding_client(
                    gs.clone(),
                    embedder,
                )) as Arc<dyn zero_stores::KnowledgeGraphStore>
            });

        let episode_repo_ref = episode_repo.clone();

        // Create settings service (before distiller & runtime, so we can read execution settings)
        let settings = Arc::new(SettingsService::new(paths.clone()));

        let wiki_repo = Arc::new(WardWikiRepository::new(knowledge_db.clone(), wiki_vec));

        let mut distiller_inner = SessionDistiller::new(
            provider_service.clone(),
            embedding_client.clone(),
            conversation_repo.clone(),
            memory_repo.clone(),
            graph_storage,
            Some(distillation_repo.clone()),
            Some(episode_repo),
            paths.clone(), // For loading distillation_prompt.md
            Some(settings.clone()),
        );
        distiller_inner.set_wiki_repo(wiki_repo);
        distiller_inner.set_procedure_repo(procedure_repo.clone());
        let distiller = Arc::new(distiller_inner);

        // Keep a handle for on-demand distillation (backfill, trigger)
        let distiller_ref = distiller.clone();
        let max_parallel_agents = settings
            .get_execution_settings()
            .map(|s| s.max_parallel_agents)
            .unwrap_or(2);
        tracing::info!(max_parallel_agents, "Execution settings loaded");

        // Create streaming ingestion queue + backpressure BEFORE the runtime so the
        // runner can be wired with an IngestionAdapter.
        // Requires graph_storage — if the graph failed to initialize, we skip.
        let (ingestion_queue, ingestion_backpressure) = match runner_graph_storage.as_ref().cloned()
        {
            Some(gs) => {
                let extractor = Arc::new(gateway_execution::ingest::extractor::LlmExtractor::new(
                    provider_service.clone(),
                    "root".to_string(),
                ));
                let queue = Arc::new(gateway_execution::ingest::IngestionQueue::start(
                    2,
                    kg_episode_repo.clone(),
                    gs,
                    extractor,
                ));
                let bp = Arc::new(gateway_execution::ingest::Backpressure::new(
                    gateway_execution::ingest::BackpressureConfig::default(),
                    kg_episode_repo.clone(),
                ));
                (
                    Some(queue) as Option<Arc<gateway_execution::ingest::IngestionQueue>>,
                    Some(bp) as Option<Arc<gateway_execution::ingest::Backpressure>>,
                )
            }
            None => (None, None),
        };

        // Build agent-tool adapters so runner can register `ingest` + `goal` tools.
        // The adapter needs graph_storage so the structured-ingest path can
        // call `store_knowledge` directly without going through LLM extraction.
        let ingestion_adapter: Option<Arc<dyn agent_tools::IngestionAccess>> =
            match (ingestion_queue.as_ref(), runner_graph_storage.as_ref()) {
                (Some(q), Some(gs)) => Some(Arc::new(
                    gateway_execution::invoke::ingest_adapter::IngestionAdapter::new(
                        q.clone(),
                        kg_episode_repo.clone(),
                        gs.clone(),
                    ),
                )
                    as Arc<dyn agent_tools::IngestionAccess>),
                _ => None,
            };
        let goal_adapter: Option<Arc<dyn agent_tools::GoalAccess>> = Some(Arc::new(
            gateway_execution::invoke::goal_adapter::GoalAdapter::new(goal_repo.clone()),
        )
            as Arc<dyn agent_tools::GoalAccess>);

        // Create runtime with execution runner and connector registry
        let runtime = Arc::new(RuntimeService::with_runner_and_connectors(
            event_bus.clone(),
            agents.clone(),
            provider_service.clone(),
            paths.clone(),
            conversation_repo.clone(),
            mcp_service.clone(),
            skills.clone(),
            log_service.clone(),
            state_service.clone(),
            Some(connector_registry.clone()),
            workspace_cache.clone(),
            Some(memory_repo.clone()),
            Some(distiller),
            Some(memory_recall),
            Some(bridge_registry.clone()),
            Some(bridge_outbox.clone()),
            runner_embedding_client,
            max_parallel_agents,
            runner_graph_storage.clone(),
            Some(kg_episode_repo.clone()),
            ingestion_adapter,
            goal_adapter,
        ));

        // Phase 4: CompactionRepository + SleepTimeWorker (background maintenance).
        let compaction_repo = Arc::new(gateway_database::CompactionRepository::new(
            knowledge_db.clone(),
        ));

        // One-shot backfill: populate legacy kg_entities / kg_relationships
        // rows with the richer metadata introduced in commits b816702,
        // 1bc21f6, 5bf3013. Marker row in kg_compactions gates this so
        // subsequent daemon starts are a no-op. Non-fatal on failure —
        // a backfill bug must never prevent the daemon from booting.
        {
            let backfiller = gateway_execution::sleep::KgBackfiller::new(knowledge_db.clone());
            match backfiller.run_once_blocking() {
                Ok(stats) if stats.already_done => {
                    tracing::debug!("kg_backfill: marker present, skipping");
                }
                Ok(stats) => {
                    tracing::info!(
                        entities_scanned = stats.entities_scanned,
                        entities_updated = stats.entities_updated,
                        relationships_scanned = stats.relationships_scanned,
                        relationships_updated = stats.relationships_updated,
                        "kg_backfill: completed",
                    );
                }
                Err(e) => {
                    tracing::error!(error = %e, "kg_backfill: failed (non-fatal)");
                }
            }
        }

        let sleep_time_worker = runner_graph_storage.as_ref().cloned().map(|gs| {
            let verifier: Option<Arc<dyn gateway_execution::sleep::compactor::PairwiseVerifier>> =
                Some(Arc::new(
                    gateway_execution::sleep::LlmPairwiseVerifier::new(provider_service.clone()),
                ));
            let compactor = Arc::new(gateway_execution::sleep::Compactor::new(
                gs.clone(),
                compaction_repo.clone(),
                verifier,
            ));
            let decay = Arc::new(gateway_execution::sleep::DecayEngine::new(
                gs.clone(),
                gateway_execution::sleep::DecayConfig::default(),
            ));
            let pruner = Arc::new(gateway_execution::sleep::Pruner::new(
                gs,
                compaction_repo.clone(),
            ));
            // Synthesizer + PatternExtractor — both depend on a default LLM
            // provider being configured. We construct them unconditionally;
            // the ops themselves log+skip if provider listing fails at run
            // time, so a bootless config never aborts the cycle.
            let synth_llm = Arc::new(gateway_execution::sleep::LlmSynthesizer::new(
                provider_service.clone(),
            ));
            let synthesizer = Arc::new(gateway_execution::sleep::Synthesizer::new(
                knowledge_db.clone(),
                memory_repo.clone(),
                compaction_repo.clone(),
                synth_llm,
                embedding_client.clone(),
            ));
            let pattern_llm = Arc::new(gateway_execution::sleep::LlmPatternExtractor::new(
                provider_service.clone(),
            ));
            let pattern_extractor = Arc::new(gateway_execution::sleep::PatternExtractor::new(
                knowledge_db.clone(),
                db_manager.clone(),
                procedure_repo.clone(),
                compaction_repo.clone(),
                pattern_llm,
            ));
            // kg_store is Some whenever runner_graph_storage (and therefore this
            // closure) is Some — safe to unwrap here.
            let orphan_kg_store = kg_store
                .clone()
                .expect("kg_store is Some when graph_storage is Some");
            let orphan_archiver = Arc::new(gateway_execution::sleep::OrphanArchiver::new(
                knowledge_db.clone(),
                orphan_kg_store,
                compaction_repo.clone(),
            ));
            let ops = gateway_execution::sleep::SleepOps {
                synthesizer: Some(synthesizer),
                pattern_extractor: Some(pattern_extractor),
                orphan_archiver: Some(orphan_archiver),
            };
            Arc::new(gateway_execution::sleep::SleepTimeWorker::start_with_ops(
                compactor,
                decay,
                pruner,
                ops,
                std::time::Duration::from_secs(60 * 60),
                "root".to_string(),
            ))
        });

        // Create hook registry
        let hook_registry = Arc::new(HookRegistry::new(event_bus.clone()));

        // Create delegation registry
        let delegation_registry = Arc::new(DelegationRegistry::new());

        // Create plugin manager
        let plugin_manager = Arc::new(gateway_bridge::PluginManager::new(
            paths.plugins_dir(),
            bridge_registry.clone(),
            bridge_outbox.clone(),
            None, // bus is set later by server.start()
        ));

        Self {
            agents,
            skills,
            provider_service,
            mcp_service,
            runtime,
            event_bus,
            hook_registry: Some(hook_registry),
            delegation_registry,
            conversations: conversation_repo,
            knowledge_db,
            settings,
            log_service,
            state_service,
            connector_registry,
            bridge_registry,
            bridge_outbox,
            bridge_bus: None,     // Set by server.start() before router creation
            cron_scheduler: None, // Initialized by server.start()
            session_archiver: Some(session_archiver),
            sleep_time_worker,
            compaction_repo: Some(compaction_repo),
            plugin_manager,
            model_registry,
            embedding_service,
            workspace_cache,
            paths,
            config_dir,
            memory_repo: Some(memory_repo),
            goal_repo: Some(goal_repo),
            distillation_repo: Some(distillation_repo),
            distiller: Some(distiller_ref),
            episode_repo: Some(episode_repo_ref),
            kg_episode_repo: Some(kg_episode_repo),
            graph_service,
            kg_store,
            ingestion_queue,
            ingestion_backpressure,
        }
    }

    /// Create a minimal state without execution runner (for testing).
    pub fn minimal(config_dir: PathBuf) -> Self {
        let paths = Arc::new(VaultPaths::new(config_dir.clone()));
        let agents_dir = paths.agents_dir();
        let skills_roots = paths.skills_dirs();
        let event_bus = Arc::new(EventBus::new());

        // Initialize SQLite database for conversation persistence
        let db_manager = Arc::new(
            DatabaseManager::new(paths.clone())
                .expect("Failed to initialize conversation database"),
        );
        let conversation_repo = Arc::new(ConversationRepository::new(db_manager.clone()));
        let log_service = Arc::new(LogService::new(db_manager.clone()));
        let bridge_outbox = Arc::new(gateway_bridge::OutboxRepository::new(db_manager.clone()));
        let state_service = Arc::new(StateService::new(db_manager));
        let knowledge_db = Arc::new(
            KnowledgeDatabase::new(paths.clone()).expect("Failed to initialize knowledge database"),
        );
        let memory_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "memory_facts_index", "fact_id")
                .expect("vec index init"),
        );
        let memory_repo = Arc::new(MemoryRepository::new(knowledge_db.clone(), memory_vec));

        // Create connector registry
        let connector_service = ConnectorService::new(paths.clone());
        let connector_registry = Arc::new(ConnectorRegistry::new(connector_service));

        // Create bridge registry
        let bridge_registry = Arc::new(gateway_bridge::BridgeRegistry::new());

        // Create plugin manager
        let plugin_manager = Arc::new(gateway_bridge::PluginManager::new(
            paths.plugins_dir(),
            bridge_registry.clone(),
            bridge_outbox.clone(),
            None, // bus is set later by server.start()
        ));

        Self {
            agents: Arc::new(AgentService::new(agents_dir)),
            skills: Arc::new(SkillService::with_roots(skills_roots)),
            provider_service: Arc::new(ProviderService::new(paths.clone())),
            mcp_service: Arc::new(McpService::new(paths.clone())),
            runtime: Arc::new(RuntimeService::new(event_bus.clone())),
            event_bus,
            hook_registry: None,
            delegation_registry: Arc::new(DelegationRegistry::new()),
            conversations: conversation_repo,
            knowledge_db,
            settings: Arc::new(SettingsService::new(paths.clone())),
            log_service,
            state_service,
            connector_registry,
            bridge_registry,
            bridge_outbox,
            bridge_bus: None,
            cron_scheduler: None,
            session_archiver: None,
            sleep_time_worker: None,
            compaction_repo: None,
            model_registry: Arc::new(ModelRegistry::load(&[], paths.vault_dir())),
            embedding_service: Arc::new(
                EmbeddingService::with_config(paths.clone(), Default::default())
                    .expect("default EmbeddingService must build"),
            ),
            plugin_manager,
            workspace_cache: new_workspace_cache(),
            paths,
            config_dir,
            memory_repo: Some(memory_repo),
            goal_repo: None,
            distillation_repo: None,
            distiller: None,
            episode_repo: None,
            kg_episode_repo: None,
            graph_service: None,
            kg_store: None,
            ingestion_queue: None,
            ingestion_backpressure: None,
        }
    }

    /// Create with custom components.
    #[allow(clippy::too_many_arguments)]
    pub fn with_components(
        agents: Arc<AgentService>,
        skills: Arc<SkillService>,
        provider_service: Arc<ProviderService>,
        mcp_service: Arc<McpService>,
        runtime: Arc<RuntimeService>,
        event_bus: Arc<EventBus>,
        conversations: Arc<ConversationRepository>,
        log_service: Arc<LogService<DatabaseManager>>,
        state_service: Arc<StateService<DatabaseManager>>,
        connector_registry: Arc<ConnectorRegistry>,
        paths: SharedVaultPaths,
    ) -> Self {
        let config_dir = paths.vault_dir().clone();
        let knowledge_db = Arc::new(
            KnowledgeDatabase::new(paths.clone()).expect("Failed to initialize knowledge database"),
        );
        let memory_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "memory_facts_index", "fact_id")
                .expect("vec index init"),
        );
        let memory_repo = Arc::new(MemoryRepository::new(knowledge_db.clone(), memory_vec));

        // Create bridge registry and outbox
        let bridge_registry = Arc::new(gateway_bridge::BridgeRegistry::new());
        let bridge_outbox = {
            let db = Arc::new(
                DatabaseManager::new(paths.clone())
                    .expect("Failed to initialize database for bridge outbox"),
            );
            Arc::new(gateway_bridge::OutboxRepository::new(db))
        };

        // Create plugin manager
        let plugin_manager = Arc::new(gateway_bridge::PluginManager::new(
            paths.plugins_dir(),
            bridge_registry.clone(),
            bridge_outbox.clone(),
            None, // bus is set later by server.start()
        ));

        Self {
            agents,
            skills,
            provider_service,
            mcp_service,
            runtime,
            event_bus,
            hook_registry: None,
            delegation_registry: Arc::new(DelegationRegistry::new()),
            conversations,
            knowledge_db,
            settings: Arc::new(SettingsService::new(paths.clone())),
            log_service,
            state_service,
            connector_registry,
            bridge_registry,
            bridge_outbox,
            bridge_bus: None,
            cron_scheduler: None,
            session_archiver: None,
            sleep_time_worker: None,
            compaction_repo: None,
            model_registry: Arc::new(ModelRegistry::load(&[], paths.vault_dir())),
            embedding_service: Arc::new(
                EmbeddingService::with_config(paths.clone(), Default::default())
                    .expect("default EmbeddingService must build"),
            ),
            plugin_manager,
            workspace_cache: new_workspace_cache(),
            paths,
            config_dir,
            memory_repo: Some(memory_repo),
            goal_repo: None,
            distillation_repo: None,
            distiller: None,
            episode_repo: None,
            kg_episode_repo: None,
            graph_service: None,
            kg_store: None,
            ingestion_queue: None,
            ingestion_backpressure: None,
        }
    }

    /// Create with hook registry.
    pub fn with_hook_registry(mut self, hook_registry: Arc<HookRegistry>) -> Self {
        self.hook_registry = Some(hook_registry);
        self
    }

    /// Reconcile the indexed embedding dim against the current client dim.
    ///
    /// Runs at boot (from `GatewayServer::start`) and performs three things:
    ///
    /// 1. Pre-emptive Ollama ping — surfaces unreachability in `Health`
    ///    immediately instead of waiting for the periodic health loop.
    /// 2. If `needs_reindex()`, invokes
    ///    [`gateway_execution::sleep::embedding_reindex::reindex_all`] against
    ///    the live knowledge database, then writes the `.embedding-state`
    ///    marker on success.
    /// 3. Spawns the periodic health-check loop (60s tick).
    ///
    /// All failures are logged — embeddings-based recall degrades to FTS in
    /// the existing `recall_unified` path if the reindex does not complete.
    pub async fn reconcile_embeddings_at_boot(&self) {
        // 1. Preflight.
        self.embedding_service.preflight().await;

        // 2. Reindex if the marker dim disagrees with the live dim.
        if self.embedding_service.needs_reindex() {
            let current_dim = self.embedding_service.dimensions();
            tracing::info!(
                dim = current_dim,
                "Embedding dim/model mismatch vs marker — reindexing at boot"
            );
            let client = self.embedding_service.client();
            let svc = self.embedding_service.clone();
            let on_progress = move |table: &'static str, current: usize, total: usize| {
                svc.publish_health(gateway_services::Health::Reindexing {
                    table: table.to_string(),
                    current,
                    total,
                });
            };
            match gateway_execution::sleep::embedding_reindex::reindex_all(
                &self.knowledge_db,
                client,
                current_dim,
                &on_progress,
            )
            .await
            {
                Ok(_) => {
                    if let Err(e) = self.embedding_service.mark_indexed(current_dim) {
                        tracing::warn!("mark_indexed failed after boot reindex: {e}");
                    } else {
                        tracing::info!("Boot reindex complete at dim={current_dim}");
                    }
                    self.embedding_service
                        .publish_health(gateway_services::Health::Ready);
                }
                Err(e) => {
                    tracing::error!(
                        "Boot reindex failed: {e} — embeddings will be stale until next reconfigure"
                    );
                    // Leave health as-is (preflight already set any Ollama state);
                    // recall_unified falls back to FTS.
                }
            }
        }

        // 3. Start periodic health loop.
        let _handle = self.embedding_service.clone().start_health_loop();
        // JoinHandle intentionally dropped — loop lives for the process
        // lifetime; daemon shutdown drops the runtime.
    }

    /// Seed default agents and other initial data.
    ///
    /// This should be called after creating the state to set up default subagents
    /// that can be delegated to.
    pub async fn seed_defaults(&self) {
        // Get default provider ID
        let default_provider_id = self
            .provider_service
            .list()
            .ok()
            .and_then(|providers| {
                providers
                    .iter()
                    .find(|p| p.is_default)
                    .or_else(|| providers.first())
                    .and_then(|p| p.id.clone())
            })
            .unwrap_or_else(|| "default".to_string());

        // Resolve default model from default provider (first model in list)
        let default_model = self
            .provider_service
            .list()
            .ok()
            .and_then(|providers| {
                providers
                    .iter()
                    .find(|p| p.is_default)
                    .or_else(|| providers.first())
                    .and_then(|p| p.default_model().to_string().into())
            })
            .unwrap_or_else(|| "gpt-4o".to_string());

        // Seed default agents from bundled templates (configs + AGENTS.md instructions)
        let agent_template =
            gateway_templates::Templates::get("default_agents.json").map(|f| f.data.to_vec());
        if let Err(e) = self
            .agents
            .seed_default_agents(
                &default_provider_id,
                &default_model,
                agent_template.as_deref(),
                |name| {
                    let path = format!("agents/{}.md", name);
                    gateway_templates::Templates::get(&path)
                        .map(|f| String::from_utf8_lossy(&f.data).to_string())
                },
            )
            .await
        {
            tracing::warn!("Failed to seed default agents: {}", e);
        }

        // Seed default skills from bundled templates if skills dir is empty
        self.seed_default_skills();

        // Seed default policies from bundled template if no policies exist
        self.seed_default_policies();

        // Preload skills into cache
        if let Err(e) = self.skills.preload().await {
            tracing::warn!("Failed to preload skills: {}", e);
        }

        // Create Python venv and Node env if missing, then seed workspace memory
        self.ensure_runtime_environments().await;

        // Discover and start plugins
        self.discover_and_start_plugins().await;
    }

    /// Discover and start all enabled plugins.
    async fn discover_and_start_plugins(&self) {
        tracing::info!("Discovering plugins...");

        match self.plugin_manager.discover().await {
            Ok(discovered) => {
                if discovered.is_empty() {
                    tracing::info!("No plugins discovered");
                } else {
                    tracing::info!(
                        "Discovered {} plugin(s): {:?}",
                        discovered.len(),
                        discovered
                    );

                    // Start all enabled plugins
                    self.plugin_manager.start_all().await;
                }
            }
            Err(e) => {
                tracing::warn!("Failed to discover plugins: {}", e);
            }
        }
    }

    /// Seed default skills from bundled templates if skills directory is empty.
    fn seed_default_skills(&self) {
        let skills_dir = self.paths.vault_dir().join("skills");

        // Only seed if skills dir is empty or doesn't exist
        let has_skills = skills_dir.exists()
            && std::fs::read_dir(&skills_dir)
                .map(|mut entries| entries.next().is_some())
                .unwrap_or(false);

        if has_skills {
            tracing::debug!("Skills directory not empty, skipping seed");
            return;
        }

        tracing::info!("Seeding default skills from bundled templates");
        std::fs::create_dir_all(&skills_dir).ok();

        // Iterate all embedded files under skills/
        for path in gateway_templates::Templates::iter() {
            let path_str = path.as_ref();
            if !path_str.starts_with("skills/") {
                continue;
            }

            // path_str is like "skills/coding/SKILL.md" or "skills/yf-data/scripts/run.py"
            let dest = self.paths.vault_dir().join(path_str);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            if let Some(file) = gateway_templates::Templates::get(path_str) {
                if let Err(e) = std::fs::write(&dest, &file.data) {
                    tracing::warn!("Failed to seed skill file {}: {}", path_str, e);
                }
            }
        }

        let count = std::fs::read_dir(&skills_dir)
            .map(|entries| entries.count())
            .unwrap_or(0);
        tracing::info!("Seeded {} default skills", count);
    }

    /// Seed default policies from bundled template if no policies/corrections exist.
    fn seed_default_policies(&self) {
        let memory_repo = match &self.memory_repo {
            Some(repo) => repo,
            None => return,
        };

        // Check if any correction facts already exist
        let existing = memory_repo
            .get_facts_by_category("root", "correction", 1)
            .unwrap_or_default();
        if !existing.is_empty() {
            tracing::debug!("Policies already exist, skipping seed");
            return;
        }

        let template = match gateway_templates::Templates::get("default_policies.json") {
            Some(f) => f.data.to_vec(),
            None => return,
        };

        let policies: Vec<serde_json::Value> = match serde_json::from_slice(&template) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to parse default_policies.json: {}", e);
                return;
            }
        };

        let now = chrono::Utc::now().to_rfc3339();
        let mut count = 0;

        for policy in &policies {
            let category = policy["category"].as_str().unwrap_or("correction");
            let key = policy["key"].as_str().unwrap_or_default();
            let content = policy["content"].as_str().unwrap_or_default();
            let confidence = policy["confidence"].as_f64().unwrap_or(1.0);
            let pinned = policy["pinned"].as_bool().unwrap_or(true);

            if key.is_empty() || content.is_empty() {
                continue;
            }

            let fact = gateway_database::MemoryFact {
                id: format!("policy-{}", uuid::Uuid::new_v4()),
                session_id: None,
                agent_id: "root".to_string(),
                scope: "agent".to_string(),
                category: category.to_string(),
                key: key.to_string(),
                content: content.to_string(),
                confidence,
                mention_count: 5,
                source_summary: Some("Default policy".to_string()),
                embedding: None,
                ward_id: "__global__".to_string(),
                contradicted_by: None,
                created_at: now.clone(),
                updated_at: now.clone(),
                expires_at: None,
                valid_from: None,
                valid_until: None,
                superseded_by: None,
                pinned,
                epistemic_class: Some("current".to_string()),
                source_episode_id: None,
                source_ref: None,
            };

            if memory_repo.upsert_memory_fact(&fact).is_ok() {
                count += 1;
            }
        }

        if count > 0 {
            tracing::info!("Seeded {} default policies/instructions", count);
        }
    }

    /// Ensure Python venv and Node.js environment exist, then seed workspace memory.
    async fn ensure_runtime_environments(&self) {
        // Create wards directory structure
        self.ensure_wards_dir();

        let venv_ok = self.ensure_python_venv().await;
        let node_ok = self.ensure_node_env();
        self.seed_workspace_env_status(venv_ok, node_ok);
        self.populate_workspace_cache().await;
    }

    /// Create the wards directory with scratch ward + wiki vault ward.
    ///
    /// The wiki ward is the Obsidian vault — it receives promoted content
    /// from producer-skill runs (book-reader, research archetypes) via the
    /// `wiki` skill. Its name is configurable via `settings.json →
    /// execution.wiki.wardName` (default `"wiki"`). We seed it at startup so
    /// delegated subagents (which cannot create wards) can just `use` it.
    fn ensure_wards_dir(&self) {
        let wards_dir = self.config_dir.join("wards");
        let scratch_dir = wards_dir.join("scratch");

        if !scratch_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&scratch_dir) {
                tracing::warn!("Failed to create wards/scratch directory: {}", e);
            } else {
                tracing::info!(
                    "Created wards directory with scratch ward at {}",
                    wards_dir.display()
                );
            }
        }

        let wiki_name = self
            .settings
            .load()
            .ok()
            .map(|s| s.execution.wiki.ward_name)
            .unwrap_or_else(|| "wiki".to_string());

        self.ensure_wiki_ward(&wards_dir, &wiki_name);
    }

    /// Create the wiki vault ward with canonical Obsidian tree + AGENTS.md marker.
    ///
    /// Idempotent — existing content is preserved. The marker
    /// `<!-- obsidian-vault -->` in AGENTS.md lets the `wiki` skill discover
    /// this ward via `ward(action="list")` regardless of the configured name.
    fn ensure_wiki_ward(&self, wards_dir: &std::path::Path, wiki_name: &str) {
        let wiki_dir = wards_dir.join(wiki_name);
        if let Err(e) = std::fs::create_dir_all(&wiki_dir) {
            tracing::warn!("Failed to create wiki ward directory: {}", e);
            return;
        }

        // Canonical Obsidian vault top-level folders.
        let vault_folders = [
            "00_Inbox",
            "10_Journal/Daily",
            "10_Journal/Weekly",
            "20_Projects",
            "30_Library/Books",
            "30_Library/Articles",
            "40_Research",
            "50_Resources",
            "60_Archive",
            "70_Assets/Knowledge_Graphs",
            "70_Assets/Images",
            "70_Assets/Documents",
            "_zztemplates",
        ];
        for folder in vault_folders {
            let _ = std::fs::create_dir_all(wiki_dir.join(folder));
        }

        // Seed AGENTS.md with the discovery marker and the full routing map.
        // This file is the source of truth for where content belongs — agents
        // that enter this ward read it on entry and follow it exactly.
        //
        // Re-seed on every startup IF the existing content starts with our
        // `<!-- obsidian-vault -->` marker (i.e. we wrote it previously, not
        // the user). This lets template updates flow through on gateway
        // restart without preserving a user-hand-edited file.
        let agents_md = wiki_dir.join("AGENTS.md");
        let should_seed = match std::fs::read_to_string(&agents_md) {
            Ok(existing) => existing.starts_with("<!-- obsidian-vault -->"),
            Err(_) => true, // missing → seed
        };
        if should_seed {
            let content = format!(
                "<!-- obsidian-vault -->\n\
                 # {wiki_name}\n\n\
                 ## Purpose\n\
                 Obsidian-style vault. Producer skills (book-reader, stock-analysis, news-research, …) emit vault-ready folders in their origin ward; the `wiki` skill promotes them here. **This AGENTS.md is the authoritative routing map.** If a memory fact contradicts it, this file wins.\n\n\
                 ## Folder map — what goes where\n\n\
                 | Vault path | What lives here | Producer source |\n\
                 | --- | --- | --- |\n\
                 | `00_Inbox/` | Unclassified items awaiting manual sorting. Never delete; the user reviews periodically. | Anything that fails classification |\n\
                 | `10_Journal/Daily/` | One `YYYY-MM-DD.md` per day. | Journal skill (future) |\n\
                 | `10_Journal/Weekly/` | One `YYYY-Www.md` per ISO week. | Journal skill (future) |\n\
                 | `20_Projects/<project>/` | Agent-produced final project reports and deliverables. One folder per project. | `reports/<project>/` in origin ward |\n\
                 | `30_Library/Books/<slug>/` | A book as `_index.md` + `chunks/ch-NN.md` + `entities/<type>-<slug>.md`. `<slug>` is kebab-case from the title (strip leading articles). | `books/<slug>/` in origin ward (book-reader) |\n\
                 | `30_Library/Articles/<slug>/` | An article as `_index.md` (+ optional supporting files). `<slug>` is kebab-case from the title. | `articles/<slug>/` in origin ward (article-reader) |\n\
                 | `40_Research/<archetype>/<subject>/<date-slug>/` | Research snapshots. `<archetype>` is the producer skill name (`stock-analysis`, `news-research`, `product-research`, `competitive-analysis`, `academic-research`, `market-research`, `technical-research`, `policy-research`). `<subject>` is kebab-case. `<date-slug>` is ISO date with optional suffix. | `research/<archetype>/<subject>/<date-slug>/` |\n\
                 | `50_Resources/` | Durable reference material the user curates. | Manual only — `wiki` skill does not write here. |\n\
                 | `60_Archive/` | Superseded or retired content. Move here manually when an item is no longer current. | Manual only. |\n\
                 | `70_Assets/Knowledge_Graphs/` | KG exports (DB dumps) if generated by a separate tool. | Reserved — `wiki` does not write here. |\n\
                 | `70_Assets/Images/` | Loose images from any ward. Renamed `<ward>__<basename>` on copy to avoid collisions. | `**/*.{{png,jpg,jpeg,svg,gif,webp}}` in origin ward |\n\
                 | `70_Assets/Documents/` | Loose PDFs from any ward. Renamed `<ward>__<basename>` on copy. | `**/*.pdf` in origin ward |\n\
                 | `_zztemplates/` | Obsidian note templates the user maintains. | Manual only — the skill never writes or reads here. |\n\n\
                 ## Slug rules (the #1 failure mode)\n\n\
                 Folder names under `30_Library/Books/`, `30_Library/Articles/`, `40_Research/<archetype>/`, `20_Projects/` are always **kebab-case slugs**, never display titles:\n\n\
                 - `30_Library/Books/christmas-carol/` ✅  not `30_Library/Books/A Christmas Carol/` ❌\n\
                 - `30_Library/Books/pride-and-prejudice/` ✅  not `30_Library/Books/Pride and Prejudice/` ❌\n\
                 - `40_Research/stock-analysis/tsla/2026-04-16-q1/` ✅  not `40_Research/Stock Analysis/TSLA Q1 2026/` ❌\n\n\
                 The display title lives in `_index.md` frontmatter (`title:`) and in wikilink aliases (`[[slug|Display Title]]`). The filesystem always uses the slug.\n\n\
                 ## Routing contract for the wiki skill\n\n\
                 The skill performs **whole-folder copy** with absolute paths, no content rewriting. For each producer folder in the origin ward:\n\n\
                 1. Compute source path: `SRC=<origin-ward>/<producer-folder>` (e.g. `<origin>/books/christmas-carol/`).\n\
                 2. Compute destination path per the folder map above: `DEST=<wiki-ward>/<vault-path>/<slug>/`.\n\
                 3. Copy `cp -a \"$SRC\" \"$DEST\"`. Preserve timestamps; preserve names; preserve nested structure.\n\
                 4. If the source doesn't match any rule, route to `00_Inbox/<relative-path>` — do NOT guess a category.\n\n\
                 ## Hard don'ts\n\n\
                 - Do NOT invent folders outside the numbered tree (`Literature/`, `StockResearch/`, `Books/`, etc. are WRONG — use the numbered paths).\n\
                 - Do NOT use display-case folder names with spaces or capitals.\n\
                 - Do NOT rewrite frontmatter, wikilinks, or markdown during the copy — producer skills own the content shape.\n\
                 - Do NOT delete from the origin ward.\n\
                 - Do NOT write into `50_Resources/`, `60_Archive/`, `_zztemplates/`, or `70_Assets/Knowledge_Graphs/` — those are user-managed or reserved.\n\
                 - Do NOT run code, fetch data, or do research in this ward. It is content-only.\n\
                 - Do NOT edit promoted files outside their `<!-- manual -->` blocks — the skill overwrites on re-promotion.\n\n\
                 ## Discovery marker\n\n\
                 The first line of this file (`<!-- obsidian-vault -->`) is the marker the wiki skill uses to find this ward via `ward(action=\"list\")`. Do not remove it.\n"
            );
            let _ = std::fs::write(&agents_md, content);
        }

        // Seed memory-bank/ scaffold so the ward matches the standard shape.
        let memory_bank = wiki_dir.join("memory-bank");
        let _ = std::fs::create_dir_all(&memory_bank);
        for file in ["ward.md", "structure.md", "core_docs.md"] {
            let path = memory_bank.join(file);
            if !path.exists() {
                let _ = std::fs::write(&path, "");
            }
        }

        tracing::info!("Wiki vault ward ready at {}", wiki_dir.display());
    }

    /// Populate the in-memory workspace cache from workspace.json.
    ///
    /// This is called once at startup after seeding. The same Arc is shared
    /// with ExecutionRunner, so all executors see the cached data without
    /// reading from disk on every invocation.
    async fn populate_workspace_cache(&self) {
        let workspace_path = self.paths.ward_dir("shared").join("workspace.json");

        let workspace = match std::fs::read_to_string(&workspace_path) {
            Ok(content) => match serde_json::from_str::<MemoryStore>(&content) {
                Ok(store) => {
                    let map: HashMap<String, serde_json::Value> = store
                        .entries
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.value.clone())))
                        .collect();
                    if map.is_empty() {
                        None
                    } else {
                        Some(map)
                    }
                }
                Err(_) => None,
            },
            Err(_) => None,
        };

        if let Some(ws) = workspace {
            let count = ws.len();
            *self.workspace_cache.write().await = Some(ws);
            tracing::info!("Populated workspace cache with {} entries", count);
        }
    }

    /// Create Python venv at `{config_dir}/wards/.venv` if it doesn't exist.
    /// Falls back to legacy `{config_dir}/venv` if it exists there.
    /// Returns true if the venv exists (either already existed or was created).
    async fn ensure_python_venv(&self) -> bool {
        let new_path = self.config_dir.join("wards").join(".venv");
        let legacy_path = self.config_dir.join("venv");

        // Use new path, but check legacy location too
        let venv_path = if new_path.exists() {
            new_path
        } else if legacy_path.exists() {
            legacy_path
        } else {
            new_path // Create at new location
        };

        let python_exe = if cfg!(windows) {
            venv_path.join("Scripts").join("python.exe")
        } else {
            venv_path.join("bin").join("python")
        };

        if python_exe.exists() {
            tracing::debug!("Python venv already exists at {}", venv_path.display());
            return true;
        }

        tracing::info!("Creating Python venv at {}", venv_path.display());
        let result = tokio::process::Command::new("python")
            .args(["-m", "venv"])
            .arg(&venv_path)
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                tracing::info!("Python venv created successfully");
                true
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("Failed to create Python venv: {}", stderr.trim());
                false
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to run python -m venv: {} (python may not be installed)",
                    e
                );
                false
            }
        }
    }

    /// Ensure Node.js working directory exists at `{config_dir}/wards/.node_env`.
    /// Falls back to legacy `{config_dir}/node_env` if it exists there.
    /// Just creates the directory — no npm init needed.
    fn ensure_node_env(&self) -> bool {
        let new_path = self.config_dir.join("wards").join(".node_env");
        let legacy_path = self.config_dir.join("node_env");

        let node_env_dir = if new_path.exists() {
            new_path
        } else if legacy_path.exists() {
            legacy_path
        } else {
            new_path
        };

        if node_env_dir.exists() {
            tracing::debug!("Node env already exists at {}", node_env_dir.display());
            return true;
        }

        tracing::info!("Creating Node env at {}", node_env_dir.display());

        if let Err(e) = std::fs::create_dir_all(&node_env_dir) {
            tracing::warn!("Failed to create node_env directory: {}", e);
            return false;
        }

        true
    }

    /// Seed workspace.json with python_env and node_env status.
    /// Only writes entries that don't already exist (preserves user state).
    /// Uses the same MemoryStore type as the memory tool to avoid format mismatch.
    fn seed_workspace_env_status(&self, venv_ok: bool, node_ok: bool) {
        let workspace_path = self.paths.ward_dir("shared").join("workspace.json");

        // Ensure parent directory exists
        if let Some(parent) = workspace_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create workspace directory: {}", e);
                return;
            }
        }

        // Load existing store using the same type as the memory tool
        let mut store: MemoryStore = if let Ok(content) = std::fs::read_to_string(&workspace_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            MemoryStore::default()
        };

        let now = Utc::now().to_rfc3339();

        // Seed python_env if not already present
        if !store.entries.contains_key("python_env") {
            store.entries.insert(
                "python_env".to_string(),
                self.build_python_env_entry(venv_ok, &now),
            );
        }

        // Seed node_env if not already present
        if !store.entries.contains_key("node_env") {
            let node_env_dir = self.config_dir.join("node_env");
            let node_modules = node_env_dir.join("node_modules");

            let value = serde_json::json!({
                "exists": node_ok,
                "env_path": node_env_dir.display().to_string(),
                "node_modules": node_modules.display().to_string(),
            });

            store.entries.insert(
                "node_env".to_string(),
                MemoryEntry {
                    value: value.to_string(),
                    tags: vec!["system".to_string(), "node".to_string(), "env".to_string()],
                    created_at: now.clone(),
                    updated_at: now,
                },
            );
        }

        // Write back using the same format as the memory tool
        match serde_json::to_string_pretty(&store) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&workspace_path, content) {
                    tracing::warn!("Failed to write workspace.json: {}", e);
                } else {
                    tracing::info!("Seeded workspace.json with environment status");
                }
            }
            Err(e) => tracing::warn!("Failed to serialize workspace.json: {}", e),
        }
    }

    fn build_python_env_entry(&self, venv_ok: bool, now: &str) -> MemoryEntry {
        let venv_path = self.config_dir.join("venv");
        let python_exe = if cfg!(windows) {
            venv_path.join("Scripts").join("python.exe")
        } else {
            venv_path.join("bin").join("python")
        };
        let pip_exe = if cfg!(windows) {
            venv_path.join("Scripts").join("pip.exe")
        } else {
            venv_path.join("bin").join("pip")
        };
        let value = serde_json::json!({
            "exists": venv_ok,
            "venv_path": venv_path.display().to_string(),
            "executable": python_exe.display().to_string(),
            "pip": pip_exe.display().to_string(),
        });
        MemoryEntry {
            value: value.to_string(),
            tags: vec![
                "system".to_string(),
                "python".to_string(),
                "env".to_string(),
            ],
            created_at: now.to_string(),
            updated_at: now.to_string(),
        }
    }
}
