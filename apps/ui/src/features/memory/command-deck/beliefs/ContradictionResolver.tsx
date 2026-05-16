// ============================================================================
// ContradictionResolver — modal with side-by-side belief A and belief B
// and three resolution buttons.
// ============================================================================

import { useEffect, useState } from "react";
import type {
  Belief,
  BeliefContradiction,
  ContradictionResolution,
} from "../types.beliefs";
import { getBeliefDetail, resolveContradiction } from "./api";

interface Props {
  agentId: string;
  contradiction: BeliefContradiction;
  onClose: () => void;
  onResolved: () => void;
}

type Resolved = "a_won" | "b_won" | "compatible";

const RESOLUTIONS: Array<{ value: Resolved; label: string }> = [
  { value: "a_won", label: "A wins" },
  { value: "b_won", label: "B wins" },
  { value: "compatible", label: "Mark compatible" },
];

export function ContradictionResolver({
  agentId,
  contradiction,
  onClose,
  onResolved,
}: Props) {
  const [a, setA] = useState<Belief | null>(null);
  const [b, setB] = useState<Belief | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const isResolved = Boolean(contradiction.resolved_at);

  useEffect(() => {
    let alive = true;
    (async () => {
      const [da, db] = await Promise.all([
        getBeliefDetail(agentId, contradiction.belief_a_id),
        getBeliefDetail(agentId, contradiction.belief_b_id),
      ]);
      if (!alive) return;
      if (da.success && da.data) setA(da.data.belief);
      if (db.success && db.data) setB(db.data.belief);
    })();
    return () => {
      alive = false;
    };
  }, [agentId, contradiction.belief_a_id, contradiction.belief_b_id]);

  async function submit(resolution: ContradictionResolution) {
    if (submitting || isResolved) return;
    setSubmitting(true);
    setError(null);
    const res = await resolveContradiction(contradiction.id, resolution);
    setSubmitting(false);
    if (res.success) {
      onResolved();
    } else {
      setError(res.error ?? "Failed to resolve contradiction");
    }
  }

  return (
    <>
      <div
        className="modal-backdrop"
        role="button"
        tabIndex={0}
        aria-label="Close contradiction resolver"
        onClick={onClose}
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
        }}
      />
      <div
        className="modal modal--lg contradiction-resolver"
        role="dialog"
        aria-label="Resolve contradiction"
      >
        <header className="modal__header">
          <div>
            <strong>{contradiction.contradiction_type}</strong> · severity{" "}
            {contradiction.severity.toFixed(2)}
          </div>
          <button
            type="button"
            className="slideover__close"
            aria-label="Close"
            onClick={onClose}
          >
            ×
          </button>
        </header>
        <div className="modal__body">
          {contradiction.judge_reasoning ? (
            <p className="contradiction-resolver__reasoning">
              {contradiction.judge_reasoning}
            </p>
          ) : null}
          <div className="contradiction-resolver__pair">
            <BeliefPanel label="A" belief={a} />
            <BeliefPanel label="B" belief={b} />
          </div>
          {error ? <div className="alert alert--error">{error}</div> : null}
          {isResolved ? (
            <div className="alert alert--info">
              Already resolved: <strong>{contradiction.resolution}</strong>
            </div>
          ) : null}
        </div>
        <footer className="modal__footer contradiction-resolver__actions">
          {RESOLUTIONS.map((r) => (
            <button
              key={r.value}
              type="button"
              className="btn btn--primary btn--md"
              disabled={submitting || isResolved}
              onClick={() => void submit(r.value)}
            >
              {r.label}
            </button>
          ))}
        </footer>
      </div>
    </>
  );
}

function BeliefPanel({ label, belief }: { label: string; belief: Belief | null }) {
  return (
    <section
      className="contradiction-resolver__panel"
      aria-label={`Belief ${label}`}
    >
      <header>
        <strong>{label}</strong>
        {belief ? (
          <span className="belief-card__subject">{belief.subject}</span>
        ) : null}
      </header>
      {belief ? (
        <>
          <p>{belief.content}</p>
          <div className="belief-card__meta">
            <span>conf {belief.confidence.toFixed(2)}</span>
            <span>
              {belief.source_fact_ids.length} source
              {belief.source_fact_ids.length === 1 ? "" : "s"}
            </span>
          </div>
        </>
      ) : (
        <p className="memory-empty">Loading…</p>
      )}
    </section>
  );
}
