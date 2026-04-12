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
use agent_runtime::llm::LocalEmbeddingClient;
use agent_tools::MemoryEntry;
use agent_tools::MemoryStore;
use api_logs::LogService;
use chrono::Utc;
use execution_state::StateService;
use gateway_database::{
    DistillationRepository, EpisodeRepository, KgEpisodeRepository, MemoryRepository,
    ProcedureRepository, RecallLogRepository, WardWikiRepository,
};
use knowledge_graph::{GraphService, GraphStorage, SqliteGraphTraversal};
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

    /// Cron scheduler for scheduled agent triggers.
    /// Optional because it requires async initialization with GatewayBus.
    pub cron_scheduler: Option<Arc<CronScheduler>>,

    /// Plugin manager for STDIO plugin lifecycle.
    pub plugin_manager: Arc<gateway_bridge::PluginManager>,

    /// Session archiver for offloading old transcripts to compressed files.
    pub session_archiver: Option<Arc<SessionArchiver>>,

    /// Model capabilities registry (bundled + local overrides).
    pub model_registry: Arc<ModelRegistry>,

    /// Cached workspace context (shared with ExecutionRunner).
    workspace_cache: WorkspaceCache,

    /// Vault paths for accessing configuration and data directories.
    pub paths: SharedVaultPaths,

    /// Configuration directory path (legacy, use paths.vault_dir() instead).
    pub config_dir: PathBuf,
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
        let skills_dir = paths.skills_dir();
        let event_bus = Arc::new(EventBus::new());
        let agents = Arc::new(AgentService::new(agents_dir));
        let skills = Arc::new(SkillService::new(skills_dir));
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

        // Initialize memory evolution services
        let memory_repo = Arc::new(MemoryRepository::new(db_manager.clone()));
        let distillation_repo = Arc::new(DistillationRepository::new(db_manager.clone()));
        let episode_repo = Arc::new(EpisodeRepository::new(db_manager.clone()));
        let kg_episode_repo = Arc::new(KgEpisodeRepository::new(db_manager.clone()));

        // Initialize knowledge graph service and storage
        let (graph_service, graph_storage): (Option<Arc<GraphService>>, Option<Arc<GraphStorage>>) =
            match GraphStorage::new(paths.knowledge_db()) {
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

        let embedding_client: Option<Arc<dyn EmbeddingClient>> = {
            let client = LocalEmbeddingClient::new();
            tracing::info!(
                "Local embedding client created (lazy, {}d)",
                client.dimensions()
            );
            Some(Arc::new(client))
        };

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

        // Wire graph traversal engine for graph-driven expansion in recall
        if let Some(ref gs) = graph_storage {
            let traversal = Arc::new(SqliteGraphTraversal::new(
                gs.clone(),
                recall_config.graph_traversal.hop_decay,
            ));
            memory_recall_inner.set_traversal(traversal);
        }

        // Wire ward wiki repository for wiki-first recall
        let wiki_repo = Arc::new(WardWikiRepository::new(db_manager.clone()));
        memory_recall_inner.set_wiki_repo(wiki_repo);

        // Wire procedure repository for procedure recall during intent analysis
        let procedure_repo = Arc::new(ProcedureRepository::new(db_manager.clone()));
        memory_recall_inner.set_procedure_repo(procedure_repo.clone());

        let memory_recall = Arc::new(memory_recall_inner);

        // Clone embedding client before it's moved into distiller — the runner
        // also needs it so the memory fact store can generate embeddings.
        let runner_embedding_client = embedding_client.clone();

        // Clone graph_storage before it's moved into the distiller — the runner
        // also needs it for the graph_query tool.
        let runner_graph_storage = graph_storage.clone();

        let episode_repo_ref = episode_repo.clone();

        // Create settings service (before distiller & runtime, so we can read execution settings)
        let settings = Arc::new(SettingsService::new(paths.clone()));

        let wiki_repo = Arc::new(WardWikiRepository::new(db_manager.clone()));

        let mut distiller_inner = SessionDistiller::new(
            provider_service.clone(),
            embedding_client,
            conversation_repo.clone(),
            memory_repo.clone(),
            graph_storage,
            Some(distillation_repo.clone()),
            Some(episode_repo),
            paths.clone(), // For loading distillation_prompt.md
            Some(settings.clone()),
        );
        distiller_inner.set_wiki_repo(wiki_repo);
        distiller_inner.set_procedure_repo(procedure_repo);
        let distiller = Arc::new(distiller_inner);

        // Keep a handle for on-demand distillation (backfill, trigger)
        let distiller_ref = distiller.clone();
        let max_parallel_agents = settings
            .get_execution_settings()
            .map(|s| s.max_parallel_agents)
            .unwrap_or(2);
        tracing::info!(max_parallel_agents, "Execution settings loaded");

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
            runner_graph_storage,
            Some(kg_episode_repo.clone()),
        ));

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
            settings,
            log_service,
            state_service,
            connector_registry,
            bridge_registry,
            bridge_outbox,
            bridge_bus: None,     // Set by server.start() before router creation
            cron_scheduler: None, // Initialized by server.start()
            session_archiver: Some(session_archiver),
            plugin_manager,
            model_registry,
            workspace_cache,
            paths,
            config_dir,
            memory_repo: Some(memory_repo),
            distillation_repo: Some(distillation_repo),
            distiller: Some(distiller_ref),
            episode_repo: Some(episode_repo_ref),
            kg_episode_repo: Some(kg_episode_repo),
            graph_service,
        }
    }

    /// Create a minimal state without execution runner (for testing).
    pub fn minimal(config_dir: PathBuf) -> Self {
        let paths = Arc::new(VaultPaths::new(config_dir.clone()));
        let agents_dir = paths.agents_dir();
        let skills_dir = paths.skills_dir();
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
        let memory_repo = Arc::new(MemoryRepository::new(Arc::new(
            DatabaseManager::new(paths.clone()).expect("Failed to initialize database for memory"),
        )));

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
            skills: Arc::new(SkillService::new(skills_dir)),
            provider_service: Arc::new(ProviderService::new(paths.clone())),
            mcp_service: Arc::new(McpService::new(paths.clone())),
            runtime: Arc::new(RuntimeService::new(event_bus.clone())),
            event_bus,
            hook_registry: None,
            delegation_registry: Arc::new(DelegationRegistry::new()),
            conversations: conversation_repo,
            settings: Arc::new(SettingsService::new(paths.clone())),
            log_service,
            state_service,
            connector_registry,
            bridge_registry,
            bridge_outbox,
            bridge_bus: None,
            cron_scheduler: None,
            session_archiver: None,
            model_registry: Arc::new(ModelRegistry::load(&[], paths.vault_dir())),
            plugin_manager,
            workspace_cache: new_workspace_cache(),
            paths,
            config_dir,
            memory_repo: Some(memory_repo),
            distillation_repo: None,
            distiller: None,
            episode_repo: None,
            kg_episode_repo: None,
            graph_service: None,
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
        let db =
            Arc::new(DatabaseManager::new(paths.clone()).expect("Failed to initialize database"));
        let memory_repo = Arc::new(MemoryRepository::new(db));

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
            settings: Arc::new(SettingsService::new(paths.clone())),
            log_service,
            state_service,
            connector_registry,
            bridge_registry,
            bridge_outbox,
            bridge_bus: None,
            cron_scheduler: None,
            session_archiver: None,
            model_registry: Arc::new(ModelRegistry::load(&[], paths.vault_dir())),
            plugin_manager,
            workspace_cache: new_workspace_cache(),
            paths,
            config_dir,
            memory_repo: Some(memory_repo),
            distillation_repo: None,
            distiller: None,
            episode_repo: None,
            kg_episode_repo: None,
            graph_service: None,
        }
    }

    /// Create with hook registry.
    pub fn with_hook_registry(mut self, hook_registry: Arc<HookRegistry>) -> Self {
        self.hook_registry = Some(hook_registry);
        self
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
                epistemic_class: None,
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

    /// Create the wards directory with scratch ward.
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
