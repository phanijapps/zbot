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
pub use models::ModelRegistry;
pub use logging::LogSettings;
pub use mcp::McpService;
pub use paths::{SharedVaultPaths, VaultPaths};
pub use plugin_service::PluginService;
pub use providers::ProviderService;
pub use recall_config::RecallConfig;
pub use settings::{AppSettings, SettingsService};
pub use skills::SkillService;
pub use watcher::{FileWatcher, WatchConfig};
