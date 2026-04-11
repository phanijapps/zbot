import { useEffect } from 'react';
import type { LogSession, SessionDetail } from '../../services/transport/types';
import { MiniWaterfall } from './MiniWaterfall';
import { SessionWaterfall } from './SessionWaterfall';
import { ErrorCallout } from './ErrorCallout';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60_000)}m ${Math.floor((ms % 60_000) / 1000)}s`;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

// ---------------------------------------------------------------------------
// SessionRow
// ---------------------------------------------------------------------------

interface SessionRowProps {
  session: LogSession;
  childSessions: LogSession[];
  isExpanded: boolean;
  onToggle: () => void;
  detail: SessionDetail | null;
  onLoadDetail: (sessionId: string) => void;
}

export function SessionRow({
  session,
  childSessions,
  isExpanded,
  onToggle,
  detail,
  onLoadDetail,
}: SessionRowProps) {
  // Load detail on first expand
  useEffect(() => {
    if (isExpanded && detail === null) {
      onLoadDetail(session.session_id);
    }
  }, [isExpanded, detail, onLoadDetail, session.session_id]);

  const handleClick = () => {
    onToggle();
  };

  const rowClassName = isExpanded
    ? 'session-row session-row--expanded'
    : 'session-row';

  const statusClassName = `session-row__status session-row__status--${session.status}`;

  const title = session.title
    || session.agent_name
    || session.session_id.slice(0, 12);

  const delegationCount = childSessions.length;

  const errorMetricClass =
    session.error_count > 0
      ? 'session-row__metric session-row__metric--error'
      : 'session-row__metric';

  // Error logs from detail (for expanded view)
  const errorLogs = detail?.logs.filter((l) => l.level === 'error') ?? [];

  return (
    <div>
      {/* Collapsed row */}
      <div className={rowClassName} onClick={handleClick} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") handleClick(); }}>
        {/* Expand arrow */}
        <span style={{ fontSize: '10px', width: '12px', flexShrink: 0, userSelect: 'none' }}>
          {isExpanded ? '\u25BC' : '\u25B6'}
        </span>

        {/* Status dot */}
        <span className={statusClassName} />

        {/* Title */}
        <span className="session-row__title">{title}</span>

        {/* Agent */}
        <span className="session-row__agent">{session.agent_name}</span>

        {/* MiniWaterfall */}
        <div className="session-row__waterfall">
          <MiniWaterfall
            startedAt={session.started_at}
            endedAt={session.ended_at}
            durationMs={session.duration_ms}
            status={session.status}
            childCount={childSessions.length}
          />
        </div>

        {/* Metrics */}
        <span className="session-row__metric">
          {session.duration_ms != null ? formatDuration(session.duration_ms) : '--'}
        </span>
        <span className="session-row__metric">
          {delegationCount > 0 ? `${delegationCount} del` : '--'}
        </span>
        <span className="session-row__metric">
          {formatTokens(session.token_count)} tok
        </span>
        <span className={errorMetricClass}>
          {session.error_count > 0 ? `${session.error_count} err` : '--'}
        </span>
      </div>

      {/* Expanded detail */}
      {isExpanded && (
        <div>
          {detail === null ? (
            /* Loading spinner while detail loads */
            <div className="waterfall">
              <span className="loading-spinner" />
            </div>
          ) : (
            <>
              {/* Session summary */}
              <div style={{ padding: 'var(--spacing-3) var(--spacing-4)', borderTop: '1px solid var(--border)', fontSize: 'var(--text-sm)' }}>
                {/* User's request */}
                {detail.session.title && (
                  <div style={{ marginBottom: 'var(--spacing-2)' }}>
                    <span style={{ color: 'var(--muted-foreground)', fontSize: 'var(--text-xs)' }}>Request: </span>
                    <span>{detail.session.title}</span>
                  </div>
                )}
                {/* Quick stats row */}
                <div style={{ display: 'flex', gap: 'var(--spacing-4)', fontSize: 'var(--text-xs)', color: 'var(--muted-foreground)' }}>
                  <span>Duration: {formatDuration(session.duration_ms || 0)}</span>
                  <span>Tool calls: {session.tool_call_count}</span>
                  <span>Delegations: {childSessions.length}</span>
                  <span>Tokens: {formatTokens(session.token_count)}</span>
                </div>
              </div>

              {/* Full waterfall timeline */}
              <SessionWaterfall
                session={detail.session}
                childSessions={childSessions}
                logs={detail.logs}
              />

              {/* Error callouts */}
              {errorLogs.map((log) => (
                <ErrorCallout
                  key={log.id}
                  timestamp={log.timestamp}
                  message={log.message}
                />
              ))}
            </>
          )}
        </div>
      )}
    </div>
  );
}
