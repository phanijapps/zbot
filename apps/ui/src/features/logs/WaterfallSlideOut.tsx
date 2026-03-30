import type { ExecutionLog, LogSession } from '../../services/transport/types';
import { formatDurationCompact, formatTimestamp, parseToolMetadata, extractInputFromMessage } from './waterfall-utils';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SlideOutData {
  type: 'tool' | 'delegation' | 'error';
  log?: ExecutionLog;
  childSession?: LogSession;
  childLogs?: ExecutionLog[];
  /** Index within the navigable items list */
  index: number;
}

interface WaterfallSlideOutProps {
  data: SlideOutData;
  /** Total count of navigable items (for prev/next) */
  totalItems: number;
  /** Surrounding logs for context (error view) */
  surroundingLogs: ExecutionLog[];
  onClose: () => void;
  onNavigate: (direction: 'prev' | 'next') => void;
}

// ---------------------------------------------------------------------------
// WaterfallSlideOut
// ---------------------------------------------------------------------------

export function WaterfallSlideOut({
  data,
  totalItems,
  surroundingLogs,
  onClose,
  onNavigate,
}: WaterfallSlideOutProps) {
  return (
    <div className="waterfall-slideout">
      {data.type === 'delegation' && data.childSession
        ? <DelegationPanel session={data.childSession} childLogs={data.childLogs} onClose={onClose} />
        : data.type === 'error' && data.log
          ? <ErrorPanel log={data.log} surroundingLogs={surroundingLogs} onClose={onClose} />
          : data.log
            ? <ToolPanel log={data.log} onClose={onClose} />
            : null}

      {/* Navigation */}
      <div className="waterfall-slideout__nav">
        <button
          onClick={() => onNavigate('prev')}
          disabled={data.index <= 0}
        >
          &#9664; Previous
        </button>
        <span className="waterfall-slideout__nav-counter">
          {data.index + 1} / {totalItems}
        </span>
        <button
          onClick={() => onNavigate('next')}
          disabled={data.index >= totalItems - 1}
        >
          Next &#9654;
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Tool Panel
// ---------------------------------------------------------------------------

function ToolPanel({ log, onClose }: { log: ExecutionLog; onClose: () => void }) {
  const meta = parseToolMetadata(log);
  const duration = log.duration_ms != null ? formatDurationCompact(log.duration_ms) : '--';
  const time = formatTimestamp(log.timestamp);

  // Derive input: prefer metadata, fall back to message extraction
  const input = meta.input || extractInputFromMessage(log.message);

  // Derive status label
  const isSuccess = meta.exitCode === 0 || meta.status === 'success';
  const isFailed = meta.exitCode != null ? meta.exitCode !== 0 : meta.status === 'error';
  const statusLabel = isSuccess ? '✓ success' : isFailed ? '✗ failed' : meta.status || '--';
  const statusClass = isSuccess
    ? 'waterfall-slideout__status--success'
    : isFailed
      ? 'waterfall-slideout__status--error'
      : '';

  return (
    <>
      <div className="waterfall-slideout__header">
        <span>{'🔧'} Tool Call: {meta.toolName}</span>
        <button className="waterfall-slideout__close" onClick={onClose} aria-label="Close">&times;</button>
      </div>

      <div className="waterfall-slideout__section">
        <div className="waterfall-slideout__meta-row">
          <span className="waterfall-slideout__label">Timestamp</span>
          <span>{time}</span>
        </div>
        <div className="waterfall-slideout__meta-row">
          <span className="waterfall-slideout__label">Duration</span>
          <span>{duration}</span>
        </div>
        <div className="waterfall-slideout__meta-row">
          <span className="waterfall-slideout__label">Status</span>
          <span className={statusClass}>{statusLabel}</span>
        </div>
      </div>

      {input && (
        <div className="waterfall-slideout__section">
          <div className="waterfall-slideout__label">Command</div>
          <div className="waterfall-slideout__code">{input}</div>
        </div>
      )}

      {meta.output && (
        <div className="waterfall-slideout__section">
          <div className="waterfall-slideout__label">Output</div>
          <div className="waterfall-slideout__code">{meta.output}</div>
        </div>
      )}
    </>
  );
}

// ---------------------------------------------------------------------------
// Delegation Panel
// ---------------------------------------------------------------------------

function DelegationPanel({
  session,
  childLogs,
  onClose,
}: {
  session: LogSession;
  childLogs?: ExecutionLog[];
  onClose: () => void;
}) {
  const duration = session.duration_ms != null ? formatDurationCompact(session.duration_ms) : '--';
  const toolCallCount = childLogs
    ? childLogs.filter((l) => l.category === 'tool_call').length
    : session.tool_call_count;

  const statusLabel = session.status === 'completed'
    ? '✓ completed'
    : session.status === 'error'
      ? '✗ error'
      : session.status;
  const statusClass = session.status === 'completed'
    ? 'waterfall-slideout__status--success'
    : session.status === 'error'
      ? 'waterfall-slideout__status--error'
      : '';

  // Key events: up to 10 significant logs
  const keyEvents = childLogs
    ? childLogs
        .filter((l) => l.category === 'tool_call' || l.category === 'delegation' || l.level === 'error' || l.level === 'warn')
        .slice(0, 10)
    : [];

  return (
    <>
      <div className="waterfall-slideout__header">
        <span>{'🔀'} Delegation: {session.agent_name}</span>
        <button className="waterfall-slideout__close" onClick={onClose} aria-label="Close">&times;</button>
      </div>

      {session.title && (
        <div className="waterfall-slideout__section">
          <div className="waterfall-slideout__label">Task</div>
          <div>{session.title}</div>
        </div>
      )}

      <div className="waterfall-slideout__section">
        <div className="waterfall-slideout__meta-row">
          <span className="waterfall-slideout__label">Duration</span>
          <span>{duration}</span>
        </div>
        <div className="waterfall-slideout__meta-row">
          <span className="waterfall-slideout__label">Status</span>
          <span className={statusClass}>{statusLabel}</span>
        </div>
        <div className="waterfall-slideout__meta-row">
          <span className="waterfall-slideout__label">Tool calls</span>
          <span>{toolCallCount}</span>
        </div>
        <div className="waterfall-slideout__meta-row">
          <span className="waterfall-slideout__label">Tokens</span>
          <span>{formatTokensCompact(session.token_count)}</span>
        </div>
      </div>

      {keyEvents.length > 0 && (
        <div className="waterfall-slideout__section">
          <div className="waterfall-slideout__label">Key Events</div>
          {keyEvents.map((log) => (
            <div key={log.id} className="waterfall-slideout__event">
              <span className="waterfall-slideout__event-time">
                {formatTimestamp(log.timestamp)}
              </span>
              <span className={log.level === 'error' ? 'waterfall-slideout__event--error' : log.level === 'warn' ? 'waterfall-slideout__event--warn' : ''}>
                {log.level === 'error' ? '❌ ' : log.level === 'warn' ? '⚠ ' : ''}{log.message}
              </span>
            </div>
          ))}
        </div>
      )}
    </>
  );
}

// ---------------------------------------------------------------------------
// Error Panel
// ---------------------------------------------------------------------------

function ErrorPanel({
  log,
  surroundingLogs,
  onClose,
}: {
  log: ExecutionLog;
  surroundingLogs: ExecutionLog[];
  onClose: () => void;
}) {
  const meta = parseToolMetadata(log);
  const time = formatTimestamp(log.timestamp);

  return (
    <>
      <div className="waterfall-slideout__header waterfall-slideout__header--error">
        <span>{'❌'} Error</span>
        <button className="waterfall-slideout__close" onClick={onClose} aria-label="Close">&times;</button>
      </div>

      <div className="waterfall-slideout__section">
        <div className="waterfall-slideout__meta-row">
          <span className="waterfall-slideout__label">Time</span>
          <span>{time}</span>
        </div>
        {meta.toolName !== log.category && (
          <div className="waterfall-slideout__meta-row">
            <span className="waterfall-slideout__label">Tool</span>
            <span>{meta.toolName}</span>
          </div>
        )}
      </div>

      <div className="waterfall-slideout__section">
        <div className="waterfall-slideout__label">Error Message</div>
        <div className="waterfall-slideout__code waterfall-slideout__code--error">
          {log.message}
        </div>
      </div>

      {meta.errorDetail && (
        <div className="waterfall-slideout__section">
          <div className="waterfall-slideout__label">Detail</div>
          <div className="waterfall-slideout__code waterfall-slideout__code--error">
            {meta.errorDetail}
          </div>
        </div>
      )}

      {surroundingLogs.length > 0 && (
        <div className="waterfall-slideout__section">
          <div className="waterfall-slideout__label">Context (surrounding log entries)</div>
          {surroundingLogs.map((entry) => (
            <div
              key={entry.id}
              className={`waterfall-slideout__event ${entry.id === log.id ? 'waterfall-slideout__event--highlight' : ''}`}
            >
              <span className="waterfall-slideout__event-time">
                {formatTimestamp(entry.timestamp)}
              </span>
              <span className={entry.level === 'error' ? 'waterfall-slideout__event--error' : ''}>
                {entry.level === 'error' ? '❌ ' : ''}{entry.message}
              </span>
            </div>
          ))}
        </div>
      )}
    </>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTokensCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}
