/** A node in the trace timeline tree. */
export interface TraceNode {
  id: string;
  type: "root" | "delegation" | "tool_call" | "error";
  agentId: string;
  /** For delegation nodes: the child session's execution ID (exec-...).
   *  Used to look up per-run token counts when the same agent is delegated
   *  multiple times (same agentId but different executionIds). */
  executionId?: string;
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
    const parsed = JSON.parse(args) as Record<string, unknown>;
    return extractFromParsed(toolName, parsed);
  } catch {
    // Not valid JSON
  }
  return "";
}

function truncate(s: string, max: number): string {
  return s.length > max ? s.slice(0, max - 3) + "..." : s;
}

function extractFromParsed(toolName: string, parsed: Record<string, unknown>): string {
  const extractors: Record<string, (p: Record<string, unknown>) => string> = {
    shell: (p) => truncate(String(p.command || ""), 60),
    read: (p) => String(p.path || ""),
    edit: (p) => String(p.path || ""),
    write: (p) => String(p.path || ""),
    grep: (p) => (p.pattern ? `/${p.pattern}/` : ""),
    glob: (p) => String(p.pattern || ""),
    web_fetch: (p) => truncate(String(p.url || ""), 60),
    respond: (p) => truncate(String(p.message || ""), 50),
  };

  const extractor = extractors[toolName];
  if (!extractor) return "";
  return extractor(parsed) || "";
}
