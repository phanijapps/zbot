// ============================================================================
// Session classification
//
// Names the domain rule for distinguishing chat sessions (created by the
// reserved `/chat` session via /api/chat/init) from research sessions
// (everything else). Both the research session drawer and the research
// landing hero consume these predicates — keeping the rule in one place
// prevents the drift that originally caused chat-v2 sessions to leak into
// the research surface's recent-session cards.
//
// Primary signal: the `mode` column on the `sessions` row, now surfaced
// on `LogSession.mode` by the gateway. `'fast'` / `'chat'` → chat-mode,
// anything else (including `null`/`undefined`) → research-mode. This
// mirrors `SessionMode::from_mode_string` in
// `gateway-execution/src/config.rs` — the authoritative classifier.
//
// Fallback signal: the legacy `sess-chat-*` conversation-id prefix minted
// by /api/chat/init. Kept for rows that predate the `mode` wire field
// (older daemons, in-flight records during the rolling upgrade). The
// prefix check is intentionally secondary; once `mode` is populated
// everywhere this fallback becomes dormant and can be retired.
// ============================================================================

/**
 * Conversation-id prefix minted by /api/chat/init for the reserved chat
 * session. See `gateway/src/http/chat.rs`.
 *
 * Exported so anywhere that needs the literal can reference this constant
 * rather than duplicating the string. Not a UI label; this is plumbing.
 */
export const CHAT_SESSION_ID_PREFIX = "sess-chat-";

/** Fields that the session-kind predicates read — kept narrow on purpose. */
export interface SessionKindRow {
  /** Value from `sessions.mode`; may be absent on older wire payloads. */
  mode?: string | null;
  /** Conversation id; used as the legacy fallback signal. */
  conversation_id?: string;
}

/**
 * True when the row represents a chat-mode session.
 *
 * Prefers the explicit `mode` field (`'fast'` / `'chat'`). Falls back to
 * the `sess-chat-*` prefix for rows produced before the wire field
 * existed.
 */
export function isChatSession(row: SessionKindRow): boolean {
  const mode = row.mode?.toLowerCase();
  if (mode === "fast" || mode === "chat") return true;
  if (mode && (mode === "deep" || mode === "research")) return false;
  // `mode` absent or unknown → fall back to id-prefix heuristic.
  return row.conversation_id?.startsWith(CHAT_SESSION_ID_PREFIX) ?? false;
}

/**
 * True when the row represents a research-mode session. The negation of
 * `isChatSession`, exposed as a named predicate so callers read naturally
 * (`rows.filter(isResearchSession)` vs `rows.filter(r => !isChatSession(r))`).
 */
export function isResearchSession(row: SessionKindRow): boolean {
  return !isChatSession(row);
}
