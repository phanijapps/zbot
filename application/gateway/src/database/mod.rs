// ============================================================================
// DATABASE MODULE
// SQLite-based persistence for conversations and messages
// ============================================================================

mod connection;
mod schema;
mod repository;

pub use connection::DatabaseManager;
pub use repository::{ConversationRepository, Conversation, Message};
