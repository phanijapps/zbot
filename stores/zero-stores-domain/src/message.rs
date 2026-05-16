// ============================================================================
// MESSAGE — conversation message row (POD domain type)
// ============================================================================
//
// Pure-data shape for a row in the `messages` table. Lives here (not in
// `zero-stores-sqlite`) so the `ConversationStore` trait in
// `zero-stores-traits` can hand back conversation history without forcing
// backends or consumers to share the sqlite crate.

use serde::{Deserialize, Serialize};

/// A persisted conversation message row.
///
/// Backend-agnostic shape. Storage-specific encoding lives in each backend
/// (e.g. `zero-stores-sqlite` parses `tool_calls` JSON, an HTTP gateway may
/// flatten differently). Consumers that need rich types (`ChatMessage`,
/// `ToolCall`) convert from this domain type via the helper in
/// `gateway-execution::sleep::handoff_writer::messages_to_chat_format` or
/// equivalent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub execution_id: Option<String>,
    pub session_id: Option<String>,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub token_count: i32,
    /// JSON blob — backend-defined shape. See
    /// `zero-stores-sqlite::ConversationRepository` for the canonical
    /// stored format.
    pub tool_calls: Option<String>,
    pub tool_results: Option<String>,
    pub tool_call_id: Option<String>,
}
