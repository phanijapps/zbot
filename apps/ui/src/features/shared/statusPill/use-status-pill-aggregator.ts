import { useEffect, useReducer } from "react";
import { type PillState, EMPTY_PILL, NARRATION_MAX } from "./types";
import { describeTool } from "./tool-phrase";

// Normalized events — aggregator only needs these kinds.
export type PillEvent =
  | { kind: "idle" }
  | { kind: "reset" }
  | { kind: "agent_started"; agent_id: string }
  | { kind: "agent_completed"; agent_id: string; is_final: boolean }
  | { kind: "thinking"; content: string }
  | { kind: "tool_call"; tool: string; args: Record<string, unknown> }
  | { kind: "respond" };

function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return s.slice(0, max - 1) + "…";
}

function handleAgentStarted(state: PillState): PillState {
  return {
    ...state,
    visible: true,
    starting: state.narration === "" && state.suffix === "",
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

function handleThinking(state: PillState, content: string): PillState {
  return {
    ...state,
    visible: true,
    starting: false,
    narration: truncate(content.trim(), NARRATION_MAX),
    swapCounter: state.swapCounter + 1,
  };
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
    // Narration from Thinking takes priority over the dictionary fallback.
    narration: state.narration || phrase.narration,
    suffix: phrase.suffix,
    category: phrase.category,
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
    case "thinking":
      return handleThinking(state, ev.content);
    case "tool_call":
      return handleToolCall(state, ev.tool, ev.args);
    case "respond":
      return { ...state, category: "respond", swapCounter: state.swapCounter + 1 };
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
  const sink: PillEventSink = { push: dispatch };
  // Kept for potential future teardown needs in strict-mode.
  useEffect(() => {
    return () => { /* no-op */ };
  }, []);
  return { state, sink };
}
