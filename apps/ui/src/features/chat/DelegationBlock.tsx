export type DelegationStatus = "active" | "completed" | "error";

export interface DelegationBlockProps {
  /** Agent being delegated to */
  agentId: string;
  /** Task description */
  task: string;
  /** Current delegation status */
  status: DelegationStatus;
  /** Number of tool calls made by the sub-agent */
  toolCallCount?: number;
  /** Token count consumed */
  tokenCount?: number;
  /** Duration in milliseconds */
  durationMs?: number;
  /** Result text (shown when completed) */
  result?: string;
}

/** Format milliseconds to a human-readable duration string */
function formatDuration(ms?: number): string {
  if (ms == null) return "";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/** Format token count with K suffix for large numbers */
function formatTokens(count?: number): string {
  if (count == null) return "";
  if (count >= 1000) return `${(count / 1000).toFixed(1)}K tok`;
  return `${count} tok`;
}

/**
 * DelegationBlock — green-bordered block for sub-agent delegations.
 * Shows agent name, task, live stats, and result when completed.
 */
export function DelegationBlock({
  agentId,
  task,
  status,
  toolCallCount,
  tokenCount,
  durationMs,
  result,
}: DelegationBlockProps) {
  return (
    <div className="delegation-block">
      <div className="delegation-block__header">
        <div className={`delegation-block__status delegation-block__status--${status}`} />
        <span>Delegating to {agentId}</span>
      </div>
      <div className="delegation-block__task">{task}</div>
      <div className="delegation-block__stats">
        {toolCallCount != null && <span>{toolCallCount} tool calls</span>}
        {tokenCount != null && <span>{formatTokens(tokenCount)}</span>}
        {durationMs != null && <span>{formatDuration(durationMs)}</span>}
      </div>
      {status === "completed" && result && (
        <div className="delegation-block__result">{result}</div>
      )}
    </div>
  );
}
