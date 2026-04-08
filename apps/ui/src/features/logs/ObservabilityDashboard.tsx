// ============================================================================
// OBSERVABILITY DASHBOARD
// Composes SessionList (left panel) + TraceTimeline (right panel) with
// a KPI bar showing aggregate metrics for the visible sessions.
// ============================================================================

import { useState, useMemo } from "react";
import type { LogSession } from "@/services/transport/types";
import { useLogSessions, useAutoRefresh } from "./log-hooks";
import { useSessionTrace } from "./useSessionTrace";
import { useTraceSubscription } from "./useTraceSubscription";
import { SessionList } from "./SessionList";
import { TraceTimeline } from "./TraceTimeline";
import { formatDuration, formatTokens } from "./trace-types";

// ============================================================================
// Component
// ============================================================================

export function ObservabilityDashboard() {
  // ---- State ---------------------------------------------------------------
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);

  // ---- Data ----------------------------------------------------------------
  const { sessions, loading: sessionsLoading, refetch } = useLogSessions({
    root_only: true,
  });
  useAutoRefresh(sessions, refetch);

  // Trace hook for selected session
  const {
    trace,
    loading: traceLoading,
    refetch: refetchTrace,
  } = useSessionTrace(selectedSessionId);

  // Real-time subscription for running sessions
  const selectedSession = useMemo<LogSession | null>(
    () => sessions.find((s) => s.session_id === selectedSessionId) ?? null,
    [sessions, selectedSessionId],
  );

  useTraceSubscription({
    session: selectedSession,
    onEvent: refetchTrace,
  });

  // ---- KPI Metrics ---------------------------------------------------------
  const kpis = useMemo(() => {
    const total = sessions.length;
    const successCount = sessions.filter((s) => s.status === "completed").length;
    const successRate = total > 0 ? Math.round((successCount / total) * 100) : 0;
    const totalTokens = sessions.reduce((sum, s) => sum + (s.token_count ?? 0), 0);
    const sessionsWithDuration = sessions.filter((s) => s.duration_ms != null);
    const avgDuration =
      sessionsWithDuration.length > 0
        ? Math.round(
            sessionsWithDuration.reduce((sum, s) => sum + (s.duration_ms ?? 0), 0) /
              sessionsWithDuration.length,
          )
        : 0;
    return { total, successRate, totalTokens, avgDuration };
  }, [sessions]);

  // ---- Render --------------------------------------------------------------
  return (
    <div className="obs-dashboard">
      {/* KPI bar */}
      <div className="obs-dashboard__kpi-bar">
        <div className="obs-dashboard__kpi-stat">
          <span className="obs-dashboard__kpi-value">{kpis.total}</span>{" "}
          sessions
        </div>
        <div className="obs-dashboard__kpi-stat">
          <span className={`obs-dashboard__kpi-value${kpis.successRate >= 80 ? " obs-dashboard__kpi-value--success" : ""}`}>
            {kpis.successRate}%
          </span>{" "}
          success
        </div>
        <div className="obs-dashboard__kpi-stat">
          <span className="obs-dashboard__kpi-value">{formatTokens(kpis.totalTokens)}</span>{" "}
          total
        </div>
        <div className="obs-dashboard__kpi-stat">
          <span className="obs-dashboard__kpi-value">{formatDuration(kpis.avgDuration)}</span>{" "}
          avg
        </div>
      </div>

      {/* Body: session list + trace timeline */}
      <div className="obs-dashboard__body">
        <SessionList
          sessions={sessions}
          selectedId={selectedSessionId}
          onSelect={setSelectedSessionId}
          loading={sessionsLoading}
        />
        <TraceTimeline trace={trace} loading={traceLoading} />
      </div>
    </div>
  );
}
