import type { PillState } from "./types";
import "./status-pill.css";

export interface StatusPillProps {
  state: PillState;
}

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
      <span className="status-pill__dot" aria-hidden="true" />
      <span className="status-pill__narration">{state.narration}</span>
      {state.suffix && <span className="status-pill__suffix">{state.suffix}</span>}
    </div>
  );
}
