// ============================================================================
// PropagatorStatsCard — latest propagator stats + retract/stale breakdown
// ============================================================================

import type {
  BeliefPropagationStats,
  PropagationHistoryEntry,
  WorkerStats,
} from "../types.beliefNetwork";
import { Sparkline } from "./Sparkline";

export interface PropagatorStatsCardProps {
  stats: WorkerStats<BeliefPropagationStats, PropagationHistoryEntry>;
}

function formatLastRun(history: PropagationHistoryEntry[]): string {
  if (history.length === 0) return "no propagation events yet";
  const last = history[history.length - 1];
  return new Date(last.timestamp).toLocaleString();
}

export function PropagatorStatsCard(props: PropagatorStatsCardProps) {
  const { stats } = props;
  const { latest, history } = stats;
  const sparkValues = history.map((e) => e.beliefs_invalidated);

  return (
    <div className="belief-network-card" data-testid="propagator-card">
      <div className="belief-network-card__header">
        <h3 className="belief-network-card__title">Belief Propagator</h3>
        <span className="belief-network-card__subtitle">
          Last event: {formatLastRun(history)}
        </span>
      </div>

      <div className="belief-network-card__metrics">
        <div className="belief-network-card__metric belief-network-card__metric--highlight">
          <div className="belief-network-card__metric-value">
            {latest.beliefs_invalidated}
          </div>
          <div className="belief-network-card__metric-label">invalidated</div>
        </div>
        <div className="belief-network-card__metric">
          <div className="belief-network-card__metric-value">
            {latest.beliefs_retracted}
          </div>
          <div className="belief-network-card__metric-label">retracted</div>
        </div>
        <div className="belief-network-card__metric">
          <div className="belief-network-card__metric-value">
            {latest.beliefs_marked_stale}
          </div>
          <div className="belief-network-card__metric-label">marked stale</div>
        </div>
        <div className="belief-network-card__metric">
          <div className="belief-network-card__metric-value">
            {latest.max_propagation_depth}
          </div>
          <div className="belief-network-card__metric-label">depth</div>
        </div>
      </div>

      <div className="belief-network-card__sparkline">
        <Sparkline
          values={sparkValues}
          ariaLabel="Beliefs invalidated over recent propagation events"
        />
        <span className="belief-network-card__sparkline-caption">
          invalidated / event ({history.length})
        </span>
      </div>
    </div>
  );
}
