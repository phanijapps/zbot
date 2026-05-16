// ============================================================================
// BeliefDetailDrawer — slide-over with full belief detail.
// ============================================================================

import { useEffect, useState } from "react";
import { getBeliefDetail } from "./api";
import type { BeliefDetailResponse } from "../types.beliefs";

interface Props {
  agentId: string;
  beliefId: string | null;
  onClose: () => void;
}

export function BeliefDetailDrawer({ agentId, beliefId, onClose }: Props) {
  const [detail, setDetail] = useState<BeliefDetailResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!beliefId) {
      setDetail(null);
      setError(null);
      return;
    }
    let alive = true;
    (async () => {
      setLoading(true);
      const res = await getBeliefDetail(agentId, beliefId);
      if (!alive) return;
      if (res.success && res.data) {
        setDetail(res.data);
        setError(null);
      } else {
        setError(res.error ?? "Failed to load belief");
      }
      setLoading(false);
    })();
    return () => {
      alive = false;
    };
  }, [agentId, beliefId]);

  if (!beliefId) return null;

  return (
    <>
      <div
        className="slideover-backdrop slideover-backdrop--open"
        role="button"
        tabIndex={0}
        aria-label="Close belief detail"
        onClick={onClose}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " " || e.key === "Escape") onClose();
        }}
      />
      <aside className="slideover slideover--open belief-detail" aria-label="Belief detail">
        <header className="slideover__header">
          <div>
            <div className="slideover__title">Belief</div>
            {detail ? (
              <div className="slideover__subtitle">{detail.belief.subject}</div>
            ) : null}
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
        <div className="slideover__body">
          {loading && <div className="memory-empty">Loading…</div>}
          {error && <div className="memory-empty">{error}</div>}
          {detail && <DetailBody detail={detail} />}
        </div>
      </aside>
    </>
  );
}

function DetailBody({ detail }: { detail: BeliefDetailResponse }) {
  const { belief, source_facts, contradictions } = detail;
  return (
    <div className="belief-detail__body">
      <section className="slideover__section">
        <h3 className="slideover__section-title">Content</h3>
        <p>{belief.content}</p>
      </section>
      <section className="slideover__section">
        <h3 className="slideover__section-title">Stats</h3>
        <dl className="belief-detail__stats">
          <dt>Confidence</dt>
          <dd>{belief.confidence.toFixed(2)}</dd>
          <dt>Valid from</dt>
          <dd>{formatDate(belief.valid_from)}</dd>
          <dt>Valid until</dt>
          <dd>{formatDate(belief.valid_until)}</dd>
          <dt>Synthesizer version</dt>
          <dd>{belief.synthesizer_version}</dd>
          {belief.stale ? (
            <>
              <dt>Status</dt>
              <dd>Stale (pending re-synthesis)</dd>
            </>
          ) : null}
        </dl>
      </section>
      {belief.reasoning ? (
        <section className="slideover__section">
          <h3 className="slideover__section-title">Reasoning</h3>
          <p>{belief.reasoning}</p>
        </section>
      ) : null}
      <section className="slideover__section">
        <h3 className="slideover__section-title">
          Source facts ({source_facts.length})
        </h3>
        {source_facts.length === 0 ? (
          <p className="memory-empty">No source facts could be loaded.</p>
        ) : (
          <ul className="belief-detail__facts">
            {source_facts.map((f) => (
              <li key={f.id} className="belief-detail__fact">
                <span className={`memory-kind memory-kind--${f.category}`}>
                  {f.category || "fact"}
                </span>
                <p>{f.content}</p>
                <span className="belief-card__age">
                  conf {f.confidence.toFixed(2)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
      <section className="slideover__section">
        <h3 className="slideover__section-title">
          Contradictions ({contradictions.length})
        </h3>
        {contradictions.length === 0 ? (
          <p className="memory-empty">No contradictions involving this belief.</p>
        ) : (
          <ul className="belief-detail__contradictions">
            {contradictions.map((c) => (
              <li key={c.id}>
                <strong>{c.contradiction_type}</strong> · severity{" "}
                {c.severity.toFixed(2)} ·{" "}
                {c.resolution ?? "unresolved"}
                {c.judge_reasoning ? <p>{c.judge_reasoning}</p> : null}
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function formatDate(iso: string | null): string {
  if (!iso) return "—";
  return new Date(iso).toLocaleString();
}
