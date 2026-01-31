// ============================================================================
// WEB LOGS PANEL
// Compact status view with activity stream drill-down
// ============================================================================

import { useEffect, useState, useCallback, useMemo } from "react";
import {
  Activity,
  AlertCircle,
  Bot,
  Check,
  ChevronDown,
  ChevronRight,
  Filter,
  Loader2,
  MessageSquare,
  RefreshCw,
  Search,
  Trash2,
  XCircle,
  ArrowRight,
  GitBranch,
  Zap,
  Play,
  Square,
} from "lucide-react";
import { getTransport, type LogSession, type SessionDetail, type ExecutionLog, type LogLevel } from "@/services/transport";

// ============================================================================
// Types
// ============================================================================

interface LocalFilter {
  agentId: string;
  level: LogLevel | "";
  search: string;
}

interface SessionTreeNode {
  session: LogSession;
  children: SessionTreeNode[];
}

// ============================================================================
// Helper Functions
// ============================================================================

function buildSessionTree(sessions: LogSession[]): SessionTreeNode[] {
  const sessionMap = new Map<string, SessionTreeNode>();
  const roots: SessionTreeNode[] = [];

  for (const session of sessions) {
    sessionMap.set(session.session_id, { session, children: [] });
  }

  for (const session of sessions) {
    const node = sessionMap.get(session.session_id)!;
    if (session.parent_session_id) {
      let foundParent = false;
      for (const [, parentNode] of sessionMap) {
        if (parentNode.session.conversation_id === session.parent_session_id) {
          parentNode.children.push(node);
          foundParent = true;
          break;
        }
      }
      if (!foundParent) roots.push(node);
    } else {
      roots.push(node);
    }
  }

  roots.sort((a, b) => new Date(b.session.started_at).getTime() - new Date(a.session.started_at).getTime());
  return roots;
}

// Format duration nicely
function formatDuration(ms?: number): string {
  if (ms === undefined) return "—";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
}

// Format time as HH:MM:SS
function formatTime(timestamp: string): string {
  return new Date(timestamp).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
}

// ============================================================================
// Status Indicator Component
// ============================================================================

function StatusDot({ status }: { status: string }) {
  const config: Record<string, { color: string; animate?: boolean; icon?: React.ReactNode }> = {
    completed: { color: "var(--success)", icon: <Check style={{ width: 10, height: 10 }} /> },
    error: { color: "var(--destructive)", icon: <XCircle style={{ width: 10, height: 10 }} /> },
    running: { color: "var(--primary)", animate: true },
    stopped: { color: "var(--warning)", icon: <Square style={{ width: 8, height: 8 }} /> },
    pending: { color: "var(--muted-foreground)" },
  };

  const { color, animate, icon } = config[status] || config.pending;

  return (
    <div
      style={{
        width: 20,
        height: 20,
        borderRadius: "50%",
        backgroundColor: animate ? "transparent" : color,
        border: animate ? `2px solid ${color}` : "none",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: "#fff",
        flexShrink: 0,
        position: "relative",
      }}
    >
      {animate && (
        <div
          style={{
            position: "absolute",
            inset: -2,
            borderRadius: "50%",
            border: `2px solid ${color}`,
            borderTopColor: "transparent",
            animation: "spin 1s linear infinite",
          }}
        />
      )}
      {icon}
    </div>
  );
}

// ============================================================================
// Activity Stream Item Component
// ============================================================================

function ActivityItem({ log, isExpanded, onToggle }: {
  log: ExecutionLog;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  const getIcon = () => {
    switch (log.category) {
      case "tool_call": return <Zap style={{ width: 14, height: 14, color: "var(--primary)" }} />;
      case "tool_result": return <Check style={{ width: 14, height: 14, color: "var(--success)" }} />;
      case "delegation": return <GitBranch style={{ width: 14, height: 14, color: "var(--warning)" }} />;
      case "session": return <Play style={{ width: 14, height: 14, color: "var(--muted-foreground)" }} />;
      case "error": return <AlertCircle style={{ width: 14, height: 14, color: "var(--destructive)" }} />;
      default: return <ArrowRight style={{ width: 14, height: 14, color: "var(--muted-foreground)" }} />;
    }
  };

  const getLevelStyle = () => {
    switch (log.level) {
      case "error": return { color: "var(--destructive)" };
      case "warn": return { color: "var(--warning)" };
      default: return { color: "var(--foreground)" };
    }
  };

  const hasMetadata = log.metadata && Object.keys(log.metadata).length > 0;

  return (
    <div style={{ fontSize: "var(--text-sm)" }}>
      <button
        onClick={hasMetadata ? onToggle : undefined}
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--spacing-3)",
          width: "100%",
          padding: "var(--spacing-2) 0",
          background: "none",
          border: "none",
          cursor: hasMetadata ? "pointer" : "default",
          textAlign: "left",
        }}
      >
        <span style={{
          fontFamily: "var(--font-mono)",
          color: "var(--muted-foreground)",
          minWidth: 70,
          flexShrink: 0,
        }}>
          {formatTime(log.timestamp)}
        </span>

        {getIcon()}

        <span style={{ ...getLevelStyle(), flex: 1, minWidth: 0 }}>
          {log.message}
        </span>

        {log.duration_ms !== undefined && (
          <span style={{ color: "var(--muted-foreground)", flexShrink: 0, fontFamily: "var(--font-mono)" }}>
            {formatDuration(log.duration_ms)}
          </span>
        )}

        {hasMetadata && (
          <ChevronRight
            style={{
              width: 14,
              height: 14,
              color: "var(--muted-foreground)",
              transform: isExpanded ? "rotate(90deg)" : "none",
              transition: "transform 0.15s",
              flexShrink: 0,
            }}
          />
        )}
      </button>

      {isExpanded && hasMetadata && (
        <pre
          style={{
            margin: "var(--spacing-1) 0 var(--spacing-3) 86px",
            padding: "var(--spacing-3)",
            backgroundColor: "var(--muted)",
            borderRadius: "var(--radius-sm)",
            fontSize: "var(--text-sm)",
            overflow: "auto",
            maxHeight: 240,
          }}
        >
          {JSON.stringify(log.metadata, null, 2)}
        </pre>
      )}
    </div>
  );
}

// ============================================================================
// Compact Session Row Component
// ============================================================================

function SessionRow({
  node,
  depth = 0,
  expandedSessions,
  loadingDetails,
  sessionDetails,
  expandedLogs,
  onToggleSession,
  onToggleLog,
  onDelete,
}: {
  node: SessionTreeNode;
  depth?: number;
  expandedSessions: Set<string>;
  loadingDetails: Set<string>;
  sessionDetails: Map<string, SessionDetail>;
  expandedLogs: Set<string>;
  onToggleSession: (id: string) => void;
  onToggleLog: (id: string) => void;
  onDelete: (id: string, e: React.MouseEvent) => void;
}) {
  const { session, children } = node;
  const isExpanded = expandedSessions.has(session.session_id);
  const isLoading = loadingDetails.has(session.session_id);
  const detail = sessionDetails.get(session.session_id);
  const hasChildren = children.length > 0;
  const isRoot = depth === 0;

  return (
    <div>
      {/* Compact row */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--spacing-3)",
          padding: "var(--spacing-3) var(--spacing-4)",
          marginLeft: depth * 28,
          borderRadius: "var(--radius-sm)",
          backgroundColor: isExpanded ? "var(--muted)" : "transparent",
          borderLeft: depth > 0 ? "2px solid var(--border)" : "none",
        }}
      >
        {/* Expand/collapse */}
        <button
          onClick={() => onToggleSession(session.session_id)}
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            width: 24,
            height: 24,
            background: "none",
            border: "none",
            cursor: "pointer",
            padding: 0,
            flexShrink: 0,
          }}
        >
          {isLoading ? (
            <Loader2 style={{ width: 16, height: 16, animation: "spin 1s linear infinite" }} />
          ) : isExpanded ? (
            <ChevronDown style={{ width: 18, height: 18, color: "var(--muted-foreground)" }} />
          ) : (
            <ChevronRight style={{ width: 18, height: 18, color: "var(--muted-foreground)" }} />
          )}
        </button>

        {/* Status dot */}
        <StatusDot status={session.status} />

        {/* Agent name */}
        <div style={{ display: "flex", alignItems: "center", gap: "var(--spacing-2)", minWidth: 0, flex: 1 }}>
          <Bot style={{ width: 16, height: 16, color: "var(--muted-foreground)", flexShrink: 0 }} />
          <span style={{
            fontWeight: 500,
            fontSize: "var(--text-base)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}>
            {session.agent_name || session.agent_id}
          </span>

          {/* Child count badge */}
          {hasChildren && (
            <span
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 2,
                fontSize: "var(--text-xs)",
                color: "var(--muted-foreground)",
                backgroundColor: "var(--muted)",
                padding: "1px 6px",
                borderRadius: "var(--radius-full)",
                flexShrink: 0,
              }}
            >
              <GitBranch style={{ width: 10, height: 10 }} />
              {children.length}
            </span>
          )}
        </div>

        {/* Inline metrics */}
        <div style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--spacing-4)",
          fontSize: "var(--text-sm)",
          color: "var(--muted-foreground)",
          flexShrink: 0,
        }}>
          {/* Tool calls */}
          <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <Zap style={{ width: 14, height: 14 }} />
            {session.tool_call_count}
          </span>

          {/* Messages (approximated from token count) */}
          <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <MessageSquare style={{ width: 14, height: 14 }} />
            {Math.ceil(session.token_count / 500) || "—"}
          </span>

          {/* Errors */}
          {session.error_count > 0 && (
            <span style={{ display: "flex", alignItems: "center", gap: 6, color: "var(--destructive)" }}>
              <AlertCircle style={{ width: 14, height: 14 }} />
              {session.error_count}
            </span>
          )}

          {/* Duration */}
          <span style={{ minWidth: 50, textAlign: "right", fontFamily: "var(--font-mono)" }}>
            {formatDuration(session.duration_ms)}
          </span>
        </div>

        {/* Delete - only at root level */}
        {isRoot ? (
          <button
            onClick={(e) => onDelete(session.session_id, e)}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              width: 28,
              height: 28,
              background: "none",
              border: "none",
              cursor: "pointer",
              borderRadius: "var(--radius-sm)",
              color: "var(--muted-foreground)",
              flexShrink: 0,
              opacity: 0.5,
            }}
            onMouseEnter={(e) => (e.currentTarget.style.opacity = "1")}
            onMouseLeave={(e) => (e.currentTarget.style.opacity = "0.5")}
          >
            <Trash2 style={{ width: 14, height: 14 }} />
          </button>
        ) : (
          <div style={{ width: 28, flexShrink: 0 }} />
        )}
      </div>

      {/* Expanded: Activity Stream */}
      {isExpanded && detail && (
        <div
          style={{
            marginLeft: depth * 24 + 24,
            padding: "var(--spacing-2) var(--spacing-3)",
            borderLeft: "2px solid var(--border)",
            marginBottom: "var(--spacing-2)",
          }}
        >
          {detail.logs.length === 0 ? (
            <div style={{
              fontSize: "var(--text-xs)",
              color: "var(--muted-foreground)",
              padding: "var(--spacing-2) 0",
            }}>
              No activity recorded
            </div>
          ) : (
            <div>
              {detail.logs.map((log) => (
                <ActivityItem
                  key={log.id}
                  log={log}
                  isExpanded={expandedLogs.has(log.id)}
                  onToggle={() => onToggleLog(log.id)}
                />
              ))}
            </div>
          )}
        </div>
      )}

      {/* Render children */}
      {children.map((child) => (
        <SessionRow
          key={child.session.session_id}
          node={child}
          depth={depth + 1}
          expandedSessions={expandedSessions}
          loadingDetails={loadingDetails}
          sessionDetails={sessionDetails}
          expandedLogs={expandedLogs}
          onToggleSession={onToggleSession}
          onToggleLog={onToggleLog}
          onDelete={onDelete}
        />
      ))}
    </div>
  );
}

// ============================================================================
// Main Component
// ============================================================================

export function WebLogsPanel() {
  const [sessions, setSessions] = useState<LogSession[]>([]);
  const [sessionDetails, setSessionDetails] = useState<Map<string, SessionDetail>>(new Map());
  const [isLoading, setIsLoading] = useState(true);
  const [loadingDetails, setLoadingDetails] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<LocalFilter>({ agentId: "", level: "", search: "" });
  const [agents, setAgents] = useState<string[]>([]);
  const [expandedSessions, setExpandedSessions] = useState<Set<string>>(new Set());
  const [expandedLogs, setExpandedLogs] = useState<Set<string>>(new Set());

  // Load sessions
  const loadSessions = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const transport = await getTransport();
      const result = await transport.listLogSessions({
        agent_id: filter.agentId || undefined,
        level: filter.level || undefined,
        limit: 100,
      });
      if (result.success && result.data) {
        setSessions(result.data);
        const uniqueAgents = [...new Set(result.data.map((s) => s.agent_id))];
        setAgents(uniqueAgents);
      } else {
        setError(result.error || "Failed to load sessions");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoading(false);
    }
  }, [filter.agentId, filter.level]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // Toggle session expansion
  const toggleSession = async (sessionId: string) => {
    if (expandedSessions.has(sessionId)) {
      setExpandedSessions((prev) => {
        const next = new Set(prev);
        next.delete(sessionId);
        return next;
      });
      return;
    }

    // Load details if not already loaded
    if (!sessionDetails.has(sessionId)) {
      setLoadingDetails((prev) => new Set(prev).add(sessionId));
      try {
        const transport = await getTransport();
        const result = await transport.getLogSession(sessionId);
        if (result.success && result.data) {
          setSessionDetails((prev) => new Map(prev).set(sessionId, result.data!));
        }
      } catch (err) {
        console.error("Failed to load session detail:", err);
      } finally {
        setLoadingDetails((prev) => {
          const next = new Set(prev);
          next.delete(sessionId);
          return next;
        });
      }
    }

    setExpandedSessions((prev) => new Set(prev).add(sessionId));
  };

  // Toggle log expansion
  const toggleLog = (logId: string) => {
    setExpandedLogs((prev) => {
      const next = new Set(prev);
      if (next.has(logId)) {
        next.delete(logId);
      } else {
        next.add(logId);
      }
      return next;
    });
  };

  // Delete session
  const deleteSession = async (sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm("Delete this session and all its logs?")) return;

    try {
      const transport = await getTransport();
      await transport.deleteLogSession(sessionId);
      setSessions((prev) => prev.filter((s) => s.session_id !== sessionId));
      setExpandedSessions((prev) => {
        const next = new Set(prev);
        next.delete(sessionId);
        return next;
      });
      setSessionDetails((prev) => {
        const next = new Map(prev);
        next.delete(sessionId);
        return next;
      });
    } catch (err) {
      console.error("Failed to delete session:", err);
    }
  };

  // Clear all logs and execution sessions
  const clearAll = async () => {
    if (!confirm("Clear all logs and execution history? This cannot be undone.")) return;

    try {
      const transport = await getTransport();
      // Clear both logs and execution sessions
      await Promise.all([
        transport.cleanupOldLogs(0), // 0 days = delete all
        transport.cleanupExecutionSessions(), // delete all execution sessions
      ]);
      setSessions([]);
      setExpandedSessions(new Set());
      setSessionDetails(new Map());
    } catch (err) {
      console.error("Failed to clear logs:", err);
    }
  };

  // Filter and build tree
  const filteredSessions = useMemo(() => {
    if (!filter.search) return sessions;
    const searchLower = filter.search.toLowerCase();
    return sessions.filter(
      (session) =>
        session.session_id.toLowerCase().includes(searchLower) ||
        session.agent_id.toLowerCase().includes(searchLower) ||
        (session.agent_name?.toLowerCase().includes(searchLower)) ||
        session.conversation_id.toLowerCase().includes(searchLower)
    );
  }, [sessions, filter.search]);

  const sessionTree = useMemo(() => buildSessionTree(filteredSessions), [filteredSessions]);

  return (
    <div className="page">
      <div className="page-container" style={{ maxWidth: "100%", padding: "0 var(--spacing-4)" }}>
        {/* Header */}
        <div className="page-header">
          <div className="page-header__content">
            <h1 className="page-title">Execution Logs</h1>
            <p className="page-subtitle">Monitor agent activity and trace execution</p>
          </div>
          <div className="page-header__actions" style={{ display: "flex", gap: "var(--spacing-2)" }}>
            <button onClick={loadSessions} className="btn btn--secondary btn--sm" disabled={isLoading}>
              <RefreshCw style={{ width: 16, height: 16 }} className={isLoading ? "animate-spin" : ""} />
              Refresh
            </button>
            {sessions.length > 0 && (
              <button onClick={clearAll} className="btn btn--destructive btn--sm">
                <Trash2 style={{ width: 16, height: 16 }} />
                Clear All
              </button>
            )}
          </div>
        </div>

        {/* Filters */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "var(--spacing-4)",
            marginBottom: "var(--spacing-5)",
            padding: "var(--spacing-3) var(--spacing-4)",
            backgroundColor: "var(--card)",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--border)",
          }}
        >
          <Filter style={{ width: 16, height: 16, color: "var(--muted-foreground)", flexShrink: 0 }} />

          <select
            value={filter.agentId}
            onChange={(e) => setFilter((f) => ({ ...f, agentId: e.target.value }))}
            className="form-input"
            style={{ width: "auto", minWidth: 200, padding: "var(--spacing-2) var(--spacing-3)" }}
          >
            <option value="">All Agents</option>
            {agents.map((agent) => (
              <option key={agent} value={agent}>{agent}</option>
            ))}
          </select>

          <select
            value={filter.level}
            onChange={(e) => setFilter((f) => ({ ...f, level: e.target.value as LogLevel | "" }))}
            className="form-input"
            style={{ width: "auto", minWidth: 120, padding: "var(--spacing-2) var(--spacing-3)" }}
          >
            <option value="">All Levels</option>
            <option value="error">Error</option>
            <option value="warn">Warning</option>
            <option value="info">Info</option>
            <option value="debug">Debug</option>
          </select>

          <div style={{ position: "relative", flex: 1, minWidth: 200, maxWidth: 320 }}>
            <Search
              style={{
                position: "absolute",
                left: 12,
                top: "50%",
                transform: "translateY(-50%)",
                width: 16,
                height: 16,
                color: "var(--muted-foreground)",
                pointerEvents: "none",
              }}
            />
            <input
              type="text"
              placeholder="Search sessions..."
              value={filter.search}
              onChange={(e) => setFilter((f) => ({ ...f, search: e.target.value }))}
              className="form-input"
              style={{ paddingLeft: 40 }}
            />
          </div>
        </div>

        {/* Error state */}
        {error && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: "var(--spacing-2)",
              padding: "var(--spacing-3)",
              backgroundColor: "var(--destructive-muted)",
              borderRadius: "var(--radius-md)",
              marginBottom: "var(--spacing-4)",
            }}
          >
            <AlertCircle style={{ width: 16, height: 16, color: "var(--destructive)" }} />
            <span style={{ color: "var(--destructive)", fontSize: "var(--text-sm)" }}>{error}</span>
          </div>
        )}

        {/* Loading state */}
        {isLoading && (
          <div style={{ display: "flex", flexDirection: "column", alignItems: "center", padding: "var(--spacing-12)", gap: "var(--spacing-3)" }}>
            <Loader2 style={{ width: 24, height: 24, animation: "spin 1s linear infinite" }} />
            <span style={{ color: "var(--muted-foreground)" }}>Loading sessions...</span>
          </div>
        )}

        {/* Empty state */}
        {!isLoading && sessionTree.length === 0 && (
          <div style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            padding: "var(--spacing-12)",
            gap: "var(--spacing-3)",
          }}>
            <Activity style={{ width: 32, height: 32, color: "var(--muted-foreground)" }} />
            <span style={{ fontWeight: 500 }}>No Sessions Found</span>
            <span style={{ color: "var(--muted-foreground)", fontSize: "var(--text-sm)" }}>
              {filter.agentId || filter.level || filter.search
                ? "No sessions match your filters"
                : "Sessions will appear here when agents run"}
            </span>
          </div>
        )}

        {/* Sessions list */}
        {!isLoading && sessionTree.length > 0 && (
          <div
            className="card"
            style={{
              padding: "var(--spacing-3)",
              display: "flex",
              flexDirection: "column",
              gap: "var(--spacing-1)",
            }}
          >
            {sessionTree.map((node) => (
              <SessionRow
                key={node.session.session_id}
                node={node}
                expandedSessions={expandedSessions}
                loadingDetails={loadingDetails}
                sessionDetails={sessionDetails}
                expandedLogs={expandedLogs}
                onToggleSession={toggleSession}
                onToggleLog={toggleLog}
                onDelete={deleteSession}
              />
            ))}
          </div>
        )}
      </div>

      {/* Keyframe for spinner */}
      <style>{`
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
      `}</style>
    </div>
  );
}
