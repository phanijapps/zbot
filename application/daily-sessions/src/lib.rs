// ============================================================================
// DAILY SESSIONS
// Core logic for daily session lifecycle, summary generation, and session chaining
// ============================================================================

pub mod types;
pub mod manager;
pub mod repository;
pub mod summary;

// Re-export common types
pub use types::*;
pub use manager::DailySessionManager;
pub use repository::DailySessionRepository;
pub use zero_core::Result;
