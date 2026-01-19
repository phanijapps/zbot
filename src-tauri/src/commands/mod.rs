// ============================================================================
// COMMANDS MODULE
// All Tauri commands organized by domain
// ============================================================================

pub mod core;
pub mod conversations;
pub mod agents;
pub mod agent_channels;
pub mod providers;
pub mod mcp;
pub mod skills;
pub mod settings;
pub mod windows;
pub mod tools;
pub mod agents_runtime;

// Re-export all commands
pub use core::*;
pub use conversations::*;
pub use agents::*;
pub use agent_channels::*;
pub use providers::*;
pub use mcp::*;
pub use skills::*;
pub use settings::*;
pub use windows::*;
pub use tools::*;
pub use agents_runtime::*;
