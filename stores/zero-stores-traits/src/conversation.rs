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
// - Trait surface is intentionally narrow. Most `ConversationRepository`
//   methods return rich row types (`Message`, `agent_runtime::ChatMessage`)
//   that live outside this dependency-light traits crate. Hoisting those
//   value types up to `zero-stores-traits` to widen the trait surface is
//   deferred until a consumer actually needs trait-erased access — the
//   point of this scaffold is symmetry with `KnowledgeGraphStore` and
//   `OutboxStore`, not full method coverage.
// - The included methods are the simple session-metadata lookups whose
//   return types (`Option<String>`) are already trait-friendly. Both
//   methods exist verbatim on `ConversationRepository` today.
// - Methods are synchronous to mirror the existing public surface (the
//   underlying `DatabaseManager::with_connection` call is blocking). Errors
//   stay as `String`, matching the existing API.

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
}
