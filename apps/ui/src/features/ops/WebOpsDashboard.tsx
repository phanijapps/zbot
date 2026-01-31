// ============================================================================
// OPS DASHBOARD
// Real-time execution monitoring and control panel
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
      color: "var(--muted)",
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
      color: "var(--muted)",
      icon: <Square size={12} />,
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
  onPause: () => void;
  onResume: () => void;
  onCancel: () => void;
  isProcessing: boolean;
}

function SessionRow({
  session,
  isExpanded,
  onToggle,
  onPause,
  onResume,
  onCancel,
  isProcessing,
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
            {session.id}
          </div>
        </div>

        <TokenDisplay tokensIn={session.tokens_in} tokensOut={session.tokens_out} />

        {duration > 0 && (
          <span className="text-sm text-muted-foreground">
            {duration}s
          </span>
        )}

        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          {canPause && (
            <button
              className="btn btn--secondary btn--sm"
              onClick={onPause}
              disabled={isProcessing}
              title="Pause execution"
            >
              <Pause size={14} />
            </button>
          )}
          {canResume && (
            <button
              className="btn btn--primary btn--sm"
              onClick={onResume}
              disabled={isProcessing}
              title="Resume execution"
            >
              <Play size={14} />
            </button>
          )}
          {canCancel && (
            <button
              className="btn btn--destructive btn--sm"
              onClick={onCancel}
              disabled={isProcessing}
              title="Cancel execution"
            >
              <Square size={14} />
            </button>
          )}
        </div>
      </div>

      {isExpanded && (
        <div className="px-10 py-3 bg-muted/30 text-sm">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <span className="text-muted-foreground">Conversation:</span>{" "}
              <span className="font-mono text-xs">{session.conversation_id}</span>
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

// Active statuses that appear on Dashboard (live monitoring)
const ACTIVE_STATUSES: ExecutionStatus[] = ["running", "paused", "queued"];

export function WebOpsDashboard() {
  const [sessions, setSessions] = useState<ExecutionSession[]>([]);
  const [statusCounts, setStatusCounts] = useState<Record<string, number>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedSession, setExpandedSession] = useState<string | null>(null);
  const [processingSession, setProcessingSession] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<ExecutionStatus | "all">("all");
  const [autoRefresh, setAutoRefresh] = useState(true);

  // Load sessions and stats
  const loadData = useCallback(async () => {
    try {
      const transport = await getTransport();

      const [sessionsResult, statsResult] = await Promise.all([
        transport.listExecutionSessions(),
        transport.getExecutionStats(),
      ]);

      if (sessionsResult.success && sessionsResult.data) {
        // Filter to only active sessions, then apply user filter
        let filtered = sessionsResult.data.filter((s) =>
          ACTIVE_STATUSES.includes(s.status)
        );
        if (statusFilter !== "all") {
          filtered = filtered.filter((s) => s.status === statusFilter);
        }
        setSessions(filtered);
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
  }, [statusFilter]);

  // Initial load and auto-refresh
  useEffect(() => {
    loadData();

    if (autoRefresh) {
      const interval = setInterval(loadData, 3000);
      return () => clearInterval(interval);
    }
  }, [loadData, autoRefresh]);

  // Subscribe to real-time events
  useEffect(() => {
    let unsubscribes: (() => void)[] = [];

    const setupSubscriptions = async () => {
      const transport = await getTransport();

      // Subscribe to global events for session updates
      sessions
        .filter((s) => s.status === "running")
        .forEach((session) => {
          const unsub = transport.subscribe(session.conversation_id, (event) => {
            if (event.type === "token_usage") {
              // Update token counts for the session
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === session.id
                    ? {
                        ...s,
                        tokens_in: (event as { tokens_in?: number }).tokens_in || s.tokens_in,
                        tokens_out: (event as { tokens_out?: number }).tokens_out || s.tokens_out,
                      }
                    : s
                )
              );
            } else if (event.type === "session_paused") {
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === session.id ? { ...s, status: "paused" } : s
                )
              );
            } else if (event.type === "session_resumed") {
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === session.id ? { ...s, status: "running" } : s
                )
              );
            } else if (event.type === "session_cancelled") {
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === session.id ? { ...s, status: "cancelled" } : s
                )
              );
            }
          });
          unsubscribes.push(unsub);
        });
    };

    setupSubscriptions();

    return () => {
      unsubscribes.forEach((unsub) => unsub());
    };
  }, [sessions]);

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
  const activeCount = runningCount + pausedCount + queuedCount;

  return (
    <div className="page">
      <div className="page-container">
        {/* Header */}
        <div className="page-header flex items-center justify-between mb-6">
          <div>
            <h1 className="text-2xl font-bold">Dashboard</h1>
            <p className="text-muted-foreground">
              Live monitoring and control of active agent executions
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
        <div className="grid grid-cols-4 gap-4 mb-6">
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
            color="var(--muted)"
          />
        </div>

        {/* Filter Bar */}
        <div className="flex items-center gap-4 mb-4">
          <span className="text-sm text-muted-foreground">Filter:</span>
          <div className="flex gap-2">
            {(["all", ...ACTIVE_STATUSES] as const).map(
              (status) => (
                <button
                  key={status}
                  className={`btn btn--sm ${
                    statusFilter === status ? "btn--primary" : "btn--secondary"
                  }`}
                  onClick={() => setStatusFilter(status)}
                >
                  {status === "all" ? "All Active" : status.charAt(0).toUpperCase() + status.slice(1)}
                </button>
              )
            )}
          </div>
        </div>

        {/* Sessions List */}
        <div className="card">
          <div className="card__header">
            <h2 className="font-semibold">Active Sessions</h2>
          </div>
          {sessions.length === 0 ? (
            <div className="p-8 text-center text-muted-foreground">
              <CheckCircle size={48} className="mx-auto mb-4 opacity-50" />
              <p>No active sessions</p>
              <p className="text-sm mt-2">Sessions will appear here when agents are running</p>
            </div>
          ) : (
            <div>
              {sessions.map((session) => (
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
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default WebOpsDashboard;
