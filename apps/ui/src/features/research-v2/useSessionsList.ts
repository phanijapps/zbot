import { useCallback, useEffect, useState } from "react";
import { getTransport } from "@/services/transport";
import type { LogSession } from "@/services/transport/types";
import type { SessionSummary } from "./types";

// ---------------------------------------------------------------------------
// Status mapping
// Wire sends "completed"; our SessionSummary uses "complete".
// ---------------------------------------------------------------------------

const STATUS_MAP: Record<string, SessionSummary["status"]> = {
  running: "running",
  completed: "complete",
  complete: "complete",
  crashed: "crashed",
  error: "crashed",
  paused: "paused",
};

function mapStatus(s: string | undefined): SessionSummary["status"] {
  if (!s) return "complete";
  return STATUS_MAP[s] ?? "complete";
}

// ---------------------------------------------------------------------------
// Title synthesis
// The /api/logs/sessions endpoint does not return a user-authored title for
// every row. We synthesise one from agent name + token/tool counts.
// ---------------------------------------------------------------------------

function synthTitle(row: LogSession): string {
  const agent = row.agent_name ?? row.agent_id ?? "unknown";
  const tokens = row.token_count ?? 0;
  const tools = row.tool_call_count ?? 0;
  if (tokens > 0 || tools > 0) {
    return `${agent} · ${tokens} tok · ${tools} tool`;
  }
  return agent;
}

// ---------------------------------------------------------------------------
// Timestamp helpers
// ---------------------------------------------------------------------------

function parseTimestamp(s: string | undefined | null): number {
  if (!s) return Date.now();
  const t = new Date(s).getTime();
  return Number.isFinite(t) ? t : Date.now();
}

// ---------------------------------------------------------------------------
// Row → SessionSummary
//
// Wire-shape quirk: `session_id` holds the *execution* id; `conversation_id`
// holds the real session id (what callers know as sess-*). We use the latter.
// `wardName` is absent from this endpoint — the ResearchPage fills it from the
// WS stream once the session is opened.
// ---------------------------------------------------------------------------

function rowToSummary(row: LogSession): SessionSummary | null {
  const id = row.conversation_id;
  if (typeof id !== "string" || id.length === 0) return null;
  return {
    id,
    title: row.title ?? synthTitle(row),
    status: mapStatus(row.status),
    wardName: null,
    updatedAt: parseTimestamp(row.ended_at ?? row.started_at),
  };
}

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

export interface UseSessionsListOptions {
  /** Invoked after a successful deleteSession (R19 extension point). */
  onAfterDelete?: (sessionId: string) => void;
}

export interface UseSessionsListReturn {
  sessions: SessionSummary[];
  loading: boolean;
  refresh: () => Promise<void>;
}

export function useSessionsList(
  _opts: UseSessionsListOptions = {},
): UseSessionsListReturn {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const transport = await getTransport();
      const result = await transport.listLogSessions();
      if (result.success && Array.isArray(result.data)) {
        const mapped = result.data
          .map((row) => rowToSummary(row))
          .filter((s): s is SessionSummary => s !== null);
        setSessions(mapped);
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { sessions, loading, refresh };
}
