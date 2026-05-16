// ============================================================================
// BeliefCard — single belief row. Shows subject, content preview,
// confidence, source-fact count, stale + contradiction indicators.
// ============================================================================

import type { Belief, BeliefContradiction } from "../types.beliefs";
import { ContradictionBadge } from "./ContradictionBadge";

interface Props {
  belief: Belief;
  contradictions?: BeliefContradiction[];
  onClick?: () => void;
}

const CONTENT_PREVIEW_CHARS = 220;

export function BeliefCard({ belief, contradictions, onClick }: Props) {
  const content =
    belief.content.length > CONTENT_PREVIEW_CHARS
      ? `${belief.content.slice(0, CONTENT_PREVIEW_CHARS)}…`
      : belief.content;

  return (
    <div
      className="belief-card"
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") onClick?.();
      }}
    >
      <div className="belief-card__head">
        <span className="belief-card__subject">{belief.subject}</span>
        {belief.stale ? (
          <span
            className="belief-badge belief-badge--stale"
            title="Stale — pending re-synthesis"
            aria-label="Stale belief"
          >
            stale
          </span>
        ) : null}
        {contradictions && contradictions.length > 0 ? (
          <ContradictionBadge contradictions={contradictions} />
        ) : null}
      </div>
      <p className="belief-card__content">{content}</p>
      <div className="belief-card__meta">
        <ConfidenceBar value={belief.confidence} />
        <span className="belief-card__sources">
          {belief.source_fact_ids.length} source
          {belief.source_fact_ids.length === 1 ? "" : "s"}
        </span>
        <span className="belief-card__age">
          {new Date(belief.updated_at).toLocaleDateString()}
        </span>
      </div>
    </div>
  );
}

function ConfidenceBar({ value }: { value: number }) {
  const pct = Math.max(0, Math.min(1, value)) * 100;
  return (
    <span
      className="belief-confidence"
      title={`Confidence ${value.toFixed(2)}`}
      aria-label={`Confidence ${value.toFixed(2)}`}
    >
      <span className="belief-confidence__track">
        <span
          className="belief-confidence__fill"
          style={{ width: `${pct}%` }}
        />
      </span>
      <span className="belief-confidence__num">{value.toFixed(2)}</span>
    </span>
  );
}
