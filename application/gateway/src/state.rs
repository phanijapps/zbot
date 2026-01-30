//! # Application State
//!
//! Shared state for the gateway application.

use crate::database::{ConversationRepository, DatabaseManager};
use crate::events::EventBus;
use crate::services::{AgentService, ProviderService, RuntimeService, SkillService};
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

    /// Runtime service for agent execution.
    pub runtime: Arc<RuntimeService>,

    /// Event bus for broadcasting events.
    pub event_bus: Arc<EventBus>,

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

        // Initialize SQLite database for conversation persistence
        let db_manager = Arc::new(
            DatabaseManager::new(config_dir.clone())
                .expect("Failed to initialize conversation database"),
        );
        let conversation_repo = Arc::new(ConversationRepository::new(db_manager));

        // Create runtime with execution runner
        let runtime = Arc::new(RuntimeService::with_runner(
            event_bus.clone(),
            agents.clone(),
            provider_service.clone(),
            config_dir.clone(),
            conversation_repo,
        ));

        Self {
            agents,
            skills,
            provider_service,
            runtime,
            event_bus,
            config_dir,
        }
    }

    /// Create a minimal state without execution runner (for testing).
    pub fn minimal(config_dir: PathBuf) -> Self {
        let agents_dir = config_dir.join("agents");
        let skills_dir = config_dir.join("skills");
        let event_bus = Arc::new(EventBus::new());

        Self {
            agents: Arc::new(AgentService::new(agents_dir)),
            skills: Arc::new(SkillService::new(skills_dir)),
            provider_service: Arc::new(ProviderService::new(config_dir.clone())),
            runtime: Arc::new(RuntimeService::new(event_bus.clone())),
            event_bus,
            config_dir,
        }
    }

    /// Create with custom components.
    pub fn with_components(
        agents: Arc<AgentService>,
        skills: Arc<SkillService>,
        provider_service: Arc<ProviderService>,
        runtime: Arc<RuntimeService>,
        event_bus: Arc<EventBus>,
        config_dir: PathBuf,
    ) -> Self {
        Self {
            agents,
            skills,
            provider_service,
            runtime,
            event_bus,
            config_dir,
        }
    }
}
