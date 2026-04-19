import { useCallback, useEffect, useReducer, useRef, type Dispatch } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getTransport } from "@/services/transport";
import type {
  Artifact,
  ConversationEvent,
  UnsubscribeFn,
} from "@/services/transport/types";
import { useStatusPill, type PillEventSink } from "../shared/statusPill";
import { EMPTY_RESEARCH_STATE } from "./types";
import { reduceResearch, type ResearchAction } from "./reducer";
import { mapGatewayEventToResearchAction, mapGatewayEventToPillEvent } from "./event-map";
import { snapshotSession } from "./session-snapshot";

const ROOT_AGENT_ID = "root";
// Client-owned conv_id prefix. Research has no `/api/chat/init`, so the UI
// mints the id and subscribes to it BEFORE invoke — that ordering is what
// lets the first token reach the UI (R14a).
const CONV_ID_PREFIX = "research-";
// pillSink from useStatusPill() has a stable identity (memoised sink).
// Omitting from useCallback deps is intentional; closure captures the latest
// reference. Per-line eslint-disable is still required for the linter.

// --- Event synthesis (respond-tool + delegation_completed) ----------------

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

interface EventHandlerCtx {
  pillSink: PillEventSink;
  dispatch: Dispatch<ResearchAction>;
  /** Called once per root `agent_completed` — used to re-snapshot. */
  onRootAgentCompleted: (executionId: string) => void;
}

function makeEventHandler(ctx: EventHandlerCtx) {
  return (event: ConversationEvent) => {
    const action = mapGatewayEventToResearchAction(event);
    if (action) ctx.dispatch(action);
    // Respond-tool path: synthesize RESPOND from tool_call.args.message
    // because turn_complete.final_message arrives empty for tool-emitted
    // responses (Done.final_message is populated only from streamed tokens).
    const raw = event as unknown as Record<string, unknown>;
    const synthesizedRespond = respondActionFromToolCall(raw);
    if (synthesizedRespond) ctx.dispatch(synthesizedRespond);
    const synthesizedChildRespond = respondActionFromDelegationCompleted(raw);
    if (synthesizedChildRespond) ctx.dispatch(synthesizedChildRespond);
    const pillEv = mapGatewayEventToPillEvent(event);
    if (pillEv) ctx.pillSink.push(pillEv);
    // R14f: re-snapshot on root agent_completed to backfill anything WS dropped
    // (session title, artifacts, subagent completions, per-turn respond).
    handleRootAgentCompleted(raw, ctx.onRootAgentCompleted);
  };
}

function handleRootAgentCompleted(
  raw: Record<string, unknown>,
  onRootAgentCompleted: (executionId: string) => void,
): void {
  if (raw["type"] !== "agent_completed") return;
  const parent = raw["parent_execution_id"];
  // Only root turns have a null/empty parent — children's completions don't
  // need a reconcile because their own state was already snapshot-sourced.
  const isRoot = parent == null || parent === "";
  if (!isRoot) return;
  const execId = raw["execution_id"];
  if (typeof execId !== "string" || execId.length === 0) return;
  onRootAgentCompleted(execId);
}

// --- Subscription refs ----------------------------------------------------

interface SubscriptionRefs {
  subscribedConvIdRef: React.MutableRefObject<string | null>;
  unsubscribeRef: React.MutableRefObject<UnsubscribeFn | null>;
}

/** Idempotent — no-op when convId matches the currently-subscribed one. */
async function ensureSubscription(
  convId: string,
  onEvent: (event: ConversationEvent) => void,
  refs: SubscriptionRefs,
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

// --- Snapshot → HYDRATE dispatch -----------------------------------------

async function hydrateFromSnapshot(
  sessionId: string,
  dispatch: Dispatch<ResearchAction>,
  latestArtifactsRef: { current: Artifact[] },
): Promise<void> {
  const transport = await getTransport();
  const snap = await snapshotSession(transport, sessionId);
  if (!snap) {
    dispatch({ type: "ERROR", message: "Failed to load session" });
    return;
  }
  dispatch({
    type: "HYDRATE",
    sessionId,
    conversationId: snap.conversationId,
    title: snap.title,
    status: snap.status,
    wardId: snap.wardId,
    wardName: snap.wardName,
    messages: snap.messages,
    turns: snap.turns,
    artifacts: snap.artifacts,
  });
  // Mirror the artifact records in the ref so the slide-out can resolve
  // id → Artifact without a second fetch. snapshotSession already pulled
  // /artifacts once; reuse its decision here via a parallel call to keep the
  // cache hot. On failure we just skip — slide-out will re-fetch on demand.
  try {
    const res = await transport.listSessionArtifacts(sessionId);
    if (res.success && res.data) latestArtifactsRef.current = res.data;
  } catch {
    // Intentionally silent — the snapshot's refs are enough for rendering.
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
  // R14g: second subscription on state.sessionId + scope="session". Receives
  // events routed by session_id (delegation_started/_completed,
  // session_title_changed, subagent agent_started/_completed, etc.) that the
  // conv-id-keyed subscription misses because those events lack a top-level
  // conversation_id field. Transport's seq-based dedup handles any overlap.
  const subscribedSessionIdRef = useRef<string | null>(null);
  const unsubscribeSessionRef = useRef<UnsubscribeFn | null>(null);
  // state holds light refs; latestArtifactsRef mirrors the full Artifact
  // records so ArtifactSlideOut can resolve id → Artifact without refetching.
  const latestArtifactsRef = useRef<Artifact[]>([]);
  // Guard against redundant re-snapshots when agent_completed fires more than
  // once for the same root execution (WS redelivery or duplicate dispatch).
  const resnapshotForExecRef = useRef<string | null>(null);

  // --- Hydrate an EXISTING session (only when URL carries one) ---
  useEffect(() => {
    if (!urlSessionId || hydratedForSessionRef.current === urlSessionId) return;
    (async () => {
      await hydrateFromSnapshot(urlSessionId, dispatch, latestArtifactsRef);
      // Set AFTER the dispatch (chat-v2 learning #6) so StrictMode's first
      // mount re-entering doesn't skip dispatch via a pre-completion flag.
      hydratedForSessionRef.current = urlSessionId;
    })();
  }, [urlSessionId]);

  // --- Subscription cleanup on unmount (StrictMode-safe). ---
  useEffect(() => {
    const convRefs: SubscriptionRefs = { subscribedConvIdRef, unsubscribeRef };
    const sessionRefs: SubscriptionRefs = {
      subscribedConvIdRef: subscribedSessionIdRef,
      unsubscribeRef: unsubscribeSessionRef,
    };
    return () => {
      teardownSubscription(convRefs);
      teardownSubscription(sessionRefs);
    };
  }, []);

  // --- R14g: session-id subscription (scope="session"). Fires whenever a
  // session is RUNNING and its sessionId is known (from snapshot hydrate OR
  // invoke_accepted). Receives session-routed events the conv-id subscription
  // misses (delegation, title change, subagent lifecycle). Idle/complete
  // sessions don't subscribe — nothing more to receive. ---
  useEffect(() => {
    const sid = state.sessionId;
    if (!sid || state.status !== "running") return;
    if (subscribedSessionIdRef.current === sid) return;
    const onRootAgentCompleted = (execId: string) => {
      if (resnapshotForExecRef.current === execId) return;
      resnapshotForExecRef.current = execId;
      void hydrateFromSnapshot(sid, dispatch, latestArtifactsRef);
    };
    const onEvent = makeEventHandler({ pillSink, dispatch, onRootAgentCompleted });
    // Tear down any prior session-id subscription, then register the new one.
    teardownSubscription({
      subscribedConvIdRef: subscribedSessionIdRef,
      unsubscribeRef: unsubscribeSessionRef,
    });
    let cancelled = false;
    void (async () => {
      const transport = await getTransport();
      if (cancelled) return;
      const unsub = transport.subscribeConversation(sid, {
        scope: "session",
        onEvent,
      });
      subscribedSessionIdRef.current = sid;
      unsubscribeSessionRef.current = unsub;
    })();
    return () => {
      cancelled = true;
    };
    // pillSink has stable identity; dispatch is stable; intentional exhaustive-deps skip.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.sessionId, state.status]);

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
      // Closure read: safe because only SESSION_BOUND (dispatched below) mutates state.conversationId.
      const convId = state.conversationId ?? `${CONV_ID_PREFIX}${crypto.randomUUID()}`;
      const refs: SubscriptionRefs = { subscribedConvIdRef, unsubscribeRef };
      const onRootAgentCompleted = (execId: string) => {
        if (resnapshotForExecRef.current === execId) return;
        resnapshotForExecRef.current = execId;
        const sid = state.sessionId;
        if (!sid) return;
        void hydrateFromSnapshot(sid, dispatch, latestArtifactsRef);
      };
      const onEvent = makeEventHandler({ pillSink, dispatch, onRootAgentCompleted });
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
          undefined,
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
    [state.status, state.conversationId, state.sessionId],
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
    resnapshotForExecRef.current = null;
    navigate("/research-v2", { replace: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- pillSink stable, see module-level note above.
  }, [navigate]);

  const toggleThinking = useCallback((turnId: string) => {
    dispatch({ type: "TOGGLE_THINKING", turnId });
  }, []);

  // ref → full Artifact lookup for ArtifactSlideOut. The ref is populated by
  // hydrateFromSnapshot (on open + on root agent_completed).
  const getFullArtifact = useCallback((id: string): Artifact | undefined => latestArtifactsRef.current.find((a) => a.id === id), []);

  return { state, pillState, sendMessage, stopAgent, startNewResearch, toggleThinking, getFullArtifact };
}
