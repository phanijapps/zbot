import { useCallback, useEffect, useState } from "react";
import { useLocation, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { Menu, PanelRight } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { SessionWithExecutions } from "@/services/transport/types";
import { QuickChat } from "../chat-v2/QuickChat";
import { ChatConversation, ResearchConversation, type PendingMessage } from "./Conversations";
import { sessionMode, type SessionMode } from "./mode";
import { DesktopRail } from "./DesktopRail";
import { useRecentSessions } from "./useRecentSessions";

export function SessionShell({ initialSessionId }: {initialSessionId?: string} = {}) {
  const { sessionId: routeSessionId } = useParams<{sessionId: string}>();
  const [search] = useSearchParams();
  const navigate = useNavigate();
  const location = useLocation();
  const [selection, setSelection] = useState<string | null | undefined>();
  const sessionId = selection === undefined ? routeSessionId ?? initialSessionId : selection ?? undefined;
  const [record, setRecord] = useState<SessionWithExecutions | null>(null);
  const [loading, setLoading] = useState(Boolean(sessionId));
  const [active, setActive] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const routedPending = (location.state as { shellPendingMessage?: PendingMessage } | null)?.shellPendingMessage;
  const [navigationOpen, setNavigationOpen] = useState(false);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const mode: SessionMode = sessionId ? sessionMode(record?.id === sessionId ? record.mode : undefined) : search.get("mode") === "research" ? "research" : "chat";
  const opening = Boolean(sessionId && record?.id !== sessionId && !error);
  const locked = loading || opening || busy || active;
  const {recents, unavailable: recentsUnavailable} = useRecentSessions(`${sessionId ?? ""}:${active}`);
  const returnTo = sessionId ? `/session/${encodeURIComponent(sessionId)}` : mode === "research" ? "/session?mode=research" : "/session";

  useEffect(() => { setSelection(undefined); }, [routeSessionId]);
  useEffect(() => {
    let cancelled = false;
    setRecord(null); setError(null); setLoading(Boolean(sessionId));
    if (!sessionId) return;
    setActive(false);
    void (async () => {
      const transport = await getTransport();
      const response = await transport.getSessionFull(sessionId);
      if (cancelled) return;
      if (!response.success || !response.data || response.data.id !== sessionId || response.data.parent_session_id) {
        setError("This conversation is unavailable.");
      } else {
        setRecord(response.data);
        setActive(response.data.executions.some(execution => !["completed", "cancelled", "crashed"].includes(execution.status)));
      }
      setLoading(false);
    })().catch(() => { if (!cancelled) { setError("This conversation is unavailable."); setLoading(false); } });
    return () => { cancelled = true; };
  }, [sessionId]);
  const reportActive = useCallback((value: boolean | undefined) => { if (value !== undefined) setActive(value); }, []);
  const sent = useCallback(() => {
    if (routedPending) navigate(location.pathname + location.search, {replace: true, state: null});
  }, [navigate, location.pathname, location.search, routedPending]);
  const start = async (next: "chat" | "research") => {
    if (locked) return;
    setError(null); setBusy(true); setNavigationOpen(false);
    try {
      if (next === "chat") {
        const transport = await getTransport();
        const created = await transport.createChatSession();
        if (!created.success || !created.data) { setError("Couldn't start a new chat. Your conversations are unchanged."); return; }
        setSelection(created.data.sessionId);
        navigate(`/session/${encodeURIComponent(created.data.sessionId)}`);
      } else {
        setSelection(null); setActive(false);
        navigate("/session?mode=research");
      }
    } catch { setError("Couldn't start a conversation. Your conversations are unchanged."); }
    finally { setBusy(false); }
  };

  return <div className={`session-shell${navigationOpen ? " session-shell--navigation-open" : ""}${detailsOpen ? " session-shell--details-open" : ""}`}>
    <DesktopRail returnTo={returnTo} recents={recents} unavailable={recentsUnavailable} sessionId={sessionId} locked={locked}
      onNewChat={() => void start("chat")} onClose={() => setNavigationOpen(false)}
      onSelect={id => { setSelection(id); setNavigationOpen(false); navigate(`/session/${encodeURIComponent(id)}`); }} />
    <main className="session-shell__main">
      <header className="session-shell__header">
        <button className="session-shell__navigation-toggle btn btn--icon-ghost" aria-label="Open navigation" aria-expanded={navigationOpen} onClick={() => setNavigationOpen(!navigationOpen)}><Menu size={18} /></button>
        <div className="session-shell__mode" role="tablist" aria-label="Conversation mode">
          {(["chat", "research"] as const).map(item => <button key={item} role="tab" type="button" disabled={locked} aria-selected={mode === item}
            onClick={() => {
              if (item === "chat" && (mode !== "chat" || sessionId)) {
                setSelection(null); setActive(false); setError(null); navigate("/session");
              } else if (item !== mode) void start(item);
            }}
            onKeyDown={event => { if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); const tabs = event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>("button"); tabs?.[item === "chat" ? 1 : 0]?.focus(); } }}>
            {item === "chat" ? "Chat" : "Research"}
          </button>)}
        </div>
        <button className="session-shell__details-toggle btn btn--icon-ghost" aria-label="Toggle session details" aria-expanded={detailsOpen} onClick={() => setDetailsOpen(!detailsOpen)}><PanelRight size={18} /></button>
      </header>
      {error && <p className="session-shell__alert" role="alert">{error}</p>}
      {loading || opening ? <p className="session-shell__hint" role="status">Opening conversation…</p>
        : error && sessionId && !record ? null
        : mode === "unknown" ? <div className="session-shell__empty"><h1>This session's mode is unknown.</h1><p>Choose Chat or Research to start a new conversation. This one will be kept.</p></div>
        : mode === "research" ? <ResearchConversation key={sessionId ?? "new-research"} sessionId={sessionId} onActive={reportActive} />
        : sessionId ? <ChatConversation key={sessionId} sessionId={sessionId} pending={routedPending ?? null} onActive={reportActive} onSent={sent} />
        : <QuickChat onActive={reportActive} />}
    </main>
    <aside className="session-shell__details" aria-label="Session details">
      <h2>Workspace</h2>
      <div className="tab-bar" role="tablist" aria-label="Session detail panels">
        {["Activity", "Sources", "Files"].map((name, index) => <button key={name} className="tab-bar__tab" role="tab" aria-selected={index === 0}>{name}</button>)}
      </div>
      <p className="session-shell__hint">Select a conversation to inspect its recorded details.</p>
      <button className="session-shell__mobile-close btn btn--ghost" onClick={() => setDetailsOpen(false)}>Close details</button>
    </aside>
  </div>;
}
