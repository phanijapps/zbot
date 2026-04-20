import type { PillState } from "./types";
import "./status-pill.css";

export interface StatusPillProps {
  state: PillState;
}

/**
 * Two-row status indicator.
 *
 * Top row: verb narration ("Thinking…", "Running shell", "Responding")
 * rendered on the paper surface with a category-coloured pulse dot.
 *
 * Bottom row: terminal-styled command preview, rendered only when
 * `suffix` is present. Dark surface via `--sidebar`, monospace, green
 * `$` prompt. Truncated with ellipsis; full command lives in Logs.
 *
 * `swapCounter` as `key` re-triggers the fade on every content swap.
 */
export function StatusPill({ state }: StatusPillProps) {
  if (!state.visible) return null;

  return (
    <div
      data-testid="status-pill"
      data-category={state.category}
      className="status-pill"
      key={state.swapCounter}
      aria-live="polite"
      aria-atomic="true"
    >
      <div className="status-pill__header">
        <span className="status-pill__dot" aria-hidden="true" />
        <span className="status-pill__narration">{state.narration}</span>
      </div>
      {state.suffix && (
        <div className="status-pill__terminal" data-testid="status-pill-terminal">
          <span className="status-pill__prompt" aria-hidden="true">$</span>
          <span className="status-pill__command">{state.suffix}</span>
        </div>
      )}
    </div>
  );
}
