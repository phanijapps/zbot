import type { LogSession } from '../../services/transport/types';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(0)}s`;
  return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
}

function Sparkline({ data, color }: { data: number[]; color: string }) {
  if (data.length < 2) return null;
  const max = Math.max(...data);
  const min = Math.min(...data);
  const range = max - min || 1;
  const points = data
    .map(
      (v, i) =>
        `${(i / (data.length - 1)) * 56 + 2},${22 - ((v - min) / range) * 18}`,
    )
    .join(' ');
  return (
    <svg viewBox="0 0 60 24" style={{ width: 60, height: 24 }}>
      <polyline
        points={points}
        fill="none"
        stroke={color}
        strokeWidth="1.5"
        opacity="0.7"
      />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// KpiCards
// ---------------------------------------------------------------------------

interface KpiCardsProps {
  sessions: LogSession[];
}

export function KpiCards({ sessions }: KpiCardsProps) {
  const total = sessions.length;

  // -- Success rate ----------------------------------------------------------
  const completed = sessions.filter((s) => s.status === 'completed').length;
  const successRate = total > 0 ? (completed / total) * 100 : 0;
  const successColor =
    successRate > 80 ? 'var(--success)' : successRate > 50 ? 'var(--warning)' : 'var(--destructive)';
  const successValueClass =
    successRate > 80
      ? 'kpi-card__value kpi-card__value--success'
      : successRate > 50
        ? 'kpi-card__value kpi-card__value--warning'
        : 'kpi-card__value';

  // Sparkline: per-session binary (1 = completed, 0 = not)
  const successSparkData = sessions.map((s) => (s.status === 'completed' ? 1 : 0));

  // -- Tokens ---------------------------------------------------------------
  const totalTokens = sessions.reduce((sum, s) => sum + s.token_count, 0);
  const avgTokens = total > 0 ? Math.round(totalTokens / total) : 0;
  const tokenSparkData = sessions.map((s) => s.token_count);

  // -- Tool calls -----------------------------------------------------------
  const totalToolCalls = sessions.reduce((sum, s) => sum + s.tool_call_count, 0);
  const avgToolCalls = total > 0 ? Math.round(totalToolCalls / total) : 0;
  const toolSparkData = sessions.map((s) => s.tool_call_count);

  // -- Avg duration ---------------------------------------------------------
  const durationsMs = sessions
    .filter((s) => s.duration_ms != null)
    .map((s) => s.duration_ms!);
  const avgDuration =
    durationsMs.length > 0
      ? Math.round(durationsMs.reduce((a, b) => a + b, 0) / durationsMs.length)
      : 0;
  const durationSparkData = durationsMs;

  return (
    <div className="exec-dashboard__kpis">
      {/* Success Rate */}
      <div className="kpi-card">
        <div className="kpi-card__header">
          <div>
            <div className="kpi-card__label">Success Rate</div>
            <div className={successValueClass}>
              {successRate.toFixed(0)}%
            </div>
          </div>
          <Sparkline data={successSparkData} color={successColor} />
        </div>
        <div className="kpi-card__detail">
          {completed}/{total} sessions completed
        </div>
      </div>

      {/* Total Tokens */}
      <div className="kpi-card">
        <div className="kpi-card__header">
          <div>
            <div className="kpi-card__label">Total Tokens</div>
            <div className="kpi-card__value">
              {totalTokens.toLocaleString()}
            </div>
          </div>
          <Sparkline data={tokenSparkData} color="var(--primary)" />
        </div>
        <div className="kpi-card__detail">
          avg {avgTokens.toLocaleString()} per session
        </div>
      </div>

      {/* Tool Calls */}
      <div className="kpi-card">
        <div className="kpi-card__header">
          <div>
            <div className="kpi-card__label">Tool Calls</div>
            <div className="kpi-card__value">
              {totalToolCalls.toLocaleString()}
            </div>
          </div>
          <Sparkline data={toolSparkData} color="var(--blue)" />
        </div>
        <div className="kpi-card__detail">
          avg {avgToolCalls} per session
        </div>
      </div>

      {/* Avg Duration */}
      <div className="kpi-card">
        <div className="kpi-card__header">
          <div>
            <div className="kpi-card__label">Avg Duration</div>
            <div className="kpi-card__value">
              {avgDuration > 0 ? formatDuration(avgDuration) : '--'}
            </div>
          </div>
          <Sparkline data={durationSparkData} color="var(--teal)" />
        </div>
        <div className="kpi-card__detail">
          {durationsMs.length} sessions with timing data
        </div>
      </div>
    </div>
  );
}
