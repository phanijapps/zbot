//! # Application State
//!
//! Shared state for the gateway application.

use api_logs::LogService;
use crate::database::{ConversationRepository, DatabaseManager};
use crate::events::EventBus;
use crate::execution::DelegationRegistry;
use crate::hooks::HookRegistry;
use crate::services::{AgentService, McpService, ProviderService, RuntimeService, SettingsService, SkillService};
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
        let log_service = Arc::new(LogService::new(db_manager));

        // Create runtime with execution runner
        let runtime = Arc::new(RuntimeService::with_runner(
            event_bus.clone(),
            agents.clone(),
            provider_service.clone(),
            config_dir.clone(),
            conversation_repo.clone(),
            mcp_service.clone(),
            skills.clone(),
            log_service.clone(),
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
        let log_service = Arc::new(LogService::new(db_manager));

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
    }
}
