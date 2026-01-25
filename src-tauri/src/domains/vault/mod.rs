// ============================================================================
// VAULT MODULE
// Manages vault (profile) system for isolating agent configurations
// ============================================================================

pub mod types;
pub mod registry;
pub mod manager;

pub use types::*;
pub use registry::*;
pub use manager::*;
