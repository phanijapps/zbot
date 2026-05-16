// ============================================================================
// ContradictionBadge — a small dot rendered on rows whose belief has at
// least one unresolved contradiction. Click opens the resolver.
// ============================================================================

import type { BeliefContradiction } from "../types.beliefs";

interface Props {
  contradictions: BeliefContradiction[];
  onClick?: () => void;
}

export function ContradictionBadge({ contradictions, onClick }: Props) {
  const unresolved = contradictions.filter((c) => !c.resolved_at);
  if (unresolved.length === 0) return null;

  const maxSeverity = unresolved.reduce(
    (acc, c) => (c.severity > acc ? c.severity : acc),
    0,
  );
  const summary = unresolved
    .map((c) => `${c.contradiction_type} (sev ${c.severity.toFixed(2)})`)
    .join(", ");
  const title = `${unresolved.length} unresolved contradiction${
    unresolved.length === 1 ? "" : "s"
  } — max severity ${maxSeverity.toFixed(2)}: ${summary}`;

  if (!onClick) {
    return (
      <span
        className="belief-badge belief-badge--contradiction"
        title={title}
        aria-label={title}
      >
        !
      </span>
    );
  }
  return (
    <button
      type="button"
      className="belief-badge belief-badge--contradiction belief-badge--clickable"
      title={title}
      aria-label={title}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
    >
      !
    </button>
  );
}
