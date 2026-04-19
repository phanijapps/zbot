import { describe, it, expect } from "vitest";
import { reducePillState } from "./use-status-pill-aggregator";
import { EMPTY_PILL } from "./types";

describe("reducePillState", () => {
  it("starts hidden", () => {
    expect(reducePillState(EMPTY_PILL, { kind: "idle" })).toEqual(EMPTY_PILL);
  });

  it("shows starting on AgentStarted with no prior events", () => {
    const s = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    expect(s.visible).toBe(true);
    expect(s.starting).toBe(true);
    expect(s.category).toBe("neutral");
  });

  it("updates narration on Thinking", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const s2 = reducePillState(s1, { kind: "thinking", content: "Let me recall fundamentals…" });
    expect(s2.narration).toBe("Let me recall fundamentals…");
    expect(s2.starting).toBe(false);
    expect(s2.swapCounter).toBeGreaterThan(s1.swapCounter);
  });

  it("truncates narration to 80 chars", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const long = "x".repeat(120);
    const s2 = reducePillState(s1, { kind: "thinking", content: long });
    expect(s2.narration.length).toBeLessThanOrEqual(80);
  });

  it("updates suffix and color on ToolCall", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const s2 = reducePillState(s1, { kind: "tool_call", tool: "write_file", args: { path: "a.py" } });
    expect(s2.suffix).toBe("· a.py");
    expect(s2.category).toBe("write");
  });

  it("hides on AgentCompleted when it is the last active agent", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const s2 = reducePillState(s1, { kind: "agent_completed", agent_id: "root", is_final: true });
    expect(s2.visible).toBe(false);
  });

  it("stays visible on AgentCompleted with continuation pending", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const s2 = reducePillState(s1, { kind: "agent_completed", agent_id: "root", is_final: false });
    expect(s2.visible).toBe(true);
  });

  it("resets via reset event (new session)", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const s2 = reducePillState(s1, { kind: "reset" });
    expect(s2).toEqual(EMPTY_PILL);
  });
});
