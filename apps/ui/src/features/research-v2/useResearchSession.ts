import { useCallback, useEffect, useReducer, useRef, type Dispatch } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getTransport } from "@/services/transport";
import type { Transport } from "@/services/transport";
import type { ConversationEvent, SessionMessage, UnsubscribeFn } from "@/services/transport/types";
import { useStatusPill, type PillEventSink } from "../shared/statusPill";
import { type ResearchMessage, EMPTY_RESEARCH_STATE } from "./types";
import { reduceResearch, type ResearchAction } from "./reducer";
import { mapGatewayEventToResearchAction, mapGatewayEventToPillEvent } from "./event-map";

const ROOT_AGENT_ID = "root";
const RESEARCH_MODE: string | undefined = undefined; // executor picks SessionMode::Research
const HISTORY_TAIL_LIMIT = 50;
const CONV_ID_PREFIX = "research-";

// Client-owned conv_id. Research has no `/api/chat/init`, so the UI mints
// the id and subscribes to it BEFORE invoke — that ordering is what lets
// the first token reach the UI (R14a).
function generateConvId(): string {
  return `${CONV_ID_PREFIX}${crypto.randomUUID()}`;
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
    // Assistant content renders via turn blocks; messages[] only holds prompts.
    role: m.role === "user" ? "user" : "system",
    content: m.content,
    timestamp: new Date(m.created_at).getTime(),
  };
}

// Fetch the tail of the session's root-scoped message history. Uses
// `/messages?scope=root` (returns `200 []` for extant-but-empty sessions)
// because `/api/sessions/:id/state` 404s spuriously.
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

function makeEventHandler(pillSink: PillEventSink, dispatch: Dispatch<ResearchAction>) {
  return (event: ConversationEvent) => {
    const action = mapGatewayEventToResearchAction(event);
    if (action) dispatch(action);
    const pillEv = mapGatewayEventToPillEvent(event);
    if (pillEv) pillSink.push(pillEv);
  };
}

interface SubscriptionRefs {
  subscribedConvIdRef: React.MutableRefObject<string | null>;
  unsubscribeRef: React.MutableRefObject<UnsubscribeFn | null>;
}

/** Idempotent — no-op when convId matches the currently-subscribed one. */
async function ensureSubscription(
  convId: string,
  onEvent: (event: ConversationEvent) => void,
  refs: SubscriptionRefs
): Promise<void> {
  if (refs.subscribedConvIdRef.current === convId) return;
  const transport = await getTransport();
  const unsubscribe = transport.subscribeConversation(convId, { onEvent });
  refs.subscribedConvIdRef.current = convId;
  refs.unsubscribeRef.current = unsubscribe;
}

function teardownSubscription(refs: SubscriptionRefs): void {
  const unsub = refs.unsubscribeRef.current;
  refs.unsubscribeRef.current = null;
  refs.subscribedConvIdRef.current = null;
  if (!unsub) return;
  try { unsub(); } catch { /* transport already gone is fine */ }
}

// -------------------------------------------------------------------------
// Hook
// -------------------------------------------------------------------------

export function useResearchSession() {
  const { sessionId: urlSessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const [state, dispatch] = useReducer(reduceResearch, EMPTY_RESEARCH_STATE);
  const { state: pillState, sink: pillSink } = useStatusPill();

  // Idempotency for one-shot hydration. Set AFTER async completes so
  // StrictMode's double-mount doesn't skip dispatch (learnings #6).
  const hydratedForSessionRef = useRef<string | null>(null);
  // Subscription ownership (R14a): sendMessage owns this, NOT an effect
  // keyed on state.conversationId (that was the chicken-and-egg bug).
  const subscribedConvIdRef = useRef<string | null>(null);
  const unsubscribeRef = useRef<UnsubscribeFn | null>(null);

  // --- Hydrate an EXISTING session (only when URL carries one) ---
  useEffect(() => {
    if (!urlSessionId || hydratedForSessionRef.current === urlSessionId) return;
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
        conversationId: null,
        title: "",
        status: "idle",
        wardId: null,
        wardName: null,
        messages: snapshot.messages,
        turns: [],
        artifacts: [],
      });
    })();
  }, [urlSessionId]);

  // --- Subscription cleanup on unmount (StrictMode-safe: no-op if nothing
  //     is subscribed yet; cleanup captures refs, not their .current). ---
  useEffect(() => {
    const refs: SubscriptionRefs = { subscribedConvIdRef, unsubscribeRef };
    return () => teardownSubscription(refs);
  }, []);

  // --- Sync URL when the backend hands us a session id ---
  useEffect(() => {
    if (state.sessionId && urlSessionId !== state.sessionId) {
      navigate(`/research-v2/${state.sessionId}`, { replace: true });
    }
  }, [state.sessionId, urlSessionId, navigate]);

  // --- Send a user message (subscribes BEFORE invoke, R14a) ---
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
      const convId = state.conversationId ?? generateConvId();
      const refs: SubscriptionRefs = { subscribedConvIdRef, unsubscribeRef };
      const onEvent = makeEventHandler(pillSink, dispatch);
      await ensureSubscription(convId, onEvent, refs);
      // Pre-invoke SESSION_BOUND seeds state.conversationId. The server's
      // invoke_accepted SESSION_BOUND re-dispatches with session_id; the
      // reducer's null-guard preserves whichever id lands first.
      dispatch({
        type: "SESSION_BOUND",
        conversationId: convId,
        sessionId: state.sessionId,
      });
      const transport = await getTransport();
      const result = await transport.executeAgent(
        ROOT_AGENT_ID,
        convId,
        trimmed,
        state.sessionId ?? undefined,
        RESEARCH_MODE
      );
      if (!result.success) {
        dispatch({ type: "ERROR", message: result.error ?? "Failed to send" });
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- pillSink is a
    // memoised stable reference from useStatusPill; listing it would rebuild
    // sendMessage every render and churn subscriptions.
    [state.status, state.conversationId, state.sessionId]
  );

  const stopAgent = useCallback(async () => {
    if (!state.conversationId) return;
    const transport = await getTransport();
    await transport.stopAgent(state.conversationId);
  }, [state.conversationId]);

  // --- Reset for a brand-new research session ---
  const startNewResearch = useCallback(() => {
    teardownSubscription({ subscribedConvIdRef, unsubscribeRef });
    pillSink.push({ kind: "reset" });
    dispatch({ type: "RESET" });
    hydratedForSessionRef.current = null;
    navigate("/research-v2", { replace: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- pillSink is a
    // memoised stable reference; see sendMessage.
  }, [navigate]);

  const toggleThinking = useCallback((turnId: string) => {
    dispatch({ type: "TOGGLE_THINKING", turnId });
  }, []);

  return { state, pillState, sendMessage, stopAgent, startNewResearch, toggleThinking };
}
