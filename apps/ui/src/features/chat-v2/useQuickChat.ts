import { useCallback, useEffect, useReducer, useRef } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getTransport } from "@/services/transport";
import type { SessionMessage } from "@/services/transport/types";
import { useStatusPill } from "../shared/statusPill";
import {
  type QuickChatMessage,
  EMPTY_QUICK_CHAT_STATE,
} from "./types";
import { reduceQuickChat } from "./reducer";
import {
  mapGatewayEventToQuickChatAction,
  mapGatewayEventToPillEvent,
} from "./event-map";

const QUICK_CHAT_AGENT_ID = "quick-chat";

function newConvId(): string {
  return `quick-chat-${crypto.randomUUID()}`;
}

function sessionMessageToQuickChat(m: SessionMessage): QuickChatMessage {
  return {
    id: m.id,
    role: m.role === "user" ? "user" : "assistant",
    content: m.content,
    timestamp: new Date(m.created_at).getTime(),
  };
}

export function useQuickChat() {
  const { sessionId: urlSessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const [state, dispatch] = useReducer(reduceQuickChat, {
    ...EMPTY_QUICK_CHAT_STATE,
    conversationId: newConvId(),
  });
  const { state: pillState, sink: pillSink } = useStatusPill();
  const subscribedConvIdRef = useRef<string | null>(null);

  // --- Hydrate from snapshot on mount or URL change ---
  useEffect(() => {
    if (!urlSessionId) return;
    let cancelled = false;
    (async () => {
      const transport = await getTransport();
      const [stateResult, messagesResult] = await Promise.all([
        transport.getSessionState(urlSessionId),
        transport.getSessionMessages(urlSessionId, { scope: "root" }),
      ]);
      if (cancelled) return;
      const wardName = stateResult.success && stateResult.data?.ward?.name ? stateResult.data.ward.name : null;
      const history: QuickChatMessage[] = messagesResult.success && messagesResult.data
        ? messagesResult.data.map(sessionMessageToQuickChat)
        : [];
      dispatch({
        type: "HYDRATE",
        sessionId: urlSessionId,
        conversationId: state.conversationId,
        messages: history,
        wardName,
      });
    })();
    return () => { cancelled = true; };
    // Intentionally omit state.conversationId from deps — it doesn't change
    // outside of RESET flows, and re-running hydrate on RESET would race with
    // the navigate call in startNewChat.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [urlSessionId]);

  // --- Subscribe to WS event stream on current conversationId ---
  useEffect(() => {
    const convId = state.conversationId;
    if (!convId || subscribedConvIdRef.current === convId) return;
    subscribedConvIdRef.current = convId;
    const unsubscribe = Promise.resolve().then(async () => {
      const transport = await getTransport();
      return transport.subscribeConversation(convId, {
        onEvent: (event) => {
          const action = mapGatewayEventToQuickChatAction(event);
          if (action) dispatch(action);
          const pillEv = mapGatewayEventToPillEvent(event);
          if (pillEv) pillSink.push(pillEv);
        },
      });
    });
    return () => {
      unsubscribe.then((fn) => fn && fn()).catch(() => { /* no-op */ });
      if (subscribedConvIdRef.current === convId) {
        subscribedConvIdRef.current = null;
      }
    };
  }, [state.conversationId, pillSink]);

  // --- Sync URL when backend emits SESSION_BOUND ---
  useEffect(() => {
    if (state.sessionId && urlSessionId !== state.sessionId) {
      navigate(`/chat-v2/${state.sessionId}`, { replace: true });
    }
  }, [state.sessionId, urlSessionId, navigate]);

  // --- Send message ---
  const sendMessage = useCallback(async (text: string) => {
    const trimmed = text.trim();
    if (!trimmed || state.status === "running") return;
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
      QUICK_CHAT_AGENT_ID,
      state.conversationId,
      trimmed,
      state.sessionId ?? undefined,
      "chat"
    );
    if (!result.success) {
      dispatch({ type: "ERROR", message: result.error ?? "Failed to send" });
    }
  }, [state.status, state.conversationId, state.sessionId]);

  // --- Start new chat (discard current, fresh conv, navigate back to empty) ---
  const startNewChat = useCallback(() => {
    pillSink.push({ kind: "reset" });
    dispatch({ type: "RESET", conversationId: newConvId() });
    navigate("/chat-v2", { replace: true });
  }, [navigate, pillSink]);

  // --- Stop running agent ---
  const stopAgent = useCallback(async () => {
    if (state.status !== "running") return;
    const transport = await getTransport();
    await transport.stopAgent(state.conversationId);
  }, [state.status, state.conversationId]);

  // --- Lazy-load older turns (no-op for v1; backend doesn't expose pagination yet) ---
  const loadOlder = useCallback(async () => {
    // v1 loads the full root message list in one HYDRATE call; hasMoreOlder
    // stays false. Wire up pagination when the backend supports `before` cursor.
  }, []);

  return { state, pillState, sendMessage, startNewChat, stopAgent, loadOlder };
}
