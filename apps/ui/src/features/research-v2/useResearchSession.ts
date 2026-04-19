import { useCallback, useEffect, useReducer, useRef, type Dispatch } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getTransport } from "@/services/transport";
import type { Transport } from "@/services/transport";
import type {
  ConversationEvent,
  SessionMessage,
} from "@/services/transport/types";
import { useStatusPill, type PillEventSink } from "../shared/statusPill";
import {
  type ResearchMessage,
  EMPTY_RESEARCH_STATE,
} from "./types";
import { reduceResearch, type ResearchAction } from "./reducer";
import {
  mapGatewayEventToResearchAction,
  mapGatewayEventToPillEvent,
} from "./event-map";

const ROOT_AGENT_ID = "root";
// `undefined` leaves the executor to pick its default (`SessionMode::Research`).
// Passing the string "research" works too but is misleading — it looks
// meaningful but is equivalent.
const RESEARCH_MODE: string | undefined = undefined;
// Research conversations can be long; fetch the last N root-scoped messages.
const HISTORY_TAIL_LIMIT = 50;
// Placeholder conv id used ONLY for the very first invoke of a brand-new
// session. The WS `invoke` command requires a non-optional `conversation_id`
// field; the backend discards ours when `session_id` is null and assigns
// its own, which we pick up from the `invoke_accepted` event.
function placeholderConvId(): string {
  return `research-${crypto.randomUUID()}`;
}

// -------------------------------------------------------------------------
// Pure helpers
// -------------------------------------------------------------------------

function isVisibleResearchMessage(m: SessionMessage): boolean {
  if (m.role === "tool") return false;
  if (m.role === "assistant" && m.content.trim() === "[tool calls]") return false;
  return m.role === "user" || m.role === "assistant";
}

function messageFromApi(m: SessionMessage): ResearchMessage {
  return {
    id: m.id,
    // Assistant content renders via turn blocks, not the messages[] array.
    // The hook history only keeps user prompts for the "message log" surface.
    role: m.role === "user" ? "user" : "system",
    content: m.content,
    timestamp: new Date(m.created_at).getTime(),
  };
}

/**
 * Fetch the tail of the session's root-scoped message history.
 *
 * DOES NOT call `/api/sessions/:id/state` — that endpoint returns 404 even
 * for sessions that exist in the DB (observed during chat-v2 testing).
 * `/messages?scope=root` returns `200 []` for extant-but-empty sessions,
 * so absence of rows doesn't get conflated with a missing session.
 */
async function hydrateExistingSession(
  transport: Transport,
  sessionId: string
): Promise<{ messages: ResearchMessage[] } | null> {
  const msgs = await transport.getSessionMessages(sessionId, { scope: "root" });
  if (!msgs.success || !msgs.data) return null;
  const messages = msgs.data
    .filter(isVisibleResearchMessage)
    .slice(-HISTORY_TAIL_LIMIT)
    .map(messageFromApi);
  return { messages };
}

function makeEventHandler(
  pillSink: PillEventSink,
  dispatch: Dispatch<ResearchAction>
) {
  return (event: ConversationEvent) => {
    const action = mapGatewayEventToResearchAction(event);
    if (action) dispatch(action);
    const pillEv = mapGatewayEventToPillEvent(event);
    if (pillEv) pillSink.push(pillEv);
  };
}

// -------------------------------------------------------------------------
// Hook
// -------------------------------------------------------------------------

export function useResearchSession() {
  const { sessionId: urlSessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  // NO client-side conversationId seed — the server assigns it on the first
  // invoke and broadcasts it via `invoke_accepted`.
  const [state, dispatch] = useReducer(reduceResearch, EMPTY_RESEARCH_STATE);
  const { state: pillState, sink: pillSink } = useStatusPill();

  // Idempotency ref: set AFTER the async completes, inside the dispatch
  // block, so StrictMode's double-mount doesn't leave us in a "started but
  // never dispatched" state.
  const hydratedForSessionRef = useRef<string | null>(null);
  const subscribedConvIdRef = useRef<string | null>(null);

  // --- Hydrate an EXISTING session (only when URL carries one) ---
  useEffect(() => {
    if (!urlSessionId) return;
    if (hydratedForSessionRef.current === urlSessionId) return;
    (async () => {
      const transport = await getTransport();
      const snapshot = await hydrateExistingSession(transport, urlSessionId);
      if (hydratedForSessionRef.current === urlSessionId) return;
      hydratedForSessionRef.current = urlSessionId;
      if (!snapshot) {
        dispatch({ type: "ERROR", message: "Failed to load session" });
        return;
      }
      dispatch({
        type: "HYDRATE",
        sessionId: urlSessionId,
        conversationId: null, // populated on next invoke_accepted
        title: "",            // SessionTitleChanged will fill in
        status: "idle",
        wardId: null,         // WardChanged will fill in
        wardName: null,
        messages: snapshot.messages,
        turns: [],
        artifacts: [],
      });
    })();
  }, [urlSessionId]);

  // --- WS subscription for the current conversationId ---
  // `pillSink` is memoised in useStatusPill (stable identity). Listing it in
  // the deps would force a teardown+resubscribe every render, dropping WS
  // events. The closure captures the current sink; React keeps it fresh.
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
      unsubscribe.then((fn) => fn && fn()).catch(() => { /* no-op */ });
      if (subscribedConvIdRef.current === convId) {
        subscribedConvIdRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.conversationId]);

  // --- Sync URL when the backend hands us a session id ---
  useEffect(() => {
    if (state.sessionId && urlSessionId !== state.sessionId) {
      navigate(`/research-v2/${state.sessionId}`, { replace: true });
    }
  }, [state.sessionId, urlSessionId, navigate]);

  // --- Send a user message ---
  const sendMessage = useCallback(
    async (text: string) => {
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
      // For a BRAND-NEW session both sessionId and conversationId are null.
      // The transport schema requires conversationId to be a non-empty
      // string, so we pass a disposable placeholder; the backend assigns
      // its own ids and returns them via `invoke_accepted`.
      const convIdForInvoke = state.conversationId ?? placeholderConvId();
      const result = await transport.executeAgent(
        ROOT_AGENT_ID,
        convIdForInvoke,
        trimmed,
        state.sessionId ?? undefined,
        RESEARCH_MODE
      );
      if (!result.success) {
        dispatch({ type: "ERROR", message: result.error ?? "Failed to send" });
      }
    },
    [state.status, state.conversationId, state.sessionId]
  );

  // --- Stop a running turn ---
  const stopAgent = useCallback(async () => {
    if (!state.conversationId) return;
    const transport = await getTransport();
    await transport.stopAgent(state.conversationId);
  }, [state.conversationId]);

  // --- Reset for a brand-new research session ---
  // Does NOT generate a client conversationId. RESET clears state; the next
  // sendMessage invoke yields new ids via invoke_accepted.
  const startNewResearch = useCallback(() => {
    pillSink.push({ kind: "reset" });
    dispatch({ type: "RESET" });
    hydratedForSessionRef.current = null;
    subscribedConvIdRef.current = null;
    navigate("/research-v2", { replace: true });
  }, [navigate, pillSink]);

  const toggleThinking = useCallback((turnId: string) => {
    dispatch({ type: "TOGGLE_THINKING", turnId });
  }, []);

  return { state, pillState, sendMessage, stopAgent, startNewResearch, toggleThinking };
}
