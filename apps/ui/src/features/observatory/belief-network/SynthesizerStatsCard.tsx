// ============================================================================
// SynthesizerStatsCard — latest belief-synthesizer cycle + sparkline
// ============================================================================

import type {
  BeliefSynthesisHistoryEntry,
  BeliefSynthesisStats,
  WorkerStats,
} from "../types.beliefNetwork";
import { Sparkline } from "./Sparkline";

export interface SynthesizerStatsCardProps {
  stats: WorkerStats<BeliefSynthesisStats, BeliefSynthesisHistoryEntry>;
}

function shortCircuitRatio(s: BeliefSynthesisStats): number {
  const total = s.beliefs_synthesized + s.beliefs_llm_synthesized;
  if (total === 0) return 0;
  return s.beliefs_short_circuited / total;
}

function formatLastRun(history: BeliefSynthesisHistoryEntry[]): string {
  if (history.length === 0) return "no cycles yet";
  const last = history[history.length - 1];
  return new Date(last.timestamp).toLocaleString();
}

export function SynthesizerStatsCard(props: SynthesizerStatsCardProps) {
  const { stats } = props;
  const { latest, history } = stats;
  const ratio = shortCircuitRatio(latest);
  const ratioPct = (ratio * 100).toFixed(0);
  const sparkValues = history.map((e) => e.beliefs_synthesized);

  return (
    <div className="belief-network-card" data-testid="synthesizer-card">
      <div className="belief-network-card__header">
        <h3 className="belief-network-card__title">Belief Synthesizer</h3>
        <span className="belief-network-card__subtitle">
          Last run: {formatLastRun(history)}
        </span>
      </div>

      <div className="belief-network-card__metrics">
        <div className="belief-network-card__metric">
          <div className="belief-network-card__metric-value">
            {latest.beliefs_synthesized}
          </div>
          <div className="belief-network-card__metric-label">synthesized</div>
        </div>
        <div className="belief-network-card__metric">
          <div className="belief-network-card__metric-value">
            {latest.subjects_examined}
          </div>
          <div className="belief-network-card__metric-label">subjects</div>
        </div>
        <div className="belief-network-card__metric belief-network-card__metric--highlight">
          <div className="belief-network-card__metric-value">{ratioPct}%</div>
          <div className="belief-network-card__metric-label">
            short-circuited
          </div>
        </div>
        <div className="belief-network-card__metric">
          <div className="belief-network-card__metric-value">
            {latest.llm_calls}
          </div>
          <div className="belief-network-card__metric-label">LLM calls</div>
        </div>
      </div>

      <div className="belief-network-card__sparkline">
        <Sparkline
          values={sparkValues}
          ariaLabel="Beliefs synthesized over recent cycles"
        />
        <span className="belief-network-card__sparkline-caption">
          synthesized / cycle ({history.length})
        </span>
      </div>

      {latest.errors > 0 && (
        <div
          className="belief-network-card__warning"
          data-testid="synthesizer-errors"
        >
          {latest.errors} error{latest.errors === 1 ? "" : "s"} in last cycle
        </div>
      )}
    </div>
  );
}
