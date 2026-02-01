// ============================================================================
// DATABASE MODULE
// SQLite-based persistence for sessions, executions, and messages
// ============================================================================

mod connection;
mod schema;
mod repository;

pub use connection::DatabaseManager;
pub use repository::{ConversationRepository, Message};
