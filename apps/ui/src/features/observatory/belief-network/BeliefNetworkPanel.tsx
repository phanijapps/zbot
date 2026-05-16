// ============================================================================
// BeliefNetworkPanel — top-level container shown in the Observatory page
// ============================================================================
//
// Stacks the three per-worker cards plus a recent-activity feed and the
// propagation-chain visualization. Conditional on the daemon's
// `beliefNetwork.enabled` flag — disabled state shows a friendly hint
// instead of empty cards.

import { BeliefActivityFeed } from "./BeliefActivityFeed";
import { ContradictionDetectorStatsCard } from "./ContradictionDetectorStatsCard";
import { PropagationChainVis } from "./PropagationChainVis";
import { PropagatorStatsCard } from "./PropagatorStatsCard";
import { SynthesizerStatsCard } from "./SynthesizerStatsCard";
import {
  useBeliefNetworkActivity,
  useBeliefNetworkStats,
} from "./hooks";

export interface BeliefNetworkPanelProps {
  /** Optional contradiction-detector budget per cycle (from settings),
   * threaded down so the detector card can render budget utilisation. */
  contradictionBudgetPerCycle?: number;
}

export function BeliefNetworkPanel(props: BeliefNetworkPanelProps) {
  const { contradictionBudgetPerCycle } = props;
  const { stats, loading, error, refetch } = useBeliefNetworkStats();
  const { events } = useBeliefNetworkActivity(50);

  if (loading) {
    return (
      <section
        className="belief-network-panel belief-network-panel--loading"
        data-testid="belief-network-panel-loading"
        aria-label="Belief network panel loading"
      >
        Loading belief network…
      </section>
    );
  }

  if (error || !stats) {
    return (
      <section
        className="belief-network-panel belief-network-panel--error"
        data-testid="belief-network-panel-error"
      >
        <p>Belief network stats unavailable: {error ?? "unknown error"}</p>
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={refetch}
        >
          Retry
        </button>
      </section>
    );
  }

  if (!stats.enabled) {
    return (
      <section
        className="belief-network-panel belief-network-panel--disabled"
        data-testid="belief-network-panel-disabled"
      >
        Belief Network: disabled. Enable via settings.json.
      </section>
    );
  }

  return (
    <section
      className="belief-network-panel"
      data-testid="belief-network-panel"
      aria-label="Belief network worker stats"
    >
      <header className="belief-network-panel__header">
        <h2 className="belief-network-panel__title">Belief Network</h2>
        <div className="belief-network-panel__totals">
          <span>{stats.totals.total_beliefs} beliefs</span>
          <span>{stats.totals.total_contradictions} contradictions</span>
          <span>
            {stats.totals.total_unresolved_contradictions} unresolved
          </span>
        </div>
      </header>

      <div className="belief-network-panel__cards">
        <SynthesizerStatsCard stats={stats.synthesizer} />
        <ContradictionDetectorStatsCard
          stats={stats.contradiction_detector}
          budgetPerCycle={contradictionBudgetPerCycle}
        />
        <PropagatorStatsCard stats={stats.propagator} />
      </div>

      <div className="belief-network-panel__cascade">
        <PropagationChainVis latest={stats.propagator.latest} />
      </div>

      <div className="belief-network-panel__activity">
        <h3 className="belief-network-panel__activity-title">
          Recent activity
        </h3>
        <BeliefActivityFeed events={events} />
      </div>
    </section>
  );
}
