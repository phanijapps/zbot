// ============================================================================
// USE TRACE SUBSCRIPTION HOOK
// Real-time updates for trace data via the WebSocket conversation channel.
// Replaces the previous 3-second polling loop. When a tool_call / tool_result
// / delegation / error event lands on the WS for the selected session's
// conversation, we trigger a trace refetch — same data the polling path used
// to fetch, just pushed instead of pulled.
//
// When the session is not running we skip subscribing entirely; the trace is
// already terminal and will never change.
// ============================================================================

import { useEffect, useRef } from "react";
import { getTransport } from "@/services/transport";
import type { LogSession, ConversationEvent } from "@/services/transport/types";

interface UseTraceSubscriptionOptions {
  /** The session to watch. Null = no subscription. */
  session: LogSession | null;
  /** Called whenever new data may be available (triggers trace refetch). */
  onEvent: () => void;
}

/** Event types that mean the trace has likely changed. */
const TRACE_RELEVANT_TYPES = new Set([
  "tool_call",
  "tool_result",
  "delegation",
  "agent_started",
  "agent_completed",
  "error",
  "session_status_changed",
]);

/**
 * Subscribe to the conversation WebSocket for the active session and refetch
 * the trace whenever a tool/agent/delegation event arrives. No polling. When
 * the session is not running, this hook is a no-op.
 */
export function useTraceSubscription({
  session,
  onEvent,
}: UseTraceSubscriptionOptions): void {
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  useEffect(() => {
    if (!session || session.status !== "running") return;
    const conversationId = session.conversation_id;
    if (!conversationId) return;

    let cancelled = false;
    let unsubscribe: (() => void) | null = null;

    (async () => {
      try {
        const transport = await getTransport();
        if (cancelled) return;
        unsubscribe = transport.subscribeConversation(conversationId, {
          onEvent: (event: ConversationEvent) => {
            if (TRACE_RELEVANT_TYPES.has(event.type)) {
              onEventRef.current();
            }
          },
        });
      } catch {
        // Subscription failure shouldn't crash the page — the rendered trace
        // already reflects the last successful fetch. The list-level
        // useAutoRefresh still tickles things every 5s as a backstop.
      }
    })();

    return () => {
      cancelled = true;
      if (unsubscribe) unsubscribe();
    };
  }, [session, session?.session_id, session?.status, session?.conversation_id]);
}
