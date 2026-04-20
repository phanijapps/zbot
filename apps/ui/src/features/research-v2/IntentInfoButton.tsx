// =============================================================================
// IntentInfoButton — small (i) button next to the intent classification line.
// Clicking it opens a popover with the full intent analysis JSON pulled from
// GET /api/sessions/:id/state. Data is fetched once per session and cached
// across open/close cycles.
// =============================================================================

import { useCallback, useEffect, useRef, useState } from "react";
import { Info, X } from "lucide-react";
import { getTransport } from "@/services/transport";

interface IntentAnalysisJson {
  primary_intent?: string;
  hidden_intents?: string[];
  recommended_skills?: string[];
  recommended_agents?: string[];
  ward_recommendation?: {
    action?: string;
    ward_name?: string;
    subdirectory?: string | null;
    reason?: string;
  };
  execution_strategy?: {
    approach?: string;
    explanation?: string;
  };
}

interface IntentInfoButtonProps {
  sessionId: string;
}

export function IntentInfoButton({ sessionId }: IntentInfoButtonProps) {
  const [open, setOpen] = useState(false);
  const [intent, setIntent] = useState<IntentAnalysisJson | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const popoverRef = useRef<HTMLDivElement | null>(null);

  // Click-outside → close.
  useEffect(() => {
    if (!open) return;
    const onDocClick = (e: MouseEvent) => {
      if (!popoverRef.current) return;
      if (!popoverRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [open]);

  const fetchIntent = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const transport = await getTransport();
      const result = await transport.getSessionState(sessionId);
      if (!result.success || !result.data) {
        setError(result.error ?? "Failed to load intent");
        return;
      }
      setIntent((result.data.intentAnalysis ?? null) as IntentAnalysisJson | null);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, [sessionId]);

  const toggle = useCallback(() => {
    setOpen((wasOpen) => {
      const nextOpen = !wasOpen;
      if (nextOpen && intent === null && !loading) {
        void fetchIntent();
      }
      return nextOpen;
    });
  }, [intent, loading, fetchIntent]);

  return (
    <span className="intent-info">
      <button
        type="button"
        className="intent-info__btn"
        onClick={toggle}
        aria-expanded={open}
        aria-label={open ? "Hide intent analysis" : "Show intent analysis"}
        title="Intent analysis"
      >
        <Info size={14} />
      </button>
      {open && (
        <div className="intent-info__popover" ref={popoverRef} role="dialog" aria-label="Intent analysis">
          <div className="intent-info__header">
            <span>Intent analysis</span>
            <button
              type="button"
              className="intent-info__close"
              onClick={() => setOpen(false)}
              aria-label="Close"
            >
              <X size={12} />
            </button>
          </div>
          <div className="intent-info__body">
            {loading && <div className="intent-info__muted">loading…</div>}
            {error && <div className="intent-info__error">{error}</div>}
            {!loading && !error && <IntentDetails data={intent} />}
          </div>
        </div>
      )}
    </span>
  );
}

function IntentDetails({ data }: { data: IntentAnalysisJson | null }) {
  if (!data) {
    return <div className="intent-info__muted">No intent analysis recorded for this session.</div>;
  }
  const skills = data.recommended_skills ?? [];
  const agents = data.recommended_agents ?? [];
  const hidden = data.hidden_intents ?? [];
  const ward = data.ward_recommendation;
  const strategy = data.execution_strategy;

  return (
    <dl className="intent-info__dl">
      {data.primary_intent && (
        <>
          <dt>Primary intent</dt>
          <dd>{data.primary_intent}</dd>
        </>
      )}
      {hidden.length > 0 && (
        <>
          <dt>Hidden intents</dt>
          <dd>
            <ul>
              {hidden.map((h, i) => <li key={i}>{h}</li>)}
            </ul>
          </dd>
        </>
      )}
      {strategy?.approach && (
        <>
          <dt>Approach</dt>
          <dd>
            <strong>{strategy.approach}</strong>
            {strategy.explanation && <> — {strategy.explanation}</>}
          </dd>
        </>
      )}
      {ward?.ward_name && (
        <>
          <dt>Ward</dt>
          <dd>
            <strong>{ward.ward_name}</strong>
            {ward.action && <> ({ward.action})</>}
            {ward.subdirectory && <> · <code>{ward.subdirectory}</code></>}
            {ward.reason && <div className="intent-info__reason">{ward.reason}</div>}
          </dd>
        </>
      )}
      {skills.length > 0 && (
        <>
          <dt>Skills</dt>
          <dd>{skills.map((s) => <code key={s} className="intent-info__chip">{s}</code>)}</dd>
        </>
      )}
      {agents.length > 0 && (
        <>
          <dt>Agents</dt>
          <dd>{agents.map((a) => <code key={a} className="intent-info__chip">{a}</code>)}</dd>
        </>
      )}
    </dl>
  );
}
