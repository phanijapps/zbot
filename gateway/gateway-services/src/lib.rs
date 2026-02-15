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

pub mod agent_registry;
pub mod agents;
pub mod logging;
pub mod mcp;
pub mod paths;
pub mod providers;
pub mod settings;
pub mod skills;

pub use agent_registry::AgentRegistry;
pub use agents::AgentService;
pub use logging::LogSettings;
pub use mcp::McpService;
pub use paths::{SharedVaultPaths, VaultPaths};
pub use providers::ProviderService;
pub use settings::{AppSettings, SettingsService};
pub use skills::SkillService;
