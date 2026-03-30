// ============================================================================
// EXECUTION DASHBOARD
// Visual observability dashboard — composes KpiCards, SessionRow, filters.
// Replaces the flat log list with an execution intelligence view.
// ============================================================================

import { useState, useEffect, useMemo } from "react";
import { Activity, AlertCircle, Loader2, RefreshCw, Search } from "lucide-react";
import type { LogSession, SessionDetail } from "@/services/transport/types";
import { useLogSessions, useSessionDetail, useAutoRefresh } from "./log-hooks";
import { KpiCards } from "./KpiCards";
import { SessionRow } from "./SessionRow";

// ============================================================================
// ExecutionDashboard
// ============================================================================

export function ExecutionDashboard() {
  // ---- State ---------------------------------------------------------------
  const [agentFilter, setAgentFilter] = useState<string | undefined>();
  const [levelFilter, setLevelFilter] = useState<string | undefined>();
  const [searchTerm, setSearchTerm] = useState("");
  const [expandedSessionId, setExpandedSessionId] = useState<string | null>(null);
  const [sessionDetails, setSessionDetails] = useState<Record<string, SessionDetail>>({});

  // ---- Data ----------------------------------------------------------------
  const { sessions, loading, error, refetch } = useLogSessions();
  const { detail } = useSessionDetail(expandedSessionId);
  useAutoRefresh(sessions, refetch);

  // Cache session details as they load
  useEffect(() => {
    if (detail && expandedSessionId) {
      setSessionDetails((prev) => ({ ...prev, [expandedSessionId]: detail }));
    }
  }, [detail, expandedSessionId]);

  // ---- Derived data --------------------------------------------------------

  // Filter sessions
  const filteredSessions = useMemo(() => {
    let filtered = sessions;

    // Only show root sessions (no parent) — children are rendered inside SessionRow
    filtered = filtered.filter((s) => !s.parent_session_id);

    if (agentFilter) {
      filtered = filtered.filter((s) => s.agent_id === agentFilter);
    }
    if (levelFilter === "error") {
      filtered = filtered.filter((s) => s.error_count > 0);
    }
    if (levelFilter === "warning") {
      // Show sessions with errors OR non-completed status
      filtered = filtered.filter(
        (s) => s.error_count > 0 || s.status === "error",
      );
    }
    if (searchTerm) {
      const lower = searchTerm.toLowerCase();
      filtered = filtered.filter(
        (s) =>
          s.session_id.toLowerCase().includes(lower) ||
          s.agent_id.toLowerCase().includes(lower) ||
          (s.agent_name || "").toLowerCase().includes(lower),
      );
    }
    return filtered;
  }, [sessions, agentFilter, levelFilter, searchTerm]);

  // Unique agents for filter pills
  const agents = useMemo(
    () => [...new Set(sessions.map((s) => s.agent_id))],
    [sessions],
  );

  // Resolve child sessions for a given session
  const getChildSessions = (session: LogSession): LogSession[] => {
    return sessions.filter(
      (s) => s.parent_session_id === session.session_id,
    );
  };

  // ---- Loading state -------------------------------------------------------
  if (loading && sessions.length === 0) {
    return (
      <div className="exec-dashboard">
        <div className="loading-spinner">
          <Loader2 className="loading-spinner__icon" />
        </div>
      </div>
    );
  }

  // ---- Render --------------------------------------------------------------
  return (
    <div className="exec-dashboard">
      {/* KPI Cards */}
      <KpiCards sessions={sessions} />

      {/* Filter Bar */}
      <div className="exec-dashboard__filters">
        {/* Agent pills */}
        <button
          className={`filter-chip ${!agentFilter ? "filter-chip--active" : ""}`}
          onClick={() => setAgentFilter(undefined)}
        >
          All
        </button>
        {agents.map((agent) => (
          <button
            key={agent}
            className={`filter-chip ${agentFilter === agent ? "filter-chip--active" : ""}`}
            onClick={() =>
              setAgentFilter(agentFilter === agent ? undefined : agent)
            }
          >
            {agent}
          </button>
        ))}

        {/* Separator */}
        <div
          style={{
            width: 1,
            height: 20,
            backgroundColor: "var(--border)",
            flexShrink: 0,
          }}
        />

        {/* Level toggles */}
        <button
          className={`filter-chip ${levelFilter === "error" ? "filter-chip--active" : ""}`}
          onClick={() =>
            setLevelFilter(levelFilter === "error" ? undefined : "error")
          }
        >
          Errors Only
        </button>
        <button
          className={`filter-chip ${levelFilter === "warning" ? "filter-chip--active" : ""}`}
          onClick={() =>
            setLevelFilter(levelFilter === "warning" ? undefined : "warning")
          }
        >
          Warnings
        </button>

        {/* Spacer */}
        <div style={{ flex: 1 }} />

        {/* Search */}
        <div className="action-bar__search">
          <Search
            style={{
              position: "absolute",
              left: 10,
              top: "50%",
              transform: "translateY(-50%)",
              width: 14,
              height: 14,
              color: "var(--muted-foreground)",
              pointerEvents: "none",
            }}
          />
          <input
            type="text"
            placeholder="Search sessions..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="action-bar__search-input"
            style={{ paddingLeft: 30 }}
          />
        </div>

        {/* Refresh button */}
        <button
          className="btn btn--ghost btn--sm"
          onClick={refetch}
          disabled={loading}
          title="Refresh sessions"
        >
          <RefreshCw
            style={{
              width: 14,
              height: 14,
              animation: loading ? "spin 1s linear infinite" : "none",
            }}
          />
        </button>
      </div>

      {/* Error state */}
      {error && (
        <div className="alert alert--error" style={{ margin: "var(--spacing-3) var(--spacing-4)", flexShrink: 0 }}>
          <AlertCircle style={{ width: 14, height: 14, flexShrink: 0 }} />
          <span>{error}</span>
          <button className="btn btn--ghost btn--sm" onClick={refetch} style={{ marginLeft: "auto" }}>
            Retry
          </button>
        </div>
      )}

      {/* Session List */}
      <div className="exec-dashboard__sessions">
        {filteredSessions.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state__icon">
              <Activity style={{ width: 32, height: 32 }} />
            </div>
            <div className="empty-state__title">No execution logs</div>
            <div className="empty-state__description">
              {agentFilter || levelFilter || searchTerm
                ? "No sessions match your current filters."
                : "Sessions will appear here when agents run."}
            </div>
          </div>
        ) : (
          filteredSessions.map((session) => (
            <SessionRow
              key={session.session_id}
              session={session}
              childSessions={getChildSessions(session)}
              isExpanded={expandedSessionId === session.session_id}
              onToggle={() =>
                setExpandedSessionId(
                  expandedSessionId === session.session_id
                    ? null
                    : session.session_id,
                )
              }
              detail={sessionDetails[session.session_id] || null}
              onLoadDetail={(id) => setExpandedSessionId(id)}
            />
          ))
        )}
      </div>
    </div>
  );
}
