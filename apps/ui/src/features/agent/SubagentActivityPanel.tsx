// ============================================================================
// SUBAGENT ACTIVITY PANEL
// Shows real-time activity of delegated subagents
// ============================================================================

import { useState } from "react";
import {
  GitBranch,
  ChevronDown,
  ChevronUp,
  Loader2,
  CheckCircle2,
  Wrench,
  MessageSquare,
  XCircle,
} from "lucide-react";

// ============================================================================
// Types
// ============================================================================

export interface SubagentActivity {
  childAgentId: string;
  childConversationId: string;
  task: string;
  startedAt: Date;
  status: "running" | "completed" | "error";
  completedAt?: Date;
  result?: string;
  error?: string;
  tokens: number;
  toolCalls: ToolCallActivity[];
}

interface ToolCallActivity {
  toolName: string;
  status: "running" | "completed" | "error";
  result?: string;
  timestamp: Date;
}

interface SubagentActivityPanelProps {
  activities: Map<string, SubagentActivity>;
  onClose?: (conversationId: string) => void;
}

// ============================================================================
// Component
// ============================================================================

export function SubagentActivityPanel({
  activities,
  onClose,
}: SubagentActivityPanelProps) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const toggleExpanded = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const activityList = Array.from(activities.values());
  const runningCount = activityList.filter((a) => a.status === "running").length;

  if (activityList.length === 0) {
    return null;
  }

  return (
    <div className="border-t border-[var(--border)] bg-[var(--card)]">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-[var(--border)]">
        <div className="flex items-center gap-2">
          <GitBranch className="w-4 h-4 text-violet-500" />
          <span className="text-sm font-medium text-[var(--foreground)]">
            Subagent Activity
          </span>
          {runningCount > 0 && (
            <span className="text-xs bg-violet-100 text-violet-700 px-2 py-0.5 rounded-full">
              {runningCount} running
            </span>
          )}
        </div>
      </div>

      {/* Activity List */}
      <div className="max-h-64 overflow-y-auto">
        {activityList.map((activity) => (
          <SubagentActivityItem
            key={activity.childConversationId}
            activity={activity}
            isExpanded={expanded.has(activity.childConversationId)}
            onToggle={() => toggleExpanded(activity.childConversationId)}
            onClose={onClose}
          />
        ))}
      </div>
    </div>
  );
}

// ============================================================================
// Subagent Activity Item
// ============================================================================

interface SubagentActivityItemProps {
  activity: SubagentActivity;
  isExpanded: boolean;
  onToggle: () => void;
  onClose?: (conversationId: string) => void;
}

function SubagentActivityItem({
  activity,
  isExpanded,
  onToggle,
  onClose,
}: SubagentActivityItemProps) {
  const duration = activity.completedAt
    ? Math.round((activity.completedAt.getTime() - activity.startedAt.getTime()) / 1000)
    : Math.round((Date.now() - activity.startedAt.getTime()) / 1000);

  return (
    <div className="border-b border-[var(--border)] last:border-b-0">
      {/* Item Header */}
      <button
        onClick={onToggle}
        className="w-full flex items-center justify-between px-4 py-3 hover:bg-[var(--muted)] transition-colors text-left"
      >
        <div className="flex items-center gap-3 min-w-0">
          {/* Status Icon */}
          {activity.status === "running" ? (
            <Loader2 className="w-4 h-4 text-violet-500 animate-spin flex-shrink-0" />
          ) : activity.status === "completed" ? (
            <CheckCircle2 className="w-4 h-4 text-emerald-500 flex-shrink-0" />
          ) : (
            <XCircle className="w-4 h-4 text-red-500 flex-shrink-0" />
          )}

          {/* Agent Info */}
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium text-[var(--foreground)]">
                {activity.childAgentId}
              </span>
              <span className="text-xs text-[var(--muted-foreground)]">
                {formatDuration(duration)}
              </span>
            </div>
            <p className="text-xs text-[var(--muted-foreground)] truncate max-w-md">
              {activity.task}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2 flex-shrink-0">
          {/* Stats */}
          <div className="flex items-center gap-3 text-xs text-[var(--muted-foreground)]">
            {activity.tokens > 0 && (
              <span className="flex items-center gap-1">
                <MessageSquare className="w-3 h-3" />
                {activity.tokens}
              </span>
            )}
            {activity.toolCalls.length > 0 && (
              <span className="flex items-center gap-1">
                <Wrench className="w-3 h-3" />
                {activity.toolCalls.length}
              </span>
            )}
          </div>

          {/* Expand Toggle */}
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-[var(--muted-foreground)]" />
          ) : (
            <ChevronDown className="w-4 h-4 text-[var(--muted-foreground)]" />
          )}
        </div>
      </button>

      {/* Expanded Content */}
      {isExpanded && (
        <div className="px-4 pb-3 space-y-3">
          {/* Task Description */}
          <div className="text-xs bg-[var(--muted)] rounded-lg p-3">
            <div className="font-medium text-[var(--muted-foreground)] mb-1">Task</div>
            <p className="text-[var(--foreground)]">{activity.task}</p>
          </div>

          {/* Tool Calls */}
          {activity.toolCalls.length > 0 && (
            <div>
              <div className="text-xs font-medium text-[var(--muted-foreground)] mb-2">
                Tool Calls
              </div>
              <div className="space-y-1">
                {activity.toolCalls.map((tool, index) => (
                  <div
                    key={index}
                    className="flex items-center gap-2 text-xs bg-amber-50 text-amber-900 px-2 py-1 rounded"
                  >
                    {tool.status === "running" ? (
                      <Loader2 className="w-3 h-3 animate-spin" />
                    ) : tool.status === "completed" ? (
                      <CheckCircle2 className="w-3 h-3 text-emerald-600" />
                    ) : (
                      <XCircle className="w-3 h-3 text-red-600" />
                    )}
                    <span className="font-medium">{tool.toolName}</span>
                    {tool.result && (
                      <span className="text-amber-700 truncate max-w-xs">
                        {tool.result.substring(0, 50)}
                        {tool.result.length > 50 ? "..." : ""}
                      </span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Result */}
          {activity.status === "completed" && activity.result && (
            <div className="text-xs bg-emerald-50 rounded-lg p-3">
              <div className="font-medium text-emerald-700 mb-1">Result</div>
              <p className="text-emerald-900">
                {activity.result.substring(0, 300)}
                {activity.result.length > 300 ? "..." : ""}
              </p>
            </div>
          )}

          {/* Error */}
          {activity.status === "error" && activity.error && (
            <div className="text-xs bg-red-50 rounded-lg p-3">
              <div className="font-medium text-red-700 mb-1">Error</div>
              <p className="text-red-900">{activity.error}</p>
            </div>
          )}

          {/* Close Button (for completed/errored) */}
          {activity.status !== "running" && onClose && (
            <button
              onClick={() => onClose(activity.childConversationId)}
              className="text-xs text-[var(--muted-foreground)] hover:text-[var(--foreground)]"
            >
              Dismiss
            </button>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Helpers
// ============================================================================

function formatDuration(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes}m ${remainingSeconds}s`;
}
