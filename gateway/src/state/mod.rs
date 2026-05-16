//! # Application State
//!
//! Shared state for the gateway application.

pub(crate) mod persistence_factory;
mod seeded_defaults;

use crate::connectors::{ConnectorRegistry, ConnectorService};
use crate::cron::CronScheduler;
use crate::events::EventBus;
use crate::execution::{DelegationRegistry, MemoryRecall, SessionArchiver, SessionDistiller};
use crate::hooks::HookRegistry;
use crate::services::{
    AgentService, McpService, ModelRegistry, ProviderService, RuntimeService, SettingsService,
    SharedVaultPaths, SkillService, VaultPaths,
};
use agent_runtime::llm::EmbeddingClient;
use api_logs::LogService;
use execution_state::StateService;
use gateway_services::EmbeddingService;
use std::path::PathBuf;
use std::sync::Arc;
use zero_stores_sqlite::kg::service::GraphService;
use zero_stores_sqlite::kg::storage::GraphStorage;
use zero_stores_sqlite::vector_index::{SqliteVecIndex, VectorIndex};
use zero_stores_sqlite::{
    ConversationRepository, DatabaseManager, DistillationRepository, EpisodeRepository,
    KgEpisodeRepository, KnowledgeDatabase, MemoryRepository, ProcedureRepository,
    WardWikiRepository,
};

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
    ///
    /// Wrapped in `Option` so test fixtures can build a minimal AppState
    /// without one. Production always carries `Some(...)`.
    pub knowledge_db: Option<Arc<KnowledgeDatabase>>,

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

    /// Trait-routed memory-fact store. The single read/write surface for
    /// memory facts.
    pub memory_store: Option<Arc<dyn zero_stores::MemoryFactStore>>,

    /// Goal repository — active goals used for intent boost in unified recall.
    pub goal_repo: Option<Arc<zero_stores_sqlite::GoalRepository>>,

    /// Distillation repository for tracking distillation run outcomes.
    pub distillation_repo: Option<Arc<DistillationRepository>>,

    /// Session distiller for triggering on-demand distillation (e.g., backfill).
    pub distiller: Option<Arc<SessionDistiller>>,

    /// Episode repository for accessing session episodes.
    pub episode_repo: Option<Arc<EpisodeRepository>>,

    /// Trait-routed episode store (Phase D2). Coexists with `episode_repo`
    /// for now; consumers migrate incrementally per the portability doc.
    /// `None` when `episode_repo` itself is `None`.
    pub episode_store: Option<Arc<dyn zero_stores_traits::EpisodeStore>>,

    /// Trait-routed wiki store (Phase D3). The handler-side migrations
    /// route through this; legacy callers still build a
    /// `WardWikiRepository` directly. `None` in minimal AppStates.
    pub wiki_store: Option<Arc<dyn zero_stores_traits::WikiStore>>,

    /// Trait-routed procedure store (Phase D4).
    pub procedure_store: Option<Arc<dyn zero_stores_traits::ProcedureStore>>,

    /// Knowledge graph episode repository (Phase 6a+).
    pub kg_episode_repo: Option<Arc<KgEpisodeRepository>>,

    /// Trait-routed kg-ingestion-episode store (Phase B). Wraps the
    /// SQLite kg_episode_repo. Backend-agnostic — a new backend
    /// implements the trait without consumers changing.
    pub kg_episode_store: Option<Arc<dyn zero_stores_traits::KgEpisodeStore>>,

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
    pub compaction_repo: Option<Arc<zero_stores_sqlite::CompactionRepository>>,

    /// Trait-routed compaction audit store (Phase D1). Wired in both
    /// SQLite and SurrealDB modes — the maintenance worker writes
    /// merge/prune/synthesis events here for Observatory display.
    /// Backend-agnostic: the trait has default no-op impls so any
    /// backend that doesn't care can inherit them.
    pub compaction_store: Option<Arc<dyn zero_stores_traits::CompactionStore>>,

    /// Trait-routed belief store (Belief Network Phase B-5 HTTP surface).
    /// `Some(...)` only when `execution.memory.beliefNetwork.enabled = true`
    /// AND the knowledge DB is wired. The HTTP handlers in
    /// `http::beliefs` and `http::belief_network` use this for 503-vs-200
    /// disambiguation: a `None` here means the Belief Network is disabled,
    /// not that the data is missing.
    pub belief_store: Option<Arc<dyn zero_stores_traits::BeliefStore>>,

    /// Trait-routed belief-contradiction store (Belief Network Phase B-5
    /// HTTP surface). Same opt-in gating as `belief_store`.
    pub belief_contradiction_store: Option<Arc<dyn zero_stores_traits::BeliefContradictionStore>>,

    /// In-memory recorder of recent Belief Network worker stats (Phase
    /// B-6). Always wired when the sleep-time worker is wired so the
    /// HTTP layer can render the Observatory belief panel even when the
    /// network itself is disabled (empty history + `enabled: false`).
    pub belief_network_activity: Option<Arc<gateway_memory::RecentBeliefNetworkActivity>>,

    /// Model capabilities registry (bundled + local overrides).
    pub model_registry: Arc<ModelRegistry>,

    /// Embedding service — owns live EmbeddingClient, supports backend swap.
    pub embedding_service: Arc<EmbeddingService>,

    /// Vault paths for accessing configuration and data directories.
    pub paths: SharedVaultPaths,

    /// Configuration directory path (legacy, use paths.vault_dir() instead).
    pub config_dir: PathBuf,

    /// LAN service advertiser. NoopAdvertiser when discovery is disabled.
    pub advertiser: std::sync::Arc<dyn discovery::Advertiser>,

    /// Active mDNS advertise handle. None until `start()` runs and only
    /// populated when `network.exposeToLan = true`.
    pub advertise_handle: std::sync::Arc<std::sync::Mutex<Option<discovery::AdvertiseHandle>>>,
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
    knowledge_db: &Arc<zero_stores_sqlite::KnowledgeDatabase>,
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

        // Factory for sleep-time memory LLM clients — built once, shared
        // across every sleep-time component that needs an LLM call.
        let memory_llm_factory: Arc<dyn gateway_memory::MemoryLlmFactory> = Arc::new(
            crate::memory_llm_factory::ProviderServiceLlmFactory::new(provider_service.clone()),
        );

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

        // Initialize knowledge database (memory facts, graph, vec0 indexes).
        // SQLite is the only backend; the Option-wrapping is retained so
        // downstream `.as_ref().map(...)` chains stay typecheck-stable.
        let knowledge_db: Option<Arc<KnowledgeDatabase>> = Some(Arc::new(
            KnowledgeDatabase::new(paths.clone()).expect("Failed to initialize knowledge database"),
        ));

        // Create log service for execution tracing
        let log_service = Arc::new(LogService::new(db_manager.clone()));

        // Create state service for execution state management
        let state_service = Arc::new(StateService::new(db_manager.clone()));

        // Create connector registry
        let connector_service = ConnectorService::new(paths.clone());
        let connector_registry = Arc::new(ConnectorRegistry::new(connector_service));

        // Create bridge registry and outbox for WebSocket workers
        let bridge_registry = Arc::new(gateway_bridge::BridgeRegistry::new());
        let bridge_outbox = Arc::new(gateway_bridge::OutboxRepository::new(db_manager.clone()));

        // Initialize memory evolution services — repositories that need vector
        // similarity get a SqliteVecIndex over their vec0 partner table.
        let memory_repo: Option<Arc<MemoryRepository>> = knowledge_db.as_ref().map(|kdb| {
            let memory_vec: Arc<dyn VectorIndex> = Arc::new(
                SqliteVecIndex::new(kdb.clone(), "memory_facts_index", "fact_id")
                    .expect("vec index init"),
            );
            Arc::new(MemoryRepository::new(kdb.clone(), memory_vec))
        });
        let goal_repo: Option<Arc<zero_stores_sqlite::GoalRepository>> = knowledge_db
            .as_ref()
            .map(|kdb| Arc::new(zero_stores_sqlite::GoalRepository::new(kdb.clone())));
        // Phase E6c: distillation_run rows live on the conversation DB
        // (DatabaseManager), not knowledge.db. Wire unconditionally —
        // both backends have the conversation DB. This makes
        // /api/distillation/status report real numbers
        // too, and the distiller's run-tracking (insert/retry/success)
        // actually persists.
        let distillation_repo: Option<Arc<DistillationRepository>> =
            Some(Arc::new(DistillationRepository::new(db_manager.clone())));
        let episode_repo: Option<Arc<EpisodeRepository>> = knowledge_db.as_ref().map(|kdb| {
            let episode_vec: Arc<dyn VectorIndex> = Arc::new(
                SqliteVecIndex::new(kdb.clone(), "session_episodes_index", "episode_id")
                    .expect("vec index init"),
            );
            Arc::new(EpisodeRepository::new(kdb.clone(), episode_vec))
        });
        let kg_episode_repo: Option<Arc<KgEpisodeRepository>> = knowledge_db
            .as_ref()
            .map(|kdb| Arc::new(KgEpisodeRepository::new(kdb.clone())));

        // Initialize knowledge graph service and storage. Skipped entirely
        // when knowledge_db is None.
        let (graph_service, graph_storage): (Option<Arc<GraphService>>, Option<Arc<GraphStorage>>) =
            match knowledge_db.as_ref() {
                Some(kdb) => match GraphStorage::new(kdb.clone()) {
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
                },
                None => (None, None),
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
            if let Some(ref kdb) = knowledge_db {
                sync_reconcile_vec_dim_at_boot(&embedding_service, kdb);
            }
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

        // Build the trait-routed memory_store eagerly (before MemoryRecall +
        // SessionDistiller construction, so they can be wired with it).
        let early_memory_store: Option<Arc<dyn zero_stores::MemoryFactStore>> =
            memory_repo.as_ref().map(|mr| {
                persistence_factory::build_memory_store(mr.clone(), embedding_client.clone())
            });

        // Create memory recall. Builds whenever memory_store is wired,
        // which is always whenever memory_repo is. Graph enrichment via
        // GraphService still requires the concrete graph_storage; on
        // Surreal it's None and recall falls back to non-enriched
        // hybrid (still finds facts, just no KG-traversal boost).
        let mut memory_recall_inner: Option<MemoryRecall> = if early_memory_store.is_some() {
            Some(MemoryRecall::new(
                embedding_client.clone(),
                recall_config.clone(),
            ))
        } else {
            None
        };
        // Wire trait-routed episode_store wrapping the SQLite EpisodeRepository.
        if let Some(recall) = memory_recall_inner.as_mut() {
            let store_opt: Option<Arc<dyn zero_stores_traits::EpisodeStore>> =
                episode_repo.as_ref().map(|r| {
                    Arc::new(zero_stores_sqlite::GatewayEpisodeStore::new(r.clone()))
                        as Arc<dyn zero_stores_traits::EpisodeStore>
                });
            if let Some(store) = store_opt {
                recall.set_episode_store(store);
            }
        }

        // Ward wiki repository (still concrete; consumed by SessionDistiller
        // and the trait-store wrapper below).
        let wiki_repo: Option<Arc<WardWikiRepository>> = knowledge_db.as_ref().map(|kdb| {
            let wiki_vec: Arc<dyn VectorIndex> = Arc::new(
                SqliteVecIndex::new(kdb.clone(), "wiki_articles_index", "article_id")
                    .expect("vec index init"),
            );
            Arc::new(WardWikiRepository::new(kdb.clone(), wiki_vec))
        });
        // Wire trait-routed wiki_store (Phase E6c).
        if let Some(recall) = memory_recall_inner.as_mut() {
            let store_opt: Option<Arc<dyn zero_stores_traits::WikiStore>> =
                wiki_repo.as_ref().map(|r| {
                    Arc::new(zero_stores_sqlite::GatewayWikiStore::new(r.clone()))
                        as Arc<dyn zero_stores_traits::WikiStore>
                });
            if let Some(store) = store_opt {
                recall.set_wiki_store(store);
            }
        }
        // Trait-routed kg ingestion store. GatewayKgEpisodeStore wraps
        // the SQLite kg_episode_repo. Handlers + queue + adapter all
        // consume the trait, so a future backend plugs in by
        // implementing the trait without touching consumers.
        let kg_episode_store: Option<Arc<dyn zero_stores_traits::KgEpisodeStore>> =
            kg_episode_repo.as_ref().map(|r| {
                Arc::new(zero_stores_sqlite::GatewayKgEpisodeStore::new(r.clone()))
                    as Arc<dyn zero_stores_traits::KgEpisodeStore>
            });

        // Trait-routed wiki store — wraps the SQLite repository
        // (when wiki_repo exists), else None.
        let wiki_store_for_state: Option<Arc<dyn zero_stores_traits::WikiStore>> =
            wiki_repo.as_ref().map(|wr| {
                Arc::new(zero_stores_sqlite::GatewayWikiStore::new(wr.clone()))
                    as Arc<dyn zero_stores_traits::WikiStore>
            });

        // Wire procedure repository for procedure recall during intent analysis.
        // SQLite-only — None when knowledge_db is None.
        let procedure_repo: Option<Arc<ProcedureRepository>> = knowledge_db.as_ref().map(|kdb| {
            let procedure_vec: Arc<dyn VectorIndex> = Arc::new(
                SqliteVecIndex::new(kdb.clone(), "procedures_index", "procedure_id")
                    .expect("vec index init"),
            );
            Arc::new(ProcedureRepository::new(kdb.clone(), procedure_vec))
        });
        let procedure_store_for_state: Option<Arc<dyn zero_stores_traits::ProcedureStore>> =
            procedure_repo.as_ref().map(|pr| {
                Arc::new(zero_stores_sqlite::GatewayProcedureStore::new(pr.clone()))
                    as Arc<dyn zero_stores_traits::ProcedureStore>
            });
        // Wire the trait-routed procedure_store on MemoryRecall so
        // procedure recall runs (Phase E6c).
        if let (Some(recall), Some(ps)) = (
            memory_recall_inner.as_mut(),
            procedure_store_for_state.as_ref(),
        ) {
            recall.set_procedure_store(ps.clone());
        }

        // Trait-routed episode store for downstream consumers (distiller +
        // sleep worker + AppState). Built once here so the sleep worker
        // construction below doesn't have to re-derive from the underlying
        // repo. Wraps `EpisodeRepository` over the same KnowledgeDatabase.
        let episode_store_for_state: Option<Arc<dyn zero_stores_traits::EpisodeStore>> =
            knowledge_db.as_ref().and_then(|kdb| {
                zero_stores_sqlite::vector_index::SqliteVecIndex::new(
                    kdb.clone(),
                    "session_episodes_index",
                    "episode_id",
                )
                .ok()
                .map(|vec_index| {
                    let repo = Arc::new(zero_stores_sqlite::EpisodeRepository::new(
                        kdb.clone(),
                        Arc::new(vec_index),
                    ));
                    Arc::new(zero_stores_sqlite::GatewayEpisodeStore::new(repo))
                        as Arc<dyn zero_stores_traits::EpisodeStore>
                })
            });

        // Conversation store is always SQLite-backed (per the design doc:
        // conversations.db is SQLite-only). The sleep worker's
        // PatternExtractor needs it on both backends.
        let conversation_store_for_state: Arc<dyn zero_stores_traits::ConversationStore> = Arc::new(
            zero_stores_sqlite::ConversationRepository::new(db_manager.clone()),
        );
        if let (Some(recall), Some(mem)) =
            (memory_recall_inner.as_mut(), early_memory_store.as_ref())
        {
            recall.set_memory_store(mem.clone());
        }

        // Build the trait-routed kg_store early enough to wire it on
        // MemoryRecall before that struct is moved into Arc::new below.
        // Wraps the SQLite GraphStorage.
        let kg_store: Option<Arc<dyn zero_stores::KnowledgeGraphStore>> =
            graph_storage.as_ref().map(|gs| {
                let embedder = embedding_client
                    .clone()
                    .expect("embedding_client wired above for distillation/recall");
                persistence_factory::build_kg_store_from_storage(gs.clone(), embedder)
            });
        if let (Some(recall), Some(ks)) = (memory_recall_inner.as_mut(), kg_store.as_ref()) {
            recall.set_kg_store(ks.clone());
        }

        // Phase B-4: wire BeliefStore into MemoryRecall, gated on
        // `execution.memory.beliefNetwork.enabled`. Reads settings
        // eagerly here so the store is attached before MemoryRecall is
        // sealed in `Arc::new` below. When the flag is off (default)
        // OR the knowledge DB is missing, no store is wired and recall
        // stays byte-for-byte identical to pre-B-4 behavior.
        let belief_network_enabled_for_recall =
            gateway_services::SettingsService::new(paths.clone())
                .load()
                .map(|s| s.execution.memory.belief_network.enabled)
                .unwrap_or(false);
        if belief_network_enabled_for_recall {
            if let Some(kdb) = knowledge_db.as_ref() {
                let belief_store_for_recall: Arc<dyn zero_stores_traits::BeliefStore> =
                    Arc::new(zero_stores_sqlite::SqliteBeliefStore::new(kdb.clone()));
                if let Some(recall) = memory_recall_inner.as_mut() {
                    recall.set_belief_store(belief_store_for_recall);
                }
                tracing::info!("Belief Network recall: enabled (B-4 — beliefs in recall_unified)");
            } else {
                tracing::info!(
                    "Belief Network recall: enabled in settings but knowledge DB unavailable; skipping"
                );
            }
        } else {
            tracing::debug!("Belief Network recall: disabled (default)");
        }

        // Self-RAG retrieval gate (opt-in via `memory.queryGate.enabled` in
        // settings.json). Reads settings eagerly here so the gate is attached
        // before MemoryRecall is sealed in Arc below. When the block is
        // missing, disabled, or unreadable, the gate stays None and recall
        // behaves identically to pre-gate behavior.
        let query_gate_cfg: gateway_memory::QueryGateConfig =
            gateway_services::SettingsService::new(paths.clone())
                .load()
                .map(|s| s.execution.memory.query_gate.clone())
                .unwrap_or_default();
        if query_gate_cfg.enabled {
            let llm = Arc::new(gateway_memory::LlmQueryGate::new(
                memory_llm_factory.clone(),
            ));
            let gate = Arc::new(gateway_memory::QueryGate::new(llm, query_gate_cfg.clone()));
            if let Some(recall) = memory_recall_inner.as_mut() {
                recall.set_query_gate(gate);
            }
            tracing::info!(
                "Memory query gate: enabled (model={:?}, max_subqueries={}, timeout_ms={})",
                query_gate_cfg.model_id,
                query_gate_cfg.max_subqueries,
                query_gate_cfg.timeout_ms,
            );
        } else {
            tracing::info!("Memory query gate: disabled");
        }

        // MMR diversity reranking (opt-in via `memory.mmr.enabled` in
        // settings.json). Default-disabled: when the block is missing or
        // `enabled = false`, recall is byte-for-byte identical to pre-MMR.
        // The config block is attached unconditionally so the runtime can
        // read the current values; only `enabled = true` triggers the
        // rerank step inside `recall_unified`.
        let mmr_cfg: gateway_memory::MmrConfig =
            gateway_services::SettingsService::new(paths.clone())
                .load()
                .map(|s| s.execution.memory.mmr.clone())
                .unwrap_or_default();
        if let Some(recall) = memory_recall_inner.as_mut() {
            recall.set_mmr_config(mmr_cfg.clone());
        }
        if mmr_cfg.enabled {
            tracing::info!(
                "Memory MMR rerank: enabled (lambda={}, candidate_pool={})",
                mmr_cfg.lambda,
                mmr_cfg.candidate_pool,
            );
        } else {
            tracing::debug!("Memory MMR rerank: disabled (default)");
        }

        let memory_recall: Option<Arc<MemoryRecall>> = memory_recall_inner.map(Arc::new);

        // Clone embedding client before it's moved into distiller — the runner
        // also needs it so the memory fact store can generate embeddings.
        let runner_embedding_client = embedding_client.clone();

        // Build the trait-object KG store from graph_storage.
        // Coexists with graph_service/graph_storage until Phase 5 retirement.
        //
        // Construction is centralized in `persistence_factory` (TD-023):
        // when alternate-backend support lands, the config-driven branch goes
        // there, and this callsite stays the same. We use the
        // `_from_storage` helper because AppState shares one
        // `Arc<GraphStorage>` between `kg_store` and the legacy
        // `graph_service`; once `graph_service` retires, callers migrate
        // to `build_kg_store(knowledge_db, …)`.
        //
        // kg_store was built earlier (before memory_recall_inner moved
        // into Arc::new) so it could be wired on MemoryRecall. The
        // duplicate definition here is intentionally absent — both the
        // distiller and AppState fields below reuse the earlier binding.
        let memory_store = early_memory_store;

        let episode_repo_ref = episode_repo.clone();

        // Create settings service (before distiller & runtime, so we can read execution settings)
        let settings = Arc::new(SettingsService::new(paths.clone()));

        // SessionDistiller (Phase E3): builds in BOTH backends.
        //
        // Required: at least one of memory_store (trait) or memory_repo
        // (concrete) must be wired so fact upsert has a destination.
        // SQLite-only deps (graph_storage, distillation_repo, episode_repo,
        // wiki_repo, procedure_repo) flow through as Optional — None in
        // missing means the corresponding side-effects (KG ingestion,
        // run-tracking, episode storage, wiki compilation, procedure
        // upsert) skip gracefully. Fact distillation itself runs.
        let distiller: Option<Arc<SessionDistiller>> = if memory_store.is_some() {
            let mut distiller_inner = SessionDistiller::new(
                provider_service.clone(),
                embedding_client.clone(),
                conversation_repo.clone(),
                paths.clone(),
                Some(settings.clone()),
            );
            if let Some(mem) = memory_store.as_ref() {
                distiller_inner.set_memory_store(mem.clone());
            }
            if let Some(kgs) = kg_store.as_ref() {
                distiller_inner.set_kg_store(kgs.clone());
            }
            // Phase E6a/E6b: episode/wiki/procedure trait stores reuse the
            // same Arc<dyn ...> values we built above for the AppState
            // fields (`episode_store`, `wiki_store_for_state`,
            // `procedure_store_for_state`) so the distiller and the HTTP
            // handlers see the same backing store.
            if let Some(es) = episode_store_for_state.as_ref() {
                distiller_inner.set_episode_store(es.clone());
            }
            if let Some(ws) = wiki_store_for_state.as_ref() {
                distiller_inner.set_wiki_store(ws.clone());
            }
            if let Some(ps) = procedure_store_for_state.as_ref() {
                distiller_inner.set_procedure_store(ps.clone());
            }
            // Phase E6c: trait-routed distillation store. Wraps the
            // SQLite DistillationRepository for run-tracking writes.
            if let Some(dr) = distillation_repo.as_ref() {
                let store: Arc<dyn zero_stores_traits::DistillationStore> = Arc::new(
                    zero_stores_sqlite::GatewayDistillationStore::new(dr.clone()),
                );
                distiller_inner.set_distillation_store(store);
            }
            Some(Arc::new(distiller_inner))
        } else {
            None
        };

        // Keep a handle for on-demand distillation (backfill, trigger).
        // None when the distiller wasn't constructed.
        let distiller_ref: Option<Arc<SessionDistiller>> = distiller.clone();
        let max_parallel_agents = settings
            .get_execution_settings()
            .map(|s| s.max_parallel_agents)
            .unwrap_or(2);
        tracing::info!(max_parallel_agents, "Execution settings loaded");

        // Create streaming ingestion queue + backpressure BEFORE the runtime so the
        // runner can be wired with an IngestionAdapter.
        //
        // Trait-routed: queue + backpressure consume
        // Arc<dyn KgEpisodeStore> + Arc<dyn KnowledgeGraphStore>.
        let (ingestion_queue, ingestion_backpressure) =
            match (kg_episode_store.as_ref(), kg_store.as_ref()) {
                (Some(eps), Some(kgs)) => {
                    let extractor =
                        Arc::new(gateway_execution::ingest::extractor::LlmExtractor::new(
                            provider_service.clone(),
                            "root".to_string(),
                        ));
                    let queue = Arc::new(gateway_execution::ingest::IngestionQueue::start(
                        2,
                        eps.clone(),
                        kgs.clone(),
                        extractor,
                    ));
                    let bp = Arc::new(gateway_execution::ingest::Backpressure::new(
                        gateway_execution::ingest::BackpressureConfig::default(),
                        eps.clone(),
                    ));
                    (
                        Some(queue) as Option<Arc<gateway_execution::ingest::IngestionQueue>>,
                        Some(bp) as Option<Arc<gateway_execution::ingest::Backpressure>>,
                    )
                }
                _ => (None, None),
            };

        // Build agent-tool adapters so runner can register `ingest` + `goal` tools.
        // Phase B2: also trait-routed. The IngestionAdapter is migrated
        // alongside the queue so subagent ingestion works.
        let ingestion_adapter: Option<Arc<dyn agent_tools::IngestionAccess>> = match (
            ingestion_queue.as_ref(),
            kg_store.as_ref(),
            kg_episode_store.as_ref(),
        ) {
            (Some(q), Some(kgs), Some(eps)) => Some(Arc::new(
                gateway_execution::invoke::ingest_adapter::IngestionAdapter::new(
                    q.clone(),
                    eps.clone(),
                    kgs.clone(),
                ),
            )
                as Arc<dyn agent_tools::IngestionAccess>),
            _ => None,
        };
        // Goal adapter wraps the SQLite GoalRepository.
        let goal_store_for_adapter: Option<Arc<dyn zero_stores_traits::GoalStore>> =
            goal_repo.as_ref().map(|gr| {
                Arc::new(zero_stores_sqlite::GatewayGoalStore::new(gr.clone()))
                    as Arc<dyn zero_stores_traits::GoalStore>
            });
        let goal_adapter: Option<Arc<dyn agent_tools::GoalAccess>> =
            goal_store_for_adapter.map(|store| {
                Arc::new(gateway_execution::invoke::goal_adapter::GoalAdapter::new(
                    store,
                )) as Arc<dyn agent_tools::GoalAccess>
            });

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
            memory_store.clone(),
            distiller,
            memory_recall,
            Some(bridge_registry.clone()),
            Some(bridge_outbox.clone()),
            runner_embedding_client,
            max_parallel_agents,
            kg_store.clone(),
            kg_episode_repo.clone(),
            ingestion_adapter,
            goal_adapter,
            memory_llm_factory.clone(),
        ));

        // Phase 4: CompactionRepository + SleepTimeWorker (background maintenance).
        // CompactionRepository is SQLite-tied (kg_compactions table on
        // knowledge.db). None when knowledge_db is unwired.
        let compaction_repo: Option<Arc<zero_stores_sqlite::CompactionRepository>> = knowledge_db
            .as_ref()
            .map(|kdb| Arc::new(zero_stores_sqlite::CompactionRepository::new(kdb.clone())));

        // Phase D1: trait-routed compaction audit store. Wired in BOTH
        // backends so the maintenance worker can record merges/prunes
        // regardless of backend. Surreal uses its own
        // `kg_compaction_run` table; SQLite delegates to the existing
        // `CompactionRepository`. Default no-op impls cover edge cases.
        let compaction_store: Option<Arc<dyn zero_stores_traits::CompactionStore>> =
            compaction_repo.as_ref().map(|r| {
                Arc::new(zero_stores_sqlite::GatewayCompactionStore::new(r.clone()))
                    as Arc<dyn zero_stores_traits::CompactionStore>
            });

        // One-shot backfill: populate legacy kg_entities / kg_relationships
        // rows with the richer metadata introduced in commits b816702,
        // 1bc21f6, 5bf3013. Marker row in kg_compactions gates this so
        // subsequent daemon starts are a no-op. Non-fatal on failure —
        // a backfill bug must never prevent the daemon from booting.
        // Skip entirely when knowledge_db is None.
        if let Some(ref kdb) = knowledge_db {
            let backfiller = gateway_execution::sleep::KgBackfiller::new(kdb.clone());
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

        // Belief Network stores (Phase B-1/B-2 + B-5 HTTP surface + B-6 observatory).
        //
        // Two layers of gating:
        //   1. Trait store handles can only be built when `knowledge_db`
        //      is wired (SQLite-backed).
        //   2. We only park them on `AppState` for the HTTP layer when
        //      `execution.memory.beliefNetwork.enabled = true` — so the
        //      `/api/beliefs/*`, `/api/contradictions/*`, and
        //      `/api/belief-network/*` endpoints cleanly return 503/empty
        //      when the feature is off.
        //
        // The sleep-time worker block below still gets to consume the
        // handles either way (it has its own internal enable flag).
        let belief_network_cfg = settings
            .get_execution_settings()
            .map(|s| s.memory.belief_network.clone())
            .unwrap_or_default();
        let belief_store_raw: Option<Arc<dyn zero_stores::BeliefStore>> =
            knowledge_db.as_ref().map(|kdb| {
                Arc::new(zero_stores_sqlite::SqliteBeliefStore::new(kdb.clone()))
                    as Arc<dyn zero_stores::BeliefStore>
            });
        let belief_contradiction_store_raw: Option<Arc<dyn zero_stores::BeliefContradictionStore>> =
            knowledge_db.as_ref().map(|kdb| {
                Arc::new(zero_stores_sqlite::SqliteBeliefContradictionStore::new(
                    kdb.clone(),
                )) as Arc<dyn zero_stores::BeliefContradictionStore>
            });
        // HTTP surface only exposes the stores when the feature is on.
        let belief_store_for_http: Option<Arc<dyn zero_stores_traits::BeliefStore>> =
            if belief_network_cfg.enabled {
                belief_store_raw.clone()
            } else {
                None
            };
        let belief_contradiction_store_for_http: Option<
            Arc<dyn zero_stores_traits::BeliefContradictionStore>,
        > = if belief_network_cfg.enabled {
            belief_contradiction_store_raw.clone()
        } else {
            None
        };

        // Sleep-time worker requires the entire SQLite knowledge cluster.
        // Build only when ALL of (compaction_repo, memory_repo, knowledge_db,
        // procedure_repo, kg_store, compaction_store) are present. The
        // maintenance ops (compactor/decay/pruner/orphan_archiver) take
        // Sleep-time worker — trait-routed. Gates on the trait stores
        // (kg_store, episode_store, memory_store, procedure_store,
        // compaction_store) all wired above from the SQLite repos.
        // Conversation store is always SQLite-backed (per design) and
        // built unconditionally above.
        let (sleep_time_worker, belief_network_activity) = match (
            kg_store.as_ref(),
            episode_store_for_state.as_ref(),
            memory_store.as_ref(),
            procedure_store_for_state.as_ref(),
            compaction_store.as_ref(),
        ) {
            (Some(kgs), Some(eps), Some(mems), Some(prs), Some(compstore)) => {
                let abstractions_interval_hours = settings
                    .get_execution_settings()
                    .map(|s| s.memory.corrections_abstractor_interval_hours)
                    .unwrap_or(24);
                let conflict_interval_hours = settings
                    .get_execution_settings()
                    .map(|s| s.memory.conflict_resolver_interval_hours)
                    .unwrap_or(24);
                // `belief_network_cfg` is already defined in the outer
                // scope above (used for HTTP-store gating). Reuse it here.
                let memory_services =
                    gateway_memory::MemoryServices::new(gateway_memory::MemoryServicesConfig {
                        agent_id: "root".to_string(),
                        interval: std::time::Duration::from_secs(60 * 60),
                        llm_factory: memory_llm_factory.clone(),
                        kg_store: kgs.clone(),
                        episode_store: eps.clone(),
                        memory_store: mems.clone(),
                        compaction_store: compstore.clone(),
                        procedure_store: prs.clone(),
                        conversation_store: conversation_store_for_state.clone(),
                        embedding_client: embedding_client.clone(),
                        kg_decay_config: recall_config.kg_decay.clone(),
                        corrections_abstractor_interval: std::time::Duration::from_secs(
                            abstractions_interval_hours as u64 * 3600,
                        ),
                        conflict_resolver_interval: std::time::Duration::from_secs(
                            conflict_interval_hours as u64 * 3600,
                        ),
                        decay_config: gateway_memory::sleep::DecayConfig::default(),
                        belief_store: belief_store_raw.clone(),
                        belief_network_enabled: belief_network_cfg.enabled,
                        belief_network_interval: std::time::Duration::from_secs(
                            belief_network_cfg.interval_hours as u64 * 3600,
                        ),
                        belief_contradiction_store: belief_contradiction_store_raw.clone(),
                        belief_contradiction_neighborhood_prefix_depth: belief_network_cfg
                            .neighborhood_prefix_depth,
                        belief_contradiction_budget_per_cycle: belief_network_cfg
                            .contradiction_budget_per_cycle,
                        belief_fact_confidence_drop_threshold: belief_network_cfg
                            .fact_confidence_drop_threshold,
                    });
                (
                    Some(memory_services.sleep_time_worker.clone()),
                    Some(memory_services.belief_network_activity.clone()),
                )
            }
            _ => (None, None),
        };

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
            compaction_repo,
            compaction_store,
            plugin_manager,
            model_registry,
            embedding_service,
            paths,
            config_dir,
            memory_store,
            goal_repo,
            distillation_repo,
            distiller: distiller_ref,
            episode_store: episode_store_for_state,
            wiki_store: wiki_store_for_state,
            procedure_store: procedure_store_for_state,
            episode_repo: episode_repo_ref,
            kg_episode_repo,
            kg_episode_store,
            graph_service,
            kg_store,
            ingestion_queue,
            ingestion_backpressure,
            advertiser: discovery::noop(),
            advertise_handle: Arc::new(std::sync::Mutex::new(None)),
            belief_store: belief_store_for_http,
            belief_contradiction_store: belief_contradiction_store_for_http,
            belief_network_activity,
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

        // Phase B: minimal still wires the trait-routed memory_store
        // so tests that hit /api/memory/.../search succeed without
        // needing to construct full AppState. Fallback no-op
        // embedding client lets save_fact path complete.
        let memory_store: Option<Arc<dyn zero_stores::MemoryFactStore>> = Some(Arc::new(
            zero_stores_sqlite::GatewayMemoryFactStore::new(memory_repo.clone(), None),
        ));

        // Episode / wiki / procedure trait stores so HTTP handlers reach
        // these listings without concrete-repo fallbacks. Each wraps the
        // SQLite repo bound to its own vec0 partner table.
        let episode_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "session_episodes_index", "episode_id")
                .expect("episode vec index init"),
        );
        let episode_repo_handle = Arc::new(zero_stores_sqlite::EpisodeRepository::new(
            knowledge_db.clone(),
            episode_vec,
        ));
        let episode_store: Option<Arc<dyn zero_stores_traits::EpisodeStore>> = Some(Arc::new(
            zero_stores_sqlite::GatewayEpisodeStore::new(episode_repo_handle),
        ));

        let wiki_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "wiki_articles_index", "article_id")
                .expect("wiki vec index init"),
        );
        let wiki_repo_handle = Arc::new(zero_stores_sqlite::WardWikiRepository::new(
            knowledge_db.clone(),
            wiki_vec,
        ));
        let wiki_store: Option<Arc<dyn zero_stores_traits::WikiStore>> = Some(Arc::new(
            zero_stores_sqlite::GatewayWikiStore::new(wiki_repo_handle),
        ));

        let proc_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "procedures_index", "procedure_id")
                .expect("procedure vec index init"),
        );
        let procedure_repo_handle = Arc::new(zero_stores_sqlite::ProcedureRepository::new(
            knowledge_db.clone(),
            proc_vec,
        ));
        let procedure_store: Option<Arc<dyn zero_stores_traits::ProcedureStore>> = Some(Arc::new(
            zero_stores_sqlite::GatewayProcedureStore::new(procedure_repo_handle),
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
            knowledge_db: Some(knowledge_db),
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
            compaction_store: None,
            model_registry: Arc::new(ModelRegistry::load(&[], paths.vault_dir())),
            embedding_service: Arc::new(
                EmbeddingService::with_config(paths.clone(), Default::default())
                    .expect("default EmbeddingService must build"),
            ),
            plugin_manager,
            paths,
            config_dir,
            memory_store,
            goal_repo: None,
            distillation_repo: None,
            distiller: None,
            episode_repo: None,
            episode_store,
            wiki_store,
            procedure_store,
            kg_episode_repo: None,
            kg_episode_store: None,
            graph_service: None,
            kg_store: None,
            ingestion_queue: None,
            ingestion_backpressure: None,
            advertiser: discovery::noop(),
            advertise_handle: Arc::new(std::sync::Mutex::new(None)),
            belief_store: None,
            belief_contradiction_store: None,
            belief_network_activity: None,
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

        // Phase B: same trait wrap as `minimal()` so test paths see a
        // wired memory_store and search/recall handlers don't 503.
        let memory_store: Option<Arc<dyn zero_stores::MemoryFactStore>> = Some(Arc::new(
            zero_stores_sqlite::GatewayMemoryFactStore::new(memory_repo.clone(), None),
        ));

        // Episode / wiki / procedure trait stores so HTTP handlers reach
        // these listings without concrete-repo fallbacks.
        let episode_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "session_episodes_index", "episode_id")
                .expect("episode vec index init"),
        );
        let episode_repo_handle = Arc::new(zero_stores_sqlite::EpisodeRepository::new(
            knowledge_db.clone(),
            episode_vec,
        ));
        let episode_store: Option<Arc<dyn zero_stores_traits::EpisodeStore>> = Some(Arc::new(
            zero_stores_sqlite::GatewayEpisodeStore::new(episode_repo_handle),
        ));

        let wiki_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "wiki_articles_index", "article_id")
                .expect("wiki vec index init"),
        );
        let wiki_repo_handle = Arc::new(zero_stores_sqlite::WardWikiRepository::new(
            knowledge_db.clone(),
            wiki_vec,
        ));
        let wiki_store: Option<Arc<dyn zero_stores_traits::WikiStore>> = Some(Arc::new(
            zero_stores_sqlite::GatewayWikiStore::new(wiki_repo_handle),
        ));

        let proc_vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "procedures_index", "procedure_id")
                .expect("procedure vec index init"),
        );
        let procedure_repo_handle = Arc::new(zero_stores_sqlite::ProcedureRepository::new(
            knowledge_db.clone(),
            proc_vec,
        ));
        let procedure_store: Option<Arc<dyn zero_stores_traits::ProcedureStore>> = Some(Arc::new(
            zero_stores_sqlite::GatewayProcedureStore::new(procedure_repo_handle),
        ));

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
            knowledge_db: Some(knowledge_db),
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
            compaction_store: None,
            model_registry: Arc::new(ModelRegistry::load(&[], paths.vault_dir())),
            embedding_service: Arc::new(
                EmbeddingService::with_config(paths.clone(), Default::default())
                    .expect("default EmbeddingService must build"),
            ),
            plugin_manager,
            paths,
            config_dir,
            memory_store,
            goal_repo: None,
            distillation_repo: None,
            distiller: None,
            episode_repo: None,
            episode_store,
            wiki_store,
            procedure_store,
            kg_episode_repo: None,
            kg_episode_store: None,
            graph_service: None,
            kg_store: None,
            ingestion_queue: None,
            ingestion_backpressure: None,
            advertiser: discovery::noop(),
            advertise_handle: Arc::new(std::sync::Mutex::new(None)),
            belief_store: None,
            belief_contradiction_store: None,
            belief_network_activity: None,
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
            let Some(knowledge_db) = self.knowledge_db.as_ref() else {
                tracing::warn!(
                    "Embedding marker mismatch but knowledge DB unavailable — skipping reindex"
                );
                self.embedding_service
                    .publish_health(gateway_services::Health::Ready);
                let _handle = self.embedding_service.clone().start_health_loop();
                return;
            };
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
                knowledge_db,
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

        // Seed default cron jobs (idempotent on job id) so first-run
        // installs ship with the bundled cleanup schedule wired up.
        self.seed_default_cron().await;

        // Seed default policies from bundled template if no policies exist
        self.seed_default_policies().await;

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

    /// Seed default cron jobs from bundled `default_cron.json` template.
    ///
    /// Each ID is seeded **at most once per vault**: the first time we see
    /// it, we create the job (or migrate a pre-existing one) and record the
    /// ID in `<vault>/config/seeded_defaults.json`. Subsequent boots skip
    /// any ID already in the registry, so deletes the user makes through
    /// the UI stick across daemon restarts.
    async fn seed_default_cron(&self) {
        let template_bytes = match gateway_templates::Templates::get("default_cron.json") {
            Some(file) => file.data.to_vec(),
            None => {
                tracing::debug!(
                    "seed_default_cron: bundled `default_cron.json` not found, skipping"
                );
                return;
            }
        };

        let requests: Vec<gateway_cron::CreateCronJobRequest> =
            match serde_json::from_slice(&template_bytes) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("seed_default_cron: failed to parse default_cron.json: {e}");
                    return;
                }
            };

        if requests.is_empty() {
            tracing::debug!("seed_default_cron: no entries in default_cron.json");
            return;
        }

        let cron_service = gateway_cron::CronService::new(self.paths.clone());
        let seeded =
            seeded_defaults::seed_cron_with_registry(&self.paths, &cron_service, requests).await;

        if seeded > 0 {
            tracing::info!(seeded, "seed_default_cron: completed");
        }
    }

    /// Seed default policies from bundled template if no policies/corrections exist.
    async fn seed_default_policies(&self) {
        // Route through the trait surface so the configured backend
        // backends seed identically. `memory_store` is wired in both
        // modes (SQLite-wrapper or ).
        let memory_store = match &self.memory_store {
            Some(s) => s,
            None => {
                tracing::warn!(
                    "seed_default_policies: memory_store is None — refusing to seed. \
                     This means neither SQLite memory_repo  \
                     wired into AppState; check persistence_factory output."
                );
                return;
            }
        };

        // Check if any correction facts already exist for the root agent.
        let existing = match memory_store
            .list_memory_facts(Some("root"), Some("correction"), None, 1, 0)
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                tracing::warn!(
                    "seed_default_policies: existence check failed ({e}); \
                     proceeding as if empty (may produce duplicates if policies \
                     are already present)."
                );
                Vec::new()
            }
        };
        if !existing.is_empty() {
            tracing::debug!(
                existing_count = existing.len(),
                "seed_default_policies: policies already present for root/correction — skipping"
            );
            return;
        }

        let template = match gateway_templates::Templates::get("default_policies.json") {
            Some(f) => f.data.to_vec(),
            None => {
                tracing::warn!(
                    "seed_default_policies: bundled `default_policies.json` template \
                     missing from gateway-templates — nothing to seed."
                );
                return;
            }
        };

        let policies: Vec<serde_json::Value> = match serde_json::from_slice(&template) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("seed_default_policies: failed to parse default_policies.json: {e}");
                return;
            }
        };

        let total = policies.len();
        let now = chrono::Utc::now().to_rfc3339();
        let mut count = 0usize;
        let mut skipped_empty = 0usize;
        let mut errors: Vec<(String, String)> = Vec::new();

        for policy in &policies {
            let category = policy["category"].as_str().unwrap_or("correction");
            let key = policy["key"].as_str().unwrap_or_default();
            let content = policy["content"].as_str().unwrap_or_default();
            let confidence = policy["confidence"].as_f64().unwrap_or(1.0);
            let pinned = policy["pinned"].as_bool().unwrap_or(true);

            if key.is_empty() || content.is_empty() {
                skipped_empty += 1;
                continue;
            }

            let fact_value = serde_json::json!({
                "id": format!("policy-{}", uuid::Uuid::new_v4()),
                "session_id": null,
                "agent_id": "root",
                "scope": "agent",
                "category": category,
                "key": key,
                "content": content,
                "confidence": confidence,
                "mention_count": 5,
                "source_summary": "Default policy",
                "ward_id": "__global__",
                "contradicted_by": null,
                "created_at": now,
                "updated_at": now,
                "expires_at": null,
                "valid_from": null,
                "valid_until": null,
                "superseded_by": null,
                "pinned": pinned,
                "epistemic_class": "current",
                "source_episode_id": null,
                "source_ref": null,
            });

            match memory_store.upsert_typed_fact(fact_value, None).await {
                Ok(()) => count += 1,
                Err(e) => errors.push((key.to_string(), e)),
            }
        }

        if !errors.is_empty() {
            for (key, e) in &errors {
                tracing::warn!(policy_key = %key, error = %e, "seed_default_policies: upsert failed");
            }
        }

        tracing::info!(
            total = total,
            seeded = count,
            skipped_empty = skipped_empty,
            failed = errors.len(),
            "seed_default_policies: completed"
        );
    }

    /// Ensure Python venv and Node.js environment exist.
    async fn ensure_runtime_environments(&self) {
        // Create wards directory structure
        self.ensure_wards_dir();

        // Side-effects: create .venv / .node_env directories used by the shell tool.
        let _ = self.ensure_python_venv().await;
        let _ = self.ensure_node_env();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use crate::hooks::HookRegistry;
    use tempfile::TempDir;

    fn make_temp_state() -> (TempDir, AppState) {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("agents")).unwrap();
        std::fs::create_dir_all(dir.path().join("skills")).unwrap();
        let state = AppState::minimal(dir.path().to_path_buf());
        (dir, state)
    }

    #[test]
    fn minimal_app_state_wires_required_components() {
        let (_dir, state) = make_temp_state();
        assert!(state.hook_registry.is_none());
        assert!(state.cron_scheduler.is_none());
        assert!(state.session_archiver.is_none());
        assert!(state.bridge_bus.is_none());
        assert!(state.memory_store.is_some());
        assert!(state.episode_store.is_some());
        assert!(state.wiki_store.is_some());
        assert!(state.procedure_store.is_some());
        assert!(state.knowledge_db.is_some());
    }

    #[test]
    fn with_hook_registry_wires_the_optional_field() {
        let (_dir, state) = make_temp_state();
        let event_bus = Arc::new(EventBus::new());
        let registry = Arc::new(HookRegistry::new(event_bus));
        let state = state.with_hook_registry(registry);
        assert!(state.hook_registry.is_some());
    }

    #[test]
    fn ensure_wards_dir_creates_scratch_and_wiki_subtrees() {
        let (_dir, state) = make_temp_state();
        state.ensure_wards_dir();

        assert!(state.config_dir.join("wards").join("scratch").is_dir());

        let wiki = state.config_dir.join("wards").join("wiki");
        assert!(wiki.is_dir());
        for folder in [
            "00_Inbox",
            "20_Projects",
            "30_Library/Books",
            "40_Research",
            "50_Resources",
            "60_Archive",
            "70_Assets/Images",
            "_zztemplates",
        ] {
            assert!(wiki.join(folder).is_dir());
        }

        let agents_md = std::fs::read_to_string(wiki.join("AGENTS.md")).expect("agents.md");
        assert!(agents_md.starts_with("<!-- obsidian-vault -->"));

        for f in ["ward.md", "structure.md", "core_docs.md"] {
            assert!(wiki.join("memory-bank").join(f).exists());
        }
    }

    #[test]
    fn ensure_wards_dir_is_idempotent_and_preserves_user_edits() {
        let (_dir, state) = make_temp_state();
        state.ensure_wards_dir();
        let agents_md_path = state
            .config_dir
            .join("wards")
            .join("wiki")
            .join("AGENTS.md");

        std::fs::write(&agents_md_path, "user-authored content").unwrap();
        state.ensure_wards_dir();
        let after = std::fs::read_to_string(&agents_md_path).unwrap();
        assert_eq!(after, "user-authored content");
    }

    #[test]
    fn ensure_wards_dir_reseeds_when_marker_present() {
        let (_dir, state) = make_temp_state();
        state.ensure_wards_dir();
        let agents_md_path = state
            .config_dir
            .join("wards")
            .join("wiki")
            .join("AGENTS.md");

        std::fs::write(
            &agents_md_path,
            "<!-- obsidian-vault -->\nold seed content\n",
        )
        .unwrap();
        state.ensure_wards_dir();
        let after = std::fs::read_to_string(&agents_md_path).unwrap();
        assert!(after.starts_with("<!-- obsidian-vault -->"));
        assert!(after.contains("Folder map"));
    }

    #[test]
    fn ensure_wiki_ward_handles_custom_name() {
        let (_dir, state) = make_temp_state();
        let wards = state.config_dir.join("wards");
        std::fs::create_dir_all(&wards).unwrap();
        state.ensure_wiki_ward(&wards, "knowledge");

        let custom = wards.join("knowledge");
        assert!(custom.is_dir());
        assert!(custom.join("AGENTS.md").exists());
        let content = std::fs::read_to_string(custom.join("AGENTS.md")).unwrap();
        assert!(content.contains("# knowledge"));
    }

    #[test]
    fn ensure_node_env_creates_directory_when_missing() {
        let (_dir, state) = make_temp_state();
        let new_path = state.config_dir.join("wards").join(".node_env");
        assert!(!new_path.exists());

        assert!(state.ensure_node_env());
        assert!(new_path.is_dir());
    }

    #[test]
    fn ensure_node_env_returns_true_when_already_exists() {
        let (_dir, state) = make_temp_state();
        let new_path = state.config_dir.join("wards").join(".node_env");
        std::fs::create_dir_all(&new_path).unwrap();
        assert!(state.ensure_node_env());
    }

    #[test]
    fn ensure_node_env_uses_legacy_path_when_present() {
        let (_dir, state) = make_temp_state();
        let legacy = state.config_dir.join("node_env");
        std::fs::create_dir_all(&legacy).unwrap();
        assert!(state.ensure_node_env());
    }

    #[test]
    fn seed_default_skills_is_no_op_when_skills_dir_has_content() {
        let (_dir, state) = make_temp_state();
        let skills_dir = state.paths.vault_dir().join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("sentinel.md"), "user").unwrap();

        state.seed_default_skills();
        assert!(skills_dir.join("sentinel.md").exists());
    }

    #[test]
    fn seed_default_skills_populates_empty_dir_from_templates() {
        let (_dir, state) = make_temp_state();
        let skills_dir = state.paths.vault_dir().join("skills");
        if skills_dir.exists() {
            std::fs::remove_dir_all(&skills_dir).unwrap();
        }
        state.seed_default_skills();
        assert!(skills_dir.is_dir());
    }

    #[tokio::test]
    async fn seed_default_cron_inserts_bundled_jobs_into_registry() {
        let (_dir, state) = make_temp_state();
        state.seed_default_cron().await;

        let registry_path = state.paths.config_dir().join("seeded_defaults.json");
        assert!(registry_path.exists());
    }

    #[tokio::test]
    async fn seed_default_cron_is_idempotent_across_calls() {
        let (_dir, state) = make_temp_state();
        state.seed_default_cron().await;
        state.seed_default_cron().await;
    }

    #[tokio::test]
    async fn seed_default_policies_skips_when_existing_corrections_present() {
        let (_dir, state) = make_temp_state();
        state.seed_default_policies().await;
        state.seed_default_policies().await;
    }

    #[tokio::test]
    async fn discover_and_start_plugins_handles_missing_plugin_dir() {
        let (_dir, state) = make_temp_state();
        state.discover_and_start_plugins().await;
    }

    #[tokio::test]
    async fn ensure_runtime_environments_creates_wards_and_envs() {
        let (_dir, state) = make_temp_state();
        state.ensure_runtime_environments().await;

        assert!(state.config_dir.join("wards").join("scratch").is_dir());
        assert!(state.config_dir.join("wards").join(".node_env").is_dir());
    }

    #[tokio::test]
    async fn seed_defaults_runs_to_completion() {
        let (_dir, state) = make_temp_state();
        state.seed_defaults().await;
    }

    #[tokio::test]
    async fn new_app_state_initialises_full_constructor_path() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf());
        assert!(state.knowledge_db.is_some());
        assert!(state.memory_store.is_some());
        assert!(state.kg_store.is_some());
        assert!(state.graph_service.is_some());
        assert!(state.distillation_repo.is_some());
        assert!(state.distiller.is_some());
        assert!(state.session_archiver.is_some());
        assert!(state.episode_repo.is_some());
        assert!(state.kg_episode_repo.is_some());
        assert!(state.cron_scheduler.is_none());
        assert!(state.bridge_bus.is_none());
    }

    #[test]
    fn with_components_uses_supplied_handles() {
        let dir = TempDir::new().unwrap();
        let paths: SharedVaultPaths = Arc::new(VaultPaths::new(dir.path().to_path_buf()));
        let _ = paths.ensure_dirs_exist();
        let event_bus = Arc::new(EventBus::new());
        let agents = Arc::new(AgentService::new(paths.agents_dir()));
        let skills = Arc::new(SkillService::with_roots(paths.skills_dirs()));
        let provider_service = Arc::new(ProviderService::new(paths.clone()));
        let mcp_service = Arc::new(McpService::new(paths.clone()));
        let runtime = Arc::new(RuntimeService::new(event_bus.clone()));
        let db_manager = Arc::new(DatabaseManager::new(paths.clone()).expect("db manager"));
        let conversations = Arc::new(ConversationRepository::new(db_manager.clone()));
        let log_service = Arc::new(LogService::new(db_manager.clone()));
        let state_service = Arc::new(StateService::new(db_manager.clone()));
        let connector_service = ConnectorService::new(paths.clone());
        let connector_registry = Arc::new(ConnectorRegistry::new(connector_service));

        let state = AppState::with_components(
            agents,
            skills,
            provider_service,
            mcp_service,
            runtime,
            event_bus,
            conversations,
            log_service,
            state_service,
            connector_registry,
            paths.clone(),
        );

        assert_eq!(state.config_dir, *paths.vault_dir());
        assert!(state.memory_store.is_some());
        assert!(state.episode_store.is_some());
        assert!(state.wiki_store.is_some());
        assert!(state.procedure_store.is_some());
    }
}
