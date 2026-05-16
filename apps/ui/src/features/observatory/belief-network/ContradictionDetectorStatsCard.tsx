// ============================================================================
// ContradictionDetectorStatsCard — latest detector cycle + sparkline
// ============================================================================

import type {
  ContradictionDetectionStats,
  ContradictionHistoryEntry,
  WorkerStats,
} from "../types.beliefNetwork";
import { Sparkline } from "./Sparkline";

export interface ContradictionDetectorStatsCardProps {
  stats: WorkerStats<ContradictionDetectionStats, ContradictionHistoryEntry>;
  /** LLM-call budget per cycle from settings; used for the
   * highlight metric. Falls back to `pairs_examined` when unknown. */
  budgetPerCycle?: number;
}

function formatLastRun(history: ContradictionHistoryEntry[]): string {
  if (history.length === 0) return "no cycles yet";
  const last = history[history.length - 1];
  return new Date(last.timestamp).toLocaleString();
}

function utilisationPct(
  llmCalls: number,
  budget: number | undefined,
): string | null {
  if (!budget || budget <= 0) return null;
  const pct = Math.min(100, Math.round((llmCalls / budget) * 100));
  return `${pct}%`;
}

export function ContradictionDetectorStatsCard(
  props: ContradictionDetectorStatsCardProps,
) {
  const { stats, budgetPerCycle } = props;
  const { latest, history } = stats;
  const sparkValues = history.map((e) => e.pairs_examined);
  const util = utilisationPct(latest.llm_calls, budgetPerCycle);
  const totalContradictions =
    latest.contradictions_logical + latest.contradictions_tension;

  return (
    <div className="belief-network-card" data-testid="detector-card">
      <div className="belief-network-card__header">
        <h3 className="belief-network-card__title">
          Contradiction Detector
        </h3>
        <span className="belief-network-card__subtitle">
          Last run: {formatLastRun(history)}
        </span>
      </div>

      <div className="belief-network-card__metrics">
        <div className="belief-network-card__metric">
          <div className="belief-network-card__metric-value">
            {latest.pairs_examined}
          </div>
          <div className="belief-network-card__metric-label">pairs</div>
        </div>
        <div className="belief-network-card__metric">
          <div className="belief-network-card__metric-value">
            {totalContradictions}
          </div>
          <div className="belief-network-card__metric-label">found</div>
        </div>
        <div className="belief-network-card__metric belief-network-card__metric--highlight">
          <div className="belief-network-card__metric-value">
            {util ?? latest.llm_calls}
          </div>
          <div className="belief-network-card__metric-label">
            {util ? "budget used" : "LLM calls"}
          </div>
        </div>
        <div className="belief-network-card__metric">
          <div className="belief-network-card__metric-value">
            {latest.pairs_skipped_existing}
          </div>
          <div className="belief-network-card__metric-label">skipped</div>
        </div>
      </div>

      <div className="belief-network-card__sparkline">
        <Sparkline
          values={sparkValues}
          ariaLabel="Pairs examined over recent cycles"
        />
        <span className="belief-network-card__sparkline-caption">
          pairs examined / cycle ({history.length})
        </span>
      </div>

      {latest.budget_exhausted && (
        <div
          className="belief-network-card__warning"
          data-testid="budget-exhausted-warning"
          role="status"
        >
          LLM budget exhausted this cycle
        </div>
      )}
    </div>
  );
}
