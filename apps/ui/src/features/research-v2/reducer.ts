import type {
  AgentTurn,
  ResearchArtifactRef,
  ResearchMessage,
  ResearchSessionState,
  TimelineEntry,
} from "./types";
import { EMPTY_RESEARCH_STATE } from "./types";

const SILENT_CRASH_MESSAGE =
  "Turn ended with no output (provider error or context limit)";

export type ResearchAction =
  | {
      type: "HYDRATE";
      sessionId: string;
      conversationId: string | null;
      title: string;
      status: ResearchSessionState["status"];
      wardId: string | null;
      wardName: string | null;
      messages: ResearchMessage[];
      turns: AgentTurn[];
      artifacts: ResearchArtifactRef[];
    }
  | { type: "APPEND_USER"; message: ResearchMessage }
  | { type: "SESSION_BOUND"; sessionId: string; conversationId: string }
  | { type: "TITLE_CHANGED"; title: string }
  | { type: "WARD_CHANGED"; wardId: string; wardName: string }
  | {
      type: "AGENT_STARTED";
      turnId: string;
      agentId: string;
      parentExecutionId: string | null;
      wardId: string | null;
      startedAt: number;
    }
  | { type: "AGENT_COMPLETED"; turnId: string; completedAt: number }
  | { type: "AGENT_STOPPED"; turnId: string; completedAt: number }
  | { type: "THINKING_DELTA"; turnId: string; entry: TimelineEntry }
  | { type: "TOOL_CALL"; turnId: string; entry: TimelineEntry }
  | { type: "TOOL_RESULT"; turnId: string; entry: TimelineEntry }
  | { type: "TOKEN"; turnId: string; text: string }
  | { type: "RESPOND"; turnId: string; text: string }
  | { type: "TOGGLE_THINKING"; turnId: string }
  | { type: "TURN_COMPLETE"; turnId: string }
  | { type: "INTENT_ANALYSIS_STARTED" }
  | { type: "INTENT_ANALYSIS_COMPLETE"; classification: string }
  | { type: "INTENT_ANALYSIS_SKIPPED" }
  | { type: "PLAN_UPDATE"; planPath: string }
  | { type: "SESSION_COMPLETE" }
  | { type: "ERROR"; message: string }
  | { type: "RESET" }
  | { type: "SET_ARTIFACTS"; artifacts: ResearchArtifactRef[] };

// ---------------------------------------------------------------------------
// Turn helpers
// ---------------------------------------------------------------------------

function ensureTurn(
  state: ResearchSessionState,
  turnId: string,
  seed?: Partial<AgentTurn>
): ResearchSessionState {
  const existing = state.turns.find((t) => t.id === turnId);
  if (existing) return state;
  const fresh: AgentTurn = {
    id: turnId,
    agentId: seed?.agentId ?? "root",
    parentExecutionId: seed?.parentExecutionId ?? null,
    startedAt: seed?.startedAt ?? Date.now(),
    completedAt: null,
    status: "running",
    wardId: seed?.wardId ?? state.wardId,
    timeline: [],
    tokenCount: 0,
    respond: null,
    respondStreaming: "",
    thinkingExpanded: false,
    errorMessage: null,
    ...seed,
  };
  return { ...state, turns: [...state.turns, fresh] };
}

function updateTurn(
  state: ResearchSessionState,
  turnId: string,
  patch: (t: AgentTurn) => AgentTurn
): ResearchSessionState {
  return {
    ...state,
    turns: state.turns.map((t) => (t.id === turnId ? patch(t) : t)),
  };
}

function turnHasMeaningfulContent(turn: AgentTurn): boolean {
  if (turn.respond && turn.respond.length > 0) return true;
  if (turn.respondStreaming && turn.respondStreaming.length > 0) return true;
  return turn.timeline.some(
    (e) => e.kind === "tool_call" || e.kind === "tool_result"
  );
}

// ---------------------------------------------------------------------------
// Per-case handlers (keeps the switch under SonarQube's complexity threshold)
// ---------------------------------------------------------------------------

function handleHydrate(
  state: ResearchSessionState,
  action: Extract<ResearchAction, { type: "HYDRATE" }>
): ResearchSessionState {
  return {
    ...state,
    sessionId: action.sessionId,
    conversationId: action.conversationId,
    title: action.title,
    status: action.status,
    wardId: action.wardId,
    wardName: action.wardName,
    messages: action.messages,
    turns: action.turns,
    artifacts: action.artifacts,
  };
}

function handleAgentStarted(
  state: ResearchSessionState,
  action: Extract<ResearchAction, { type: "AGENT_STARTED" }>
): ResearchSessionState {
  // Sticky ward: null wardId on the event inherits from state (never clear).
  const wardForTurn = action.wardId ?? state.wardId;
  return ensureTurn(state, action.turnId, {
    agentId: action.agentId,
    parentExecutionId: action.parentExecutionId,
    startedAt: action.startedAt,
    wardId: wardForTurn,
  });
}

function handleAgentCompleted(
  state: ResearchSessionState,
  action: Extract<ResearchAction, { type: "AGENT_COMPLETED" }>
): ResearchSessionState {
  return updateTurn(state, action.turnId, (t) => {
    if (turnHasMeaningfulContent(t)) {
      return { ...t, status: "completed", completedAt: action.completedAt };
    }
    // Silent-crash inference — workaround for chat-v2 backlog B3.
    return {
      ...t,
      status: "error",
      completedAt: action.completedAt,
      errorMessage: SILENT_CRASH_MESSAGE,
    };
  });
}

function handleAgentStopped(
  state: ResearchSessionState,
  action: Extract<ResearchAction, { type: "AGENT_STOPPED" }>
): ResearchSessionState {
  return updateTurn(state, action.turnId, (t) => ({
    ...t,
    status: "stopped",
    completedAt: action.completedAt,
  }));
}

function handleTimelineAppend(
  state: ResearchSessionState,
  turnId: string,
  entry: TimelineEntry
): ResearchSessionState {
  const seeded = ensureTurn(state, turnId);
  return updateTurn(seeded, turnId, (t) => ({
    ...t,
    timeline: [...t.timeline, entry],
  }));
}

function handleToken(
  state: ResearchSessionState,
  action: Extract<ResearchAction, { type: "TOKEN" }>
): ResearchSessionState {
  const seeded = ensureTurn(state, action.turnId);
  return updateTurn(seeded, action.turnId, (t) => ({
    ...t,
    respondStreaming: t.respondStreaming + action.text,
  }));
}

function handleRespond(
  state: ResearchSessionState,
  action: Extract<ResearchAction, { type: "RESPOND" }>
): ResearchSessionState {
  const seeded = ensureTurn(state, action.turnId);
  return updateTurn(seeded, action.turnId, (t) => ({
    ...t,
    respond: action.text,
    respondStreaming: "",
  }));
}

function handleToggleThinking(
  state: ResearchSessionState,
  action: Extract<ResearchAction, { type: "TOGGLE_THINKING" }>
): ResearchSessionState {
  return updateTurn(state, action.turnId, (t) => ({
    ...t,
    thinkingExpanded: !t.thinkingExpanded,
  }));
}

// ---------------------------------------------------------------------------
// Reducer
// ---------------------------------------------------------------------------

export function reduceResearch(
  state: ResearchSessionState,
  action: ResearchAction
): ResearchSessionState {
  switch (action.type) {
    case "HYDRATE":
      return handleHydrate(state, action);
    case "APPEND_USER":
      return { ...state, messages: [...state.messages, action.message], status: "running" };
    case "SESSION_BOUND":
      return { ...state, sessionId: action.sessionId, conversationId: action.conversationId };
    case "TITLE_CHANGED":
      return { ...state, title: action.title };
    case "WARD_CHANGED":
      return { ...state, wardId: action.wardId, wardName: action.wardName };
    case "AGENT_STARTED":
      return handleAgentStarted(state, action);
    case "AGENT_COMPLETED":
      return handleAgentCompleted(state, action);
    case "AGENT_STOPPED":
      return handleAgentStopped(state, action);
    case "THINKING_DELTA":
    case "TOOL_CALL":
    case "TOOL_RESULT":
      return handleTimelineAppend(state, action.turnId, action.entry);
    case "TOKEN":
      return handleToken(state, action);
    case "RESPOND":
      return handleRespond(state, action);
    case "TOGGLE_THINKING":
      return handleToggleThinking(state, action);
    case "TURN_COMPLETE":
      // Informational — real completion signal is AGENT_COMPLETED.
      return state;
    case "INTENT_ANALYSIS_STARTED":
      return { ...state, intentAnalyzing: true };
    case "INTENT_ANALYSIS_COMPLETE":
      return { ...state, intentAnalyzing: false, intentClassification: action.classification };
    case "INTENT_ANALYSIS_SKIPPED":
      return { ...state, intentAnalyzing: false };
    case "PLAN_UPDATE":
      return { ...state, planPath: action.planPath };
    case "SESSION_COMPLETE":
      return { ...state, status: "complete" };
    case "ERROR":
      return { ...state, status: "error" };
    case "RESET":
      return EMPTY_RESEARCH_STATE;
    case "SET_ARTIFACTS":
      return { ...state, artifacts: action.artifacts };
    default:
      return state;
  }
}
