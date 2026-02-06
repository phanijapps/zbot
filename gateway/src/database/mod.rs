// ============================================================================
// DATABASE MODULE
// Re-exports DatabaseManager from gateway-database crate.
// ConversationRepository stays here (depends on agent-runtime types).
// ============================================================================

mod repository;

pub use gateway_database::DatabaseManager;
pub use repository::{ConversationRepository, Message};
