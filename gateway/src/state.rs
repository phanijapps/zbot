//! # Application State
//!
//! Shared state for the gateway application.

use api_logs::LogService;
use execution_state::StateService;
use crate::connectors::{ConnectorRegistry, ConnectorService};
use crate::cron::CronScheduler;
use crate::database::{ConversationRepository, DatabaseManager};
use crate::events::EventBus;
use crate::execution::{new_workspace_cache, DelegationRegistry, MemoryRecall, SessionDistiller, WorkspaceCache};
use crate::hooks::HookRegistry;
use crate::services::{AgentService, McpService, ProviderService, RuntimeService, SettingsService, SkillService};
use agent_runtime::llm::LocalEmbeddingClient;
use agent_runtime::llm::EmbeddingClient;
use agent_tools::MemoryEntry;
use agent_tools::MemoryStore;
use chrono::Utc;
use gateway_database::MemoryRepository;
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

    /// Cron scheduler for scheduled agent triggers.
    /// Optional because it requires async initialization with GatewayBus.
    pub cron_scheduler: Option<Arc<CronScheduler>>,

    /// Cached workspace context (shared with ExecutionRunner).
    workspace_cache: WorkspaceCache,

    /// Configuration directory path.
    pub config_dir: PathBuf,
}

impl AppState {
    /// Create a new application state.
    ///
    /// This creates a fully initialized state with execution runner and SQLite database.
    pub fn new(config_dir: PathBuf) -> Self {
        let agents_dir = config_dir.join("agents");
        let skills_dir = config_dir.join("skills");
        let event_bus = Arc::new(EventBus::new());
        let agents = Arc::new(AgentService::new(agents_dir));
        let skills = Arc::new(SkillService::new(skills_dir));
        let provider_service = Arc::new(ProviderService::new(config_dir.clone()));
        let mcp_service = Arc::new(McpService::new(config_dir.clone()));

        // Initialize SQLite database for conversation persistence
        let db_manager = Arc::new(
            DatabaseManager::new(config_dir.clone())
                .expect("Failed to initialize conversation database"),
        );
        let conversation_repo = Arc::new(ConversationRepository::new(db_manager.clone()));

        // Create log service for execution tracing
        let log_service = Arc::new(LogService::new(db_manager.clone()));

        // Create state service for execution state management
        let state_service = Arc::new(StateService::new(db_manager.clone()));

        // Create connector registry
        let connector_service = ConnectorService::new(config_dir.clone());
        let connector_registry = Arc::new(ConnectorRegistry::new(connector_service));

        // Create workspace cache (shared between AppState and ExecutionRunner)
        let workspace_cache = new_workspace_cache();

        // Initialize memory evolution services
        let memory_repo = Arc::new(MemoryRepository::new(db_manager));

        let embedding_client: Option<Arc<dyn EmbeddingClient>> = match LocalEmbeddingClient::new() {
            Ok(client) => {
                tracing::info!(
                    "Local embedding client initialized ({}d)",
                    client.dimensions()
                );
                Some(Arc::new(client))
            }
            Err(e) => {
                tracing::warn!("Local embedding unavailable, FTS5-only recall: {}", e);
                None
            }
        };

        let memory_recall = Arc::new(MemoryRecall::new(
            embedding_client.clone(),
            memory_repo.clone(),
        ));

        let distiller = Arc::new(SessionDistiller::new(
            provider_service.clone(),
            embedding_client,
            conversation_repo.clone(),
            memory_repo.clone(),
            None, // graph_storage — wired when knowledge graph is configured
        ));

        // Create runtime with execution runner and connector registry
        let runtime = Arc::new(RuntimeService::with_runner_and_connectors(
            event_bus.clone(),
            agents.clone(),
            provider_service.clone(),
            config_dir.clone(),
            conversation_repo.clone(),
            mcp_service.clone(),
            skills.clone(),
            log_service.clone(),
            state_service.clone(),
            Some(connector_registry.clone()),
            workspace_cache.clone(),
            Some(memory_repo),
            Some(distiller),
            Some(memory_recall),
        ));

        // Create hook registry
        let hook_registry = Arc::new(HookRegistry::new(event_bus.clone()));

        // Create delegation registry
        let delegation_registry = Arc::new(DelegationRegistry::new());

        // Create settings service
        let settings = Arc::new(SettingsService::new(config_dir.clone()));

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
            cron_scheduler: None, // Initialized by server.start()
            workspace_cache,
            config_dir,
        }
    }

    /// Create a minimal state without execution runner (for testing).
    pub fn minimal(config_dir: PathBuf) -> Self {
        let agents_dir = config_dir.join("agents");
        let skills_dir = config_dir.join("skills");
        let event_bus = Arc::new(EventBus::new());

        // Initialize SQLite database for conversation persistence
        let db_manager = Arc::new(
            DatabaseManager::new(config_dir.clone())
                .expect("Failed to initialize conversation database"),
        );
        let conversation_repo = Arc::new(ConversationRepository::new(db_manager.clone()));
        let log_service = Arc::new(LogService::new(db_manager.clone()));
        let state_service = Arc::new(StateService::new(db_manager));

        // Create connector registry
        let connector_service = ConnectorService::new(config_dir.clone());
        let connector_registry = Arc::new(ConnectorRegistry::new(connector_service));

        Self {
            agents: Arc::new(AgentService::new(agents_dir)),
            skills: Arc::new(SkillService::new(skills_dir)),
            provider_service: Arc::new(ProviderService::new(config_dir.clone())),
            mcp_service: Arc::new(McpService::new(config_dir.clone())),
            runtime: Arc::new(RuntimeService::new(event_bus.clone())),
            event_bus,
            hook_registry: None,
            delegation_registry: Arc::new(DelegationRegistry::new()),
            conversations: conversation_repo,
            settings: Arc::new(SettingsService::new(config_dir.clone())),
            log_service,
            state_service,
            connector_registry,
            cron_scheduler: None,
            workspace_cache: new_workspace_cache(),
            config_dir,
        }
    }

    /// Create with custom components.
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
        config_dir: PathBuf,
    ) -> Self {
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
            settings: Arc::new(SettingsService::new(config_dir.clone())),
            log_service,
            state_service,
            connector_registry,
            cron_scheduler: None,
            workspace_cache: new_workspace_cache(),
            config_dir,
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

        // Seed default agents
        if let Err(e) = self.agents.seed_default_agents(&default_provider_id).await {
            tracing::warn!("Failed to seed default agents: {}", e);
        }

        // Preload skills into cache
        if let Err(e) = self.skills.preload().await {
            tracing::warn!("Failed to preload skills: {}", e);
        }

        // Create Python venv and Node env if missing, then seed workspace memory
        self.ensure_runtime_environments().await;
    }

    /// Ensure Python venv and Node.js environment exist, then seed workspace memory.
    async fn ensure_runtime_environments(&self) {
        // Create wards directory structure
        self.ensure_wards_dir();

        let venv_ok = self.ensure_python_venv().await;
        let node_ok = self.ensure_node_env().await;
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
                tracing::info!("Created wards directory with scratch ward at {}", wards_dir.display());
            }
        }
    }

    /// Populate the in-memory workspace cache from workspace.json.
    ///
    /// This is called once at startup after seeding. The same Arc is shared
    /// with ExecutionRunner, so all executors see the cached data without
    /// reading from disk on every invocation.
    async fn populate_workspace_cache(&self) {
        let workspace_path = self
            .config_dir
            .join("agents_data")
            .join("shared")
            .join("workspace.json");

        let workspace = match std::fs::read_to_string(&workspace_path) {
            Ok(content) => match serde_json::from_str::<MemoryStore>(&content) {
                Ok(store) => {
                    let map: HashMap<String, serde_json::Value> = store
                        .entries
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.value.clone())))
                        .collect();
                    if map.is_empty() { None } else { Some(map) }
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
                tracing::warn!("Failed to run python -m venv: {} (python may not be installed)", e);
                false
            }
        }
    }

    /// Create Node.js environment at `{config_dir}/wards/.node_env` if it doesn't exist.
    /// Falls back to legacy `{config_dir}/node_env` if it exists there.
    /// Returns true if the node_env exists (either already existed or was created).
    async fn ensure_node_env(&self) -> bool {
        let new_path = self.config_dir.join("wards").join(".node_env");
        let legacy_path = self.config_dir.join("node_env");

        // Use new path, but check legacy location too
        let node_env_dir = if new_path.exists() {
            new_path
        } else if legacy_path.exists() {
            legacy_path
        } else {
            new_path // Create at new location
        };
        let package_json = node_env_dir.join("package.json");

        if package_json.exists() {
            tracing::debug!("Node env already exists at {}", node_env_dir.display());
            return true;
        }

        tracing::info!("Creating Node env at {}", node_env_dir.display());

        // Create the directory
        if let Err(e) = std::fs::create_dir_all(&node_env_dir) {
            tracing::warn!("Failed to create node_env directory: {}", e);
            return false;
        }

        // Run npm init -y to create package.json
        let result = tokio::process::Command::new("npm")
            .args(["init", "-y"])
            .current_dir(&node_env_dir)
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                tracing::info!("Node env created successfully");
                true
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("Failed to initialize node_env: {}", stderr.trim());
                false
            }
            Err(e) => {
                tracing::warn!("Failed to run npm init: {} (npm may not be installed)", e);
                false
            }
        }
    }

    /// Seed workspace.json with python_env and node_env status.
    /// Only writes entries that don't already exist (preserves user state).
    /// Uses the same MemoryStore type as the memory tool to avoid format mismatch.
    fn seed_workspace_env_status(&self, venv_ok: bool, node_ok: bool) {
        let workspace_path = self
            .config_dir
            .join("agents_data")
            .join("shared")
            .join("workspace.json");

        // Ensure parent directory exists
        if let Some(parent) = workspace_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create workspace directory: {}", e);
                return;
            }
        }

        // Load existing store using the same type as the memory tool
        let mut store: MemoryStore = if let Ok(content) = std::fs::read_to_string(&workspace_path)
        {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            MemoryStore::default()
        };

        let now = Utc::now().to_rfc3339();

        // Seed python_env if not already present
        if !store.entries.contains_key("python_env") {
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

            store.entries.insert(
                "python_env".to_string(),
                MemoryEntry {
                    value: value.to_string(),
                    tags: vec![
                        "system".to_string(),
                        "python".to_string(),
                        "env".to_string(),
                    ],
                    created_at: now.clone(),
                    updated_at: now.clone(),
                },
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
                    tags: vec![
                        "system".to_string(),
                        "node".to_string(),
                        "env".to_string(),
                    ],
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
}
