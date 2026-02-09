//! # Gateway Services
//!
//! Config-based services for the AgentZero gateway.
//!
//! Provides file-backed services for managing:
//! - Agent configurations
//! - LLM provider configurations
//! - MCP server configurations
//! - Skill configurations
//! - Application settings
//! - Agent registry (delegation permissions)

pub mod agent_registry;
pub mod agents;
pub mod mcp;
pub mod providers;
pub mod settings;
pub mod skills;

pub use agent_registry::AgentRegistry;
pub use agents::AgentService;
pub use mcp::McpService;
pub use providers::ProviderService;
pub use settings::SettingsService;
pub use skills::SkillService;
