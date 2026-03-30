// ============================================================================
// LEARNING HEALTH BAR — Bottom status bar for Observatory
// ============================================================================

import { useGraphStats, useDistillationStatus, useBackfill } from "./graph-hooks";

export function LearningHealthBar() {
  const { stats, loading: statsLoading } = useGraphStats();
  const { status, loading: distLoading, refetch: refetchStatus } = useDistillationStatus();

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
        Sessions distilled:
        <span className="observatory__health-value">
          {distilled} / {total}
        </span>
      </div>

      {/* Counts from graph stats */}
      {stats && (
        <>
          <div className="observatory__health-item">
            Facts:
            <span className="observatory__health-value">{stats.facts}</span>
          </div>
          <div className="observatory__health-item">
            Entities:
            <span className="observatory__health-value">{stats.entities}</span>
          </div>
          <div className="observatory__health-item">
            Relationships:
            <span className="observatory__health-value">{stats.relationships}</span>
          </div>
          <div className="observatory__health-item">
            Episodes:
            <span className="observatory__health-value">{stats.episodes}</span>
          </div>
        </>
      )}

      {/* Distillation counts from /api/distillation/status */}
      {status && (
        <>
          {failed > 0 && (
            <div className="observatory__health-item">
              Failed:
              <span className="observatory__health-value observatory__health-value--error">
                {failed}
              </span>
            </div>
          )}
          {skipped > 0 && (
            <div className="observatory__health-item">
              Skipped:
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
      </div>
    </div>
  );
}
