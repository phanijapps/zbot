import { useCallback, useEffect, useState } from "react";
import { getTransport } from "@/services/transport";
import type { LogSession } from "@/services/transport/types";
import { isChatSession } from "@/services/session-kind";
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
// The /api/logs/sessions endpoint derives `title` from the first user message;
// for brand-new sessions that field is still empty. Fall back to a
// user-friendly "New research · HH:MM" over the started_at timestamp so the
// drawer never shows raw agent names or token counts.
// ---------------------------------------------------------------------------

const UNTITLED_LABEL = "New research";

function formatClock(ms: number): string {
  const d = new Date(ms);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${hh}:${mm}`;
}

function synthTitle(row: LogSession): string {
  return `${UNTITLED_LABEL} · ${formatClock(parseTimestamp(row.started_at))}`;
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
  deleteSession: (sessionId: string) => Promise<void>;
}

// Extracted to keep `deleteSession` under SonarQube cognitive-complexity budget
// and to centralize the exact confirmation copy (R19 spec).
const DELETE_CONFIRM_TEXT =
  "Delete this session permanently?\n\n" +
  "This removes the conversation, executions, and artifact pointers " +
  "for this session. Memory facts, embeddings, and knowledge graph " +
  "entries are preserved. Files on disk are not deleted.";

export function useSessionsList(
  opts: UseSessionsListOptions = {},
): UseSessionsListReturn {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const { onAfterDelete } = opts;

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const transport = await getTransport();
      const result = await transport.listLogSessions();
      if (result.success && Array.isArray(result.data)) {
        // Drop (1) subagent executions (/api/logs/sessions emits one row
        // per execution, including children) and (2) chat-mode sessions,
        // which otherwise leak into the research drawer. Chat detection
        // lives in the shared `session-kind` module so the research hero
        // and the drawer can't drift from each other.
        const rootResearchRows = result.data.filter((row) => {
          const isChild = row.parent_session_id && row.parent_session_id.length > 0;
          return !isChild && !isChatSession(row);
        });
        const mapped = rootResearchRows
          .map((row) => rowToSummary(row))
          .filter((s): s is SessionSummary => s !== null);
        setSessions(mapped);
      }
    } finally {
      setLoading(false);
    }
  }, []);

  const deleteSession = useCallback(async (sessionId: string) => {
    if (!window.confirm(DELETE_CONFIRM_TEXT)) return;
    const transport = await getTransport();
    const result = await transport.deleteSession(sessionId);
    if (!result.success) {
      console.error("Failed to delete session:", result.error);
      return;
    }
    await refresh();
    onAfterDelete?.(sessionId);
  }, [refresh, onAfterDelete]);

  useEffect(() => {
    refresh().catch(() => {});
  }, [refresh]);

  return { sessions, loading, refresh, deleteSession };
}
