import { describe, it, expect } from "vitest";
import { mapGatewayEventToQuickChatAction, mapGatewayEventToPillEvent } from "./event-map";

describe("mapGatewayEventToQuickChatAction", () => {
  it("maps Token to TOKEN action", () => {
    expect(mapGatewayEventToQuickChatAction({ type: "token", content: "hi" } as any))
      .toEqual({ type: "TOKEN", text: "hi" });
  });
  it("maps Respond to RESPOND action", () => {
    expect(mapGatewayEventToQuickChatAction({ type: "respond", content: "done" } as any))
      .toEqual({ type: "RESPOND", text: "done" });
  });
  it("maps WardChanged to WARD_CHANGED only when name present", () => {
    expect(mapGatewayEventToQuickChatAction({ type: "ward_changed", ward: { name: "x" } } as any))
      .toEqual({ type: "WARD_CHANGED", wardName: "x" });
    expect(mapGatewayEventToQuickChatAction({ type: "ward_changed" } as any)).toBeNull();
  });
  it("maps SessionInitialized → SESSION_BOUND", () => {
    expect(mapGatewayEventToQuickChatAction({ type: "session_initialized", session_id: "sess-1" } as any))
      .toEqual({ type: "SESSION_BOUND", sessionId: "sess-1" });
  });
  it("maps tool_call delegate_to_agent to ADD_CHIP", () => {
    const a = mapGatewayEventToQuickChatAction({
      type: "tool_call", tool: "delegate_to_agent", args: { agent_id: "writer-agent" },
    } as any);
    expect(a?.type).toBe("ADD_CHIP");
    expect((a as any).chip.kind).toBe("delegate");
  });
  it("returns null for unmapped events", () => {
    expect(mapGatewayEventToQuickChatAction({ type: "iterations_extended" } as any)).toBeNull();
  });
});

describe("mapGatewayEventToPillEvent", () => {
  it("maps agent_started", () => {
    expect(mapGatewayEventToPillEvent({ type: "agent_started", agent_id: "quick-chat" } as any))
      .toEqual({ kind: "agent_started", agent_id: "quick-chat" });
  });
  it("maps thinking", () => {
    expect(mapGatewayEventToPillEvent({ type: "thinking", content: "…" } as any))
      .toEqual({ kind: "thinking", content: "…" });
  });
  it("maps tool_call", () => {
    expect(mapGatewayEventToPillEvent({ type: "tool_call", tool: "write_file", args: { path: "a.py" } } as any))
      .toEqual({ kind: "tool_call", tool: "write_file", args: { path: "a.py" } });
  });
  it("maps agent_completed with is_final inferred from last=true flag", () => {
    expect(mapGatewayEventToPillEvent({ type: "agent_completed", agent_id: "quick-chat", last: true } as any))
      .toEqual({ kind: "agent_completed", agent_id: "quick-chat", is_final: true });
  });
});
