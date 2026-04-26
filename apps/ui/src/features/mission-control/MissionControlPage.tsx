// ============================================================================
// MISSION CONTROL — page composition
// Three zones:
//   1. KpiStrip — live aggregate counts (running, queued, done, failed, paused)
//   2. SessionListPanel — left rail with search + filters
//   3. SessionDetailPane — right pane with messages + tools dual view
//
// Data: useLogSessions (status + metadata) + useSessionTokens (per-session +
// per-execution input/output tokens from the v2 API). Two endpoints are
// joined by execution id; LogSession.session_id == AgentExecution.id of the
// root execution.
// Live: ToolsPane wires the WS subscription per selected session; the list
// + tokens both auto-refresh every 5s while any session is running.
// ============================================================================

import { useMemo, useState } from "react";
import type { LogSession } from "@/services/transport/types";
import { useLogSessions, useAutoRefresh } from "../logs/log-hooks";
import { computeKpis } from "./kpi";
import { KpiStrip } from "./KpiStrip";
import { SessionListPanel, applyFilters, DEFAULT_FILTERS } from "./SessionListPanel";
import { SessionDetailPane } from "./SessionDetailPane";
import { useSessionTokens, applyV2Status } from "./useSessionTokens";
import type { SessionFilters } from "./types";

export function MissionControlPage() {
  const { sessions: rawSessions, loading, refetch } = useLogSessions({ root_only: true, limit: 200 });
  useAutoRefresh(rawSessions, refetch);

  // Pull tokens AND canonical status from /api/executions/v2/sessions/full —
  // that endpoint has truthful status (the logs endpoint can lie, e.g. report
  // "completed" while the v2 endpoint correctly says "running"). We poll
  // while either source thinks something is running, so a stale "completed"
  // in the logs API can't disable polling.
  const tokenIndexBootstrap = useSessionTokens(rawSessions.some((s) => s.status === "running"));
  const anyRunningV2 = useMemo(
    () => Array.from(tokenIndexBootstrap.byRootExecId.values()).some((e) => e.status === "running"),
    [tokenIndexBootstrap],
  );
  const tokenIndex = useSessionTokens(
    rawSessions.some((s) => s.status === "running") || anyRunningV2,
  );

  // Merge the canonical v2 status into each LogSession. Every downstream
  // consumer (KPI compute, list display, Live badges, WS subscription gating)
  // reads `session.status` — by overriding it here, they all become correct.
  const sessions = useMemo(() => applyV2Status(rawSessions, tokenIndex), [rawSessions, tokenIndex]);

  const [filters, setFilters] = useState<SessionFilters>(DEFAULT_FILTERS);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // Auto-pick the first matching session when nothing's selected (or the
  // current selection has dropped out of the list).
  const visible = useMemo(() => applyFilters(sessions, filters), [sessions, filters]);
  const selected: LogSession | null = useMemo(() => {
    if (!selectedId) return visible[0] ?? null;
    const found = sessions.find((s) => s.session_id === selectedId);
    if (found) return found;
    return visible[0] ?? null;
  }, [sessions, visible, selectedId]);

  const kpis = useMemo(() => computeKpis(sessions), [sessions]);

  const handleStatusToggle = (key: keyof SessionFilters["status"]) => {
    setFilters((prev) => ({
      ...prev,
      status: { ...prev.status, [key]: !prev.status[key] },
    }));
  };

  return (
    <div className="mission-control">
      <KpiStrip kpis={kpis} />
      <div className="mission-control__body">
        <SessionListPanel
          sessions={sessions}
          selectedId={selected?.session_id ?? null}
          filters={filters}
          loading={loading}
          tokenIndex={tokenIndex}
          onSearchChange={(value) =>
            setFilters((prev) => ({ ...prev, search: value }))
          }
          onStatusToggle={handleStatusToggle}
          onSelect={setSelectedId}
        />
        <SessionDetailPane session={selected} tokenIndex={tokenIndex} />
      </div>
    </div>
  );
}
