import type { ConversationEvent } from "@/services/transport/types";
import type { PillEvent } from "../shared/statusPill";
import type { QuickChatAction } from "./reducer";

// ---------------------------------------------------------------------------
// mapGatewayEventToQuickChatAction
// ---------------------------------------------------------------------------

function mapTokenEvent(ev: Record<string, unknown>): QuickChatAction | null {
  const content = ev["content"];
  if (typeof content !== "string" || content.length === 0) return null;
  return { type: "TOKEN", text: content };
}

function mapRespondEvent(ev: Record<string, unknown>): QuickChatAction | null {
  const content = ev["content"];
  if (typeof content !== "string") return null;
  return { type: "RESPOND", text: content };
}

function mapWardChangedEvent(ev: Record<string, unknown>): QuickChatAction | null {
  const ward = ev["ward"] as Record<string, unknown> | undefined;
  const name = ward?.["name"];
  if (!name) return null;
  return { type: "WARD_CHANGED", wardName: name as string };
}

function mapSessionInitializedEvent(ev: Record<string, unknown>): QuickChatAction | null {
  const sid = ev["session_id"];
  if (!sid) return null;
  return { type: "SESSION_BOUND", sessionId: sid as string };
}

function mapDelegateToolCall(args: Record<string, unknown>): QuickChatAction {
  const agentId = (args["agent_id"] ?? args["agentId"] ?? "subagent") as string;
  return {
    type: "ADD_CHIP",
    chip: { id: crypto.randomUUID(), kind: "delegate", label: `→ ${agentId}` },
  };
}

function mapLoadSkillToolCall(args: Record<string, unknown>): QuickChatAction {
  const skill = (args["skill"] ?? "skill") as string;
  return {
    type: "ADD_CHIP",
    chip: { id: crypto.randomUUID(), kind: "skill", label: `loaded ${skill}` },
  };
}

function mapMemoryToolCall(args: Record<string, unknown>): QuickChatAction | null {
  if (args["action"] !== "recall" && args["action"] !== "get_fact") return null;
  return {
    type: "ADD_CHIP",
    chip: { id: crypto.randomUUID(), kind: "recall", label: "recalled" },
  };
}

function mapToolCallEvent(ev: Record<string, unknown>): QuickChatAction | null {
  const tool = ev["tool"] as string;
  const args = (ev["args"] ?? {}) as Record<string, unknown>;
  switch (tool) {
    case "delegate_to_agent": return mapDelegateToolCall(args);
    case "load_skill":        return mapLoadSkillToolCall(args);
    case "memory":            return mapMemoryToolCall(args);
    default:                  return null;
  }
}

export function mapGatewayEventToQuickChatAction(ev: ConversationEvent): QuickChatAction | null {
  const raw = ev as unknown as Record<string, unknown>;
  const type = raw["type"] as string;
  switch (type) {
    case "token":               return mapTokenEvent(raw);
    case "respond":             return mapRespondEvent(raw);
    case "ward_changed":        return mapWardChangedEvent(raw);
    case "session_initialized": return mapSessionInitializedEvent(raw);
    case "agent_started":       return { type: "AGENT_STARTED", agentId: (raw["agent_id"] ?? "") as string };
    case "turn_complete":       return { type: "TURN_COMPLETE" };
    case "tool_call":           return mapToolCallEvent(raw);
    case "error":               return { type: "ERROR", message: (raw["message"] ?? "error") as string };
    default:                    return null;
  }
}

// ---------------------------------------------------------------------------
// mapGatewayEventToPillEvent
// ---------------------------------------------------------------------------

function mapPillThinking(ev: Record<string, unknown>): PillEvent | null {
  const content = ev["content"];
  if (typeof content !== "string" || content.length === 0) return null;
  return { kind: "thinking", content };
}

function mapPillToolCall(ev: Record<string, unknown>): PillEvent | null {
  const tool = ev["tool"];
  if (typeof tool !== "string") return null;
  return { kind: "tool_call", tool, args: (ev["args"] ?? {}) as Record<string, unknown> };
}

function mapPillAgentCompleted(ev: Record<string, unknown>): PillEvent {
  return {
    kind: "agent_completed",
    agent_id: (ev["agent_id"] ?? "") as string,
    is_final: Boolean(ev["last"]) || Boolean(ev["is_final"]),
  };
}

export function mapGatewayEventToPillEvent(ev: ConversationEvent): PillEvent | null {
  const raw = ev as unknown as Record<string, unknown>;
  const type = raw["type"] as string;
  switch (type) {
    case "agent_started":   return { kind: "agent_started", agent_id: (raw["agent_id"] ?? "") as string };
    case "agent_completed": return mapPillAgentCompleted(raw);
    case "thinking":        return mapPillThinking(raw);
    case "tool_call":       return mapPillToolCall(raw);
    case "respond":         return { kind: "respond" };
    default:                return null;
  }
}
