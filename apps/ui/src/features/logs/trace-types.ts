/** A node in the trace timeline tree. */
export interface TraceNode {
  id: string;
  type: "root" | "delegation" | "tool_call" | "error";
  agentId: string;
  label: string;
  summary?: string;
  durationMs?: number;
  tokenCount?: number;
  status?: "running" | "completed" | "error" | "crashed";
  error?: string;
  timestamp: string;
  args?: string;
  result?: string;
  children: TraceNode[];
}

export interface ExecutionEntry {
  agentId: string;
  task?: string;
  executionId: string;
}

const INTERNAL_TOOLS = new Set([
  "analyze_intent",
  "update_plan",
  "set_session_title",
]);

export function isInternalTool(toolName: string): boolean {
  return INTERNAL_TOOLS.has(toolName);
}

export function formatDuration(ms: number | undefined): string {
  if (ms === undefined || ms === null) return "";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  const mins = Math.floor(ms / 60000);
  const secs = Math.round((ms % 60000) / 1000);
  return `${mins}m ${secs}s`;
}

export function formatTokens(n: number | undefined): string {
  if (n === undefined || n === null || n === 0) return "";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M tok`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k tok`;
  return `${n} tok`;
}

export function extractToolSummary(toolName: string, args?: string): string {
  if (!args) return "";
  try {
    const parsed = JSON.parse(args);
    if (toolName === "shell" && parsed.command) {
      const cmd = String(parsed.command);
      return cmd.length > 60 ? cmd.slice(0, 57) + "..." : cmd;
    }
    if ((toolName === "read" || toolName === "edit" || toolName === "write") && parsed.path) {
      return String(parsed.path);
    }
    if (toolName === "grep" && parsed.pattern) {
      return `/${parsed.pattern}/`;
    }
    if (toolName === "glob" && parsed.pattern) {
      return String(parsed.pattern);
    }
    if (toolName === "web_fetch" && parsed.url) {
      return String(parsed.url).slice(0, 60);
    }
    if (toolName === "respond" && parsed.message) {
      const msg = String(parsed.message);
      return msg.length > 50 ? msg.slice(0, 47) + "..." : msg;
    }
  } catch {
    // Not valid JSON
  }
  return "";
}
