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
  Plus,
} from "lucide-react";
import { useNavigate } from "react-router-dom";
import { ChatSlider } from "../../components/ChatSlider";
import { SessionChatViewer } from "../../components/SessionChatViewer";

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
// Conversation Group Types and Helpers
// ============================================================================

/**
 * Represents a conversation with all its sessions grouped together.
 * A conversation is the top-level grouping (starts with /new or hook).
 */
interface ConversationGroup {
  conversationId: string;
  /** The primary/root agent session for opening chat */
  primarySession: ExecutionSession;
  /** All sessions in this conversation, sorted chronologically */
  sessions: ExecutionSession[];
  /** Total token usage across all sessions */
  totalTokens: { in: number; out: number };
  /** Overall status (crashed > cancelled > completed) */
  overallStatus: ExecutionStatus;
  /** Earliest created_at */
  createdAt: string;
}

/**
 * Build conversation groups from flat sessions list.
 * Groups sessions by their root conversation (strips -sub-* suffix).
 */
function buildConversationGroups(sessions: ExecutionSession[]): ConversationGroup[] {
  // Helper to get root conversation ID (strip -sub-* suffix)
  const getRootConversationId = (convId: string): string => {
    const subIndex = convId.indexOf('-sub-');
    return subIndex > 0 ? convId.substring(0, subIndex) : convId;
  };

  // Group sessions by root conversation
  const conversationMap = new Map<string, ExecutionSession[]>();
  for (const session of sessions) {
    const rootConvId = getRootConversationId(session.conversation_id);
    const group = conversationMap.get(rootConvId) || [];
    group.push(session);
    conversationMap.set(rootConvId, group);
  }

  // Build groups
  const groups: ConversationGroup[] = [];

  for (const [convId, convSessions] of conversationMap) {
    // Sort by created_at
    convSessions.sort((a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime());

    // Calculate aggregates
    let totalIn = 0, totalOut = 0;
    let overallStatus: ExecutionStatus = 'completed';

    for (const session of convSessions) {
      totalIn += session.tokens_in;
      totalOut += session.tokens_out;
      // Priority: crashed > cancelled > completed
      if (session.status === 'crashed') overallStatus = 'crashed';
      else if (session.status === 'cancelled' && overallStatus !== 'crashed') overallStatus = 'cancelled';
    }

    // Find primary session (first "root" agent session)
    const primarySession = convSessions.find(s => s.agent_id === 'root') || convSessions[0];

    if (primarySession) {
      groups.push({
        conversationId: convId,
        primarySession,
        sessions: convSessions,
        totalTokens: { in: totalIn, out: totalOut },
        overallStatus,
        createdAt: convSessions[0]?.created_at || '',
      });
    }
  }

  // Sort groups by created_at descending (newest first)
  groups.sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime());

  return groups;
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
  depth?: number;
  isSubagent?: boolean;
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
  depth = 0,
  isSubagent = false,
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

  const totalTokens = session.tokens_in + session.tokens_out;
  const indentPx = depth * 24; // 24px per level

  return (
    <div className={depth === 0 ? "border-b border-border last:border-b-0" : ""}>
      <div
        className="flex items-center gap-2 p-3 hover:bg-muted/50 cursor-pointer"
        style={{ paddingLeft: `${12 + indentPx}px` }}
        onClick={onToggle}
      >
        {/* Subagent connector line */}
        {isSubagent && (
          <span className="text-muted-foreground/50 flex-shrink-0" style={{ marginLeft: -8 }}>↳</span>
        )}

        <button className="p-1 hover:bg-muted rounded flex-shrink-0">
          {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </button>

        <div className="flex-1 min-w-0 overflow-hidden">
          <div className="flex items-center gap-2">
            <Bot size={14} className={isSubagent ? "text-primary/60 flex-shrink-0" : "text-muted-foreground flex-shrink-0"} />
            <span className="font-medium truncate text-sm" style={{ minWidth: 0 }}>{session.agent_id}</span>
            <span className="flex-shrink-0"><StatusBadge status={session.status} /></span>
          </div>
        </div>

        {/* Compact info */}
        <div className="flex items-center gap-3 text-xs text-muted-foreground flex-shrink-0">
          {totalTokens > 0 && (
            <span title={`In: ${session.tokens_in} / Out: ${session.tokens_out}`}>
              {totalTokens.toLocaleString()} tok
            </span>
          )}
          {duration > 0 && (
            <span>{formatDuration(duration)}</span>
          )}
        </div>

        <div className="flex items-center gap-1 flex-shrink-0" onClick={(e) => e.stopPropagation()}>
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
        <div className="px-8 py-3 bg-muted/30 text-xs">
          <div className="grid grid-cols-2 gap-x-4 gap-y-2">
            <div>
              <span className="text-muted-foreground">Conversation:</span>{" "}
              <span className="font-mono">{session.conversation_id}</span>
            </div>
            <div>
              <span className="text-muted-foreground">Tokens:</span>{" "}
              <span>{session.tokens_in.toLocaleString()} in / {session.tokens_out.toLocaleString()} out</span>
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
                <span className="font-mono">{session.parent_session_id}</span>
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
// Conversation Group Component (for Session History)
// ============================================================================

interface ConversationGroupProps {
  group: ConversationGroup;
  isExpanded: boolean;
  onToggleGroup: () => void;
  onOpenChat: (session: ExecutionSession) => void;
}

function ConversationGroupComponent({
  group,
  isExpanded,
  onToggleGroup,
  onOpenChat,
}: ConversationGroupProps) {
  const totalTokens = group.totalTokens.in + group.totalTokens.out;

  // Count subagents (sessions with parent_session_id)
  const subagentCount = group.sessions.filter(s => s.parent_session_id).length;

  return (
    <div className="border-b border-border">
      {/* Group header */}
      <div
        className="flex items-center gap-2 p-3 hover:bg-muted/50 cursor-pointer"
        onClick={onToggleGroup}
      >
        <button className="p-1 hover:bg-muted rounded flex-shrink-0">
          {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </button>

        <div className="flex-1 min-w-0 overflow-hidden">
          <div className="flex items-center gap-2">
            <Bot size={14} className="text-muted-foreground flex-shrink-0" />
            <span className="font-medium text-sm">root</span>
            {subagentCount > 0 && (
              <span className="text-xs text-primary">
                +{subagentCount} subagent{subagentCount > 1 ? 's' : ''}
              </span>
            )}
            <span className="flex-shrink-0"><StatusBadge status={group.overallStatus} /></span>
          </div>
        </div>

        {/* Aggregated info */}
        <div className="flex items-center gap-3 text-xs text-muted-foreground flex-shrink-0">
          {totalTokens > 0 && (
            <span title={`In: ${group.totalTokens.in} / Out: ${group.totalTokens.out}`}>
              {totalTokens.toLocaleString()} tok
            </span>
          )}
          <span>{new Date(group.createdAt).toLocaleDateString()}</span>
        </div>

        {/* Open primary chat */}
        <div className="flex items-center gap-1 flex-shrink-0" onClick={(e) => e.stopPropagation()}>
          <button
            className="btn btn--secondary btn--sm"
            onClick={() => onOpenChat(group.primarySession)}
            title="Open chat"
          >
            <MessageSquare size={14} />
          </button>
        </div>
      </div>

      {/* Expanded: show subagents only */}
      {isExpanded && subagentCount > 0 && (
        <div className="border-t border-border/50 bg-muted/10">
          {group.sessions
            .filter(s => s.parent_session_id)
            .map((session) => (
              <div
                key={session.id}
                className="flex items-center gap-2 py-2 px-4 hover:bg-muted/30"
                style={{ paddingLeft: 40 }}
              >
                <span className="text-muted-foreground/50">↳</span>
                <Bot size={12} className="text-primary/60" />
                <span className="text-sm">{session.agent_id}</span>
                <StatusBadge status={session.status} />

                {(session.tokens_in + session.tokens_out) > 0 && (
                  <span className="text-xs text-muted-foreground ml-auto">
                    {(session.tokens_in + session.tokens_out).toLocaleString()} tok
                  </span>
                )}

                <button
                  className="btn btn--ghost btn--sm p-1"
                  onClick={() => onOpenChat(session)}
                  title="View subagent chat (read-only)"
                >
                  <MessageSquare size={12} />
                </button>
              </div>
            ))}
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
  const navigate = useNavigate();
  const [allSessions, setAllSessions] = useState<ExecutionSession[]>([]);
  const [statusCounts, setStatusCounts] = useState<Record<string, number>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedSession, setExpandedSession] = useState<string | null>(null);
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [processingSession, setProcessingSession] = useState<string | null>(null);
  const [activeFilter, setActiveFilter] = useState<ExecutionStatus | "all">("all");
  const [historyFilter, setHistoryFilter] = useState<ExecutionStatus | "all">("all");
  const [autoRefresh, setAutoRefresh] = useState(true);

  // Chat slider state
  const [selectedSession, setSelectedSession] = useState<{
    conversationId: string;
    agentId: string;
    isSubagent: boolean;
  } | null>(null);

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

  // Build conversation groups for history (grouped by conversation)
  const conversationGroups = buildConversationGroups(filteredClosedSessions);

  // Toggle handler for expanding conversation groups
  const handleToggleGroup = useCallback((conversationId: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(conversationId)) {
        next.delete(conversationId);
      } else {
        next.add(conversationId);
      }
      return next;
    });
  }, []);

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

  const handleOpenChat = useCallback((session: ExecutionSession) => {
    const isSubagent = !!session.parent_session_id;
    setSelectedSession({
      conversationId: session.conversation_id,
      agentId: session.agent_id,
      isSubagent,
    });
  }, []);

  const handleCloseChat = useCallback(() => {
    setSelectedSession(null);
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

  // Calculate stats based on conversations, not individual sessions
  // Active sessions still use raw counts (they're in-progress)
  const runningCount = statusCounts.running || 0;
  const pausedCount = statusCounts.paused || 0;
  const queuedCount = statusCounts.queued || 0;
  const activeCount = runningCount + pausedCount + queuedCount;

  // Completed count = number of completed conversations (from ALL closed sessions, not filtered)
  const allConversationGroups = buildConversationGroups(closedSessions);
  const completedCount = allConversationGroups.filter(g => g.overallStatus === 'completed').length;

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
            <button
              className="btn btn--primary btn--md"
              onClick={() => navigate("/chat")}
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

        {/* Two-column layout for sessions - equal width columns */}
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
          <div className="card" style={{ minHeight: "400px", minWidth: 0, display: "flex", flexDirection: "column", overflow: "hidden" }}>
            <div
              className="flex items-center justify-between border-b border-border"
              style={{ padding: "16px 20px" }}
            >
              <div className="flex items-center gap-3">
                <History size={18} className="text-muted-foreground" />
                <h2 className="font-semibold">Session History</h2>
                <span className="badge" title={`${conversationGroups.length} conversations, ${filteredClosedSessions.length} total sessions`}>
                  {conversationGroups.length}
                </span>
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
              {conversationGroups.length === 0 ? (
                <div className="p-8 text-center text-muted-foreground" style={{ height: "100%", display: "flex", flexDirection: "column", justifyContent: "center", alignItems: "center" }}>
                  <History size={40} className="mx-auto mb-3 opacity-30" />
                  <p className="text-sm">No session history</p>
                </div>
              ) : (
                <>
                  {conversationGroups.slice(0, 50).map((group) => (
                    <ConversationGroupComponent
                      key={group.conversationId}
                      group={group}
                      isExpanded={expandedGroups.has(group.conversationId)}
                      onToggleGroup={() => handleToggleGroup(group.conversationId)}
                      onOpenChat={handleOpenChat}
                    />
                  ))}
                  {conversationGroups.length > 50 && (
                    <div className="p-3 text-center text-sm text-muted-foreground border-t border-border">
                      Showing 50 of {conversationGroups.length} conversations
                    </div>
                  )}
                </>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Chat Slider */}
      <ChatSlider isOpen={selectedSession !== null} onClose={handleCloseChat}>
        {selectedSession && (
          <SessionChatViewer
            conversationId={selectedSession.conversationId}
            agentId={selectedSession.agentId}
            readOnly={selectedSession.isSubagent}
          />
        )}
      </ChatSlider>
    </div>
  );
}

export default WebOpsDashboard;
