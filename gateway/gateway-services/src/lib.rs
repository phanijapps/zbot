//! # Gateway Services
//!
//! Config-based services for the AgentZero gateway.
//!
//! Provides file-backed services for managing:
//! - Agent configurations
//! - LLM provider configurations
//! - MCP server configurations
//! - Skill configurations
//! - Application settings (tools, logging)
//! - Agent registry (delegation permissions)
//! - Plugin configurations

pub mod agent_registry;
pub mod agents;
pub mod embedding_service;
pub mod lang_config;
pub mod logging;
pub mod mcp;
pub mod models;
pub mod paths;
pub mod plugin_service;
pub mod providers;
pub mod recall_config;
pub mod settings;
pub mod skills;
pub mod watcher;

pub use agent_registry::AgentRegistry;
pub use agents::AgentService;
pub use embedding_service::{
    curated_lookup, CuratedModel, EmbeddingBackend, EmbeddingConfig, EmbeddingService, Health,
    OllamaConfig, CURATED_MODELS,
};
pub use lang_config::{load_all_lang_configs, load_lang_config, LangConfig};
pub use logging::LogSettings;
pub use mcp::McpService;
pub use models::ModelRegistry;
pub use paths::{SharedVaultPaths, VaultPaths};
pub use plugin_service::PluginService;
pub use providers::ProviderService;
pub use recall_config::RecallConfig;
pub use settings::{
    AppSettings, ChatConfig, DistillationConfig, ExecutionSettings, MultimodalConfig,
    OrchestratorConfig, SettingsService,
};
pub use skills::{SkillFrontmatter, SkillService, WardAgentsMdConfig, WardSetup};
pub use watcher::{FileWatcher, WatchConfig};
