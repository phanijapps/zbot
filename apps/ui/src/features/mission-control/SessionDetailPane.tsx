// ============================================================================
// MISSION CONTROL — SessionDetailPane
// Right side of the page: header (title, status, meta, actions) + dual-pane
// grid hosting MessagesPane and ToolsPane side-by-side.
// ============================================================================

import { useNavigate } from "react-router-dom";
import { Pause, Square, RotateCcw, ExternalLink } from "lucide-react";
import type { LogSession } from "@/services/transport/types";
import { formatDuration } from "../logs/trace-types";
import { MessagesPane } from "./MessagesPane";
import { ToolsPane } from "./ToolsPane";
import { TokenPair } from "./SessionListPanel";
import type { SessionTokenIndex } from "./useSessionTokens";

interface SessionDetailPaneProps {
  session: LogSession | null;
  /** Optional — when supplied, header shows in/out tokens, ToolsPane shows
   *  per-execution tokens in each agent group header. */
  tokenIndex?: SessionTokenIndex;
}

export function SessionDetailPane({ session, tokenIndex }: SessionDetailPaneProps) {
  const navigate = useNavigate();

  if (!session) {
    return (
      <section className="session-detail-pane session-detail-pane--empty">
        <div className="session-detail-pane__placeholder">
          <p>Select a session from the list to inspect its messages and tool calls.</p>
        </div>
      </section>
    );
  }

  const status = session.status;
  const isRunning = status === "running";
  const title = session.title || session.agent_name || session.session_id;
  const duration = formatDuration(session.duration_ms);
  const sessionTokens = tokenIndex?.byRootExecId.get(session.session_id);

  return (
    <section className="session-detail-pane">
      <header className="session-detail-pane__head">
        <div className="session-detail-pane__title">
          #{shortId(session.session_id)} · {title}
        </div>
        <div className="session-detail-pane__meta">
          <span className={`session-detail-pane__status session-detail-pane__status--${status}`}>
            ● {status}
          </span>
          {duration && (
            <>
              <span>·</span>
              <span><strong>{duration}</strong> elapsed</span>
            </>
          )}
          <span>·</span>
          <span>{session.agent_name}</span>
          {sessionTokens && sessionTokens.total > 0 && (
            <>
              <span>·</span>
              <TokenPair inTok={sessionTokens.in} outTok={sessionTokens.out} />
            </>
          )}
        </div>
        <div className="session-detail-pane__actions">
          <button
            type="button"
            className="btn btn--secondary btn--sm"
            disabled={!isRunning}
            title="Pause session"
          >
            <Pause size={14} /> Pause
          </button>
          <button
            type="button"
            className="btn btn--destructive btn--sm"
            disabled={!isRunning}
            title="Stop session"
          >
            <Square size={14} /> Stop
          </button>
          <button
            type="button"
            className="btn btn--secondary btn--sm"
            title="Retry session"
          >
            <RotateCcw size={14} /> Retry
          </button>
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            title="Open in Research"
            onClick={() => navigate(`/research/${session.session_id}`)}
          >
            <ExternalLink size={14} /> Open in Research
          </button>
        </div>
      </header>
      <div className="session-detail-pane__panes">
        <MessagesPane session={session} />
        <ToolsPane session={session} tokenIndex={tokenIndex} />
      </div>
    </section>
  );
}

function shortId(id: string): string {
  return id.length > 8 ? id.slice(-6) : id;
}
