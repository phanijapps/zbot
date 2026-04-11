// ============================================================================
// DAILY SESSIONS
// Daily session management for Agent Channel architecture
// ============================================================================

pub mod cache;
pub mod manager;
pub mod repository;
pub mod types;

pub use cache::*;
pub use manager::*;
pub use repository::*;
pub use types::*;

// Result type for this crate
pub type Result<T> = std::result::Result<T, DailySessionError>;
