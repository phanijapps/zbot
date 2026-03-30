export interface ToolExecutionBlockProps {
  /** Tool name (e.g., "web_fetch", "python") */
  toolName: string;
  /** Summarized input (command / args) */
  input: string;
  /** Full output text */
  output?: string;
  /** Execution duration in milliseconds */
  durationMs?: number;
  /** Whether the tool call errored */
  isError?: boolean;
  /** Whether the body is expanded */
  isExpanded: boolean;
  /** Toggle expand/collapse */
  onToggle: () => void;
}

/** Format milliseconds to a human-readable duration string */
function formatDuration(ms?: number): string {
  if (ms == null) return "";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/**
 * ToolExecutionBlock — expandable block for tool calls.
 * Header always visible: arrow + tool name (amber) + input summary + duration + status.
 * Body (when expanded): full input + output in monospace.
 * Defaults to COLLAPSED — user clicks to expand.
 */
export function ToolExecutionBlock({
  toolName,
  input,
  output,
  durationMs,
  isError,
  isExpanded,
  onToggle,
}: ToolExecutionBlockProps) {
  const blockClass = `tool-block${isError ? " tool-block--error" : ""}`;
  const statusClass = isError ? "tool-block__status--error" : "tool-block__status--success";
  const statusIcon = isError ? "\u2717" : "\u2713";
  const arrow = isExpanded ? "\u25BC" : "\u25B6";

  return (
    <div className={blockClass}>
      <div className="tool-block__header" onClick={onToggle}>
        <span className="tool-block__arrow">{arrow}</span>
        <span className="tool-block__name">{toolName}</span>
        <span className="tool-block__summary">{input}</span>
        {durationMs != null && (
          <span className="tool-block__duration">{formatDuration(durationMs)}</span>
        )}
        <span className={statusClass}>{statusIcon}</span>
      </div>
      {isExpanded && (
        <div className="tool-block__body">
          {input && (
            <>
              {"> "}{input}{"\n\n"}
            </>
          )}
          {output ?? ""}
        </div>
      )}
    </div>
  );
}
