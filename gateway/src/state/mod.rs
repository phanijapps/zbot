//! # Application State
//!
//! Shared state for the gateway application.

pub(crate) mod persistence_factory;

use crate::connectors::{ConnectorRegistry, ConnectorService};
use crate::cron::CronScheduler;
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
use gateway_services::EmbeddingService;
use std::collections::HashMap;
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
    /// `None` when the user has opted into the SurrealDB backend
    /// (`execution.featureFlags.surreal_backend = true` in settings.json).
    /// In that mode the daemon never opens `knowledge.db` — all reads /
    /// writes go through the trait-routed stores
    /// (`memory_store`, `kg_store`, `episode_store`, `wiki_store`,
    /// `procedure_store`). HTTP handlers that haven't migrated to the
    /// trait surface return `503 Service Unavailable` in this mode
    /// rather than reach for a SQLite handle that doesn't exist.
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
    /// memory facts on both SQLite and SurrealDB backends.
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

    /// Trait-routed kg-ingestion-episode store (Phase B). Wired in both
    /// SQLite (wraps kg_episode_repo) and SurrealDB modes (via
    /// surreal_bundle.kg_episode). Backend-agnostic — adding a new
    /// datastore means implementing the trait and adding a build branch
    /// in persistence_factory.rs; consumers don't change.
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

        // Phase E2: detect SurrealDB opt-in BEFORE touching SQLite knowledge.
        // When the user has selected Surreal (settings.json:
        // `execution.featureFlags.surreal_backend = true`) we never open
        // `knowledge.db` and skip the entire SQLite-knowledge cluster
        // (KnowledgeDatabase + memory_repo + episode_repo + goal_repo +
        // kg_episode_repo + graph_storage/service + vec indexes +
        // wiki_repo + procedure_repo + sleep_time_worker). All those
        // become `None`; the runtime is wired with trait-routed stores
        // via `surreal_bundle` instead.
        let use_surreal_for_knowledge = persistence_factory::is_surreal_backend_opt_in(&paths);
        if use_surreal_for_knowledge {
            tracing::info!(
                "SurrealDB backend opted in — skipping SQLite knowledge.db / memory_repo / graph_storage / sleep_time_worker"
            );
        }

        // Initialize knowledge database (memory facts, graph, vec0 indexes).
        // `None` when Surreal is on — handlers route through trait stores.
        let knowledge_db: Option<Arc<KnowledgeDatabase>> = if use_surreal_for_knowledge {
            None
        } else {
            Some(Arc::new(
                KnowledgeDatabase::new(paths.clone())
                    .expect("Failed to initialize knowledge database"),
            ))
        };

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
        // Phase E2: when SurrealDB is on, knowledge_db is None and every
        // SQLite-tied repo below is None too. Trait-routed stores cover
        // the same surface via `surreal_bundle` further down.
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
        // /api/distillation/status report real numbers on Surreal mode
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
        // when SurrealDB is on (knowledge_db is None).
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
        // SessionDistiller construction, so they can be wired with it). The
        // KG override branch lives later because it depends on
        // runner_graph_storage which isn't available yet — we re-derive
        // surreal_override there with the cached bundle below.
        //
        // When the user has opted into Surreal, the *full* store bundle is
        // built here so the trait-routed `episode_store`, `wiki_store` and
        // `procedure_store` fields below can route to Surreal too — without
        // the bundle they'd silently fall through to SQLite-backed
        // counterparts (or, for episodes built later, the wrong DB) and
        // the UI would show zeroes.
        #[cfg(feature = "surreal-backend")]
        let surreal_bundle: Option<persistence_factory::SurrealStoreBundle> =
            persistence_factory::maybe_build_surreal_full(&paths);
        #[cfg(not(feature = "surreal-backend"))]
        let _surreal_bundle: Option<()> = None;
        // The trait-routed memory store: Surreal when opted in, SQLite-wrapper
        // when memory_repo is Some, None otherwise (Surreal feature off + flag on
        // — defensive, not reachable in normal builds).
        let early_memory_store: Option<Arc<dyn zero_stores::MemoryFactStore>> = {
            #[cfg(feature = "surreal-backend")]
            {
                surreal_bundle
                    .as_ref()
                    .map(|b| b.memory.clone())
                    .or_else(|| {
                        memory_repo.as_ref().map(|mr| {
                            persistence_factory::build_memory_store(
                                mr.clone(),
                                embedding_client.clone(),
                            )
                        })
                    })
            }
            #[cfg(not(feature = "surreal-backend"))]
            {
                memory_repo.as_ref().map(|mr| {
                    persistence_factory::build_memory_store(mr.clone(), embedding_client.clone())
                })
            }
        };

        // Create memory recall. Phase E8: builds whenever EITHER
        // memory_store (trait) OR memory_repo (concrete) is wired —
        // recall now runs on Surreal too. Graph enrichment via
        // GraphService still requires the concrete graph_storage; on
        // Surreal it's None and recall falls back to non-enriched
        // hybrid (still finds facts, just no KG-traversal boost).
        let mut memory_recall_inner: Option<MemoryRecall> =
            if early_memory_store.is_some() {
                Some(MemoryRecall::new(
                    embedding_client.clone(),
                    recall_config.clone(),
                ))
            } else {
                None
            };
        // Wire trait-routed episode_store (Phase E6c). Picks
        // surreal_bundle.episode in Surreal mode, falls back to
        // wrapping the SQLite EpisodeRepository.
        if let Some(recall) = memory_recall_inner.as_mut() {
            #[cfg(feature = "surreal-backend")]
            let store_opt: Option<Arc<dyn zero_stores_traits::EpisodeStore>> = surreal_bundle
                .as_ref()
                .map(|b| b.episode.clone())
                .or_else(|| {
                    episode_repo.as_ref().map(|r| {
                        Arc::new(zero_stores_sqlite::GatewayEpisodeStore::new(r.clone()))
                            as Arc<dyn zero_stores_traits::EpisodeStore>
                    })
                });
            #[cfg(not(feature = "surreal-backend"))]
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
            #[cfg(feature = "surreal-backend")]
            let store_opt: Option<Arc<dyn zero_stores_traits::WikiStore>> =
                surreal_bundle.as_ref().map(|b| b.wiki.clone()).or_else(|| {
                    wiki_repo.as_ref().map(|r| {
                        Arc::new(zero_stores_sqlite::GatewayWikiStore::new(r.clone()))
                            as Arc<dyn zero_stores_traits::WikiStore>
                    })
                });
            #[cfg(not(feature = "surreal-backend"))]
            let store_opt: Option<Arc<dyn zero_stores_traits::WikiStore>> =
                wiki_repo.as_ref().map(|r| {
                    Arc::new(zero_stores_sqlite::GatewayWikiStore::new(r.clone()))
                        as Arc<dyn zero_stores_traits::WikiStore>
                });
            if let Some(store) = store_opt {
                recall.set_wiki_store(store);
            }
        }
        // Phase B: trait-routed kg ingestion store. Prefer the surreal
        // bundle's impl (wired when the user opts in); fall back to a
        // GatewayKgEpisodeStore wrapping the SQLite kg_episode_repo.
        // Backend-agnostic — handlers + queue + adapter all consume the
        // trait, so a third backend (Postgres / etc.) plugs in by
        // implementing the trait and adding a build branch above.
        let kg_episode_store: Option<Arc<dyn zero_stores_traits::KgEpisodeStore>> = {
            #[cfg(feature = "surreal-backend")]
            {
                surreal_bundle
                    .as_ref()
                    .map(|b| b.kg_episode.clone())
                    .or_else(|| {
                        kg_episode_repo.as_ref().map(|r| {
                            Arc::new(zero_stores_sqlite::GatewayKgEpisodeStore::new(r.clone()))
                                as Arc<dyn zero_stores_traits::KgEpisodeStore>
                        })
                    })
            }
            #[cfg(not(feature = "surreal-backend"))]
            {
                kg_episode_repo.as_ref().map(|r| {
                    Arc::new(zero_stores_sqlite::GatewayKgEpisodeStore::new(r.clone()))
                        as Arc<dyn zero_stores_traits::KgEpisodeStore>
                })
            }
        };

        // Trait-routed wiki store — Surreal when opted in, else SQLite wrapper
        // (when wiki_repo exists), else None.
        let wiki_store_for_state: Option<Arc<dyn zero_stores_traits::WikiStore>> = {
            #[cfg(feature = "surreal-backend")]
            {
                surreal_bundle.as_ref().map(|b| b.wiki.clone()).or_else(|| {
                    wiki_repo.as_ref().map(|wr| {
                        Arc::new(zero_stores_sqlite::GatewayWikiStore::new(wr.clone()))
                            as Arc<dyn zero_stores_traits::WikiStore>
                    })
                })
            }
            #[cfg(not(feature = "surreal-backend"))]
            {
                wiki_repo.as_ref().map(|wr| {
                    Arc::new(zero_stores_sqlite::GatewayWikiStore::new(wr.clone()))
                        as Arc<dyn zero_stores_traits::WikiStore>
                })
            }
        };

        // Wire procedure repository for procedure recall during intent analysis.
        // SQLite-only — None when knowledge_db is None.
        let procedure_repo: Option<Arc<ProcedureRepository>> = knowledge_db.as_ref().map(|kdb| {
            let procedure_vec: Arc<dyn VectorIndex> = Arc::new(
                SqliteVecIndex::new(kdb.clone(), "procedures_index", "procedure_id")
                    .expect("vec index init"),
            );
            Arc::new(ProcedureRepository::new(kdb.clone(), procedure_vec))
        });
        let procedure_store_for_state: Option<Arc<dyn zero_stores_traits::ProcedureStore>> = {
            #[cfg(feature = "surreal-backend")]
            {
                surreal_bundle
                    .as_ref()
                    .map(|b| b.procedure.clone())
                    .or_else(|| {
                        procedure_repo.as_ref().map(|pr| {
                            Arc::new(zero_stores_sqlite::GatewayProcedureStore::new(pr.clone()))
                                as Arc<dyn zero_stores_traits::ProcedureStore>
                        })
                    })
            }
            #[cfg(not(feature = "surreal-backend"))]
            {
                procedure_repo.as_ref().map(|pr| {
                    Arc::new(zero_stores_sqlite::GatewayProcedureStore::new(pr.clone()))
                        as Arc<dyn zero_stores_traits::ProcedureStore>
                })
            }
        };
        // Wire the trait-routed procedure_store on MemoryRecall so
        // procedure recall runs on Surreal too (Phase E6c).
        if let (Some(recall), Some(ps)) = (
            memory_recall_inner.as_mut(),
            procedure_store_for_state.as_ref(),
        ) {
            recall.set_procedure_store(ps.clone());
        }

        // Trait-routed episode store for downstream consumers (distiller +
        // sleep worker + AppState). Built once here so the sleep worker
        // construction below doesn't have to re-derive from
        // backend-specific repos. SurrealDB mode picks `surreal_bundle.episode`;
        // SQLite mode wraps `EpisodeRepository` (built lazily from
        // `knowledge_db` since the original repo is composed elsewhere).
        let episode_store_for_state: Option<Arc<dyn zero_stores_traits::EpisodeStore>> = {
            #[cfg(feature = "surreal-backend")]
            {
                if let Some(b) = surreal_bundle.as_ref() {
                    Some(b.episode.clone())
                } else {
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
                    })
                }
            }
            #[cfg(not(feature = "surreal-backend"))]
            {
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
                })
            }
        };

        // Conversation store is always SQLite-backed (per the design doc:
        // conversations.db never moves to Surreal). The sleep worker's
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
        // Picks the surreal_bundle's impl when present, else wraps the
        // SQLite GraphStorage (Phase E6c).
        let kg_store: Option<Arc<dyn zero_stores::KnowledgeGraphStore>> = {
            #[cfg(feature = "surreal-backend")]
            {
                surreal_bundle.as_ref().map(|b| b.kg.clone()).or_else(|| {
                    graph_storage.as_ref().map(|gs| {
                        let embedder = embedding_client
                            .clone()
                            .expect("embedding_client wired above for distillation/recall");
                        persistence_factory::build_kg_store_from_storage(gs.clone(), embedder)
                    })
                })
            }
            #[cfg(not(feature = "surreal-backend"))]
            {
                graph_storage.as_ref().map(|gs| {
                    let embedder = embedding_client
                        .clone()
                        .expect("embedding_client wired above for distillation/recall");
                    persistence_factory::build_kg_store_from_storage(gs.clone(), embedder)
                })
            }
        };
        if let (Some(recall), Some(ks)) = (memory_recall_inner.as_mut(), kg_store.as_ref()) {
            recall.set_kg_store(ks.clone());
        }

        let memory_recall: Option<Arc<MemoryRecall>> = memory_recall_inner.map(Arc::new);

        // Clone embedding client before it's moved into distiller — the runner
        // also needs it so the memory fact store can generate embeddings.
        let runner_embedding_client = embedding_client.clone();

        // Clone graph_storage before it's moved into the distiller — the runner
        // also needs it for the graph_query tool.
        let runner_graph_storage = graph_storage.clone();

        // Build the trait-object KG store from runner_graph_storage.
        // Coexists with graph_service/graph_storage until Phase 5 retirement.
        //
        // Construction is centralized in `persistence_factory` (TD-023):
        // when SurrealDB support lands, the config-driven branch goes
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
        // Surreal mode means the corresponding side-effects (KG ingestion,
        // run-tracking, episode storage, wiki compilation, procedure
        // upsert) skip gracefully. Fact distillation itself runs.
        let distiller: Option<Arc<SessionDistiller>> =
            if memory_store.is_some() {
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
        // None when SurrealDB mode skipped distiller construction.
        let distiller_ref: Option<Arc<SessionDistiller>> = distiller.clone();
        let max_parallel_agents = settings
            .get_execution_settings()
            .map(|s| s.max_parallel_agents)
            .unwrap_or(2);
        tracing::info!(max_parallel_agents, "Execution settings loaded");

        // Create streaming ingestion queue + backpressure BEFORE the runtime so the
        // runner can be wired with an IngestionAdapter.
        //
        // Phase B2: trait-routed. Queue + backpressure now consume
        // Arc<dyn KgEpisodeStore> + Arc<dyn KnowledgeGraphStore>. Both
        // are wired in BOTH backends (kg_episode_store from
        // surreal_bundle.kg_episode or GatewayKgEpisodeStore wrap;
        // kg_store from surreal_bundle.kg or sqlite kg_store builder).
        // So the queue runs on Surreal too — pending ingestion episodes
        // get processed instead of accumulating forever.
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
        // alongside the queue so subagent ingestion works on Surreal.
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
        // Goal adapter — backend-agnostic. Picks Surreal store when on
        // surreal-backend, falls back to wrapping the SQLite GoalRepository.
        let goal_store_for_adapter: Option<Arc<dyn zero_stores_traits::GoalStore>> = {
            #[cfg(feature = "surreal-backend")]
            {
                surreal_bundle.as_ref().map(|b| b.goal.clone()).or_else(|| {
                    goal_repo.as_ref().map(|gr| {
                        Arc::new(zero_stores_sqlite::GatewayGoalStore::new(gr.clone()))
                            as Arc<dyn zero_stores_traits::GoalStore>
                    })
                })
            }
            #[cfg(not(feature = "surreal-backend"))]
            {
                goal_repo.as_ref().map(|gr| {
                    Arc::new(zero_stores_sqlite::GatewayGoalStore::new(gr.clone()))
                        as Arc<dyn zero_stores_traits::GoalStore>
                })
            }
        };
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
            workspace_cache.clone(),
            memory_store.clone(),
            distiller,
            memory_recall,
            Some(bridge_registry.clone()),
            Some(bridge_outbox.clone()),
            runner_embedding_client,
            max_parallel_agents,
            runner_graph_storage.clone(),
            kg_store.clone(),
            kg_episode_repo.clone(),
            ingestion_adapter,
            goal_adapter,
        ));

        // Phase 4: CompactionRepository + SleepTimeWorker (background maintenance).
        // CompactionRepository is SQLite-tied (kg_compactions table on
        // knowledge.db). None in SurrealDB mode.
        let compaction_repo: Option<Arc<zero_stores_sqlite::CompactionRepository>> = knowledge_db
            .as_ref()
            .map(|kdb| Arc::new(zero_stores_sqlite::CompactionRepository::new(kdb.clone())));

        // Phase D1: trait-routed compaction audit store. Wired in BOTH
        // backends so the maintenance worker can record merges/prunes
        // regardless of backend. Surreal uses its own
        // `kg_compaction_run` table; SQLite delegates to the existing
        // `CompactionRepository`. Default no-op impls cover edge cases.
        let compaction_store: Option<Arc<dyn zero_stores_traits::CompactionStore>> = {
            #[cfg(feature = "surreal-backend")]
            {
                surreal_bundle
                    .as_ref()
                    .map(|b| b.compaction.clone())
                    .or_else(|| {
                        compaction_repo.as_ref().map(|r| {
                            Arc::new(zero_stores_sqlite::GatewayCompactionStore::new(r.clone()))
                                as Arc<dyn zero_stores_traits::CompactionStore>
                        })
                    })
            }
            #[cfg(not(feature = "surreal-backend"))]
            {
                compaction_repo.as_ref().map(|r| {
                    Arc::new(zero_stores_sqlite::GatewayCompactionStore::new(r.clone()))
                        as Arc<dyn zero_stores_traits::CompactionStore>
                })
            }
        };

        // One-shot backfill: populate legacy kg_entities / kg_relationships
        // rows with the richer metadata introduced in commits b816702,
        // 1bc21f6, 5bf3013. Marker row in kg_compactions gates this so
        // subsequent daemon starts are a no-op. Non-fatal on failure —
        // a backfill bug must never prevent the daemon from booting.
        // Skip entirely when knowledge_db is None (SurrealDB mode).
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

        // Sleep-time worker requires the entire SQLite knowledge cluster.
        // Build only when ALL of (compaction_repo, memory_repo, knowledge_db,
        // procedure_repo, kg_store, compaction_store) are present. The
        // maintenance ops (compactor/decay/pruner/orphan_archiver) take
        // trait objects (`kg_store`, `compaction_store`) so they run on
        // both backends; synthesizer / pattern_extractor are still
        // SQLite-tied via `compaction_repo` (migrated in Phase D4).
        // Sleep-time worker — fully trait-routed (Phase D5). Gates only on
        // the trait stores; on SurrealDB mode they come from
        // `surreal_bundle`, on SQLite mode from the repo wrappers.
        // Conversation store is always SQLite-backed (per design) and
        // built unconditionally above.
        let sleep_time_worker = match (
            kg_store.as_ref(),
            episode_store_for_state.as_ref(),
            memory_store.as_ref(),
            procedure_store_for_state.as_ref(),
            compaction_store.as_ref(),
        ) {
            (Some(kgs), Some(eps), Some(mems), Some(prs), Some(compstore)) => {
                let verifier: Option<
                    Arc<dyn gateway_execution::sleep::compactor::PairwiseVerifier>,
                > = Some(Arc::new(
                    gateway_execution::sleep::LlmPairwiseVerifier::new(provider_service.clone()),
                ));
                let compactor = Arc::new(gateway_execution::sleep::Compactor::new(
                    kgs.clone(),
                    compstore.clone(),
                    verifier,
                ));
                let decay = Arc::new(gateway_execution::sleep::DecayEngine::new(
                    kgs.clone(),
                    gateway_execution::sleep::DecayConfig::default(),
                ));
                let pruner = Arc::new(gateway_execution::sleep::Pruner::new(
                    kgs.clone(),
                    compstore.clone(),
                ));
                let synth_llm = Arc::new(gateway_execution::sleep::LlmSynthesizer::new(
                    provider_service.clone(),
                ));
                let synthesizer = Arc::new(gateway_execution::sleep::Synthesizer::new(
                    kgs.clone(),
                    eps.clone(),
                    mems.clone(),
                    compstore.clone(),
                    synth_llm,
                    embedding_client.clone(),
                ));
                let pattern_llm = Arc::new(gateway_execution::sleep::LlmPatternExtractor::new(
                    provider_service.clone(),
                ));
                let pattern_extractor = Arc::new(gateway_execution::sleep::PatternExtractor::new(
                    eps.clone(),
                    conversation_store_for_state.clone(),
                    prs.clone(),
                    compstore.clone(),
                    pattern_llm,
                ));
                let orphan_archiver = Arc::new(gateway_execution::sleep::OrphanArchiver::new(
                    kgs.clone(),
                    compstore.clone(),
                ));
                let ops = gateway_execution::sleep::SleepOps {
                    synthesizer: Some(synthesizer),
                    pattern_extractor: Some(pattern_extractor),
                    orphan_archiver: Some(orphan_archiver),
                };
                Some(Arc::new(
                    gateway_execution::sleep::SleepTimeWorker::start_with_ops(
                        compactor,
                        decay,
                        pruner,
                        ops,
                        std::time::Duration::from_secs(60 * 60),
                        "root".to_string(),
                    ),
                ))
            }
            _ => None,
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
            workspace_cache,
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
            workspace_cache: new_workspace_cache(),
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
            workspace_cache: new_workspace_cache(),
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
        // The reindex pipeline targets SQLite vec0 tables — when the user
        // is on the SurrealDB backend (`knowledge_db is None`), there is
        // nothing for this path to operate on; the Surreal backend keeps
        // its own embeddings inline. Mark the marker as in sync and bail.
        if self.embedding_service.needs_reindex() {
            let current_dim = self.embedding_service.dimensions();
            let Some(knowledge_db) = self.knowledge_db.as_ref() else {
                tracing::info!(
                    dim = current_dim,
                    "Embedding marker mismatch but SQLite knowledge DB is disabled (SurrealDB backend) — marking indexed without reindex"
                );
                if let Err(e) = self.embedding_service.mark_indexed(current_dim) {
                    tracing::warn!("mark_indexed failed in surreal mode: {e}");
                }
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

    /// Seed default policies from bundled template if no policies/corrections exist.
    async fn seed_default_policies(&self) {
        // Route through the trait surface so both SQLite and SurrealDB
        // backends seed identically. `memory_store` is wired in both
        // modes (SQLite-wrapper or SurrealMemoryStore via the bundle).
        let memory_store = match &self.memory_store {
            Some(s) => s,
            None => {
                tracing::warn!(
                    "seed_default_policies: memory_store is None — refusing to seed. \
                     This means neither SQLite memory_repo nor a SurrealDB bundle was \
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
