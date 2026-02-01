//! # Services
//!
//! Shared services for gateway operations.

pub mod agent_registry;
pub mod agents;
pub mod mcp;
pub mod providers;
pub mod runtime;
pub mod settings;
pub mod skills;

pub use agent_registry::AgentRegistry;
pub use agents::AgentService;
pub use mcp::McpService;
pub use providers::ProviderService;
pub use runtime::RuntimeService;
pub use settings::SettingsService;
pub use skills::SkillService;
