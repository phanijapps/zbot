// ============================================================================
// ContradictionList — top-level list of recent contradictions in a
// partition. Each row is a compact two-belief preview; click opens the
// resolver.
// ============================================================================

import { useCallback, useEffect, useState } from "react";
import { listContradictions } from "./api";
import { ContradictionResolver } from "./ContradictionResolver";
import type { BeliefContradiction } from "../types.beliefs";

interface Props {
  agentId: string;
  partitionId: string | null;
}

interface State {
  rows: BeliefContradiction[];
  loading: boolean;
  disabled: boolean;
  error: string | null;
}

const INITIAL_STATE: State = {
  rows: [],
  loading: false,
  disabled: false,
  error: null,
};

export function ContradictionList({ agentId, partitionId }: Props) {
  const [state, setState] = useState<State>(INITIAL_STATE);
  const [open, setOpen] = useState<BeliefContradiction | null>(null);
  const [unresolvedOnly, setUnresolvedOnly] = useState(true);

  const load = useCallback(async () => {
    if (!partitionId) return;
    setState((s) => ({ ...s, loading: true }));
    const res = await listContradictions(partitionId, 50);
    if (res.disabled) {
      setState({ ...INITIAL_STATE, disabled: true });
      return;
    }
    if (!res.success || !res.data) {
      setState({
        ...INITIAL_STATE,
        error: res.error ?? "Failed to load contradictions",
      });
      return;
    }
    setState({
      rows: res.data,
      loading: false,
      disabled: false,
      error: null,
    });
  }, [partitionId]);

  useEffect(() => {
    if (!partitionId) {
      setState(INITIAL_STATE);
      return;
    }
    void load();
  }, [partitionId, load]);

  if (!partitionId) {
    return <div className="memory-empty">Select a ward to view contradictions.</div>;
  }
  if (state.disabled) {
    return (
      <div className="memory-empty">
        Belief Network is disabled. Enable it under Settings → Advanced →
        Memory.
      </div>
    );
  }

  const visible = unresolvedOnly
    ? state.rows.filter((c) => !c.resolved_at)
    : state.rows;

  return (
    <div className="contradiction-list">
      <label
        className="beliefs-filter-bar__chip"
        htmlFor="contradiction-filter-unresolved"
      >
        <input
          id="contradiction-filter-unresolved"
          type="checkbox"
          checked={unresolvedOnly}
          onChange={(e) => setUnresolvedOnly(e.target.checked)}
        />
        Unresolved only
      </label>
      {state.loading && state.rows.length === 0 ? (
        <div className="memory-empty">Loading…</div>
      ) : null}
      {state.error ? <div className="memory-empty">{state.error}</div> : null}
      {visible.length === 0 && !state.loading && !state.error ? (
        <div className="memory-empty">No contradictions match the filter.</div>
      ) : null}
      <ul className="contradiction-list__items">
        {visible.map((c) => (
          <li key={c.id}>
            <ContradictionRow
              row={c}
              onOpen={() => setOpen(c)}
            />
          </li>
        ))}
      </ul>
      {open ? (
        <ContradictionResolver
          agentId={agentId}
          contradiction={open}
          onClose={() => setOpen(null)}
          onResolved={() => {
            setOpen(null);
            void load();
          }}
        />
      ) : null}
    </div>
  );
}

interface RowProps {
  row: BeliefContradiction;
  onOpen: () => void;
}

function ContradictionRow({ row, onOpen }: RowProps) {
  return (
    <div
      className="contradiction-row"
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") onOpen();
      }}
    >
      <div className="contradiction-row__head">
        <span className={`belief-badge belief-badge--${row.contradiction_type}`}>
          {row.contradiction_type}
        </span>
        <span className="contradiction-row__severity">
          sev {row.severity.toFixed(2)}
        </span>
        {row.resolved_at ? (
          <span className="belief-badge belief-badge--resolved">
            {row.resolution ?? "resolved"}
          </span>
        ) : (
          <span className="belief-badge belief-badge--unresolved">unresolved</span>
        )}
      </div>
      <div className="contradiction-row__pair">
        <span>A: {row.belief_a_id}</span>
        <span>B: {row.belief_b_id}</span>
      </div>
      {row.judge_reasoning ? (
        <p className="contradiction-row__reasoning">{row.judge_reasoning}</p>
      ) : null}
    </div>
  );
}
