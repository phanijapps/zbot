import type { ExecutionLog, LogSession } from '../../services/transport/types';
import { formatDurationCompact, parseToolMetadata } from './waterfall-utils';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface TooltipData {
  type: 'tool' | 'delegation' | 'error';
  x: number; // pixel position relative to waterfall container
  y: number;
  log?: ExecutionLog;
  childSession?: LogSession;
  childLogs?: ExecutionLog[];
}

interface WaterfallTooltipProps {
  data: TooltipData | null;
  containerRect: DOMRect | null;
}

// ---------------------------------------------------------------------------
// WaterfallTooltip
// ---------------------------------------------------------------------------

export function WaterfallTooltip({ data, containerRect }: WaterfallTooltipProps) {
  if (!data || !containerRect) return null;

  // Position: keep tooltip within the container bounds
  const tooltipWidth = 300;
  let left = data.x + 12;
  if (left + tooltipWidth > containerRect.width) {
    left = data.x - tooltipWidth - 12;
  }
  if (left < 0) left = 4;

  let top = data.y - 8;
  if (top < 0) top = 4;

  return (
    <div
      className="waterfall-tooltip"
      style={{ left, top }}
    >
      {data.type === 'delegation' && data.childSession
        ? <DelegationTooltip session={data.childSession} childLogs={data.childLogs} />
        : data.type === 'error' && data.log
          ? <ErrorTooltip log={data.log} />
          : data.log
            ? <ToolTooltip log={data.log} />
            : null}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function ToolTooltip({ log }: { log: ExecutionLog }) {
  const meta = parseToolMetadata(log);
  const duration = log.duration_ms != null ? formatDurationCompact(log.duration_ms) : null;

  return (
    <>
      <div className="waterfall-tooltip__header">
        <span>{'🔧'} {meta.toolName}</span>
        {duration && <span>{duration}</span>}
      </div>
      {meta.input && (
        <div className="waterfall-tooltip__body">{meta.input}</div>
      )}
      {meta.exitCode != null && (
        <div className={`waterfall-tooltip__status ${meta.exitCode === 0 ? 'waterfall-tooltip__status--success' : 'waterfall-tooltip__status--error'}`}>
          {meta.exitCode === 0 ? '✓' : '✗'} exit_code: {meta.exitCode}
        </div>
      )}
      {meta.exitCode == null && meta.status && (
        <div className={`waterfall-tooltip__status ${meta.status === 'success' ? 'waterfall-tooltip__status--success' : 'waterfall-tooltip__status--error'}`}>
          {meta.status === 'success' ? '✓' : '✗'} {meta.status}
        </div>
      )}
    </>
  );
}

function DelegationTooltip({ session, childLogs }: { session: LogSession; childLogs?: ExecutionLog[] }) {
  const duration = session.duration_ms != null ? formatDurationCompact(session.duration_ms) : null;
  const toolCallCount = childLogs
    ? childLogs.filter((l) => l.category === 'tool_call').length
    : session.tool_call_count;

  return (
    <>
      <div className="waterfall-tooltip__header">
        <span>{'🔀'} {session.agent_name}</span>
        {duration && <span>{duration}</span>}
      </div>
      {session.title && (
        <div className="waterfall-tooltip__body">{session.title}</div>
      )}
      <div className={`waterfall-tooltip__status ${session.status === 'completed' ? 'waterfall-tooltip__status--success' : session.status === 'error' ? 'waterfall-tooltip__status--error' : ''}`}>
        {session.status === 'completed' ? '✓' : session.status === 'error' ? '✗' : '◐'} {session.status}
        {toolCallCount > 0 ? ` — ${toolCallCount} tool calls` : ''}
      </div>
    </>
  );
}

function ErrorTooltip({ log }: { log: ExecutionLog }) {
  const meta = parseToolMetadata(log);
  const duration = log.duration_ms != null ? formatDurationCompact(log.duration_ms) : null;

  return (
    <>
      <div className="waterfall-tooltip__header">
        <span>{'❌'} {meta.toolName} FAILED</span>
        {duration && <span>{duration}</span>}
      </div>
      <div className="waterfall-tooltip__body">{log.message}</div>
      {meta.errorDetail && (
        <div className="waterfall-tooltip__status waterfall-tooltip__status--error">
          {meta.errorDetail}
        </div>
      )}
    </>
  );
}
