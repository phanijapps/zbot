// ============================================================================
// TODO PANEL
// Side panel for viewing and managing agent TODO list with nested agent groups
// ============================================================================

import { useState, useEffect, useMemo, useRef } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  CheckSquare,
  X,
  Loader2,
  RefreshCw,
  Circle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Bot,
  Cpu,
} from "lucide-react";
import { cn } from "@/shared/utils";
import type { TodoList, Todo } from "@/shared/types";
import { getAgentTodos, updateAgentTodo } from "@/services/agentChannels";

interface TodoPanelProps {
  open: boolean;
  onClose: () => void;
  agentId: string;
  sessionId?: string;
}

interface TodoUpdateEvent {
  type: "todo_update";
  timestamp: number;
  todos: TodoList;
}

interface AgentGroup {
  agentId: string;
  agentName: string;
  isOrchestrator: boolean;
  todos: Todo[];
  pendingCount: number;
  completedCount: number;
}

export function TodoPanel({ open, onClose, agentId, sessionId }: TodoPanelProps) {
  const [todos, setTodos] = useState<TodoList | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<"all" | "pending" | "completed">("all");
  const [updatingIds, setUpdatingIds] = useState<Set<string>>(new Set());
  const [collapsedAgents, setCollapsedAgents] = useState<Set<string>>(new Set());
  const unlistenRef = useRef<UnlistenFn | null>(null);

  // Load TODOs when panel opens
  useEffect(() => {
    if (open && agentId) {
      loadTodos();
    }
  }, [open, agentId]);

  // Listen for todo_update events from the agent stream
  useEffect(() => {
    if (!open || !sessionId) return;

    const eventName = `agent-stream://${sessionId}`;

    const setupListener = async () => {
      // Clean up any existing listener
      if (unlistenRef.current) {
        unlistenRef.current();
      }

      unlistenRef.current = await listen<TodoUpdateEvent>(eventName, (event) => {
        if (event.payload.type === "todo_update" && event.payload.todos) {
          setTodos(event.payload.todos);
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
      // Update local state
      setTodos((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          items: prev.items.map((item) =>
            item.id === todoId
              ? {
                  ...item,
                  completed: !currentCompleted,
                  completedAt: !currentCompleted ? new Date().toISOString() : undefined,
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

  const toggleAgentCollapse = (agentId: string) => {
    setCollapsedAgents((prev) => {
      const next = new Set(prev);
      if (next.has(agentId)) {
        next.delete(agentId);
      } else {
        next.add(agentId);
      }
      return next;
    });
  };

  // Group TODOs by agent
  const agentGroups = useMemo((): AgentGroup[] => {
    if (!todos?.items) return [];

    // Filter first
    let filtered = todos.items;
    if (filter === "pending") {
      filtered = filtered.filter((t) => !t.completed);
    } else if (filter === "completed") {
      filtered = filtered.filter((t) => t.completed);
    }

    // Group by agentId
    const groups = new Map<string, AgentGroup>();

    for (const todo of filtered) {
      const key = todo.agentId || "unknown";
      if (!groups.has(key)) {
        groups.set(key, {
          agentId: todo.agentId || "unknown",
          agentName: todo.agentName || "Unknown Agent",
          isOrchestrator: todo.isOrchestrator ?? true,
          todos: [],
          pendingCount: 0,
          completedCount: 0,
        });
      }
      const group = groups.get(key)!;
      group.todos.push(todo);
      if (todo.completed) {
        group.completedCount++;
      } else {
        group.pendingCount++;
      }
    }

    // Sort todos within each group: pending first, then by creation date
    for (const group of groups.values()) {
      group.todos.sort((a, b) => {
        if (a.completed !== b.completed) {
          return a.completed ? 1 : -1;
        }
        return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
      });
    }

    // Convert to array and sort: orchestrator first, then subagents alphabetically
    return Array.from(groups.values()).sort((a, b) => {
      if (a.isOrchestrator !== b.isOrchestrator) {
        return a.isOrchestrator ? -1 : 1;
      }
      return a.agentName.localeCompare(b.agentName);
    });
  }, [todos, filter]);

  // Stats
  const stats = useMemo(() => {
    if (!todos?.items) return { total: 0, pending: 0, completed: 0 };
    return {
      total: todos.items.length,
      pending: todos.items.filter((t) => !t.completed).length,
      completed: todos.items.filter((t) => t.completed).length,
    };
  }, [todos]);

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

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffMins < 1) return "just now";
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays === 1) return "yesterday";
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString();
  };

  if (!open) return null;

  return (
    <div className="fixed right-0 top-0 bottom-0 w-80 bg-background border-l border-border z-40 flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border">
        <div className="flex items-center gap-2">
          <CheckSquare className="size-5 text-violet-400" />
          <h2 className="text-lg font-semibold text-foreground">TODO List</h2>
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

      {/* Filter tabs */}
      <div className="flex gap-2 p-3 border-b border-border">
        <button
          onClick={() => setFilter("all")}
          className={cn(
            "px-3 py-1 text-xs rounded-full transition-colors",
            filter === "all"
              ? "bg-violet-600 text-white"
              : "bg-accent text-muted-foreground hover:text-foreground"
          )}
        >
          All ({stats.total})
        </button>
        <button
          onClick={() => setFilter("pending")}
          className={cn(
            "px-3 py-1 text-xs rounded-full transition-colors",
            filter === "pending"
              ? "bg-violet-600 text-white"
              : "bg-accent text-muted-foreground hover:text-foreground"
          )}
        >
          Pending ({stats.pending})
        </button>
        <button
          onClick={() => setFilter("completed")}
          className={cn(
            "px-3 py-1 text-xs rounded-full transition-colors",
            filter === "completed"
              ? "bg-violet-600 text-white"
              : "bg-accent text-muted-foreground hover:text-foreground"
          )}
        >
          Done ({stats.completed})
        </button>
      </div>

      {/* TODO list grouped by agent */}
      <div className="flex-1 overflow-y-auto p-4">
        {loading && !todos ? (
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
            <CheckSquare className="size-8 text-muted-foreground/50 mx-auto mb-2" />
            <p className="text-sm text-muted-foreground">
              {filter === "all"
                ? "No TODOs yet"
                : filter === "pending"
                ? "No pending TODOs"
                : "No completed TODOs"}
            </p>
            <p className="text-xs text-muted-foreground/70 mt-1">
              Ask the agent to create TODOs using the todos tool
            </p>
          </div>
        ) : (
          <div className="space-y-3">
            {agentGroups.map((group) => {
              const isCollapsed = collapsedAgents.has(group.agentId);
              return (
                <div key={group.agentId} className="border border-border rounded-lg overflow-hidden">
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
                    <span className="text-xs text-muted-foreground">
                      {group.pendingCount > 0 && (
                        <span className="text-yellow-400">{group.pendingCount}</span>
                      )}
                      {group.pendingCount > 0 && group.completedCount > 0 && " / "}
                      {group.completedCount > 0 && (
                        <span className="text-green-400">{group.completedCount}</span>
                      )}
                    </span>
                  </button>

                  {/* Agent's TODOs */}
                  {!isCollapsed && (
                    <div className="p-2 space-y-2">
                      {group.todos.map((todo) => {
                        const isUpdating = updatingIds.has(todo.id);
                        return (
                          <div
                            key={todo.id}
                            className={cn(
                              "bg-accent/50 border border-border rounded-lg p-3 transition-all",
                              todo.completed && "opacity-60"
                            )}
                          >
                            <div className="flex items-start gap-3">
                              <button
                                onClick={() => handleToggleTodo(todo.id, todo.completed)}
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
                                  <Loader2 className="size-5 animate-spin" />
                                ) : todo.completed ? (
                                  <CheckCircle2 className="size-5" />
                                ) : (
                                  <Circle className="size-5" />
                                )}
                              </button>
                              <div className="flex-1 min-w-0">
                                <p
                                  className={cn(
                                    "text-sm font-medium",
                                    todo.completed
                                      ? "text-muted-foreground line-through"
                                      : "text-foreground"
                                  )}
                                >
                                  {todo.title}
                                </p>
                                {todo.description && (
                                  <p className="text-xs text-muted-foreground mt-1 line-clamp-2">
                                    {todo.description}
                                  </p>
                                )}
                                <div className="flex items-center gap-2 mt-2">
                                  <span
                                    className={cn(
                                      "text-xs font-medium",
                                      getPriorityColor(todo.priority)
                                    )}
                                  >
                                    {todo.priority}
                                  </span>
                                  <span className="text-xs text-muted-foreground">
                                    {formatDate(todo.createdAt)}
                                  </span>
                                  {todo.completed && todo.completedAt && (
                                    <span className="text-xs text-green-400/70">
                                      completed {formatDate(todo.completedAt)}
                                    </span>
                                  )}
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
      {todos && (
        <div className="p-4 border-t border-border">
          <p className="text-xs text-muted-foreground text-center">
            {stats.pending > 0
              ? `${stats.pending} pending · ${stats.completed} completed`
              : stats.completed > 0
              ? `All ${stats.completed} TODOs completed!`
              : "No TODOs"}
          </p>
        </div>
      )}
    </div>
  );
}
