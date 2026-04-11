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

const SHELL_TOOLS = new Set(["bash", "shell", "terminal", "execute_command"]);

/** Format milliseconds to a human-readable duration string */
function formatDuration(ms?: number): string {
  if (ms == null) return "";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/**
 * ToolExecutionBlock — expandable block for tool calls.
 *
 * Shell tools (bash, shell, terminal) render as a terminal-style block
 * with prompt + command visible by default.
 * Other tools render as a compact collapsible header.
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
  const isShell = SHELL_TOOLS.has(toolName);

  if (isShell) {
    return (
      <div className={`tool-block tool-block--shell${isError ? " tool-block--error" : ""}`}>
        <div className="tool-block__shell-header" onClick={onToggle} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onToggle(); }}>
          <span className="tool-block__shell-prompt">$</span>
          <span className="tool-block__shell-cmd">{input}</span>
          <span className="tool-block__shell-meta">
            {durationMs != null && <span className="tool-block__duration">{formatDuration(durationMs)}</span>}
            <span className={isError ? "tool-block__status--error" : "tool-block__status--success"}>
              {isError ? "\u2717" : "\u2713"}
            </span>
          </span>
        </div>
        {isExpanded && output && (
          <div className="tool-block__shell-output">{output}</div>
        )}
      </div>
    );
  }

  // Default: compact collapsible block for non-shell tools
  const arrow = isExpanded ? "\u25BC" : "\u25B6";

  return (
    <div className={`tool-block${isError ? " tool-block--error" : ""}`}>
      <div className="tool-block__header" onClick={onToggle} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onToggle(); }}>
        <span className="tool-block__arrow">{arrow}</span>
        <span className="tool-block__name">{toolName}</span>
        <span className="tool-block__summary">{input}</span>
        {durationMs != null && (
          <span className="tool-block__duration">{formatDuration(durationMs)}</span>
        )}
        <span className={isError ? "tool-block__status--error" : "tool-block__status--success"}>
          {isError ? "\u2717" : "\u2713"}
        </span>
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
