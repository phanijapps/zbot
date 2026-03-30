// ============================================================================
// SESSION BAR
// Top bar showing session status, title, agent, metrics, and stop button
// ============================================================================

export interface SessionBarProps {
  title: string;
  agentId: string;
  status: "running" | "completed" | "error" | "idle";
  tokenCount: number;
  durationMs: number;
  modelName?: string;
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

/**
 * SessionBar — status dot + title + agent badge + spacer + metrics + stop button.
 */
export function SessionBar({
  title,
  agentId,
  status,
  tokenCount,
  durationMs,
  modelName,
  onStop,
  onNewSession,
}: SessionBarProps) {
  const statusClass = `session-bar__status session-bar__status--${status}`;

  return (
    <div className="mission-control__session-bar">
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

      {/* New Session button */}
      {onNewSession && status !== "running" && (
        <button className="btn btn--ghost btn--sm" onClick={onNewSession}>
          + New
        </button>
      )}

      {/* Stop button - only shown when running */}
      {status === "running" && onStop && (
        <button className="btn btn--destructive btn--sm" onClick={onStop}>
          Stop
        </button>
      )}
    </div>
  );
}
