// ============================================================================
// DASHBOARD
// Real-time execution monitoring and session history
// Uses V2 API: Sessions contain Executions (root + subagents)
// ============================================================================

import { useEffect, useState, useCallback } from "react";
import { getTransport } from "../../services/transport";
import type {
  SessionWithExecutions,
  AgentExecution,
  SessionStateStatus,
  ExecutionStatus,
  DashboardStats,
  TriggerSource,
} from "../../services/transport/types";
import {
  Play,
  Pause,
  Square,
  RefreshCw,
  Activity,
  AlertCircle,
  CheckCircle,
  Clock,
  Loader2,
  ChevronDown,
  ChevronRight,
  ChevronLeft,
  Bot,
  History,
  XCircle,
  MessageSquare,
  Plus,
} from "lucide-react";
import { useNavigate } from "react-router-dom";
import { ChatSlider } from "../../components/ChatSlider";
import { SessionChatViewer } from "../../components/SessionChatViewer";
import { SourceBadge, SOURCE_CONFIG } from "./components/SourceBadge";

// ============================================================================
// Status Badge Components
// ============================================================================

function SessionStatusBadge({ status }: { status: SessionStateStatus }) {
  const config: Record<
    SessionStateStatus,
    { label: string; color: string; icon: React.ReactNode }
  > = {
    queued: {
      label: "Queued",
      color: "var(--muted-foreground)",
      icon: <Clock size={12} />,
    },
    running: {
      label: "Running",
      color: "var(--primary)",
      icon: <Loader2 size={12} className="animate-spin" />,
    },
    paused: {
      label: "Paused",
      color: "var(--warning)",
      icon: <Pause size={12} />,
    },
    crashed: {
      label: "Crashed",
      color: "var(--destructive)",
      icon: <AlertCircle size={12} />,
    },
    completed: {
      label: "Completed",
      color: "var(--success)",
      icon: <CheckCircle size={12} />,
    },
  };

  const { label, color, icon } = config[status] || config.queued;

  return (
    <span
      className="badge flex items-center gap-1"
      style={{ backgroundColor: `color-mix(in srgb, ${color} 20%, transparent)`, color }}
    >
      {icon}
      {label}
    </span>
  );
}

function ExecutionStatusBadge({ status }: { status: ExecutionStatus }) {
  const config: Record<
    ExecutionStatus,
    { label: string; color: string; icon: React.ReactNode }
  > = {
    queued: {
      label: "Queued",
      color: "var(--muted-foreground)",
      icon: <Clock size={12} />,
    },
    running: {
      label: "Running",
      color: "var(--primary)",
      icon: <Loader2 size={12} className="animate-spin" />,
    },
    paused: {
      label: "Paused",
      color: "var(--warning)",
      icon: <Pause size={12} />,
    },
    crashed: {
      label: "Crashed",
      color: "var(--destructive)",
      icon: <AlertCircle size={12} />,
    },
    cancelled: {
      label: "Cancelled",
      color: "var(--muted-foreground)",
      icon: <XCircle size={12} />,
    },
    completed: {
      label: "Completed",
      color: "var(--success)",
      icon: <CheckCircle size={12} />,
    },
  };

  const { label, color, icon } = config[status] || config.queued;

  return (
    <span
      className="badge flex items-center gap-1"
      style={{ backgroundColor: `color-mix(in srgb, ${color} 20%, transparent)`, color }}
    >
      {icon}
      {label}
    </span>
  );
}

// ============================================================================
// Session Card Component (V2 - shows session with its executions)
// ============================================================================

interface SessionCardProps {
  session: SessionWithExecutions;
  isExpanded: boolean;
  onToggle: () => void;
  onPause?: () => void;
  onResume?: () => void;
  onCancel?: () => void;
  onOpenChat?: (execution: AgentExecution) => void;
  isProcessing?: boolean;
  showControls?: boolean;
}

function SessionCard({
  session,
  isExpanded,
  onToggle,
  onPause,
  onResume,
  onCancel,
  onOpenChat,
  isProcessing = false,
  showControls = true,
}: SessionCardProps) {
  const canPause = session.status === "running";
  const canResume = session.status === "paused" || session.status === "crashed";
  const canCancel = session.status === "running" || session.status === "paused";

  const duration = session.started_at
    ? Math.round(
        (new Date(session.completed_at || Date.now()).getTime() -
          new Date(session.started_at).getTime()) /
          1000
      )
    : 0;

  const formatDuration = (seconds: number) => {
    if (seconds < 60) return `${seconds}s`;
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}m ${secs}s`;
  };

  const totalTokens = session.total_tokens_in + session.total_tokens_out;

  // Find root execution (delegation_type === 'root')
  const rootExecution = session.executions.find(e => e.delegation_type === 'root');
  const subagentExecutions = session.executions.filter(e => e.delegation_type !== 'root');

  return (
    <div className="border-b border-border last:border-b-0">
      {/* Session header */}
      <div
        className="flex items-center gap-2 p-3 hover:bg-muted/50 cursor-pointer"
        role="button"
        tabIndex={0}
        onClick={onToggle}
        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onToggle(); }}
      >
        <button className="p-1 hover:bg-muted rounded flex-shrink-0">
          {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </button>

        <div className="flex-1 min-w-0 overflow-hidden">
          <div className="flex items-center gap-2">
            <Bot size={14} className="text-muted-foreground flex-shrink-0" />
            <span className="font-medium truncate text-sm">{rootExecution?.agent_id || session.root_agent_id}</span>
            {session.subagent_count > 0 && (
              <span className="text-xs text-primary">
                +{session.subagent_count} subagent{session.subagent_count > 1 ? 's' : ''}
              </span>
            )}
            <span className="flex-shrink-0"><SessionStatusBadge status={session.status} /></span>
            <span className="flex-shrink-0"><SourceBadge source={session.source} /></span>
          </div>
        </div>

        {/* Compact info */}
        <div className="flex items-center gap-3 text-xs text-muted-foreground flex-shrink-0">
          {totalTokens > 0 && (
            <span title={`In: ${session.total_tokens_in} / Out: ${session.total_tokens_out}`}>
              {totalTokens.toLocaleString()} tok
            </span>
          )}
          {duration > 0 && (
            <span>{formatDuration(duration)}</span>
          )}
        </div>

        <div className="flex items-center gap-1 flex-shrink-0" role="toolbar" aria-label="Session controls" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
          {showControls && canPause && onPause && (
            <button
              className="btn btn--secondary btn--sm"
              onClick={onPause}
              disabled={isProcessing}
              title="Pause session"
            >
              <Pause size={14} />
            </button>
          )}
          {showControls && canResume && onResume && (
            <button
              className="btn btn--primary btn--sm"
              onClick={onResume}
              disabled={isProcessing}
              title="Resume session"
            >
              <Play size={14} />
            </button>
          )}
          {showControls && canCancel && onCancel && (
            <button
              className="btn btn--destructive btn--sm"
              onClick={onCancel}
              disabled={isProcessing}
              title="Cancel session"
            >
              <Square size={14} />
            </button>
          )}
          {rootExecution && onOpenChat && (
            <button
              className="btn btn--secondary btn--sm"
              onClick={() => onOpenChat(rootExecution)}
              title="Open chat"
            >
              <MessageSquare size={14} />
            </button>
          )}
        </div>
      </div>

      {/* Expanded: show executions hierarchy */}
      {isExpanded && (
        <div className="bg-muted/30 border-t border-border/50">
          {/* Root execution details */}
          {rootExecution && (
            <div className="px-4 py-3 border-b border-border/30">
              <div className="text-xs text-muted-foreground mb-2">Root Execution</div>
              <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                <div>
                  <span className="text-muted-foreground">Agent:</span>{" "}
                  <span className="font-medium">{rootExecution.agent_id}</span>
                </div>
                <div>
                  <span className="text-muted-foreground">Status:</span>{" "}
                  <ExecutionStatusBadge status={rootExecution.status} />
                </div>
                <div>
                  <span className="text-muted-foreground">Tokens:</span>{" "}
                  <span>{rootExecution.tokens_in.toLocaleString()} in / {rootExecution.tokens_out.toLocaleString()} out</span>
                </div>
                {rootExecution.started_at && (
                  <div>
                    <span className="text-muted-foreground">Started:</span>{" "}
                    {new Date(rootExecution.started_at).toLocaleString()}
                  </div>
                )}
                {rootExecution.error && (
                  <div className="col-span-2">
                    <span className="text-destructive">Error:</span>{" "}
                    <span className="text-destructive">{rootExecution.error}</span>
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Subagent executions */}
          {subagentExecutions.length > 0 && (
            <div className="px-4 py-3">
              <div className="text-xs text-muted-foreground mb-2">Subagent Executions</div>
              {subagentExecutions.map((exec) => (
                <div
                  key={exec.id}
                  className="flex items-center gap-2 py-2 hover:bg-muted/30 rounded"
                  style={{ paddingLeft: 16 }}
                >
                  <span className="text-muted-foreground/50">↳</span>
                  <Bot size={12} className="text-primary/60" />
                  <span className="text-sm font-medium">{exec.agent_id}</span>
                  <ExecutionStatusBadge status={exec.status} />
                  {exec.task && (
                    <span className="text-xs text-muted-foreground truncate max-w-[200px]" title={exec.task}>
                      {exec.task}
                    </span>
                  )}
                  {(exec.tokens_in + exec.tokens_out) > 0 && (
                    <span className="text-xs text-muted-foreground ml-auto">
                      {(exec.tokens_in + exec.tokens_out).toLocaleString()} tok
                    </span>
                  )}
                  {onOpenChat && (
                    <button
                      className="btn btn--ghost btn--sm p-1"
                      onClick={() => onOpenChat(exec)}
                      title="View subagent chat (read-only)"
                    >
                      <MessageSquare size={12} />
                    </button>
                  )}
                </div>
              ))}
            </div>
          )}

          {/* Session metadata */}
          <div className="px-4 py-3 border-t border-border/30 text-xs">
            <div className="grid grid-cols-2 gap-x-4 gap-y-2">
              <div>
                <span className="text-muted-foreground">Session ID:</span>{" "}
                <span className="font-mono">{session.id}</span>
              </div>
              <div>
                <span className="text-muted-foreground">Created:</span>{" "}
                {new Date(session.created_at).toLocaleString()}
              </div>
              <div>
                <span className="text-muted-foreground">Source:</span>{" "}
                <SourceBadge source={session.source} />
              </div>
              {session.title && (
                <div>
                  <span className="text-muted-foreground">Title:</span>{" "}
                  <span>{session.title}</span>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Stats Card Component
// ============================================================================

function StatsCard({
  label,
  value,
  icon,
  color,
}: {
  label: string;
  value: string | number;
  icon: React.ReactNode;
  color?: string;
}) {
  return (
    <div className="card p-4">
      <div className="flex items-center gap-3">
        <div
          className="p-2 rounded-lg"
          style={{
            backgroundColor: `color-mix(in srgb, ${color || "var(--primary)"} 20%, transparent)`,
            color: color || "var(--primary)",
          }}
        >
          {icon}
        </div>
        <div>
          <div className="text-2xl font-bold">{value}</div>
          <div className="text-sm text-muted-foreground">{label}</div>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Source Stats Component
// ============================================================================

function SourceStatsBar({ sessionsBySource }: { sessionsBySource: Record<TriggerSource, number> }) {
  const sources = Object.entries(sessionsBySource).filter(([, count]) => count > 0) as [TriggerSource, number][];
  const total = sources.reduce((sum, [, count]) => sum + count, 0);

  if (total === 0) return null;

  return (
    <div className="card p-4 mb-6">
      <div className="flex items-center justify-between mb-3">
        <span className="text-sm font-medium">Sessions by Source</span>
        <span className="text-xs text-muted-foreground">{total} total</span>
      </div>
      <div className="flex gap-1 h-2 rounded-full overflow-hidden bg-muted">
        {sources.map(([source, count]) => {
          const config = SOURCE_CONFIG[source];
          const percentage = (count / total) * 100;
          return (
            <div
              key={source}
              style={{
                width: `${percentage}%`,
                backgroundColor: config.color,
              }}
              title={`${config.label}: ${count} (${percentage.toFixed(1)}%)`}
            />
          );
        })}
      </div>
      <div className="flex flex-wrap gap-3 mt-3">
        {sources.map(([source, count]) => {
          const config = SOURCE_CONFIG[source];
          return (
            <div key={source} className="flex items-center gap-1.5 text-xs">
              <div
                className="w-2 h-2 rounded-full"
                style={{ backgroundColor: config.color }}
              />
              <span className="text-muted-foreground">{config.label}:</span>
              <span className="font-medium">{count}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ============================================================================
// Main Dashboard Component
// ============================================================================

// Session statuses for filtering
const ACTIVE_SESSION_STATUSES: SessionStateStatus[] = ["running", "paused", "queued"];
const CLOSED_SESSION_STATUSES: SessionStateStatus[] = ["completed", "crashed"];

// All trigger sources for filtering
const ALL_SOURCES: TriggerSource[] = ["web", "cli", "cron", "api", "connector"];

export function WebOpsDashboard() {
  const navigate = useNavigate();
  const [sessions, setSessions] = useState<SessionWithExecutions[]>([]);
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedSession, setExpandedSession] = useState<string | null>(null);
  const [processingSession, setProcessingSession] = useState<string | null>(null);
  const [activeFilter, setActiveFilter] = useState<SessionStateStatus | "all">("all");
  const [historyFilter, setHistoryFilter] = useState<SessionStateStatus | "all">("all");
  const [sourceFilter, setSourceFilter] = useState<TriggerSource | "all">("all");
  const [autoRefresh, setAutoRefresh] = useState(true);

  // Pagination state
  const SESSIONS_PER_PAGE = 5;
  const [activeSessionPage, setActiveSessionPage] = useState(0);
  const [historySessionPage, setHistorySessionPage] = useState(0);

  // Chat slider state
  const [selectedExecution, setSelectedExecution] = useState<{
    sessionId: string;
    executionId?: string;  // Only set for subagent views
    agentId: string;
    isSubagent: boolean;
    fromActiveSession: boolean;  // True if opened from active sessions, false if from history
  } | null>(null);

  // Derived data - split sessions into active and closed
  const activeSessions = sessions.filter((s) => ACTIVE_SESSION_STATUSES.includes(s.status));
  const closedSessions = sessions.filter((s) => CLOSED_SESSION_STATUSES.includes(s.status));

  // Apply source filter first, then status filter
  const applySourceFilter = (sessionList: SessionWithExecutions[]) => {
    if (sourceFilter === "all") return sessionList;
    return sessionList.filter((s) => s.source === sourceFilter);
  };

  // Filtered views
  const filteredActiveSessions = applySourceFilter(
    activeFilter === "all"
      ? activeSessions
      : activeSessions.filter((s) => s.status === activeFilter)
  );

  const filteredClosedSessions = applySourceFilter(
    historyFilter === "all"
      ? closedSessions
      : closedSessions.filter((s) => s.status === historyFilter)
  );

  // Pagination calculations
  const activeTotalPages = Math.ceil(filteredActiveSessions.length / SESSIONS_PER_PAGE);
  const historyTotalPages = Math.ceil(filteredClosedSessions.length / SESSIONS_PER_PAGE);

  const paginatedActiveSessions = filteredActiveSessions.slice(
    activeSessionPage * SESSIONS_PER_PAGE,
    (activeSessionPage + 1) * SESSIONS_PER_PAGE
  );

  const paginatedHistorySessions = filteredClosedSessions.slice(
    historySessionPage * SESSIONS_PER_PAGE,
    (historySessionPage + 1) * SESSIONS_PER_PAGE
  );

  // Reset pagination when filters change
  useEffect(() => {
    setActiveSessionPage(0);
  }, [activeFilter, sourceFilter]);

  useEffect(() => {
    setHistorySessionPage(0);
  }, [historyFilter, sourceFilter]);

  // Load sessions and stats using V2 API
  const loadData = useCallback(async () => {
    try {
      const transport = await getTransport();

      const [sessionsResult, statsResult] = await Promise.all([
        transport.listSessionsFull(),
        transport.getDashboardStats(),
      ]);

      if (sessionsResult.success && sessionsResult.data) {
        // Sort by created_at descending (newest first)
        const sorted = [...sessionsResult.data].sort(
          (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        );
        setSessions(sorted);
      } else if (!sessionsResult.success) {
        console.error("Failed to load sessions:", sessionsResult.error);
      }

      if (statsResult.success && statsResult.data) {
        setStats(statsResult.data);
      }

      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Initial load and auto-refresh
  useEffect(() => {
    loadData();

    if (autoRefresh) {
      const interval = setInterval(loadData, 3000);
      return () => clearInterval(interval);
    }
  }, [loadData, autoRefresh]);

  // Control handlers
  const handlePause = async (sessionId: string) => {
    setProcessingSession(sessionId);
    try {
      const transport = await getTransport();
      const result = await transport.pauseSession(sessionId);
      if (!result.success) {
        console.error("Failed to pause:", result.error);
      }
      await loadData();
    } finally {
      setProcessingSession(null);
    }
  };

  const handleResume = async (sessionId: string) => {
    setProcessingSession(sessionId);
    try {
      const transport = await getTransport();
      const result = await transport.resumeSession(sessionId);
      if (!result.success) {
        console.error("Failed to resume:", result.error);
      }
      await loadData();
    } finally {
      setProcessingSession(null);
    }
  };

  const handleCancel = async (sessionId: string) => {
    setProcessingSession(sessionId);
    try {
      const transport = await getTransport();
      const result = await transport.cancelSession(sessionId);
      if (!result.success) {
        console.error("Failed to cancel:", result.error);
      }
      await loadData();
    } finally {
      setProcessingSession(null);
    }
  };

  const handleOpenChat = useCallback((execution: AgentExecution, fromActiveSession: boolean) => {
    const isSubagent = execution.delegation_type !== 'root';
    setSelectedExecution({
      sessionId: execution.session_id,
      // Only set executionId for subagent views (to scope to that execution)
      executionId: isSubagent ? execution.id : undefined,
      agentId: execution.agent_id,
      isSubagent,
      fromActiveSession,
    });
  }, []);

  const handleCloseChat = useCallback(() => {
    setSelectedExecution(null);
  }, []);

  if (isLoading) {
    return (
      <div className="page">
        <div className="flex items-center justify-center h-64">
          <Loader2 className="animate-spin" size={32} />
        </div>
      </div>
    );
  }

  // Stats from V2 API
  const sessionsRunning = stats?.sessions_running || 0;
  const sessionsPaused = stats?.sessions_paused || 0;
  const sessionsCompleted = stats?.sessions_completed || 0;
  const executionsRunning = stats?.executions_running || 0;
  const activeCount = sessionsRunning + sessionsPaused + (stats?.sessions_queued || 0);

  return (
    <div className="page">
      <div className="page-container">
        {/* Header */}
        <div className="page-header flex items-center justify-between mb-6">
          <div>
            <h1 className="text-2xl font-bold">Dashboard</h1>
            <p className="text-muted-foreground">
              Monitor active sessions and view execution history
            </p>
          </div>
          <div className="flex items-center gap-3">
            {/* Source Filter Dropdown */}
            <select
              value={sourceFilter}
              onChange={(e) => setSourceFilter(e.target.value as TriggerSource | "all")}
              className="btn btn--secondary text-sm"
              style={{ padding: "6px 12px" }}
            >
              <option value="all">All Sources</option>
              {ALL_SOURCES.map((source) => (
                <option key={source} value={source}>
                  {SOURCE_CONFIG[source].label}
                </option>
              ))}
            </select>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={autoRefresh}
                onChange={(e) => setAutoRefresh(e.target.checked)}
                className="rounded"
              />
              <span>Auto-refresh</span>
            </label>
            <button
              className="btn btn--secondary btn--md"
              onClick={loadData}
              title="Refresh"
            >
              <RefreshCw size={16} />
            </button>
            <button
              className="btn btn--primary btn--md"
              onClick={() => navigate("/chat?new=1")}
            >
              <Plus size={16} />
              <span>New Chat</span>
            </button>
          </div>
        </div>

        {/* Error display */}
        {error && (
          <div className="card p-4 mb-6 border-destructive bg-destructive/10">
            <div className="flex items-center gap-2 text-destructive">
              <AlertCircle size={16} />
              <span>{error}</span>
            </div>
          </div>
        )}

        {/* Stats Grid - V2 with both session and execution counts */}
        <div className="grid grid-cols-5 gap-4 mb-6">
          <StatsCard
            label="Active"
            value={activeCount}
            icon={<Activity size={20} />}
          />
          <StatsCard
            label="Sessions Running"
            value={sessionsRunning}
            icon={<Loader2 size={20} className={sessionsRunning > 0 ? "animate-spin" : ""} />}
            color="var(--primary)"
          />
          <StatsCard
            label="Executions Running"
            value={executionsRunning}
            icon={<Bot size={20} />}
            color="var(--primary)"
          />
          <StatsCard
            label="Paused"
            value={sessionsPaused}
            icon={<Pause size={20} />}
            color="var(--warning)"
          />
          <StatsCard
            label="Completed"
            value={sessionsCompleted}
            icon={<CheckCircle size={20} />}
            color="var(--success)"
          />
        </div>

        {/* Source breakdown bar */}
        {stats?.sessions_by_source && (
          <SourceStatsBar sessionsBySource={stats.sessions_by_source} />
        )}

        {/* Two-column layout for sessions */}
        <div className="grid gap-6" style={{ gridTemplateColumns: "1fr 1fr" }}>
          {/* Active Sessions */}
          <div className="card" style={{ minHeight: "400px", minWidth: 0, display: "flex", flexDirection: "column", overflow: "hidden" }}>
            <div
              className="flex items-center justify-between border-b border-border"
              style={{ padding: "16px 20px" }}
            >
              <div className="flex items-center gap-3">
                <Activity size={18} className="text-primary" />
                <h2 className="font-semibold">Active Sessions</h2>
                <span className="badge">{filteredActiveSessions.length}</span>
                {sourceFilter !== "all" && (
                  <SourceBadge source={sourceFilter} />
                )}
              </div>
            </div>

            {/* Active Filter */}
            <div
              className="border-b border-border flex items-center gap-3"
              style={{ padding: "12px 16px" }}
            >
              <span className="text-xs text-muted-foreground">Status:</span>
              <div className="flex gap-1">
                {(["all", ...ACTIVE_SESSION_STATUSES] as const).map((status) => (
                  <button
                    key={status}
                    className={`btn ${
                      activeFilter === status ? "btn--primary" : "btn--ghost"
                    }`}
                    style={{ padding: "6px 12px", fontSize: "12px" }}
                    onClick={() => setActiveFilter(status)}
                  >
                    {status === "all" ? "All" : status.charAt(0).toUpperCase() + status.slice(1)}
                  </button>
                ))}
              </div>
            </div>

            <div style={{ flex: 1, overflow: "auto" }}>
              {filteredActiveSessions.length === 0 ? (
                <div className="p-8 text-center text-muted-foreground" style={{ height: "100%", display: "flex", flexDirection: "column", justifyContent: "center", alignItems: "center" }}>
                  <Activity size={40} className="mx-auto mb-3 opacity-30" />
                  <p className="text-sm">No active sessions</p>
                </div>
              ) : (
                <>
                  {paginatedActiveSessions.map((session) => (
                    <SessionCard
                      key={session.id}
                      session={session}
                      isExpanded={expandedSession === session.id}
                      onToggle={() =>
                        setExpandedSession(expandedSession === session.id ? null : session.id)
                      }
                      onPause={() => handlePause(session.id)}
                      onResume={() => handleResume(session.id)}
                      onCancel={() => handleCancel(session.id)}
                      onOpenChat={(exec) => handleOpenChat(exec, true)}
                      isProcessing={processingSession === session.id}
                      showControls={true}
                    />
                  ))}
                </>
              )}
            </div>

            {/* Pagination */}
            {activeTotalPages > 1 && (
              <div className="flex items-center justify-between border-t border-border px-4 py-2">
                <span className="text-xs text-muted-foreground">
                  {activeSessionPage * SESSIONS_PER_PAGE + 1}-{Math.min((activeSessionPage + 1) * SESSIONS_PER_PAGE, filteredActiveSessions.length)} of {filteredActiveSessions.length}
                </span>
                <div className="flex items-center gap-1">
                  <button
                    className="btn btn--ghost btn--sm p-1"
                    onClick={() => setActiveSessionPage(p => Math.max(0, p - 1))}
                    disabled={activeSessionPage === 0}
                  >
                    <ChevronLeft size={16} />
                  </button>
                  <span className="text-xs text-muted-foreground px-2">
                    {activeSessionPage + 1} / {activeTotalPages}
                  </span>
                  <button
                    className="btn btn--ghost btn--sm p-1"
                    onClick={() => setActiveSessionPage(p => Math.min(activeTotalPages - 1, p + 1))}
                    disabled={activeSessionPage >= activeTotalPages - 1}
                  >
                    <ChevronRight size={16} />
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* Session History */}
          <div className="card" style={{ minHeight: "400px", minWidth: 0, display: "flex", flexDirection: "column", overflow: "hidden" }}>
            <div
              className="flex items-center justify-between border-b border-border"
              style={{ padding: "16px 20px" }}
            >
              <div className="flex items-center gap-3">
                <History size={18} className="text-muted-foreground" />
                <h2 className="font-semibold">Session History</h2>
                <span className="badge">{filteredClosedSessions.length}</span>
                {sourceFilter !== "all" && (
                  <SourceBadge source={sourceFilter} />
                )}
              </div>
            </div>

            {/* History Filter */}
            <div
              className="border-b border-border flex items-center gap-3"
              style={{ padding: "12px 16px" }}
            >
              <span className="text-xs text-muted-foreground">Status:</span>
              <div className="flex gap-1">
                {(["all", ...CLOSED_SESSION_STATUSES] as const).map((status) => (
                  <button
                    key={status}
                    className={`btn ${
                      historyFilter === status ? "btn--primary" : "btn--ghost"
                    }`}
                    style={{ padding: "6px 12px", fontSize: "12px" }}
                    onClick={() => setHistoryFilter(status)}
                  >
                    {status === "all" ? "All" : status.charAt(0).toUpperCase() + status.slice(1)}
                  </button>
                ))}
              </div>
            </div>

            <div style={{ flex: 1, overflow: "auto" }}>
              {filteredClosedSessions.length === 0 ? (
                <div className="p-8 text-center text-muted-foreground" style={{ height: "100%", display: "flex", flexDirection: "column", justifyContent: "center", alignItems: "center" }}>
                  <History size={40} className="mx-auto mb-3 opacity-30" />
                  <p className="text-sm">No session history</p>
                </div>
              ) : (
                <>
                  {paginatedHistorySessions.map((session) => (
                    <SessionCard
                      key={session.id}
                      session={session}
                      isExpanded={expandedSession === session.id}
                      onToggle={() =>
                        setExpandedSession(expandedSession === session.id ? null : session.id)
                      }
                      onOpenChat={(exec) => handleOpenChat(exec, false)}
                      showControls={false}
                    />
                  ))}
                </>
              )}
            </div>

            {/* Pagination */}
            {historyTotalPages > 1 && (
              <div className="flex items-center justify-between border-t border-border px-4 py-2">
                <span className="text-xs text-muted-foreground">
                  {historySessionPage * SESSIONS_PER_PAGE + 1}-{Math.min((historySessionPage + 1) * SESSIONS_PER_PAGE, filteredClosedSessions.length)} of {filteredClosedSessions.length}
                </span>
                <div className="flex items-center gap-1">
                  <button
                    className="btn btn--ghost btn--sm p-1"
                    onClick={() => setHistorySessionPage(p => Math.max(0, p - 1))}
                    disabled={historySessionPage === 0}
                  >
                    <ChevronLeft size={16} />
                  </button>
                  <span className="text-xs text-muted-foreground px-2">
                    {historySessionPage + 1} / {historyTotalPages}
                  </span>
                  <button
                    className="btn btn--ghost btn--sm p-1"
                    onClick={() => setHistorySessionPage(p => Math.min(historyTotalPages - 1, p + 1))}
                    disabled={historySessionPage >= historyTotalPages - 1}
                  >
                    <ChevronRight size={16} />
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Chat Slider */}
      <ChatSlider isOpen={selectedExecution !== null} onClose={handleCloseChat}>
        {selectedExecution && (
          <SessionChatViewer
            sessionId={selectedExecution.sessionId}
            executionId={selectedExecution.executionId}
            agentId={selectedExecution.agentId}
            readOnly={selectedExecution.isSubagent}
            // Only show New Chat button for active sessions, not session history
            onNewChat={selectedExecution.fromActiveSession ? async () => {
              // End the current session and navigate to new chat
              if (selectedExecution.sessionId) {
                try {
                  const transport = await getTransport();
                  await transport.endSession(selectedExecution.sessionId);
                } catch (err) {
                  console.error("Failed to end session:", err);
                }
              }
              handleCloseChat();
              navigate("/chat?new=1");
            } : undefined}
          />
        )}
      </ChatSlider>
    </div>
  );
}

export default WebOpsDashboard;
