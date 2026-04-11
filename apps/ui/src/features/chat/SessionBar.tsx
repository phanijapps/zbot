// ============================================================================
// SESSION BAR
// Top bar showing session status, title, agent, metrics, and stop button.
// Includes session history dropdown and always-visible "+ New" button.
// ============================================================================

import { useState, useRef, useEffect } from "react";
import { Clock, Plus } from "lucide-react";
import type { LogSession } from "@/services/transport/types";
import { timeAgo, switchToSession } from "./mission-hooks";

export interface SessionBarProps {
  title: string;
  agentId: string;
  status: "running" | "completed" | "error" | "idle";
  tokenCount: number;
  durationMs: number;
  modelName?: string;
  recentSessions?: LogSession[];
  currentSessionId?: string | null;
  onStop?: () => void;
  onNewSession?: () => void;
}

/** Format milliseconds to a human-readable duration */
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const mins = Math.floor(ms / 60_000);
  const secs = Math.round((ms % 60_000) / 1000);
  return `${mins}m ${secs}s`;
}

/** Format token count with K suffix */
function formatTokens(count: number): string {
  if (count >= 1000) return `${(count / 1000).toFixed(1)}K`;
  return String(count);
}

/** Status dot color class for a session */
function sessionStatusClass(status: string): string {
  if (status === "completed") return "session-history__dot--completed";
  if (status === "error" || status === "crashed" || status === "stopped")
    return "session-history__dot--error";
  return "session-history__dot--running";
}

/**
 * SessionBar — "+ New" + history icon + status dot + title + agent badge + spacer + metrics + stop button.
 */
export function SessionBar({
  title,
  agentId,
  status,
  tokenCount,
  durationMs,
  modelName,
  recentSessions = [],
  currentSessionId,
  onStop,
  onNewSession,
}: SessionBarProps) {
  const statusClass = `session-bar__status session-bar__status--${status}`;
  const [historyOpen, setHistoryOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown on outside click
  useEffect(() => {
    if (!historyOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setHistoryOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [historyOpen]);

  return (
    <div className="mission-control__session-bar">
      {/* Always-visible New Session button */}
      {onNewSession && (
        <button className="btn btn--ghost btn--sm session-bar__new-btn" onClick={onNewSession}>
          <Plus style={{ width: 14, height: 14 }} />
          New
        </button>
      )}

      {/* Session history dropdown */}
      <div className="session-history" ref={dropdownRef}>
        <button
          className="btn btn--icon-ghost session-bar__history-btn"
          title="Session history"
          onClick={() => setHistoryOpen((prev) => !prev)}
        >
          <Clock style={{ width: 16, height: 16 }} />
        </button>

        {historyOpen && (
          <div className="session-history__dropdown">
            {onNewSession && (
              <div
                className="session-history__new"
                onClick={() => {
                  setHistoryOpen(false);
                  onNewSession();
                }}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { setHistoryOpen(false); onNewSession(); } }}
              >
                <Plus style={{ width: 14, height: 14 }} />
                New Session
              </div>
            )}

            {recentSessions.length === 0 && (
              <div className="session-history__empty">No recent sessions</div>
            )}

            {recentSessions.map((s) => {
              const isActive = s.session_id === currentSessionId;
              const displayTitle =
                s.title?.slice(0, 40) || "Untitled";
              return (
                <div
                  key={s.session_id}
                  className={`session-history__item${isActive ? " session-history__item--active" : ""}`}
                  onClick={() => {
                    setHistoryOpen(false);
                    if (!isActive) {
                      switchToSession(s.session_id, s.conversation_id);
                    }
                  }}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { setHistoryOpen(false); if (!isActive) switchToSession(s.session_id, s.conversation_id); } }}
                >
                  <span className={`session-history__dot ${sessionStatusClass(s.status)}`} />
                  <span className="session-history__title">{displayTitle}</span>
                  <span className="session-history__time">{timeAgo(s.started_at)}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className={statusClass} />
      <span className="session-bar__title">{title || "New Session"}</span>
      <span className="session-bar__badge">{agentId}</span>
      {status === "running" && (
        <span style={{ fontSize: "var(--text-xs)", color: "var(--success)", fontWeight: 500 }}>Processing...</span>
      )}

      {/* Spacer */}
      <div style={{ flex: 1 }} />

      {/* Metrics */}
      <span className="session-bar__metric">{formatTokens(tokenCount)} tok</span>
      <span className="session-bar__metric">{formatDuration(durationMs)}</span>
      {modelName && <span className="session-bar__metric">{modelName}</span>}

      {/* Stop button - only shown when running */}
      {status === "running" && onStop && (
        <button className="btn btn--destructive btn--sm" onClick={onStop}>
          Stop
        </button>
      )}
    </div>
  );
}
