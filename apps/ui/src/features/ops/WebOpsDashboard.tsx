// ============================================================================
// DASHBOARD
// Real-time execution monitoring and session history
// ============================================================================

import { useEffect, useState, useCallback } from "react";
import { getTransport } from "../../services/transport";
import type {
  ExecutionSession,
  ExecutionStatus,
} from "../../services/transport/types";
import {
  Play,
  Pause,
  Square,
  RefreshCw,
  Activity,
  Zap,
  AlertCircle,
  CheckCircle,
  Clock,
  Loader2,
  ChevronDown,
  ChevronRight,
  Bot,
  History,
  XCircle,
  MessageSquare,
} from "lucide-react";

// ============================================================================
// Status Badge Component
// ============================================================================

function StatusBadge({ status }: { status: ExecutionStatus }) {
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
// Token Display Component
// ============================================================================

function TokenDisplay({ tokensIn, tokensOut }: { tokensIn: number; tokensOut: number }) {
  const total = tokensIn + tokensOut;
  // Rough cost estimate: $3/1M input, $15/1M output (Claude Sonnet pricing)
  const estimatedCost = (tokensIn * 0.000003) + (tokensOut * 0.000015);

  return (
    <div className="flex items-center gap-4 text-sm">
      <div className="flex items-center gap-1" title="Input tokens">
        <Zap size={14} className="text-primary" />
        <span>{tokensIn.toLocaleString()}</span>
      </div>
      <div className="flex items-center gap-1" title="Output tokens">
        <Activity size={14} className="text-success" />
        <span>{tokensOut.toLocaleString()}</span>
      </div>
      {total > 0 && (
        <span className="text-muted-foreground text-xs">
          ~${estimatedCost.toFixed(4)}
        </span>
      )}
    </div>
  );
}

// ============================================================================
// Session Row Component
// ============================================================================

interface SessionRowProps {
  session: ExecutionSession;
  isExpanded: boolean;
  onToggle: () => void;
  onPause?: () => void;
  onResume?: () => void;
  onCancel?: () => void;
  onOpenChat?: () => void;
  isProcessing?: boolean;
  showControls?: boolean;
}

function SessionRow({
  session,
  isExpanded,
  onToggle,
  onPause,
  onResume,
  onCancel,
  onOpenChat,
  isProcessing = false,
  showControls = true,
}: SessionRowProps) {
  const canPause = session.status === "running";
  const canResume = session.status === "paused";
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

  return (
    <div className="border-b border-border last:border-b-0">
      <div
        className="flex items-center gap-3 p-3 hover:bg-muted/50 cursor-pointer"
        onClick={onToggle}
      >
        <button className="p-1 hover:bg-muted rounded">
          {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        </button>

        <Bot size={16} className="text-muted-foreground" />

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-medium truncate">{session.agent_id}</span>
            <StatusBadge status={session.status} />
          </div>
          <div className="text-xs text-muted-foreground truncate">
            {session.conversation_id}
          </div>
        </div>

        <TokenDisplay tokensIn={session.tokens_in} tokensOut={session.tokens_out} />

        {duration > 0 && (
          <span className="text-sm text-muted-foreground">
            {formatDuration(duration)}
          </span>
        )}

        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          {showControls && canPause && onPause && (
            <button
              className="btn btn--secondary btn--sm"
              onClick={onPause}
              disabled={isProcessing}
              title="Pause execution"
            >
              <Pause size={14} />
            </button>
          )}
          {showControls && canResume && onResume && (
            <button
              className="btn btn--primary btn--sm"
              onClick={onResume}
              disabled={isProcessing}
              title="Resume execution"
            >
              <Play size={14} />
            </button>
          )}
          {showControls && canCancel && onCancel && (
            <button
              className="btn btn--destructive btn--sm"
              onClick={onCancel}
              disabled={isProcessing}
              title="Cancel execution"
            >
              <Square size={14} />
            </button>
          )}
          {onOpenChat && (
            <button
              className="btn btn--secondary btn--sm"
              onClick={onOpenChat}
              title="Open chat"
            >
              <MessageSquare size={14} />
            </button>
          )}
        </div>
      </div>

      {isExpanded && (
        <div className="px-10 py-3 bg-muted/30 text-sm">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <span className="text-muted-foreground">Session ID:</span>{" "}
              <span className="font-mono text-xs">{session.id}</span>
            </div>
            <div>
              <span className="text-muted-foreground">Created:</span>{" "}
              {new Date(session.created_at).toLocaleString()}
            </div>
            {session.started_at && (
              <div>
                <span className="text-muted-foreground">Started:</span>{" "}
                {new Date(session.started_at).toLocaleString()}
              </div>
            )}
            {session.completed_at && (
              <div>
                <span className="text-muted-foreground">Completed:</span>{" "}
                {new Date(session.completed_at).toLocaleString()}
              </div>
            )}
            {session.parent_session_id && (
              <div>
                <span className="text-muted-foreground">Parent:</span>{" "}
                <span className="font-mono text-xs">{session.parent_session_id}</span>
              </div>
            )}
            {session.error && (
              <div className="col-span-2">
                <span className="text-destructive">Error:</span>{" "}
                <span className="text-destructive">{session.error}</span>
              </div>
            )}
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
// Main Dashboard Component
// ============================================================================

// Active statuses (live monitoring)
const ACTIVE_STATUSES: ExecutionStatus[] = ["running", "paused", "queued"];
// Closed statuses (session history)
const CLOSED_STATUSES: ExecutionStatus[] = ["completed", "cancelled", "crashed"];

export function WebOpsDashboard() {
  const [allSessions, setAllSessions] = useState<ExecutionSession[]>([]);
  const [statusCounts, setStatusCounts] = useState<Record<string, number>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedSession, setExpandedSession] = useState<string | null>(null);
  const [processingSession, setProcessingSession] = useState<string | null>(null);
  const [activeFilter, setActiveFilter] = useState<ExecutionStatus | "all">("all");
  const [historyFilter, setHistoryFilter] = useState<ExecutionStatus | "all">("all");
  const [autoRefresh, setAutoRefresh] = useState(true);

  // Derived data
  const activeSessions = allSessions.filter((s) => ACTIVE_STATUSES.includes(s.status));
  const closedSessions = allSessions.filter((s) => CLOSED_STATUSES.includes(s.status));

  // Filtered views
  const filteredActiveSessions = activeFilter === "all"
    ? activeSessions
    : activeSessions.filter((s) => s.status === activeFilter);

  const filteredClosedSessions = historyFilter === "all"
    ? closedSessions
    : closedSessions.filter((s) => s.status === historyFilter);

  // Load sessions and stats
  const loadData = useCallback(async () => {
    try {
      const transport = await getTransport();

      const [sessionsResult, statsResult] = await Promise.all([
        transport.listExecutionSessions(),
        transport.getExecutionStats(),
      ]);

      if (sessionsResult.success && sessionsResult.data) {
        // Sort by created_at descending (newest first)
        const sorted = [...sessionsResult.data].sort(
          (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        );
        setAllSessions(sorted);
      } else if (!sessionsResult.success) {
        console.error("Failed to load sessions:", sessionsResult.error);
      }

      if (statsResult.success && statsResult.data) {
        setStatusCounts(statsResult.data);
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

  const handleOpenChat = (session: ExecutionSession) => {
    // TODO: Open chat slider with this session's conversation
    console.log("Open chat for session:", session.conversation_id);
  };

  if (isLoading) {
    return (
      <div className="page">
        <div className="flex items-center justify-center h-64">
          <Loader2 className="animate-spin" size={32} />
        </div>
      </div>
    );
  }

  const runningCount = statusCounts.running || 0;
  const pausedCount = statusCounts.paused || 0;
  const queuedCount = statusCounts.queued || 0;
  const completedCount = statusCounts.completed || 0;
  const activeCount = runningCount + pausedCount + queuedCount;

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
          <div className="flex items-center gap-2">
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={autoRefresh}
                onChange={(e) => setAutoRefresh(e.target.checked)}
                className="rounded"
              />
              Auto-refresh
            </label>
            <button
              className="btn btn--secondary btn--md"
              onClick={loadData}
              title="Refresh"
            >
              <RefreshCw size={16} />
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

        {/* Stats Grid */}
        <div className="grid grid-cols-5 gap-4 mb-6">
          <StatsCard
            label="Active"
            value={activeCount}
            icon={<Activity size={20} />}
          />
          <StatsCard
            label="Running"
            value={runningCount}
            icon={<Loader2 size={20} className={runningCount > 0 ? "animate-spin" : ""} />}
            color="var(--primary)"
          />
          <StatsCard
            label="Paused"
            value={pausedCount}
            icon={<Pause size={20} />}
            color="var(--warning)"
          />
          <StatsCard
            label="Queued"
            value={queuedCount}
            icon={<Clock size={20} />}
            color="var(--muted-foreground)"
          />
          <StatsCard
            label="Completed"
            value={completedCount}
            icon={<CheckCircle size={20} />}
            color="var(--success)"
          />
        </div>

        {/* Two-column layout for sessions */}
        <div className="grid grid-cols-2 gap-6">
          {/* Active Sessions */}
          <div className="card" style={{ minHeight: "400px", display: "flex", flexDirection: "column" }}>
            <div className="card__header flex items-center justify-between">
              <div className="flex items-center gap-3">
                <Activity size={18} className="text-primary" />
                <h2 className="font-semibold">Active Sessions</h2>
                <span className="badge">{activeSessions.length}</span>
              </div>
            </div>

            {/* Active Filter */}
            <div
              className="border-b border-border flex items-center gap-3"
              style={{ padding: "12px 16px" }}
            >
              <span className="text-xs text-muted-foreground">Filter:</span>
              <div className="flex gap-1">
                {(["all", ...ACTIVE_STATUSES] as const).map((status) => (
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
                  {filteredActiveSessions.map((session) => (
                    <SessionRow
                      key={session.id}
                      session={session}
                      isExpanded={expandedSession === session.id}
                      onToggle={() =>
                        setExpandedSession(expandedSession === session.id ? null : session.id)
                      }
                      onPause={() => handlePause(session.id)}
                      onResume={() => handleResume(session.id)}
                      onCancel={() => handleCancel(session.id)}
                      isProcessing={processingSession === session.id}
                      showControls={true}
                    />
                  ))}
                </>
              )}
            </div>
          </div>

          {/* Session History */}
          <div className="card" style={{ minHeight: "400px", display: "flex", flexDirection: "column" }}>
            <div className="card__header flex items-center justify-between">
              <div className="flex items-center gap-3">
                <History size={18} className="text-muted-foreground" />
                <h2 className="font-semibold">Session History</h2>
                <span className="badge">{closedSessions.length}</span>
              </div>
            </div>

            {/* History Filter */}
            <div
              className="border-b border-border flex items-center gap-3"
              style={{ padding: "12px 16px" }}
            >
              <span className="text-xs text-muted-foreground">Filter:</span>
              <div className="flex gap-1">
                {(["all", ...CLOSED_STATUSES] as const).map((status) => (
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
                  {filteredClosedSessions.slice(0, 50).map((session) => (
                    <SessionRow
                      key={session.id}
                      session={session}
                      isExpanded={expandedSession === session.id}
                      onToggle={() =>
                        setExpandedSession(expandedSession === session.id ? null : session.id)
                      }
                      onOpenChat={() => handleOpenChat(session)}
                      showControls={false}
                    />
                  ))}
                  {filteredClosedSessions.length > 50 && (
                    <div className="p-3 text-center text-sm text-muted-foreground border-t border-border">
                      Showing 50 of {filteredClosedSessions.length} sessions
                    </div>
                  )}
                </>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default WebOpsDashboard;
