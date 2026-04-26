// ============================================================================
// MISSION CONTROL — MessagesPane
// Renders the message-like log entries for the selected session: the title
// (as the opening user prompt), intent, response, delegation, error.
//
// Reads from the existing /api/logs/sessions/:id endpoint (via transport)
// rather than the chat-only /messages API — that way the pane works for
// research sessions too. SessionChatViewer was a wrong fit because it calls
// getSessionMessages, which only returns chat-mode messages and gives an
// empty list for research sessions even when the agent has logged a
// response.
// ============================================================================

import { useEffect, useState, useMemo } from "react";
import { Brain, MessageSquare, GitBranch, AlertCircle, Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { LogSession, SessionDetail } from "@/services/transport/types";

interface MessagesPaneProps {
  session: LogSession | null;
}

/** Categories from the session log we render as message-like items. */
const RENDERED_CATEGORIES = new Set<string>(["intent", "response", "delegation", "error"]);

export function MessagesPane({ session }: MessagesPaneProps) {
  const sessionId = session?.session_id ?? null;
  const isRunning = session?.status === "running";
  const { bundle, loading, error } = useSessionDetailWithLive(sessionId, isRunning);

  const messages = useMemo(() => extractMessages(bundle.root, bundle.children), [bundle]);

  return (
    <div className="mc-pane">
      <header className="mc-pane__head">
        <span className="mc-pane__title">Messages</span>
        <LiveBadge active={isRunning} />
      </header>
      <div className="mc-pane__body">
        {!session && <Empty message="Select a session to see its messages." />}
        {session && loading && messages.length === 0 && (
          <div className="mc-pane__empty">
            <Loader2 size={14} /> Loading…
          </div>
        )}
        {session && error && messages.length === 0 && (
          <div className="mc-pane__empty">Could not load messages: {error}</div>
        )}
        {session && !loading && !error && messages.length === 0 && (
          <Empty message="No intent or response logged for this session yet." />
        )}
        {messages.length > 0 && (
          <div className="mc-messages">
            {messages.map((m) => (
              <MessageRow key={m.id} item={m} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ----------------------------------------------------------------------------
// Live data fetch — re-fetches SessionDetail every 2s while running. The
// payload is small (logs are paged at the gateway), so a short interval is
// cheap and the UX gain is large.
// ----------------------------------------------------------------------------

interface DetailBundle {
  /** The root session detail. Always set when sessionId is non-null and load succeeded. */
  root: SessionDetail | null;
  /** Detail records for each direct child session (one delegation per child). */
  children: SessionDetail[];
}

function useSessionDetailWithLive(sessionId: string | null, isRunning: boolean) {
  const [bundle, setBundle] = useState<DetailBundle>({ root: null, children: [] });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (!sessionId) { setBundle({ root: null, children: [] }); return; }
    let cancelled = false;
    setLoading(true);
    (async () => {
      try {
        const transport = await getTransport();
        const rootResult = await transport.getLogSession(sessionId);
        if (cancelled) return;
        if (!rootResult.success || !rootResult.data) {
          setError(rootResult.error ?? "failed");
          return;
        }
        const root = rootResult.data;
        const childIds = root.session.child_session_ids ?? [];
        const childResults = await Promise.all(childIds.map((id) => transport.getLogSession(id)));
        if (cancelled) return;
        const children: SessionDetail[] = [];
        for (const cr of childResults) {
          if (cr.success && cr.data) children.push(cr.data);
        }
        setBundle({ root, children });
        setError(null);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [sessionId, tick]);

  // Live refresh every 2s while running; stops when the session terminates.
  useEffect(() => {
    if (!sessionId || !isRunning) return;
    const id = setInterval(() => setTick((t) => t + 1), 2000);
    return () => clearInterval(id);
  }, [sessionId, isRunning]);

  return { bundle, loading, error };
}

// ----------------------------------------------------------------------------
// Pure: extract message-like items from session detail logs.
// ----------------------------------------------------------------------------

export interface MessageItem {
  id: string;
  category: "intent" | "response" | "delegation" | "error" | "user";
  agent: string;
  body: string;
  timestamp: string;
  meta?: Record<string, unknown>;
}

export function extractMessages(
  root: SessionDetail | null,
  children: SessionDetail[] = [],
): MessageItem[] {
  if (!root) return [];
  const out: MessageItem[] = [];

  // The session title is the first user message in research sessions —
  // surface it as the opening prompt so the conversation reads top-down.
  const title = root.session.title;
  if (title) {
    out.push({
      id: `${root.session.session_id}__user`,
      category: "user",
      agent: "user",
      body: title,
      timestamp: root.session.started_at,
    });
  }

  // Aggregate logs across the root + every direct child session so subagent
  // intents/responses appear interleaved with the root agent's messages,
  // chronologically. Each entry keeps its agent_id so the renderer can
  // attribute it correctly.
  const allLogs = [...(root.logs ?? []), ...children.flatMap((c) => c.logs ?? [])];
  for (const log of allLogs) {
    if (!RENDERED_CATEGORIES.has(log.category)) continue;
    out.push({
      id: log.id,
      category: log.category as MessageItem["category"],
      agent: log.agent_id,
      body: log.message,
      timestamp: log.timestamp,
      meta: log.metadata,
    });
  }

  out.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());
  return out;
}

// ----------------------------------------------------------------------------
// Sub-components
// ----------------------------------------------------------------------------

function MessageRow({ item }: { item: MessageItem }) {
  switch (item.category) {
    case "user":
      return (
        <div className="mc-msg mc-msg--user">
          <div className="mc-msg__from">User</div>
          <div className="mc-msg__bubble">{item.body}</div>
        </div>
      );
    case "intent": {
      const primary = stringify(item.meta?.primary_intent);
      // ward_recommendation can be a plain string ("auth-system") or an
      // object like { ward_name, action, reason, structure, … } — pluck the
      // ward_name when it's an object so we never hand a raw object to React.
      const wardField = item.meta?.ward_recommendation;
      const ward = wardField && typeof wardField === "object" && "ward_name" in wardField
        ? stringify((wardField as Record<string, unknown>).ward_name)
        : stringify(wardField);
      return (
        <div className="mc-msg mc-msg--intent">
          <div className="mc-msg__from"><Brain size={11} /> Intent · {item.agent}</div>
          <div className="mc-msg__bubble">
            <p>{item.body}</p>
            {(primary || ward) && (
              <div className="mc-msg__intent-meta">
                {primary && <span><strong>primary:</strong> {primary}</span>}
                {ward && <span><strong>ward:</strong> {ward}</span>}
              </div>
            )}
          </div>
        </div>
      );
    }
    case "delegation":
      return (
        <div className="mc-msg mc-msg--delegation">
          <div className="mc-msg__from"><GitBranch size={11} /> Delegation · {item.agent}</div>
          <div className="mc-msg__bubble">{item.body}</div>
        </div>
      );
    case "error":
      return (
        <div className="mc-msg mc-msg--error">
          <div className="mc-msg__from"><AlertCircle size={11} /> Error · {item.agent}</div>
          <div className="mc-msg__bubble">{item.body}</div>
        </div>
      );
    case "response":
    default:
      return (
        <div className="mc-msg mc-msg--assistant">
          <div className="mc-msg__from"><MessageSquare size={11} /> z-Bot · {item.agent}</div>
          <div className="mc-msg__bubble">{item.body}</div>
        </div>
      );
  }
}

function LiveBadge({ active }: { active: boolean }) {
  if (!active) return <span className="mc-pane__live mc-pane__live--off">Cached</span>;
  return <span className="mc-pane__live">Live</span>;
}

function Empty({ message }: { message: string }) {
  return <div className="mc-pane__empty">{message}</div>;
}

/** Coerce any metadata field to a renderable string (defensive — log meta is loose). */
function stringify(v: unknown): string {
  if (v === null || v === undefined) return "";
  if (typeof v === "string") return v;
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  return "";
}
