// ============================================================================
// MISSION HOOKS
// Maps WebSocket events to Mission Control state (NarrativeBlock[])
// Modeled after WebChatPanel's event handling but outputs richer block types.
// ============================================================================

import { useState, useEffect, useRef, useCallback } from "react";
import { getTransport, type StreamEvent } from "@/services/transport";
import type { LogSession } from "@/services/transport/types";
import type { SubagentStateData } from "@/services/transport/types";
import type { Phase } from "./PhaseIndicators";
import type { PlanStep, StepStatus } from "./PlanBlock";
import type { RecalledFact, SubagentInfo } from "./IntelligenceFeed";
import type { UploadedFile } from "./ChatInput";

// ============================================================================
// Types
// ============================================================================

export interface NarrativeBlock {
  id: string;
  type: "user" | "recall" | "tool" | "delegation" | "plan" | "response" | "intent_analysis";
  timestamp: string;
  /** Shape depends on `type` — see block components for expected props */
  data: Record<string, unknown>;
  isStreaming?: boolean;
  isExpanded?: boolean;
}

export interface IntentAnalysis {
  primaryIntent: string;
  hiddenIntents: string[];
  recommendedSkills: string[];
  recommendedAgents: string[];
  wardRecommendation: {
    action: string;
    wardName: string;
    subdirectory?: string;
    reason: string;
  };
  executionStrategy: {
    approach: string;
    graph?: {
      nodes: Array<{ id: string; task: string; agent: string; skills: string[] }>;
      mermaid?: string;
    };
    explanation: string;
  };
}

export interface MissionState {
  blocks: NarrativeBlock[];
  sessionTitle: string;
  status: "idle" | "running" | "completed" | "error";
  phase: Phase;
  tokenCount: number;
  durationMs: number;
  modelName: string;
  subagents: SubagentInfo[];
  plan: PlanStep[];
  recalledFacts: RecalledFact[];
  activeWard: { name: string; content: string } | null;
  intentAnalysis: IntentAnalysis | null;
}

// ============================================================================
// Helpers
// ============================================================================

const ROOT_AGENT_ID = "root";
const WEB_CONV_ID_KEY = "agentzero_web_conv_id";
const WEB_SESSION_ID_KEY = "agentzero_web_session_id";

function getOrCreateConversationId(): string {
  // Check for ?new=1 param — start fresh session
  const params = new URLSearchParams(window.location.search);
  if (params.get("new") === "1") {
    // Clear the param from URL without reload
    params.delete("new");
    const newUrl = window.location.pathname + (params.toString() ? `?${params}` : "");
    window.history.replaceState({}, "", newUrl);
    return createNewConversationId();
  }

  let convId = localStorage.getItem(WEB_CONV_ID_KEY);
  if (!convId) {
    convId = `web-${crypto.randomUUID()}`;
    localStorage.setItem(WEB_CONV_ID_KEY, convId);
  }
  return convId;
}

/** Create a fresh conversation — clears session state so the next invoke creates a new session */
function createNewConversationId(): string {
  localStorage.removeItem(WEB_SESSION_ID_KEY);
  const convId = `web-${crypto.randomUUID()}`;
  localStorage.setItem(WEB_CONV_ID_KEY, convId);
  return convId;
}

function getSessionId(): string | null {
  return localStorage.getItem(WEB_SESSION_ID_KEY);
}

function setSessionId(sessionId: string): void {
  localStorage.setItem(WEB_SESSION_ID_KEY, sessionId);
}

function now(): string {
  return new Date().toISOString();
}

/** Try to parse JSON arguments from a tool call */
function parseArgs(args: unknown): Record<string, unknown> {
  if (!args) return {};
  if (typeof args === "object") return args as Record<string, unknown>;
  if (typeof args === "string") {
    try { return JSON.parse(args); } catch { return {}; }
  }
  return {};
}

// ============================================================================
// Event handler context — passed to extracted handler functions
// ============================================================================

interface EventHandlerCtx {
  setBlocks: React.Dispatch<React.SetStateAction<NarrativeBlock[]>>;
  setStatus: React.Dispatch<React.SetStateAction<MissionState["status"]>>;
  setPhase: React.Dispatch<React.SetStateAction<Phase>>;
  setSessionTitle: React.Dispatch<React.SetStateAction<string>>;
  setTokenCount: React.Dispatch<React.SetStateAction<number>>;
  setModelName: React.Dispatch<React.SetStateAction<string>>;
  setSubagents: React.Dispatch<React.SetStateAction<SubagentInfo[]>>;
  setPlan: React.Dispatch<React.SetStateAction<PlanStep[]>>;
  setRecalledFacts: React.Dispatch<React.SetStateAction<RecalledFact[]>>;
  setActiveWard: React.Dispatch<React.SetStateAction<{ name: string; content: string } | null>>;
  setIntentAnalysis: React.Dispatch<React.SetStateAction<IntentAnalysis | null>>;
  setActiveSessionId: React.Dispatch<React.SetStateAction<string | null>>;
  toolCallBlockMapRef: React.MutableRefObject<Map<string, string>>;
  executionAgentMapRef: React.MutableRefObject<Map<string, string>>;
  titleFallbackTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>;
  lastUserMessageRef: React.MutableRefObject<string>;
  streamingBufferRef: React.MutableRefObject<string>;
  rafIdRef: React.MutableRefObject<number | null>;
  flushTokenBuffer: () => void;
  startDurationTimer: () => void;
  stopDurationTimer: () => void;
  generateFallbackTitle: (message: string) => string;
}

// ============================================================================
// Extracted event handlers
// ============================================================================

function handleInvokeAccepted(event: StreamEvent, ctx: EventHandlerCtx): void {
  if (event.session_id && typeof event.session_id === "string") {
    setSessionId(event.session_id);
    ctx.setActiveSessionId(event.session_id);
  }
}

function handleAgentStarted(event: StreamEvent, ctx: EventHandlerCtx): void {
  ctx.setStatus("running");
  ctx.startDurationTimer();
  if (event.session_id && typeof event.session_id === "string") {
    setSessionId(event.session_id);
    ctx.setActiveSessionId(event.session_id);
  }
  if (event.model && typeof event.model === "string") {
    ctx.setModelName(event.model);
  }

  // Clear any previous fallback timer
  if (ctx.titleFallbackTimerRef.current) {
    clearTimeout(ctx.titleFallbackTimerRef.current);
    ctx.titleFallbackTimerRef.current = null;
  }

  // Start fallback title timer — if no title arrives in 10s, generate from user message
  ctx.titleFallbackTimerRef.current = setTimeout(() => {
    ctx.setSessionTitle((current) => {
      if (current) return current; // Title already set
      const msg = ctx.lastUserMessageRef.current;
      if (!msg) return current;
      return ctx.generateFallbackTitle(msg);
    });
    ctx.titleFallbackTimerRef.current = null;
  }, 10_000);
}

function handleTokenEvent(event: StreamEvent, ctx: EventHandlerCtx): void {
  const delta = (event.delta ?? event.content ?? "") as string;
  if (delta) {
    ctx.streamingBufferRef.current += delta;
    if (ctx.rafIdRef.current === null) {
      ctx.rafIdRef.current = requestAnimationFrame(ctx.flushTokenBuffer);
    }
  }
  // Track tokens from any available field
  const totalTok = (event.total_tokens ?? event.tokens_in ?? event.token_count) as number | undefined;
  if (typeof totalTok === "number" && totalTok > 0) {
    ctx.setTokenCount(totalTok);
  } else {
    // Increment by 1 per token event as fallback
    ctx.setTokenCount((prev) => prev + 1);
  }
}

function handleToolCallSetSessionTitle(args: Record<string, unknown>, ctx: EventHandlerCtx): void {
  const title = (args.title ?? args.name ?? "") as string;
  if (title) {
    ctx.setSessionTitle(title);
    if (ctx.titleFallbackTimerRef.current) {
      clearTimeout(ctx.titleFallbackTimerRef.current);
      ctx.titleFallbackTimerRef.current = null;
    }
  }
}

function handleToolCallMemoryRecall(toolCallId: string, ctx: EventHandlerCtx): void {
  const blockId = crypto.randomUUID();
  if (toolCallId) ctx.toolCallBlockMapRef.current.set(toolCallId, blockId);
  ctx.setBlocks((prev) => [
    ...prev,
    {
      id: blockId,
      type: "recall",
      timestamp: now(),
      data: { raw: "" }, // Will be filled by tool_result
    },
  ]);
}

function parsePlanStepsFromArray(rawSteps: unknown[]): PlanStep[] {
  const steps: PlanStep[] = [];
  for (const s of rawSteps) {
    if (typeof s === "string") {
      steps.push({ text: s, status: "pending" as StepStatus });
    } else if (typeof s === "object" && s) {
      const obj = s as Record<string, unknown>;
      steps.push({
        text: (obj.text ?? obj.step ?? obj.description ?? "") as string,
        status: (obj.status ?? "pending") as StepStatus,
      });
    }
  }
  return steps;
}

function parsePlanStepsFromString(rawSteps: string): PlanStep[] {
  const steps: PlanStep[] = [];
  const lines = rawSteps.split("\n").filter((l: string) => l.trim());
  for (const line of lines) {
    const trimmed = line.replace(/^[\s\-*\d.]+/, "").trim();
    if (trimmed) {
      const isDone = line.includes("[x]") || line.includes("✓");
      steps.push({ text: trimmed, status: isDone ? "done" as StepStatus : "pending" as StepStatus });
    }
  }
  return steps;
}

function parsePlanSteps(args: Record<string, unknown>): PlanStep[] {
  const rawSteps = args.steps ?? args.plan ?? args.content ?? "";
  if (Array.isArray(rawSteps)) return parsePlanStepsFromArray(rawSteps);
  if (typeof rawSteps === "string" && rawSteps.trim()) return parsePlanStepsFromString(rawSteps);
  return [];
}

function upsertPlanBlock(steps: PlanStep[], ctx: EventHandlerCtx): void {
  const planData = { steps: steps.length > 0 ? steps : [{ text: "Planning...", status: "active" as StepStatus }] };
  ctx.setBlocks((prev) => {
    const existingIdx = prev.findIndex((b) => b.type === "plan");
    if (existingIdx >= 0) {
      const updated = [...prev];
      updated[existingIdx] = { ...updated[existingIdx], data: planData };
      return updated;
    }
    return [...prev, { id: crypto.randomUUID(), type: "plan", timestamp: now(), data: planData }];
  });
}

function handleToolCallUpdatePlan(args: Record<string, unknown>, ctx: EventHandlerCtx): void {
  const steps = parsePlanSteps(args);
  if (steps.length > 0) {
    ctx.setPlan(steps);
  }
  upsertPlanBlock(steps, ctx);
}

function handleToolCallRespond(args: Record<string, unknown>, ctx: EventHandlerCtx): void {
  const respondMsg = (args.message ?? "") as string;
  if (respondMsg) {
    ctx.setBlocks((prev) => [
      ...prev,
      {
        id: crypto.randomUUID(),
        type: "response",
        timestamp: now(),
        data: { content: respondMsg, timestamp: now() },
        isStreaming: false,
      },
    ]);
  }
  ctx.setPhase("responding");
}

function handleToolCallDelegateToAgent(
  args: Record<string, unknown>,
  toolCallId: string,
  inputSummary: string,
  ctx: EventHandlerCtx,
): void {
  const delegateAgentId = (args.agent_id ?? args.agentId ?? "") as string;
  const task = (args.task ?? args.message ?? inputSummary) as string;
  const blockId = crypto.randomUUID();
  if (toolCallId) ctx.toolCallBlockMapRef.current.set(toolCallId, blockId);
  ctx.setBlocks((prev) => [
    ...prev,
    {
      id: blockId,
      type: "delegation",
      timestamp: now(),
      data: { agentId: delegateAgentId, task, status: "active" },
    },
  ]);
  ctx.setSubagents((prev) => [
    ...prev.filter((s) => !(s.agentId === delegateAgentId && s.status === "active")),
    { agentId: delegateAgentId, task, status: "active" },
  ]);
  ctx.setPhase((prev) => prev === "planning" || prev === "intent" ? "executing" : prev);
}

function handleToolCallGeneric(
  toolName: string,
  toolCallId: string,
  inputSummary: string,
  ctx: EventHandlerCtx,
): void {
  const blockId = crypto.randomUUID();
  if (toolCallId) ctx.toolCallBlockMapRef.current.set(toolCallId, blockId);
  ctx.setBlocks((prev) => [
    ...prev,
    {
      id: blockId,
      type: "tool",
      timestamp: now(),
      data: {
        toolName,
        input: inputSummary,
        isExpanded: false,
      },
      isExpanded: false,
    },
  ]);
}

function handleToolCallEvent(event: StreamEvent, ctx: EventHandlerCtx): void {
  const toolName = (event.tool ?? event.tool_name ?? "") as string;
  const args = parseArgs(event.arguments ?? event.args);
  const toolCallId = (event.tool_call_id ?? event.id ?? "") as string;
  const inputSummary = (event.input ?? JSON.stringify(args).slice(0, 200)) as string;
  const action = (args.action ?? args.operation ?? "") as string;

  if (toolName === "set_session_title") {
    handleToolCallSetSessionTitle(args, ctx);
  } else if (toolName === "memory" && action === "recall") {
    handleToolCallMemoryRecall(toolCallId, ctx);
  } else if (toolName === "update_plan") {
    handleToolCallUpdatePlan(args, ctx);
  } else if (toolName === "respond") {
    handleToolCallRespond(args, ctx);
  } else if (toolName === "delegate_to_agent") {
    handleToolCallDelegateToAgent(args, toolCallId, inputSummary, ctx);
  } else {
    handleToolCallGeneric(toolName, toolCallId, inputSummary, ctx);
  }
}

function handleToolResultRecallBlock(
  block: NarrativeBlock,
  result: string,
  updated: NarrativeBlock[],
  idx: number,
  ctx: EventHandlerCtx,
): void {
  updated[idx] = { ...block, data: { raw: result } };
  try {
    const parsed = JSON.parse(result);
    const facts = parsed.results ?? parsed.facts ?? [];
    if (Array.isArray(facts) && facts.length > 0) {
      ctx.setRecalledFacts(
        facts.map((f: Record<string, unknown>) => ({
          key: (f.key ?? "") as string,
          content: (f.content ?? "") as string,
          category: (f.category ?? "") as string,
          confidence: (f.confidence ?? 0) as number,
        }))
      );
    }
  } catch { /* ignore parse failure */ }
}

function handleToolResultToolBlock(
  block: NarrativeBlock,
  result: string,
  isError: boolean,
  updated: NarrativeBlock[],
  idx: number,
  ctx: EventHandlerCtx,
): void {
  updated[idx] = {
    ...block,
    data: { ...block.data, output: result, isError },
  };
  const toolName = block.data.toolName as string;
  if (toolName === "ward" || toolName === "set_ward" || toolName === "enter_ward") {
    try {
      const parsed = JSON.parse(result);
      if (parsed.__ward_changed__ || parsed.action === "switched") {
        const wardName = (parsed.ward_name ?? parsed.name ?? "unknown") as string;
        const wardContent = (parsed.agents_md ?? parsed.content ?? "") as string;
        ctx.setActiveWard({ name: wardName, content: wardContent.slice(0, 300) });
      }
    } catch { /* not ward JSON, ignore */ }
  }
}

function handleToolResultPlanBlock(
  block: NarrativeBlock,
  result: string,
  updated: NarrativeBlock[],
  idx: number,
  ctx: EventHandlerCtx,
): void {
  try {
    const parsed = JSON.parse(result);
    if (Array.isArray(parsed.steps)) {
      const steps = parsed.steps.map((s: Record<string, unknown>) => ({
        text: (s.text ?? "") as string,
        status: (s.status ?? "pending") as StepStatus,
      }));
      updated[idx] = { ...block, data: { steps } };
      ctx.setPlan(steps);
    }
  } catch {
    // leave block as-is
  }
}

function handleToolResultEvent(event: StreamEvent, ctx: EventHandlerCtx): void {
  const toolCallId = (event.tool_call_id ?? "") as string;
  const result = (event.result ?? event.output ?? "") as string;
  const isError = event.is_error === true || event.error === true;
  const blockId = toolCallId ? ctx.toolCallBlockMapRef.current.get(toolCallId) : undefined;

  if (blockId) {
    ctx.setBlocks((prev) => {
      const idx = prev.findIndex((b) => b.id === blockId);
      if (idx < 0) return prev;
      const block = prev[idx];
      const updated = [...prev];

      if (block.type === "recall") {
        handleToolResultRecallBlock(block, result, updated, idx, ctx);
      } else if (block.type === "tool") {
        handleToolResultToolBlock(block, result, isError, updated, idx, ctx);
      } else if (block.type === "delegation") {
        updated[idx] = {
          ...block,
          data: { ...block.data, result, status: isError ? "error" : "completed" },
        };
      } else if (block.type === "plan") {
        handleToolResultPlanBlock(block, result, updated, idx, ctx);
      }

      return updated;
    });
    ctx.toolCallBlockMapRef.current.delete(toolCallId);
  }
}

function handleDelegationStarted(event: StreamEvent, ctx: EventHandlerCtx): void {
  const childAgentId = (event.child_agent_id ?? "") as string;
  const task = (event.task ?? "") as string;

  const childExecId = (event.child_execution_id ?? event.execution_id) as string | undefined;
  if (childExecId && childAgentId) {
    ctx.executionAgentMapRef.current.set(childExecId, childAgentId);
  }

  ctx.setBlocks((prev) => {
    const idx = prev.findIndex(
      (b) => b.type === "delegation" && b.data.agentId === childAgentId && b.data.status === "active",
    );
    if (idx >= 0) {
      const updated = [...prev];
      updated[idx] = {
        ...updated[idx],
        data: { ...updated[idx].data, task: task || updated[idx].data.task },
      };
      return updated;
    }
    return [
      ...prev,
      {
        id: crypto.randomUUID(),
        type: "delegation",
        timestamp: now(),
        data: { agentId: childAgentId, task, status: "active" },
      },
    ];
  });

  ctx.setSubagents((prev) => {
    const existing = prev.find((s) => s.agentId === childAgentId && s.status === "active");
    if (existing) return prev;
    return [...prev, { agentId: childAgentId, task, status: "active" }];
  });
}

function handleDelegationCompleted(event: StreamEvent, ctx: EventHandlerCtx): void {
  const childAgentId = (event.child_agent_id ?? "") as string;
  const result = (event.result ?? "") as string;

  ctx.setBlocks((prev) => {
    const idx = prev.findIndex(
      (b) => b.type === "delegation" && b.data.agentId === childAgentId && b.data.status === "active",
    );
    if (idx >= 0) {
      const updated = [...prev];
      updated[idx] = {
        ...updated[idx],
        data: { ...updated[idx].data, status: "completed", result },
      };
      return updated;
    }
    return prev;
  });

  ctx.setSubagents((prev) =>
    prev.map((s) =>
      s.agentId === childAgentId && s.status === "active"
        ? { ...s, status: "completed" }
        : s,
    ),
  );
}

function handleDelegationError(event: StreamEvent, ctx: EventHandlerCtx): void {
  const childAgentId = (event.child_agent_id ?? "") as string;
  const error = (event.error ?? "") as string;

  ctx.setBlocks((prev) => {
    const idx = prev.findIndex(
      (b) => b.type === "delegation" && b.data.agentId === childAgentId && b.data.status === "active",
    );
    if (idx >= 0) {
      const updated = [...prev];
      updated[idx] = {
        ...updated[idx],
        data: { ...updated[idx].data, status: "error", result: error },
      };
      return updated;
    }
    return prev;
  });

  ctx.setSubagents((prev) =>
    prev.map((s) =>
      s.agentId === childAgentId && s.status === "active"
        ? { ...s, status: "error" }
        : s,
    ),
  );
}

function handleSessionTitleChanged(event: StreamEvent, ctx: EventHandlerCtx): void {
  const title = (event.title ?? "") as string;
  if (title) {
    ctx.setSessionTitle(title);
    if (ctx.titleFallbackTimerRef.current) {
      clearTimeout(ctx.titleFallbackTimerRef.current);
      ctx.titleFallbackTimerRef.current = null;
    }
  }
}

function handleWardChanged(event: StreamEvent, ctx: EventHandlerCtx): void {
  const wardId = (event.ward_id ?? "") as string;
  if (wardId) {
    ctx.setActiveWard({ name: wardId, content: "" });
  }
}

function handleIntentAnalysisStarted(ctx: EventHandlerCtx): void {
  ctx.setBlocks((prev) => [
    ...prev,
    {
      id: "intent-streaming",
      type: "intent_analysis",
      timestamp: now(),
      data: {},
      isStreaming: true,
    },
  ]);
}

function handleIntentAnalysisComplete(event: StreamEvent, ctx: EventHandlerCtx): void {
  const wardRec = event.ward_recommendation as Record<string, unknown> | undefined;
  const execStrat = event.execution_strategy as Record<string, unknown> | undefined;
  const ia: IntentAnalysis = {
    primaryIntent: (event.primary_intent ?? "") as string,
    hiddenIntents: (event.hidden_intents ?? []) as string[],
    recommendedSkills: (event.recommended_skills ?? []) as string[],
    recommendedAgents: (event.recommended_agents ?? []) as string[],
    wardRecommendation: {
      action: (wardRec?.action ?? "") as string,
      wardName: (wardRec?.ward_name ?? "") as string,
      subdirectory: wardRec?.subdirectory as string | undefined,
      reason: (wardRec?.reason ?? "") as string,
    },
    executionStrategy: {
      approach: (execStrat?.approach ?? "simple") as string,
      graph: execStrat?.graph as IntentAnalysis["executionStrategy"]["graph"],
      explanation: (execStrat?.explanation ?? "") as string,
    },
  };
  ctx.setIntentAnalysis(ia);
  ctx.setPhase("planning");

  if (ia.wardRecommendation.wardName) {
    ctx.setActiveWard((prev) =>
      prev ?? { name: ia.wardRecommendation.wardName, content: ia.wardRecommendation.reason }
    );
  }

  ctx.setBlocks((prev) => {
    const idx = prev.findIndex(
      (b) => b.type === "intent_analysis" && b.isStreaming,
    );
    if (idx >= 0) {
      const updated = [...prev];
      updated[idx] = {
        ...updated[idx],
        data: { analysis: ia },
        isStreaming: false,
      };
      return updated;
    }
    return [
      ...prev,
      {
        id: crypto.randomUUID(),
        type: "intent_analysis",
        timestamp: now(),
        data: { analysis: ia },
        isStreaming: false,
      },
    ];
  });
}

function flushAndCancelRaf(ctx: EventHandlerCtx): void {
  if (ctx.rafIdRef.current !== null) {
    cancelAnimationFrame(ctx.rafIdRef.current);
    ctx.rafIdRef.current = null;
  }
  ctx.flushTokenBuffer();
}

function handleTurnComplete(event: StreamEvent, ctx: EventHandlerCtx): void {
  flushAndCancelRaf(ctx);

  const finalMessage = event.final_message as string | undefined;
  if (finalMessage) {
    ctx.setBlocks((prev) => {
      const lastIdx = prev.length - 1;
      const last = prev[lastIdx];
      if (last && last.type === "response" && last.isStreaming) {
        return [
          ...prev.slice(0, lastIdx),
          { ...last, data: { ...last.data, content: finalMessage }, isStreaming: false },
        ];
      }
      return [
        ...prev.map((b) => (b.isStreaming ? { ...b, isStreaming: false } : b)),
        {
          id: crypto.randomUUID(),
          type: "response",
          timestamp: now(),
          data: { content: finalMessage, timestamp: now() },
          isStreaming: false,
        },
      ];
    });
  }
}

function handleAgentCompleted(event: StreamEvent, ctx: EventHandlerCtx): void {
  flushAndCancelRaf(ctx);

  const result = event.result as string | undefined;
  if (result) {
    ctx.setBlocks((prev) => {
      const hasResponse = prev.some((b) => b.type === "response");
      if (hasResponse) return prev;
      return [
        ...prev,
        {
          id: crypto.randomUUID(),
          type: "response",
          timestamp: now(),
          data: { content: result, timestamp: now() },
          isStreaming: false,
        },
      ];
    });
  }

  ctx.setStatus("completed");
  ctx.setPhase("completed");
  ctx.stopDurationTimer();
  ctx.setBlocks((prev) => prev.map((b) => (b.isStreaming ? { ...b, isStreaming: false } : b)));
}

function handleErrorEvent(ctx: EventHandlerCtx): void {
  flushAndCancelRaf(ctx);
  ctx.setStatus("error");
  ctx.setPhase("error");
  ctx.stopDurationTimer();
  ctx.setBlocks((prev) => prev.map((b) => (b.isStreaming ? { ...b, isStreaming: false } : b)));
}

function handleSystemOrMessage(event: StreamEvent, ctx: EventHandlerCtx): void {
  const content = (event.content ?? event.message ?? "") as string;
  if (content) {
    ctx.setBlocks((prev) => [
      ...prev,
      {
        id: crypto.randomUUID(),
        type: "response",
        timestamp: now(),
        data: { content, timestamp: now() },
        isStreaming: false,
      },
    ]);
  }
}

function parseIntentAnalysisFromState(ia: Record<string, unknown>): IntentAnalysis {
  const wr = (ia.ward_recommendation ?? {}) as Record<string, unknown>;
  const es = (ia.execution_strategy ?? {}) as Record<string, unknown>;
  return {
    primaryIntent: (ia.primary_intent ?? "") as string,
    hiddenIntents: (ia.hidden_intents ?? []) as string[],
    recommendedSkills: (ia.recommended_skills ?? []) as string[],
    recommendedAgents: (ia.recommended_agents ?? []) as string[],
    wardRecommendation: {
      action: (wr.action ?? "") as string,
      wardName: (wr.ward_name ?? "") as string,
      subdirectory: wr.subdirectory as string | undefined,
      reason: (wr.reason ?? "") as string,
    },
    executionStrategy: {
      approach: (es.approach ?? "simple") as string,
      graph: es.graph as IntentAnalysis["executionStrategy"]["graph"],
      explanation: (es.explanation ?? "") as string,
    },
  };
}

// ============================================================================
// Hook: useMissionControl
// ============================================================================

export function useMissionControl() {
  // -- Core state --
  const [blocks, setBlocks] = useState<NarrativeBlock[]>([]);
  const [sessionTitle, setSessionTitle] = useState("");
  const [status, setStatus] = useState<MissionState["status"]>("idle");
  const [phase, setPhase] = useState<Phase>("idle");
  const [tokenCount, setTokenCount] = useState(0);
  const [modelName, setModelName] = useState("");

  // -- Sidebar data (extracted from blocks) --
  const [subagents, setSubagents] = useState<SubagentInfo[]>([]);
  const [plan, setPlan] = useState<PlanStep[]>([]);
  const [recalledFacts, setRecalledFacts] = useState<RecalledFact[]>([]);
  const [activeWard, setActiveWard] = useState<{ name: string; content: string } | null>(null);
  const [intentAnalysis, setIntentAnalysis] = useState<IntentAnalysis | null>(null);

  // -- Session/conversation IDs --
  // On fresh mount: if there's a stale session ID but no explicit resume flag,
  // clear it so the next invoke creates a new session.
  const [conversationId, setConversationId] = useState<string>(() => {
    const logSessionId = localStorage.getItem("agentzero_log_session_id");
    if (!logSessionId && localStorage.getItem(WEB_SESSION_ID_KEY)) {
      // Stale session from a previous run — clear it for a fresh start
      localStorage.removeItem(WEB_SESSION_ID_KEY);
    }
    return getOrCreateConversationId();
  });
  const [activeSessionId, setActiveSessionId] = useState<string | null>(() => getSessionId());

  // -- Load flag to prevent double-load --
  const hasLoadedSessionRef = useRef(false);

  // -- Timing --
  const startTimeRef = useRef<number | null>(null);
  const [durationMs, setDurationMs] = useState(0);
  const durationIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // -- Token streaming buffer (same pattern as WebChatPanel) --
  const streamingBufferRef = useRef("");
  const rafIdRef = useRef<number | null>(null);

  // -- Sequence-based dedup --
  const lastSeqRef = useRef(0);

  // -- Guard against double submission --
  const isSubmittingRef = useRef(false);

  // -- Fallback title generation --
  const titleFallbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastUserMessageRef = useRef<string>("");

  // -- Map of tool_call_id to block id for correlating ToolResult --
  const toolCallBlockMapRef = useRef<Map<string, string>>(new Map());

  // -- Map of execution_id to agent_id for subagent tool routing --
  const executionAgentMapRef = useRef<Map<string, string>>(new Map());

  // ========================================================================
  // Duration timer
  // ========================================================================

  const startDurationTimer = useCallback(() => {
    startTimeRef.current = Date.now();
    if (durationIntervalRef.current) clearInterval(durationIntervalRef.current);
    durationIntervalRef.current = setInterval(() => {
      if (startTimeRef.current) {
        setDurationMs(Date.now() - startTimeRef.current);
      }
    }, 500);
  }, []);

  const stopDurationTimer = useCallback(() => {
    if (durationIntervalRef.current) {
      clearInterval(durationIntervalRef.current);
      durationIntervalRef.current = null;
    }
    if (startTimeRef.current) {
      setDurationMs(Date.now() - startTimeRef.current);
    }
  }, []);

  // ========================================================================
  // Flush streaming buffer
  // ========================================================================

  const flushTokenBuffer = useCallback(() => {
    const buffered = streamingBufferRef.current;
    if (!buffered) return;
    streamingBufferRef.current = "";
    rafIdRef.current = null;

    setBlocks((prev) => {
      // Find the most recent streaming response block (not necessarily last —
      // intent_analysis or tool blocks may have been inserted in between)
      let targetIdx = -1;
      for (let i = prev.length - 1; i >= 0; i--) {
        if (prev[i].type === "response" && prev[i].isStreaming) {
          targetIdx = i;
          break;
        }
      }

      if (targetIdx >= 0) {
        const updated = [...prev];
        const target = updated[targetIdx];
        updated[targetIdx] = {
          ...target,
          data: { ...target.data, content: (target.data.content as string) + buffered },
        };
        return updated;
      }

      // No streaming response found — create new block
      setPhase((prev) => prev !== "responding" && prev !== "completed" ? "responding" : prev);
      return [
        ...prev,
        {
          id: crypto.randomUUID(),
          type: "response",
          timestamp: now(),
          data: { content: buffered, timestamp: now() },
          isStreaming: true,
        },
      ];
    });
  }, []);

  // ========================================================================
  // Fallback title helper
  // ========================================================================

  const generateFallbackTitle = (message: string): string => {
    const clean = message.replace(/\s+/g, " ").trim();
    if (clean.length <= 50) return clean;
    const truncated = clean.slice(0, 50);
    const lastSpace = truncated.lastIndexOf(" ");
    return (lastSpace > 20 ? truncated.slice(0, lastSpace) : truncated) + "...";
  };

  // ========================================================================
  // Event handler
  // ========================================================================

  const handleStreamEvent = useCallback((event: StreamEvent) => {
    // Sequence-based dedup
    const seq = event.seq as number | undefined;
    if (seq !== undefined && seq > 0) {
      if (seq <= lastSeqRef.current) return;
      lastSeqRef.current = seq;
    }

    const ctx: EventHandlerCtx = {
      setBlocks, setStatus, setPhase, setSessionTitle, setTokenCount, setModelName,
      setSubagents, setPlan, setRecalledFacts, setActiveWard, setIntentAnalysis,
      setActiveSessionId, toolCallBlockMapRef, executionAgentMapRef,
      titleFallbackTimerRef, lastUserMessageRef, streamingBufferRef, rafIdRef,
      flushTokenBuffer, startDurationTimer, stopDurationTimer, generateFallbackTitle,
    };

    switch (event.type) {
      case "invoke_accepted":
        handleInvokeAccepted(event, ctx);
        lastSeqRef.current = 0;
        break;
      case "agent_started":
        handleAgentStarted(event, ctx);
        break;
      case "token":
        handleTokenEvent(event, ctx);
        break;
      case "tool_call":
        handleToolCallEvent(event, ctx);
        break;
      case "tool_result":
        handleToolResultEvent(event, ctx);
        break;
      case "delegation_started":
        handleDelegationStarted(event, ctx);
        break;
      case "delegation_completed":
        handleDelegationCompleted(event, ctx);
        break;
      case "delegation_error":
        handleDelegationError(event, ctx);
        break;
      case "session_title_changed":
        handleSessionTitleChanged(event, ctx);
        break;
      case "ward_changed":
        handleWardChanged(event, ctx);
        break;
      case "intent_analysis_started":
        handleIntentAnalysisStarted(ctx);
        break;
      case "intent_analysis_complete":
        handleIntentAnalysisComplete(event, ctx);
        break;
      case "turn_complete":
        handleTurnComplete(event, ctx);
        break;
      case "agent_completed":
        handleAgentCompleted(event, ctx);
        break;
      case "error":
        handleErrorEvent(ctx);
        break;
      case "system_message":
      case "message":
        handleSystemOrMessage(event, ctx);
        break;
      default:
        break;
    }
  }, [flushTokenBuffer, startDurationTimer, stopDurationTimer]);

  // ========================================================================
  // WebSocket subscription
  // ========================================================================

  useEffect(() => {
    if (!conversationId) return;

    const unsubs: (() => void)[] = [];
    let cancelled = false;

    const setup = async () => {
      const transport = await getTransport();
      if (cancelled) return;

      // Always subscribe to conversationId — this catches early events
      // (invoke_accepted, intent_analysis) before session ID is known
      unsubs.push(
        transport.subscribeConversation(conversationId, {
          onEvent: handleStreamEvent,
          scope: "session",
        })
      );

      // If we already have a session ID (e.g. resuming), also subscribe to it
      if (activeSessionId && activeSessionId !== conversationId) {
        unsubs.push(
          transport.subscribeConversation(activeSessionId, {
            onEvent: handleStreamEvent,
            scope: "session",
          })
        );
      }
    };

    setup();

    return () => {
      cancelled = true;
      unsubs.forEach((u) => u());
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }
      streamingBufferRef.current = "";
      lastSeqRef.current = 0;
    };
  }, [activeSessionId, conversationId, handleStreamEvent]);

  // ========================================================================
  // Cleanup duration interval on unmount
  // ========================================================================

  useEffect(() => {
    return () => {
      if (durationIntervalRef.current) {
        clearInterval(durationIntervalRef.current);
      }
    };
  }, []);

  // ========================================================================
  // Cleanup fallback title timer on unmount
  // ========================================================================

  useEffect(() => {
    return () => {
      if (titleFallbackTimerRef.current) {
        clearTimeout(titleFallbackTimerRef.current);
      }
    };
  }, []);

  // ========================================================================
  // Load existing session messages on mount (for resuming past sessions)
  // ========================================================================

  useEffect(() => {
    const logSessionId = localStorage.getItem("agentzero_log_session_id");
    console.log("[MissionControl] loadSession check:", { logSessionId, activeSessionId, hasLoaded: hasLoadedSessionRef.current });
    if ((!activeSessionId && !logSessionId) || hasLoadedSessionRef.current) return;
    hasLoadedSessionRef.current = true;

    const loadSession = async () => {
      try {
        const transport = await getTransport();
        const sessionToLoad = logSessionId || activeSessionId;
        console.log("[MissionControl] Loading session state:", sessionToLoad);
        if (!sessionToLoad) return;

        const res = await transport.getSessionState(sessionToLoad);
        console.log("[MissionControl] getSessionState result:", res.success, res.error, res.data ? "has data" : "no data");
        if (!res.success || !res.data) {
          console.warn("[MissionControl] getSessionState failed:", res.error);
          return;
        }

        const s = res.data;

        // Session bar
        if (s.session.title) setSessionTitle(s.session.title);
        setTokenCount(s.session.tokenCount);
        setDurationMs(s.session.durationMs);
        if (s.session.model) setModelName(s.session.model);

        // Status
        const statusMap: Record<string, "idle" | "running" | "completed" | "error"> = {
          running: "running", completed: "completed", error: "error", stopped: "completed",
        };
        setStatus(statusMap[s.session.status] ?? "completed");
        if (s.session.status === "running") startDurationTimer();

        // Phase
        setPhase(s.phase as Phase);

        // Sidebar — intent analysis
        if (s.intentAnalysis) {
          setIntentAnalysis(
            parseIntentAnalysisFromState(s.intentAnalysis as Record<string, unknown>)
          );
        }

        // Sidebar — ward, facts, plan
        if (s.ward) setActiveWard(s.ward);
        if (s.recalledFacts?.length > 0) {
          setRecalledFacts(s.recalledFacts.map((f: Record<string, unknown>) => ({
            key: (f.key ?? "") as string,
            content: (f.content ?? f.text ?? "") as string,
            category: (f.category ?? "") as string,
            confidence: (f.confidence ?? f.score) as number | undefined,
          })));
        }
        if (s.plan?.length > 0) {
          setPlan(s.plan.map((p) => ({
            text: p.text,
            status: (p.status === "completed" ? "done" : p.status === "in_progress" ? "active" : "pending") as "done" | "active" | "pending",
          })));
        }

        // Sidebar — subagents
        if (s.subagents?.length > 0) {
          setSubagents(s.subagents.map((sa: SubagentStateData) => ({
            agentId: sa.agentId,
            task: sa.task,
            status: sa.status === "running" ? "active" : sa.status as "active" | "completed" | "error",
          })));
          // Seed execution→agent map for live event routing
          for (const sa of s.subagents) {
            executionAgentMapRef.current.set(sa.executionId, sa.agentId);
          }
        }

        // Center — build minimal blocks (user message + response only)
        const loadedBlocks: NarrativeBlock[] = [];
        if (s.userMessage) {
          loadedBlocks.push({
            id: "user-" + sessionToLoad,
            type: "user",
            timestamp: s.session.startedAt,
            data: { content: s.userMessage, timestamp: s.session.startedAt },
          });
        }
        if (s.response) {
          loadedBlocks.push({
            id: "response-" + sessionToLoad,
            type: "response",
            timestamp: s.session.startedAt,
            data: { content: s.response, timestamp: s.session.startedAt },
            isStreaming: false,
          });
        }
        if (loadedBlocks.length > 0) setBlocks(loadedBlocks);

      } catch (err) {
        console.error("[MissionControl] Failed to load session:", err);
      } finally {
        localStorage.removeItem("agentzero_log_session_id");
      }
    };

    loadSession();
  }, [activeSessionId, startDurationTimer]);

  // ========================================================================
  // Send message
  // ========================================================================

  const sendMessage = useCallback(
    async (text: string, attachments: UploadedFile[] = []) => {
      if (!text.trim() || isSubmittingRef.current) return;
      isSubmittingRef.current = true;

      // Add user message block
      const attachmentNames = attachments.map((a) => a.name);
      lastUserMessageRef.current = text.trim();
      setBlocks((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          type: "user",
          timestamp: now(),
          data: {
            content: text.trim(),
            timestamp: now(),
            attachments: attachmentNames.length > 0 ? attachmentNames : undefined,
          },
        },
      ]);

      setStatus("running");
      setPhase("intent");
      executionAgentMapRef.current.clear();
      startDurationTimer();

      try {
        const transport = await getTransport();
        const currentSessionId = getSessionId() ?? undefined;

        // Build message — if attachments, include their references
        let message = text.trim();
        if (attachments.length > 0) {
          const header = "| File | Type | Size | Path |";
          const sep = "|------|------|------|------|";
          const rows = attachments.map((a) => {
            const sizeStr = a.size < 1024 ? `${a.size} B`
              : a.size < 1024 * 1024 ? `${(a.size / 1024).toFixed(1)} KB`
              : `${(a.size / (1024 * 1024)).toFixed(1)} MB`;
            return `| ${a.name} | ${a.mimeType} | ${sizeStr} | ${a.path} |`;
          }).join("\n");
          message = `${message}\n\n**Attached files:**\n${header}\n${sep}\n${rows}`;
        }

        await transport.executeAgent(ROOT_AGENT_ID, conversationId, message, currentSessionId);
      } catch (error) {
        console.error("[MissionControl] Failed to send message:", error);
        setStatus("error");
        stopDurationTimer();
      } finally {
        isSubmittingRef.current = false;
      }
    },
    [conversationId, startDurationTimer, stopDurationTimer],
  );

  // ========================================================================
  // Stop agent
  // ========================================================================

  const stopAgent = useCallback(async () => {
    try {
      const transport = await getTransport();
      await transport.stopAgent(conversationId);
    } catch (error) {
      console.error("[MissionControl] Failed to stop agent:", error);
    }
  }, [conversationId]);

  // ========================================================================
  // Return state
  // ========================================================================

  // ========================================================================
  // Start new session — clears all state and creates fresh conversation ID
  // ========================================================================

  const startNewSession = useCallback(() => {
    const newConvId = createNewConversationId();
    setConversationId(newConvId);
    setActiveSessionId(null);
    setBlocks([]);
    setSessionTitle("");
    setStatus("idle");
    setPhase("idle");
    setTokenCount(0);
    setModelName("");
    setSubagents([]);
    setPlan([]);
    setRecalledFacts([]);
    setActiveWard(null);
    setIntentAnalysis(null);
    stopDurationTimer();
    setDurationMs(0);
    lastSeqRef.current = 0;
    streamingBufferRef.current = "";
    toolCallBlockMapRef.current.clear();
    executionAgentMapRef.current.clear();
  }, [stopDurationTimer]);

  const state: MissionState = {
    blocks,
    sessionTitle,
    status,
    phase,
    tokenCount,
    durationMs,
    modelName,
    subagents,
    plan,
    recalledFacts,
    activeWard,
    intentAnalysis,
  };

  return { state, sendMessage, stopAgent, startNewSession };
}

// ============================================================================
// Helper: relative time
// ============================================================================

export function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

// ============================================================================
// Helper: switch to an existing session
// ============================================================================

export function switchToSession(sessionId: string, conversationId: string): void {
  // The logs API uses exec- prefixed IDs for sessions, but the backend
  // state DB uses sess- prefixed conversation IDs as the real session ID.
  // Store the conversation_id as the session ID so sendMessage can
  // continue the correct session, and keep the exec- ID for log loading.
  localStorage.setItem(WEB_SESSION_ID_KEY, conversationId);
  localStorage.setItem(WEB_CONV_ID_KEY, conversationId);
  // Store the log session ID separately for loading session details
  localStorage.setItem("agentzero_log_session_id", sessionId);
  window.location.reload();
}

// ============================================================================
// Hook: useRecentSessions
// ============================================================================

export function useRecentSessions() {
  const [sessions, setSessions] = useState<LogSession[]>([]);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const transport = await getTransport();
        const res = await transport.listLogSessions({ limit: 5, root_only: true });
        if (cancelled || !res.success || !res.data) return;
        setSessions(res.data);
      } catch (err) {
        console.error("[useRecentSessions] Failed to load sessions:", err);
      }
    };
    load();
    return () => { cancelled = true; };
  }, []);

  return { sessions };
}
