// ============================================================================
// AGENT CHANNELS SERVICE
// Tauri command invokers for Agent Channel model (daily sessions)
// ============================================================================

import { invoke } from "@tauri-apps/api/core";

import type {
  DailySession,
  DaySummary,
  SessionMessage,
  AgentChannel,
  AgentExecutionStatus,
  StopExecutionResult,
  TodoList,
} from "@/shared/types";

// ============================================================================
// SESSION MANAGEMENT
// ============================================================================

/**
 * Get or create today's session for an agent
 * Automatically creates a new session if today's doesn't exist
 */
export async function getOrCreateTodaySession(
  agentId: string
): Promise<DailySession> {
  return await invoke<DailySession>("get_or_create_today_session", {
    agentId,
  });
}

/**
 * List previous days for an agent
 * Returns day summaries for previous days, most recent first
 */
export async function listPreviousDays(
  agentId: string,
  limit = 30
): Promise<DaySummary[]> {
  return await invoke<DaySummary[]>("list_previous_days", {
    agentId,
    limit,
  });
}

/**
 * Load messages for a specific session
 */
export async function loadSessionMessages(
  sessionId: string
): Promise<SessionMessage[]> {
  return await invoke<SessionMessage[]>("load_session_messages", {
    sessionId,
  });
}

// ============================================================================
// MESSAGE RECORDING
// ============================================================================

/**
 * Record a message in a session
 */
export async function recordSessionMessage(
  sessionId: string,
  role: string,
  content: string,
  toolCalls?: Record<string, unknown>,
  toolResults?: Record<string, unknown>
): Promise<string> {
  return await invoke<string>("record_session_message", {
    sessionId,
    role,
    content,
    toolCalls,
    toolResults,
  });
}

// ============================================================================
// HISTORY MANAGEMENT
// ============================================================================

/**
 * Delete agent history before a certain date
 * Returns the number of sessions deleted
 */
export async function deleteAgentHistory(
  agentId: string,
  beforeDate: string // ISO date string YYYY-MM-DD
): Promise<number> {
  return await invoke<number>("delete_agent_history", {
    agentId,
    beforeDate,
  });
}

/**
 * Generate end-of-day summary for a session
 */
export async function generateSessionSummary(
  sessionId: string
): Promise<string> {
  return await invoke<string>("generate_session_summary", {
    sessionId,
  });
}

// ============================================================================
// AGENT CHANNEL LIST
// ============================================================================

/**
 * Get agent channel info for the sidebar
 * TODO: Currently returns empty array - needs backend implementation
 */
export async function listAgentChannels(): Promise<AgentChannel[]> {
  return await invoke<AgentChannel[]>("list_agent_channels");
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/**
 * Format ISO datetime string as relative time
 * e.g., "2 hours ago", "Yesterday", "3 days ago"
 */
export function formatRelativeTime(isoDateTime: string): string {
  const now = new Date();
  const past = new Date(isoDateTime);
  const diffMs = now.getTime() - past.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSecs < 60) return "just now";
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays === 1) return "Yesterday";
  if (diffDays < 7) return `${diffDays} days ago`;

  // For older dates, show actual date
  return past.toLocaleDateString();
}

/**
 * Check if a session date is today
 */
export function isSessionToday(sessionDate: string): boolean {
  const today = new Date().toISOString().split("T")[0];
  return sessionDate === today;
}

/**
 * Format session date for display
 * e.g., "Today", "Yesterday", "Jan 15", "Jan 15, 2025"
 */
export function formatSessionDate(sessionDate: string): string {
  const today = new Date().toISOString().split("T")[0];
  const yesterday = new Date(Date.now() - 86400000).toISOString().split("T")[0];

  if (sessionDate === today) return "Today";
  if (sessionDate === yesterday) return "Yesterday";

  const date = new Date(sessionDate + "T00:00:00");

  // Check if date is invalid
  if (isNaN(date.getTime())) {
    return sessionDate || "Unknown Date";
  }

  const isCurrentYear = date.getFullYear() === new Date().getFullYear();

  if (isCurrentYear) {
    return date.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
    });
  }

  return date.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

// ============================================================================
// EXECUTION CONTROL
// ============================================================================

/**
 * Stop agent execution
 * Sets a stop flag that the agent checks at each iteration
 */
export async function stopAgentExecution(
  agentId: string
): Promise<StopExecutionResult> {
  return await invoke<StopExecutionResult>("stop_agent_execution", {
    agentId,
  });
}

/**
 * Get agent execution status
 * Returns current iteration count and stop state
 */
export async function getAgentExecutionStatus(
  agentId: string
): Promise<AgentExecutionStatus> {
  return await invoke<AgentExecutionStatus>("get_agent_execution_status", {
    agentId,
  });
}

// ============================================================================
// TODO LIST MANAGEMENT
// ============================================================================

/**
 * Get TODO list for an agent
 */
export async function getAgentTodos(agentId: string): Promise<TodoList> {
  return await invoke<TodoList>("get_agent_todos", {
    agentId,
  });
}

/**
 * Save TODO list for an agent
 */
export async function saveAgentTodos(
  agentId: string,
  todos: TodoList
): Promise<{ success: boolean; message: string }> {
  return await invoke<{ success: boolean; message: string }>(
    "save_agent_todos",
    {
      agentId,
      todos,
    }
  );
}

/**
 * Update a single TODO item's completion status
 */
export async function updateAgentTodo(
  agentId: string,
  todoId: string,
  completed: boolean
): Promise<{ success: boolean; todoId: string; completed: boolean }> {
  return await invoke<{ success: boolean; todoId: string; completed: boolean }>(
    "update_agent_todo",
    {
      agentId,
      todoId,
      completed,
    }
  );
}
