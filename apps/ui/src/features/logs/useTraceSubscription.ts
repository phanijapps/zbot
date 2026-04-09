// ============================================================================
// USE TRACE SUBSCRIPTION HOOK
// Real-time updates for trace data. Uses polling via useAutoRefresh pattern
// for running sessions (3-second interval). Falls back gracefully when the
// session is not running.
// ============================================================================

import { useEffect, useRef } from "react";
import type { LogSession } from "@/services/transport/types";

interface UseTraceSubscriptionOptions {
  /** The session to watch. Null = no subscription. */
  session: LogSession | null;
  /** Called whenever new data may be available (triggers trace refetch). */
  onEvent: () => void;
}

/**
 * Polls for updates while a session is running.
 *
 * We use a simple interval-based approach (3 seconds) rather than
 * WebSocket subscriptions because:
 * 1. subscribeConversation uses conversation_id, not session_id
 * 2. The observability dashboard doesn't need sub-second latency
 * 3. Polling stops automatically when the session completes
 */
export function useTraceSubscription({
  session,
  onEvent,
}: UseTraceSubscriptionOptions): void {
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  useEffect(() => {
    // Only poll while the session is actively running
    if (!session || session.status !== "running") return;

    const intervalId = setInterval(() => {
      onEventRef.current();
    }, 3000);

    return () => clearInterval(intervalId);
  }, [session?.session_id, session?.status]);
}
