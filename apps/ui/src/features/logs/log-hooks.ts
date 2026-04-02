// ============================================================================
// LOG DATA HOOKS
// Data fetching hooks for the Execution Intelligence Dashboard.
// ============================================================================

import { useState, useEffect, useCallback, useRef } from "react";
import { getTransport } from "@/services/transport";
import type { LogSession, SessionDetail, LogFilter } from "@/services/transport/types";

// ============================================================================
// useLogSessions
// ============================================================================

interface UseLogSessionsResult {
  sessions: LogSession[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/**
 * Fetch the list of log sessions from the transport layer.
 * Follows the same pattern as WebLogsPanel's `loadSessions` and
 * graph-hooks' tick-based refetch mechanism.
 */
export function useLogSessions(filters?: LogFilter): UseLogSessionsResult {
  const [sessions, setSessions] = useState<LogSession[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const refetch = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const transport = await getTransport();
        const result = await transport.listLogSessions({
          agent_id: filters?.agent_id,
          level: filters?.level,
          limit: filters?.limit ?? 100,
        });

        if (cancelled) return;

        if (result.success && result.data) {
          setSessions(result.data);
        } else {
          setError(result.error || "Failed to load sessions");
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Unknown error");
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    load();
    return () => { cancelled = true; };
  }, [tick, filters?.agent_id, filters?.level, filters?.limit]);

  return { sessions, loading, error, refetch };
}

// ============================================================================
// useSessionDetail
// ============================================================================

interface UseSessionDetailResult {
  detail: SessionDetail | null;
  loading: boolean;
}

/**
 * Fetch full session detail (logs + child sessions) when sessionId is non-null.
 * Only fetches when sessionId changes and is non-null.
 */
export function useSessionDetail(sessionId: string | null): UseSessionDetailResult {
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!sessionId) {
      setDetail(null);
      setLoading(false);
      return;
    }

    let cancelled = false;

    const load = async () => {
      setLoading(true);
      try {
        const transport = await getTransport();
        const result = await transport.getLogSession(sessionId);

        if (cancelled) return;

        if (result.success && result.data) {
          setDetail(result.data);
        } else {
          // Don't overwrite existing detail on error
          console.error("Failed to load session detail:", result.error);
        }
      } catch (err) {
        if (!cancelled) {
          console.error("Failed to load session detail:", err);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    load();
    return () => { cancelled = true; };
  }, [sessionId]);

  return { detail, loading };
}

// ============================================================================
// useAutoRefresh
// ============================================================================

/**
 * Auto-refetch sessions every 5 seconds while any session has status 'running'.
 * Stops polling when all sessions are completed/failed/stopped.
 */
export function useAutoRefresh(
  sessions: LogSession[],
  refetch: () => void,
): void {
  const refetchRef = useRef(refetch);
  refetchRef.current = refetch;

  useEffect(() => {
    const hasRunning = sessions.some((s) => s.status === "running");
    if (!hasRunning) return;

    const intervalId = setInterval(() => {
      refetchRef.current();
    }, 5000);

    return () => clearInterval(intervalId);
  }, [sessions]);
}
