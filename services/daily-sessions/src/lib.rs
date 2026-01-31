// ============================================================================
// DAILY SESSIONS
// Daily session management for Agent Channel architecture
// ============================================================================

pub mod types;
pub mod manager;
pub mod repository;
pub mod cache;

pub use types::*;
pub use manager::*;
pub use repository::*;
pub use cache::*;

// Result type for this crate
pub type Result<T> = std::result::Result<T, DailySessionError>;
