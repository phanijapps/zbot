// ============================================================================
// LEARNING HEALTH BAR — Bottom status bar for Observatory
// ============================================================================

import { useState } from "react";
import { useGraphStats, useDistillationStatus, useBackfill } from "./graph-hooks";
import { useBeliefNetworkStats } from "./belief-network/hooks";
import { Slideover } from "@/components/Slideover";
import { BeliefNetworkPanel } from "./belief-network/BeliefNetworkPanel";

export function LearningHealthBar() {
  const { stats, loading: statsLoading } = useGraphStats();
  const { status, loading: distLoading, refetch: refetchStatus } = useDistillationStatus();
  // Belief Network totals fold into this strip so the page doesn't carry
  // a separate 3-card panel. The detail surface (3 cards + activity feed
  // + propagation chain) lives in a slideover opened from the strip.
  const { stats: beliefStats } = useBeliefNetworkStats();
  const [detailsOpen, setDetailsOpen] = useState(false);

  const { run, isRunning, isDone, progress, error: backfillError } = useBackfill(
    refetchStatus
  );

  if (statsLoading && distLoading) return null;

  const distilled = status?.success_count ?? 0;
  const failed = status?.failed_count ?? 0;
  const skipped = status?.skipped_count ?? 0;
  const total = distilled + failed + skipped + (status?.permanently_failed_count ?? 0);

  return (
    <div className="observatory__health">
      {/* Distillation status */}
      <div className="observatory__health-item">
        Sessions distilled:{" "}
        <span className="observatory__health-value">
          {distilled} / {total}
        </span>
      </div>

      {/* Counts from graph stats */}
      {stats && (
        <>
          <div className="observatory__health-item">
            Facts:{" "}
            <span className="observatory__health-value">{stats.facts}</span>
          </div>
          <div className="observatory__health-item">
            Entities:{" "}
            <span className="observatory__health-value">{stats.entities}</span>
          </div>
          <div className="observatory__health-item">
            Relationships:{" "}
            <span className="observatory__health-value">{stats.relationships}</span>
          </div>
          <div className="observatory__health-item">
            Episodes:{" "}
            <span className="observatory__health-value">{stats.episodes}</span>
          </div>
        </>
      )}

      {/* Belief Network totals — folded into this strip so the page
          stays compact. Detail surface lives in the Memory tab
          (Beliefs / Contradictions sub-tabs). */}
      {beliefStats?.enabled ? (
        <>
          <div className="observatory__health-item">
            Beliefs:{" "}
            <span className="observatory__health-value">
              {beliefStats.totals.total_beliefs}
            </span>
          </div>
          {beliefStats.totals.total_unresolved_contradictions > 0 ? (
            <div className="observatory__health-item">
              Contradictions:{" "}
              <span className="observatory__health-value observatory__health-value--warning">
                {beliefStats.totals.total_unresolved_contradictions} unresolved
              </span>
            </div>
          ) : beliefStats.totals.total_contradictions > 0 ? (
            <div className="observatory__health-item">
              Contradictions:{" "}
              <span className="observatory__health-value">
                {beliefStats.totals.total_contradictions}
              </span>
            </div>
          ) : null}
        </>
      ) : null}

      {/* Distillation counts from /api/distillation/status */}
      {status && (
        <>
          {failed > 0 && (
            <div className="observatory__health-item">
              Failed:{" "}
              <span className="observatory__health-value observatory__health-value--error">
                {failed}
              </span>
            </div>
          )}
          {skipped > 0 && (
            <div className="observatory__health-item">
              Skipped:{" "}
              <span className="observatory__health-value observatory__health-value--warning">
                {skipped}
              </span>
            </div>
          )}
        </>
      )}

      {/* Backfill controls */}
      <div className="observatory__health-item" style={{ marginLeft: "auto" }}>
        {isRunning ? (
          <span className="observatory__health-value">
            Distilling {progress.current}/{progress.total}...
          </span>
        ) : isDone ? (
          <span className="observatory__health-value">{"\u2713"} Done</span>
        ) : backfillError ? (
          <span className="observatory__health-value observatory__health-value--error">
            Backfill failed
          </span>
        ) : null}

        {!isDone && (
          <button
            className="btn btn--sm btn--secondary"
            onClick={run}
            disabled={isRunning}
          >
            Backfill
          </button>
        )}

        {/* Belief Network details drawer trigger \u2014 only when the
            network is enabled. Opens a right-side slideover with
            the 3 worker stats cards + activity feed + propagation
            chain. Default state is closed so the strip stays clean. */}
        {beliefStats?.enabled ? (
          <button
            type="button"
            className="btn btn--sm btn--secondary"
            onClick={() => setDetailsOpen(true)}
            aria-label="Open belief network details"
            title="Belief network worker stats, activity feed, propagation chain"
          >
            \u2197 details
          </button>
        ) : null}
      </div>

      <Slideover
        open={detailsOpen}
        onClose={() => setDetailsOpen(false)}
        title="Belief Network details"
        subtitle="Worker stats \u00b7 activity feed \u00b7 propagation chain"
      >
        <BeliefNetworkPanel />
      </Slideover>
    </div>
  );
}
