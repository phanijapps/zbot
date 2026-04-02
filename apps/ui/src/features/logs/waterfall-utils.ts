import type { ExecutionLog } from '../../services/transport/types';

// ---------------------------------------------------------------------------
// Duration formatting
// ---------------------------------------------------------------------------

/** Format a duration in ms as a compact human-readable string (e.g. 1.2s, 2m 10s). */
export function formatDurationCompact(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const m = Math.floor(ms / 60_000);
  const s = Math.floor((ms % 60_000) / 1000);
  if (m < 60) return s > 0 ? `${m}m ${s}s` : `${m}m`;
  const h = Math.floor(m / 60);
  const rm = m % 60;
  return rm > 0 ? `${h}h ${rm}m` : `${h}h`;
}

/** Format a timestamp string as HH:MM:SS. */
export function formatTimestamp(ts: string): string {
  return new Date(ts).toLocaleTimeString('en-US', { hour12: false });
}

// ---------------------------------------------------------------------------
// Metadata parsing
// ---------------------------------------------------------------------------

export interface ToolMeta {
  toolName: string;
  input?: string;
  output?: string;
  exitCode?: number;
  status?: string;
  errorDetail?: string;
}

/** Safely parse tool metadata from an ExecutionLog. */
export function parseToolMetadata(log: ExecutionLog): ToolMeta {
  const meta = log.metadata ?? {};
  const raw = typeof meta === 'object' ? meta : {};

  // Tool name: try metadata.tool_name, then extract from message prefix
  const toolName =
    (raw.tool_name as string) ||
    (raw.tool as string) ||
    extractToolName(log.message) ||
    log.category;

  const input = (raw.input as string) || (raw.command as string) || undefined;
  const output = (raw.output as string) || (raw.result as string) || undefined;
  const exitCode = raw.exit_code != null ? Number(raw.exit_code) : undefined;
  const status = (raw.status as string) || undefined;
  const errorDetail = (raw.error as string) || (raw.error_detail as string) || undefined;

  return { toolName, input, output, exitCode, status, errorDetail };
}

/** Extract a tool name from a log message like "shell: ls -la" -> "shell". */
function extractToolName(message: string): string | undefined {
  const match = message.match(/^([a-z_]+)\s*:/i);
  return match ? match[1] : undefined;
}

/** Extract an input/command snippet from a message after the colon. */
export function extractInputFromMessage(message: string): string | undefined {
  const idx = message.indexOf(':');
  if (idx >= 0 && idx < 30) {
    return message.slice(idx + 1).trim() || undefined;
  }
  return undefined;
}
