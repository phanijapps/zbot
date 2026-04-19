import { useMemo, useReducer } from "react";
import { type PillState, EMPTY_PILL } from "./types";
import { describeTool } from "./tool-phrase";

// Normalized events consumed by the pill.
//
// Thinking events are intentionally *not* in this union: provider-emitted
// thinking is either absent (simple prompts) or streams per-token (complex
// prompts), so driving pill narration from it flashes unreadably or leaves
// it empty. The pill is deterministic: AgentStarted → "Thinking…",
// ToolCall → tool phrase, Respond → "Responding", AgentCompleted → fade.
export type PillEvent =
  | { kind: "idle" }
  | { kind: "reset" }
  | { kind: "agent_started"; agent_id: string }
  | { kind: "agent_completed"; agent_id: string; is_final: boolean }
  | { kind: "tool_call"; tool: string; args: Record<string, unknown> }
  | { kind: "respond" };

const STARTING_NARRATION = "Thinking…";
const RESPONDING_NARRATION = "Responding";

function handleAgentStarted(state: PillState): PillState {
  return {
    ...state,
    visible: true,
    starting: true,
    narration: STARTING_NARRATION,
    suffix: "",
    category: "neutral",
    swapCounter: state.swapCounter + 1,
  };
}

function handleAgentCompleted(
  state: PillState,
  ev: { kind: "agent_completed"; agent_id: string; is_final: boolean }
): PillState {
  if (ev.is_final) {
    return { ...EMPTY_PILL, swapCounter: state.swapCounter + 1 };
  }
  return state;
}

function handleToolCall(
  state: PillState,
  tool: string,
  args: Record<string, unknown>
): PillState {
  const phrase = describeTool(tool, args);
  return {
    ...state,
    visible: true,
    starting: false,
    narration: phrase.narration,
    suffix: phrase.suffix,
    category: phrase.category,
    swapCounter: state.swapCounter + 1,
  };
}

function handleRespond(state: PillState): PillState {
  return {
    ...state,
    visible: true,
    starting: false,
    narration: RESPONDING_NARRATION,
    suffix: "",
    category: "respond",
    swapCounter: state.swapCounter + 1,
  };
}

export function reducePillState(state: PillState, ev: PillEvent): PillState {
  switch (ev.kind) {
    case "idle":
      return state;
    case "reset":
      return EMPTY_PILL;
    case "agent_started":
      return handleAgentStarted(state);
    case "agent_completed":
      return handleAgentCompleted(state, ev);
    case "tool_call":
      return handleToolCall(state, ev.tool, ev.args);
    case "respond":
      return handleRespond(state);
    default:
      return state;
  }
}

/**
 * React hook wrapper — subscribe to pill events through the returned sink.
 * The parent page's event router calls `sink.push(PillEvent)` for each relevant
 * event; the hook folds them into PillState via the pure reducer.
 */
export interface PillEventSink {
  push(ev: PillEvent): void;
}

export function useStatusPill(): { state: PillState; sink: PillEventSink } {
  const [state, dispatch] = useReducer(reducePillState, EMPTY_PILL);
  // `dispatch` is already stable across renders; memoising `sink` preserves
  // its identity so consumers that pass it into effect-deps don't tear down
  // and rebuild their subscriptions on every render.
  const sink = useMemo<PillEventSink>(() => ({ push: dispatch }), []);
  return { state, sink };
}
