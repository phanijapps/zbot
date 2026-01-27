// ============================================================================
// ACTIVITY PANEL
// Unified panel for tool calls and TODOs with tabs
// ============================================================================

import { useState, useEffect, useMemo, useRef } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  Activity,
  X,
  Loader2,
  RefreshCw,
  Circle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Bot,
  Cpu,
  Wrench,
  CheckSquare,
  AlertCircle,
  Clock,
} from "lucide-react";
import { cn } from "@/shared/utils";
import type {
  TodoList,
  Todo,
  ActivityItem,
} from "@/shared/types";
import { getAgentTodos, updateAgentTodo } from "@/services/agentChannels";

// ============================================================================
// TYPES
// ============================================================================

interface ActivityPanelProps {
  open: boolean;
  onClose: () => void;
  agentId: string;
  sessionId?: string;
}

type ActivityTab = "all" | "tools" | "todos";

interface AgentGroup {
  agentId: string;
  agentName: string;
  isOrchestrator: boolean;
  isSubagent: boolean;
  todos: Todo[];
  toolCalls: ActivityItem[];
  pendingTodos: number;
  completedTodos: number;
  runningTools: number;
  completedTools: number;
}

// ============================================================================
// COMPONENT
// ============================================================================

export function ActivityPanel({
  open,
  onClose,
  agentId,
  sessionId,
}: ActivityPanelProps) {
  const [tab, setTab] = useState<ActivityTab>("all");
  const [todos, setTodos] = useState<TodoList | null>(null);
  const [activity, setActivity] = useState<ActivityItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updatingIds, setUpdatingIds] = useState<Set<string>>(new Set());
  const [collapsedAgents, setCollapsedAgents] = useState<Set<string>>(
    new Set()
  );
  const unlistenRef = useRef<UnlistenFn | null>(null);

  // Load TODOs when panel opens
  useEffect(() => {
    if (open && agentId) {
      loadTodos();
    }
  }, [open, agentId]);

  // Listen for events from the agent stream
  useEffect(() => {
    if (!open || !sessionId) return;

    const eventName = `agent-stream://${sessionId}`;

    const setupListener = async () => {
      if (unlistenRef.current) {
        unlistenRef.current();
      }

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      unlistenRef.current = await listen<any>(eventName, (event) => {
        const payload = event.payload as Record<string, unknown>;

        // Handle todo_update events
        if (payload.type === "todo_update" && payload.todos) {
          const incomingTodos = payload.todos as TodoList;

          // If from subagent, merge with existing TODOs instead of replacing
          if (payload.isSubagent || payload.fromSubagent) {
            setTodos((prev) => {
              if (!prev) {
                return incomingTodos;
              }
              // Merge: keep existing TODOs, add/update subagent TODOs
              const merged = { ...prev };
              const existingIds = new Set(prev.items.map((t) => t.id));

              for (const todo of incomingTodos.items) {
                if (existingIds.has(todo.id)) {
                  // Update existing TODO
                  merged.items = merged.items.map((t) =>
                    t.id === todo.id ? todo : t
                  );
                } else {
                  // Add new TODO from subagent
                  merged.items.push(todo);
                }
              }
              merged.lastUpdated = incomingTodos.lastUpdated;
              return merged;
            });
          } else {
            // From orchestrator - replace orchestrator TODOs, keep subagent TODOs
            setTodos((prev) => {
              if (!prev) {
                return incomingTodos;
              }
              // Keep subagent TODOs (isOrchestrator === false)
              const subagentTodos = prev.items.filter((t) => !t.isOrchestrator);
              // Replace with new orchestrator TODOs + existing subagent TODOs
              return {
                items: [...incomingTodos.items, ...subagentTodos],
                lastUpdated: incomingTodos.lastUpdated,
              };
            });
          }
        }

        // Handle activity_update events
        if (payload.type === "activity_update" && payload.activity) {
          setActivity(payload.activity as ActivityItem[]);
        }

        // Handle subagent tool events (from subagent streaming)
        if (
          payload.isSubagent &&
          (payload.type === "subagent_tool_call" ||
            payload.type === "subagent_tool_result")
        ) {
          handleSubagentToolEvent(payload);
        }
      });
    };

    setupListener();

    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    };
  }, [open, sessionId]);

  // Handle subagent tool events
  const handleSubagentToolEvent = (payload: Record<string, unknown>) => {
    const subagentId = payload.subagentId as string;
    const subagentName = payload.subagentName as string;

    if (payload.type === "subagent_tool_call") {
      // Add new running tool call from subagent
      const newItem: ActivityItem = {
        id: `subagent_tool_${payload.toolId}_${Date.now()}`,
        agentId: subagentId,
        agentName: subagentName,
        isOrchestrator: false,
        itemType: "tool_call",
        timestamp: new Date().toISOString(),
        toolCall: {
          id: payload.toolId as string,
          name: payload.toolName as string,
          status: "running",
          argumentsPreview: payload.args
            ? (payload.args as string).substring(0, 100)
            : undefined,
        },
      };
      setActivity((prev) => [...prev, newItem]);
    } else if (payload.type === "subagent_tool_result") {
      // Update existing tool call with result
      setActivity((prev) =>
        prev.map((item) => {
          if (
            item.toolCall &&
            item.toolCall.id === payload.toolId &&
            item.agentId === subagentId
          ) {
            return {
              ...item,
              toolCall: {
                ...item.toolCall,
                status: "success" as const,
                durationMs: payload.durationMs as number | undefined,
                resultPreview: payload.resultPreview as string | undefined,
              },
            };
          }
          return item;
        })
      );
    }
  };

  const loadTodos = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await getAgentTodos(agentId);
      setTodos(result);
    } catch (err) {
      console.error("Failed to load TODOs:", err);
      setError(err instanceof Error ? err.message : "Failed to load TODOs");
    } finally {
      setLoading(false);
    }
  };

  const handleToggleTodo = async (todoId: string, currentCompleted: boolean) => {
    setUpdatingIds((prev) => new Set([...prev, todoId]));
    try {
      await updateAgentTodo(agentId, todoId, !currentCompleted);
      setTodos((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          items: prev.items.map((item) =>
            item.id === todoId
              ? {
                  ...item,
                  completed: !currentCompleted,
                  completedAt: !currentCompleted
                    ? new Date().toISOString()
                    : undefined,
                }
              : item
          ),
          lastUpdated: new Date().toISOString(),
        };
      });
    } catch (err) {
      console.error("Failed to update TODO:", err);
    } finally {
      setUpdatingIds((prev) => {
        const next = new Set(prev);
        next.delete(todoId);
        return next;
      });
    }
  };

  const toggleAgentCollapse = (agentIdKey: string) => {
    setCollapsedAgents((prev) => {
      const next = new Set(prev);
      if (next.has(agentIdKey)) {
        next.delete(agentIdKey);
      } else {
        next.add(agentIdKey);
      }
      return next;
    });
  };

  // Group items by agent
  const agentGroups = useMemo((): AgentGroup[] => {
    const groups = new Map<string, AgentGroup>();
    const baseAgentName = agentId.split(".").pop() || agentId;

    // Add TODOs to groups
    if (todos?.items && (tab === "all" || tab === "todos")) {
      for (const todo of todos.items) {
        const key = todo.agentId || agentId;
        if (!groups.has(key)) {
          groups.set(key, {
            agentId: todo.agentId || agentId,
            agentName: todo.agentName || baseAgentName,
            isOrchestrator: todo.isOrchestrator ?? true,
            isSubagent: false,
            todos: [],
            toolCalls: [],
            pendingTodos: 0,
            completedTodos: 0,
            runningTools: 0,
            completedTools: 0,
          });
        }
        const group = groups.get(key)!;
        group.todos.push(todo);
        if (todo.completed) {
          group.completedTodos++;
        } else {
          group.pendingTodos++;
        }
      }
    }

    // Add tool calls to groups
    if (tab === "all" || tab === "tools") {
      for (const item of activity) {
        if (item.itemType !== "tool_call" || !item.toolCall) continue;

        const key = item.agentId;
        if (!groups.has(key)) {
          groups.set(key, {
            agentId: item.agentId,
            agentName: item.agentName || baseAgentName,
            isOrchestrator: item.isOrchestrator,
            isSubagent: !item.isOrchestrator,
            todos: [],
            toolCalls: [],
            pendingTodos: 0,
            completedTodos: 0,
            runningTools: 0,
            completedTools: 0,
          });
        }
        const group = groups.get(key)!;
        group.toolCalls.push(item);
        if (item.toolCall.status === "running") {
          group.runningTools++;
        } else {
          group.completedTools++;
        }
      }
    }

    // Sort groups: orchestrator first, then subagents
    return Array.from(groups.values()).sort((a, b) => {
      if (a.isOrchestrator !== b.isOrchestrator) {
        return a.isOrchestrator ? -1 : 1;
      }
      return a.agentName.localeCompare(b.agentName);
    });
  }, [todos, activity, tab, agentId]);

  // Stats
  const stats = useMemo(() => {
    const todoCount = todos?.items?.length || 0;
    const pendingTodos = todos?.items?.filter((t) => !t.completed).length || 0;
    const toolCount = activity.filter((a) => a.itemType === "tool_call").length;
    const runningTools = activity.filter(
      (a) => a.toolCall?.status === "running"
    ).length;
    return { todoCount, pendingTodos, toolCount, runningTools };
  }, [todos, activity]);

  const formatDuration = (ms?: number) => {
    if (!ms) return "";
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  };

  const getPriorityColor = (priority: string) => {
    switch (priority) {
      case "high":
        return "text-red-400";
      case "medium":
        return "text-yellow-400";
      case "low":
        return "text-green-400";
      default:
        return "text-muted-foreground";
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case "running":
        return <Loader2 className="size-3 animate-spin text-blue-400" />;
      case "success":
        return <CheckCircle2 className="size-3 text-green-400" />;
      case "error":
        return <AlertCircle className="size-3 text-red-400" />;
      default:
        return <Circle className="size-3 text-muted-foreground" />;
    }
  };

  if (!open) return null;

  return (
    <div className="fixed right-0 top-0 bottom-0 w-80 bg-background border-l border-border z-40 flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border">
        <div className="flex items-center gap-2">
          <Activity className="size-5 text-violet-400" />
          <h2 className="text-lg font-semibold text-foreground">Activity</h2>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={loadTodos}
            disabled={loading}
            className="p-1 text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50"
            title="Refresh"
          >
            <RefreshCw className={cn("size-4", loading && "animate-spin")} />
          </button>
          <button
            onClick={onClose}
            className="p-1 text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="size-5" />
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-2 p-3 border-b border-border">
        <button
          onClick={() => setTab("all")}
          className={cn(
            "px-3 py-1 text-xs rounded-full transition-colors flex items-center gap-1",
            tab === "all"
              ? "bg-violet-600 text-white"
              : "bg-accent text-muted-foreground hover:text-foreground"
          )}
        >
          All
        </button>
        <button
          onClick={() => setTab("tools")}
          className={cn(
            "px-3 py-1 text-xs rounded-full transition-colors flex items-center gap-1",
            tab === "tools"
              ? "bg-violet-600 text-white"
              : "bg-accent text-muted-foreground hover:text-foreground"
          )}
        >
          <Wrench className="size-3" />
          Tools
          {stats.runningTools > 0 && (
            <span className="ml-1 text-blue-400">({stats.runningTools})</span>
          )}
        </button>
        <button
          onClick={() => setTab("todos")}
          className={cn(
            "px-3 py-1 text-xs rounded-full transition-colors flex items-center gap-1",
            tab === "todos"
              ? "bg-violet-600 text-white"
              : "bg-accent text-muted-foreground hover:text-foreground"
          )}
        >
          <CheckSquare className="size-3" />
          TODOs
          {stats.pendingTodos > 0 && (
            <span className="ml-1 text-yellow-400">({stats.pendingTodos})</span>
          )}
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {loading && !todos && !activity.length ? (
          <div className="flex items-center justify-center h-32">
            <Loader2 className="size-6 text-violet-400 animate-spin" />
          </div>
        ) : error ? (
          <div className="text-center py-8">
            <p className="text-sm text-red-400">{error}</p>
            <button
              onClick={loadTodos}
              className="mt-2 text-xs text-violet-400 hover:text-violet-300"
            >
              Try again
            </button>
          </div>
        ) : agentGroups.length === 0 ? (
          <div className="text-center py-8">
            <Activity className="size-8 text-muted-foreground/50 mx-auto mb-2" />
            <p className="text-sm text-muted-foreground">No activity yet</p>
            <p className="text-xs text-muted-foreground/70 mt-1">
              Tool calls and TODOs will appear here
            </p>
          </div>
        ) : (
          <div className="space-y-3">
            {agentGroups.map((group) => {
              const isCollapsed = collapsedAgents.has(group.agentId);
              const hasContent =
                group.todos.length > 0 || group.toolCalls.length > 0;
              if (!hasContent) return null;

              return (
                <div
                  key={group.agentId}
                  className="border border-border rounded-lg overflow-hidden"
                >
                  {/* Agent Header */}
                  <button
                    onClick={() => toggleAgentCollapse(group.agentId)}
                    className="w-full flex items-center gap-2 p-2 bg-accent/30 hover:bg-accent/50 transition-colors"
                  >
                    {isCollapsed ? (
                      <ChevronRight className="size-4 text-muted-foreground" />
                    ) : (
                      <ChevronDown className="size-4 text-muted-foreground" />
                    )}
                    {group.isOrchestrator ? (
                      <Bot className="size-4 text-violet-400" />
                    ) : (
                      <Cpu className="size-4 text-blue-400" />
                    )}
                    <span className="text-sm font-medium text-foreground flex-1 text-left truncate">
                      {group.agentName}
                    </span>
                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                      {group.runningTools > 0 && (
                        <span className="flex items-center gap-1 text-blue-400">
                          <Loader2 className="size-3 animate-spin" />
                          {group.runningTools}
                        </span>
                      )}
                      {group.completedTools > 0 && (
                        <span className="flex items-center gap-1 text-green-400">
                          <Wrench className="size-3" />
                          {group.completedTools}
                        </span>
                      )}
                      {group.pendingTodos > 0 && (
                        <span className="text-yellow-400">
                          {group.pendingTodos} pending
                        </span>
                      )}
                    </div>
                  </button>

                  {/* Agent's Content */}
                  {!isCollapsed && (
                    <div className="p-2 space-y-2">
                      {/* Tool Calls */}
                      {(tab === "all" || tab === "tools") &&
                        group.toolCalls.map((item) => {
                          const tc = item.toolCall!;
                          return (
                            <div
                              key={item.id}
                              className={cn(
                                "bg-accent/30 border border-border rounded-lg p-2 text-xs",
                                tc.status === "running" && "border-blue-500/30"
                              )}
                            >
                              <div className="flex items-center gap-2">
                                {getStatusIcon(tc.status)}
                                <span className="font-medium text-foreground truncate flex-1">
                                  {tc.name}
                                </span>
                                {tc.durationMs && (
                                  <span className="text-muted-foreground flex items-center gap-1">
                                    <Clock className="size-3" />
                                    {formatDuration(tc.durationMs)}
                                  </span>
                                )}
                              </div>
                              {tc.argumentsPreview && (
                                <p className="text-muted-foreground mt-1 truncate">
                                  {tc.argumentsPreview}
                                </p>
                              )}
                              {tc.resultPreview && tc.status === "success" && (
                                <p className="text-green-400/70 mt-1 truncate">
                                  {tc.resultPreview.substring(0, 50)}...
                                </p>
                              )}
                              {tc.error && (
                                <p className="text-red-400 mt-1 truncate">
                                  {tc.error}
                                </p>
                              )}
                            </div>
                          );
                        })}

                      {/* TODOs */}
                      {(tab === "all" || tab === "todos") &&
                        group.todos.map((todo) => {
                          const isUpdating = updatingIds.has(todo.id);
                          return (
                            <div
                              key={todo.id}
                              className={cn(
                                "bg-accent/50 border border-border rounded-lg p-2",
                                todo.completed && "opacity-60"
                              )}
                            >
                              <div className="flex items-start gap-2">
                                <button
                                  onClick={() =>
                                    handleToggleTodo(todo.id, todo.completed)
                                  }
                                  disabled={isUpdating}
                                  className={cn(
                                    "mt-0.5 transition-colors",
                                    isUpdating && "opacity-50",
                                    todo.completed
                                      ? "text-green-400 hover:text-green-300"
                                      : "text-muted-foreground hover:text-foreground"
                                  )}
                                >
                                  {isUpdating ? (
                                    <Loader2 className="size-4 animate-spin" />
                                  ) : todo.completed ? (
                                    <CheckCircle2 className="size-4" />
                                  ) : (
                                    <Circle className="size-4" />
                                  )}
                                </button>
                                <div className="flex-1 min-w-0">
                                  <p
                                    className={cn(
                                      "text-xs font-medium",
                                      todo.completed
                                        ? "text-muted-foreground line-through"
                                        : "text-foreground"
                                    )}
                                  >
                                    {todo.title}
                                  </p>
                                  <div className="flex items-center gap-2 mt-1">
                                    <span
                                      className={cn(
                                        "text-xs",
                                        getPriorityColor(todo.priority)
                                      )}
                                    >
                                      {todo.priority}
                                    </span>
                                  </div>
                                </div>
                              </div>
                            </div>
                          );
                        })}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="p-3 border-t border-border">
        <p className="text-xs text-muted-foreground text-center">
          {stats.toolCount > 0 && (
            <span>
              {stats.toolCount} tools
              {stats.runningTools > 0 && ` (${stats.runningTools} running)`}
            </span>
          )}
          {stats.toolCount > 0 && stats.todoCount > 0 && " · "}
          {stats.todoCount > 0 && (
            <span>
              {stats.pendingTodos} pending / {stats.todoCount} todos
            </span>
          )}
          {stats.toolCount === 0 && stats.todoCount === 0 && "No activity"}
        </p>
      </div>
    </div>
  );
}
