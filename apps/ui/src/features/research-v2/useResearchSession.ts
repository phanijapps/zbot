import { useCallback, useEffect, useReducer, useRef, type Dispatch } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getTransport, type Transport } from "@/services/transport";
import type {
  Artifact,
  ConversationEvent,
  SessionMessage,
  UnsubscribeFn,
} from "@/services/transport/types";
import { useStatusPill, type PillEventSink } from "../shared/statusPill";
import { type ResearchArtifactRef, type ResearchMessage, EMPTY_RESEARCH_STATE } from "./types";
import { reduceResearch, type ResearchAction } from "./reducer";
import { mapGatewayEventToResearchAction, mapGatewayEventToPillEvent } from "./event-map";
import { fetchArtifactsOnce, startArtifactPolling } from "./artifact-poll";

const ROOT_AGENT_ID = "root";
const HISTORY_TAIL_LIMIT = 50;
// Client-owned conv_id prefix. Research has no `/api/chat/init`, so the UI
// mints the id and subscribes to it BEFORE invoke — that ordering is what
// lets the first token reach the UI (R14a).
const CONV_ID_PREFIX = "research-";
// pillSink from useStatusPill() has a stable identity (memoised sink).
// Omitting from useCallback deps is intentional; closure captures the latest
// reference. Per-line eslint-disable is still required for the linter.

// --- Pure helpers ---------------------------------------------------------

function isVisibleResearchMessage(m: SessionMessage): boolean {
  if (m.role === "tool") return false;
  if (m.role === "assistant" && m.content.trim() === "[tool calls]") return false;
  return m.role === "user" || m.role === "assistant";
}

function messageFromApi(m: SessionMessage): ResearchMessage {
  return {
    id: m.id,
    // Live sessions render the agent's answer through turn blocks; hydrated
    // history lacks turns to rebuild from, so the page renders these assistant
    // messages directly as markdown (see ResearchPage.MainColumn).
    role: m.role === "user" ? "user" : "assistant",
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

/**
 * When the agent calls the `respond` tool, the gateway broadcasts:
 *   - `tool_call` with `tool_name: "respond"` and `args: { message: "..." }`
 *   - `tool_result` with the acknowledgement
 *   - `turn_complete` with `final_message: ""` (empty — the respond message
 *     is NOT in final_message; Done.final_message is populated only from
 *     streamed tokens, not from the respond tool).
 *
 * So the definitive source of the final answer is `tool_call.args.message`
 * on the `respond` tool. Synthesize a RESPOND action for it.
 */
function respondActionFromToolCall(
  event: Record<string, unknown>,
): ResearchAction | null {
  if (event["type"] !== "tool_call") return null;
  const toolName = event["tool_name"] ?? event["tool"];
  if (toolName !== "respond") return null;
  const args = event["args"];
  if (!args || typeof args !== "object") return null;
  const message = (args as Record<string, unknown>)["message"];
  if (typeof message !== "string" || message.length === 0) return null;
  const execId = event["execution_id"];
  const turnId = typeof execId === "string" && execId.length > 0 ? execId : "orphan";
  return { type: "RESPOND", turnId, text: message };
}

/**
 * `delegation_completed` is the only event we reliably receive for a child
 * subagent (its own WS events run on a different conv_id). The `result`
 * field carries the child's final answer — populate the child turn's
 * respond body from it so the nested turn renders its output.
 */
function respondActionFromDelegationCompleted(
  event: Record<string, unknown>,
): ResearchAction | null {
  if (event["type"] !== "delegation_completed") return null;
  const childExec = event["child_execution_id"];
  if (typeof childExec !== "string" || childExec.length === 0) return null;
  const result = event["result"];
  if (typeof result !== "string" || result.length === 0) return null;
  return { type: "RESPOND", turnId: childExec, text: result };
}

function makeEventHandler(pillSink: PillEventSink, dispatch: Dispatch<ResearchAction>) {
  return (event: ConversationEvent) => {
    const action = mapGatewayEventToResearchAction(event);
    if (action) dispatch(action);
    // Respond-tool path: synthesize RESPOND from tool_call.args.message
    // because turn_complete.final_message arrives empty for tool-emitted
    // responses (Done.final_message is populated only from streamed tokens).
    const raw = event as unknown as Record<string, unknown>;
    const synthesizedRespond = respondActionFromToolCall(raw);
    if (synthesizedRespond) dispatch(synthesizedRespond);
    const synthesizedChildRespond = respondActionFromDelegationCompleted(raw);
    if (synthesizedChildRespond) dispatch(synthesizedChildRespond);
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
  try {
    unsub();
  } catch (err) {
    console.warn("[research-v2] unsubscribe failed", err);
  }
}

// --- Hook -----------------------------------------------------------------

export function useResearchSession() {
  const { sessionId: urlSessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const [state, dispatch] = useReducer(reduceResearch, EMPTY_RESEARCH_STATE);
  const { state: pillState, sink: pillSink } = useStatusPill();

  const hydratedForSessionRef = useRef<string | null>(null); // one-shot hydration guard (StrictMode)
  const subscribedConvIdRef = useRef<string | null>(null); // R14a: sendMessage owns subscription
  const unsubscribeRef = useRef<UnsubscribeFn | null>(null);
  // R14d: full Artifact[] from last poll (state only holds light refs); artifactsRef
  // mirrors state.artifacts so the poll closure diffs without re-running every render.
  const latestArtifactsRef = useRef<Artifact[]>([]);
  const artifactsRef = useRef<ResearchArtifactRef[]>(state.artifacts);
  artifactsRef.current = state.artifacts;

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
      // TODO: populate title/wardId/wardName/turns/artifacts from /state when the
      // spurious-404 issue documented in hydrateExistingSession() is fixed.
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

  // --- Subscription cleanup on unmount (StrictMode-safe). ---
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

  // --- Poll artifacts while running; one final fetch on transition to complete (R14d) ---
  useEffect(() => {
    const sid = state.sessionId;
    if (!sid) return;
    if (state.status === "running") return startArtifactPolling(sid, artifactsRef, dispatch, latestArtifactsRef);
    if (state.status === "complete") void fetchArtifactsOnce(sid, artifactsRef.current, dispatch, latestArtifactsRef);
    return undefined;
  }, [state.sessionId, state.status]);

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
      // Closure read: safe because only SESSION_BOUND (dispatched below) mutates state.conversationId.
      const convId = state.conversationId ?? `${CONV_ID_PREFIX}${crypto.randomUUID()}`;
      const refs: SubscriptionRefs = { subscribedConvIdRef, unsubscribeRef };
      const onEvent = makeEventHandler(pillSink, dispatch);
      try {
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
          // mode undefined → executor defaults to SessionMode::Research
          undefined
        );
        if (!result.success) {
          dispatch({ type: "ERROR", message: result.error ?? "Failed to send" });
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to send";
        dispatch({ type: "ERROR", message });
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- pillSink stable, see module-level note above.
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
    // eslint-disable-next-line react-hooks/exhaustive-deps -- pillSink stable, see module-level note above.
  }, [navigate]);

  const toggleThinking = useCallback((turnId: string) => {
    dispatch({ type: "TOGGLE_THINKING", turnId });
  }, []);

  // R14d: ref → full Artifact lookup for ArtifactSlideOut (polling keeps latestArtifactsRef fresh).
  const getFullArtifact = useCallback((id: string): Artifact | undefined => latestArtifactsRef.current.find((a) => a.id === id), []);

  return { state, pillState, sendMessage, stopAgent, startNewResearch, toggleThinking, getFullArtifact };
}
