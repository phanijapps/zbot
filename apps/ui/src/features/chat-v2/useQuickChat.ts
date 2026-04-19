import { useCallback, useEffect, useReducer, useRef, type Dispatch } from "react";
import { getTransport } from "@/services/transport";
import type { Transport } from "@/services/transport";
import type {
  ConversationEvent,
  SessionMessage,
} from "@/services/transport/types";
import { useStatusPill, type PillEventSink } from "../shared/statusPill";
import {
  type QuickChatMessage,
  EMPTY_QUICK_CHAT_STATE,
} from "./types";
import { reduceQuickChat, type QuickChatAction } from "./reducer";
import {
  mapGatewayEventToQuickChatAction,
  mapGatewayEventToPillEvent,
} from "./event-map";

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/** Agent id for Quick Chat. The reserved chat session is bound to root server-side. */
const CHAT_AGENT_ID = "root";

/** Pinned execution mode — skips intent analysis / planning / research pipeline. */
const CHAT_MODE = "fast";

/** How many root-scoped messages to fetch on hydrate. */
const HISTORY_TAIL_LIMIT = 50;

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/**
 * Filter server-side messages to the ones the user actually cares about.
 *
 * The root-scope feed carries intermediate rows — `role: "tool"` results,
 * and assistant placeholders whose content is the literal `"[tool calls]"`
 * marker emitted when the model calls a tool. Those belong in the
 * Thinking timeline (future work), not the chat bubbles.
 */
function isVisibleChatMessage(m: SessionMessage): boolean {
  if (m.role === "tool") return false;
  if (m.role === "assistant" && m.content.trim() === "[tool calls]") return false;
  return m.role === "user" || m.role === "assistant";
}

function sessionMessageToQuickChat(m: SessionMessage): QuickChatMessage {
  return {
    id: m.id,
    role: m.role === "user" ? "user" : "assistant",
    content: m.content,
    timestamp: new Date(m.created_at).getTime(),
  };
}

/** Idempotent bootstrap: init the reserved session and pull the history tail. */
async function bootstrapChatSession(
  transport: Transport
): Promise<{
  sessionId: string;
  conversationId: string;
  messages: QuickChatMessage[];
} | null> {
  const init = await transport.initChatSession();
  if (!init.success || !init.data) return null;

  const { sessionId, conversationId, created } = init.data;

  // New sessions have no history to fetch — skip the round-trip.
  if (created) {
    return { sessionId, conversationId, messages: [] };
  }

  const history = await transport.getSessionMessages(sessionId, {
    scope: "root",
  });
  const messages =
    history.success && history.data
      ? history.data
          .filter(isVisibleChatMessage)
          .slice(-HISTORY_TAIL_LIMIT)
          .map(sessionMessageToQuickChat)
      : [];

  return { sessionId, conversationId, messages };
}

/** Build the WS event handler once; closure captures the stable pill sink. */
function makeEventHandler(
  pillSink: PillEventSink,
  dispatch: Dispatch<QuickChatAction>
) {
  return (event: ConversationEvent) => {
    const action = mapGatewayEventToQuickChatAction(event);
    if (action) dispatch(action);
    const pillEv = mapGatewayEventToPillEvent(event);
    if (pillEv) pillSink.push(pillEv);
  };
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useQuickChat() {
  const [state, dispatch] = useReducer(reduceQuickChat, EMPTY_QUICK_CHAT_STATE);
  const { state: pillState, sink: pillSink } = useStatusPill();

  // Bootstrap idempotency guard. Set AFTER the async work resolves, not
  // before, so StrictMode's synthetic unmount doesn't leave us in a "bootstrap
  // started but never completed" state. The server-side init is idempotent
  // (same session ids on every call), so two concurrent calls in dev are
  // harmless — this ref only guarantees we dispatch HYDRATE once.
  const bootstrappedRef = useRef(false);
  const subscribedConvIdRef = useRef<string | null>(null);

  // --- Bootstrap: init reserved session + hydrate history ---
  useEffect(() => {
    if (bootstrappedRef.current) return;
    (async () => {
      const transport = await getTransport();
      const result = await bootstrapChatSession(transport);
      if (bootstrappedRef.current) return;
      bootstrappedRef.current = true;
      if (!result) {
        dispatch({ type: "ERROR", message: "Failed to initialise chat" });
        return;
      }
      dispatch({
        type: "HYDRATE",
        sessionId: result.sessionId,
        conversationId: result.conversationId,
        messages: result.messages,
        wardName: null, // populated by later WardChanged events
      });
    })();
  }, []);

  // --- Subscribe to WS events for the persisted conversationId ---
  useEffect(() => {
    const convId = state.conversationId;
    if (!convId || subscribedConvIdRef.current === convId) return;
    subscribedConvIdRef.current = convId;
    const onEvent = makeEventHandler(pillSink, dispatch);
    const unsubscribe = Promise.resolve().then(async () => {
      const transport = await getTransport();
      return transport.subscribeConversation(convId, { onEvent });
    });
    return () => {
      unsubscribe.then((fn) => fn && fn()).catch(() => {
        /* no-op */
      });
      if (subscribedConvIdRef.current === convId) {
        subscribedConvIdRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.conversationId]);

  // --- Send a user message against the reserved session ---
  const sendMessage = useCallback(
    async (text: string) => {
      const trimmed = text.trim();
      if (!trimmed || state.status === "running") return;
      if (!state.sessionId || !state.conversationId) return;
      dispatch({
        type: "APPEND_USER",
        message: {
          id: crypto.randomUUID(),
          role: "user",
          content: trimmed,
          timestamp: Date.now(),
        },
      });
      const transport = await getTransport();
      const result = await transport.executeAgent(
        CHAT_AGENT_ID,
        state.conversationId,
        trimmed,
        state.sessionId,
        CHAT_MODE
      );
      if (!result.success) {
        dispatch({ type: "ERROR", message: result.error ?? "Failed to send" });
      }
    },
    [state.status, state.conversationId, state.sessionId]
  );

  // --- Stop a running turn ---
  const stopAgent = useCallback(async () => {
    if (state.status !== "running" || !state.conversationId) return;
    const transport = await getTransport();
    await transport.stopAgent(state.conversationId);
  }, [state.status, state.conversationId]);

  // --- Clear the reserved session and bootstrap a fresh one ---
  const clearSession = useCallback(async () => {
    const transport = await getTransport();
    const deleted = await transport.deleteChatSession();
    if (!deleted.success) {
      dispatch({ type: "ERROR", message: deleted.error ?? "Failed to clear chat" });
      return;
    }
    // Bootstrap again; the init endpoint self-heals into a new session.
    const fresh = await bootstrapChatSession(transport);
    if (!fresh) {
      dispatch({ type: "ERROR", message: "Failed to initialise a new chat after clear" });
      return;
    }
    dispatch({
      type: "HYDRATE",
      sessionId: fresh.sessionId,
      conversationId: fresh.conversationId,
      messages: fresh.messages,
      wardName: null,
    });
  }, []);

  return { state, pillState, sendMessage, stopAgent, clearSession };
}
