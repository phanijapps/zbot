// ============================================================================
// CONVERSATION STORE TRAIT
// Backend-agnostic interface for the conversations subsystem
// ============================================================================
//
// Conversations stay SQLite forever per the design
// (`memory-bank/future-state/persistence-readiness-design.md`). This trait
// exists for hygiene + symmetry with `KnowledgeGraphStore` and
// `MemoryFactStore`. The concrete impl in `gateway-database` implements this
// trait.
//
// Note on scope:
// - Trait surface is intentionally narrow. Only methods that an existing
//   trait-erased consumer needs are exposed. Rich-type methods
//   (`agent_runtime::ChatMessage` etc.) are deliberately NOT hoisted here
//   — that would force `zero-stores-traits` to depend on `agent-runtime`
//   which cycles back through `agent-tools`. Consumers convert the POD
//   `Message` rows returned here into rich types themselves.
// - The included methods cover (a) session-metadata lookups whose return
//   types are trait-friendly and (b) the message-history read that
//   `HandoffWriter` needs (returning the POD `Message` from
//   `zero-stores-domain`).
// - Methods are synchronous to mirror the existing public surface (the
//   underlying `DatabaseManager::with_connection` call is blocking). Errors
//   stay as `String`, matching the existing API.

use zero_stores_domain::Message;

/// Backend-agnostic interface for the conversation message stream.
///
/// Mirrors a subset of `zero_stores_sqlite::ConversationRepository`'s public
/// surface. Surface intentionally narrow — see module docs.
pub trait ConversationStore: Send + Sync {
    /// Get the `ward_id` for a session.
    ///
    /// Returns `Ok(None)` if the session has no ward or the session
    /// doesn't exist.
    fn get_session_ward_id(&self, session_id: &str) -> Result<Option<String>, String>;

    /// Get the `root_agent_id` for a session.
    ///
    /// Returns `Ok(None)` if the session doesn't exist.
    fn get_session_agent_id(&self, session_id: &str) -> Result<Option<String>, String>;

    /// Ordered list of tool names called by the assistant in a
    /// session, parsed from each `messages.tool_calls` blob in
    /// `created_at ASC` order. Used by the sleep-time
    /// `PatternExtractor` to detect repeated tool sequences across
    /// successful sessions. Default: empty so backends that haven't
    /// implemented yet make pattern extraction a quiet no-op.
    fn tool_sequence_for_session(&self, _session_id: &str) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }

    /// Load up to `limit` messages for a session, oldest-to-newest.
    /// Returns POD `Message` rows; rich-type conversion (to
    /// `agent_runtime::ChatMessage`) lives in the consumer crate.
    /// Default: empty so backends that haven't implemented yet make
    /// sleep-time handoff a quiet no-op.
    fn get_session_messages(
        &self,
        _session_id: &str,
        _limit: usize,
    ) -> Result<Vec<Message>, String> {
        Ok(Vec::new())
    }
}
