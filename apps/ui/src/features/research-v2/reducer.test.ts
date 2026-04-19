import { describe, it, expect } from "vitest";
import { reduceResearch, type ResearchAction } from "./reducer";
import { EMPTY_RESEARCH_STATE } from "./types";

describe("reduceResearch", () => {
  it("APPEND_USER adds a user message and flips status to running", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "APPEND_USER",
      message: { id: "m1", role: "user", content: "go", timestamp: 1 },
    });
    expect(s.messages).toHaveLength(1);
    expect(s.status).toBe("running");
  });

  it("WARD_CHANGED sets sticky ward", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "WARD_CHANGED", wardId: "stock-analysis", wardName: "Stock Analysis",
    });
    expect(s.wardId).toBe("stock-analysis");
    expect(s.wardName).toBe("Stock Analysis");
  });

  it("AGENT_STARTED without wardId does NOT clear sticky ward", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "WARD_CHANGED", wardId: "w1", wardName: "W1",
    });
    s = reduceResearch(s, {
      type: "AGENT_STARTED",
      turnId: "exec-1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 2,
    });
    expect(s.wardId).toBe("w1");
    expect(s.turns).toHaveLength(1);
    expect(s.turns[0].wardId).toBe("w1"); // turn inherits sticky ward
  });

  it("THINKING_DELTA appends to turn timeline", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    s = reduceResearch(s, {
      type: "THINKING_DELTA", turnId: "t1", entry: { id: "e1", at: 2, kind: "thinking", text: "thinking…" },
    });
    expect(s.turns[0].timeline).toHaveLength(1);
  });

  it("TOOL_CALL appends to turn timeline and carries tool metadata", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    s = reduceResearch(s, {
      type: "TOOL_CALL", turnId: "t1", entry: {
        id: "e1", at: 2, kind: "tool_call", text: "write_file", toolName: "write_file", toolArgsPreview: "path=a.py",
      },
    });
    expect(s.turns[0].timeline[0].toolName).toBe("write_file");
  });

  it("TOKEN streams into the turn's respondStreaming buffer", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    s = reduceResearch(s, { type: "TOKEN", turnId: "t1", text: "par" });
    s = reduceResearch(s, { type: "TOKEN", turnId: "t1", text: "tial" });
    expect(s.turns[0].respondStreaming).toBe("partial");
  });

  it("RESPOND sets final respond and clears the streaming buffer", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    s = reduceResearch(s, { type: "TOKEN", turnId: "t1", text: "streaming" });
    s = reduceResearch(s, { type: "RESPOND", turnId: "t1", text: "final" });
    expect(s.turns[0].respond).toBe("final");
    expect(s.turns[0].respondStreaming).toBe("");
  });

  it("RESPOND without a prior AGENT_STARTED creates an orphan turn and persists the respond", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, { type: "RESPOND", turnId: "t1", text: "final" });
    expect(s.turns).toHaveLength(1);
    expect(s.turns[0].respond).toBe("final");
  });

  it("AGENT_COMPLETED flips turn status to completed", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    // A turn with a respond has meaningful content — should complete cleanly.
    s = reduceResearch(s, { type: "RESPOND", turnId: "t1", text: "done" });
    s = reduceResearch(s, { type: "AGENT_COMPLETED", turnId: "t1", completedAt: 10 });
    expect(s.turns[0].status).toBe("completed");
  });

  it("AGENT_COMPLETED on an empty turn infers an error (silent-crash workaround)", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    s = reduceResearch(s, { type: "AGENT_COMPLETED", turnId: "t1", completedAt: 2 });
    expect(s.turns[0].status).toBe("error");
    expect(s.turns[0].errorMessage).toContain("no output");
  });

  it("AGENT_STOPPED does NOT get error-inferred even with empty turn", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    s = reduceResearch(s, { type: "AGENT_STOPPED", turnId: "t1", completedAt: 2 });
    expect(s.turns[0].status).toBe("stopped");
    expect(s.turns[0].errorMessage).toBeNull();
  });

  it("TOGGLE_THINKING flips the per-turn expanded flag", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    expect(s.turns[0].thinkingExpanded).toBe(false);
    s = reduceResearch(s, { type: "TOGGLE_THINKING", turnId: "t1" });
    expect(s.turns[0].thinkingExpanded).toBe(true);
  });

  it("HYDRATE seeds state from snapshot with artifacts", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "HYDRATE",
      sessionId: "sess-1",
      conversationId: null,
      title: "Research X",
      status: "complete",
      wardId: "w1",
      wardName: "W1",
      messages: [{ id: "m1", role: "user", content: "go", timestamp: 1 }],
      turns: [],
      artifacts: [{ id: "art-1", fileName: "plan.md", fileType: "md" }],
    });
    expect(s.sessionId).toBe("sess-1");
    expect(s.wardId).toBe("w1");
    expect(s.title).toBe("Research X");
    expect(s.artifacts).toHaveLength(1);
  });

  it("INTENT_ANALYSIS_STARTED flips the flag", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, { type: "INTENT_ANALYSIS_STARTED" });
    expect(s.intentAnalyzing).toBe(true);
  });

  it("INTENT_ANALYSIS_COMPLETE clears flag and stores classification", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, { type: "INTENT_ANALYSIS_STARTED" });
    s = reduceResearch(s, { type: "INTENT_ANALYSIS_COMPLETE", classification: "research" });
    expect(s.intentAnalyzing).toBe(false);
    expect(s.intentClassification).toBe("research");
  });

  it("SESSION_COMPLETE transitions top-level status", () => {
    const s = reduceResearch(
      { ...EMPTY_RESEARCH_STATE, status: "running" },
      { type: "SESSION_COMPLETE" }
    );
    expect(s.status).toBe("complete");
  });

  it("RESET clears state fully", () => {
    const seeded = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "HYDRATE", sessionId: "sess-x", conversationId: "c1", title: "t", status: "idle",
      wardId: "w", wardName: "W", messages: [], turns: [], artifacts: [],
    });
    expect(seeded.sessionId).toBe("sess-x");
    const cleared = reduceResearch(seeded, { type: "RESET" });
    expect(cleared).toEqual(EMPTY_RESEARCH_STATE);
  });

  it("SET_ARTIFACTS replaces the artifact list", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "SET_ARTIFACTS",
      artifacts: [{ id: "a1", fileName: "x.md", fileType: "md" }],
    });
    expect(s.artifacts).toHaveLength(1);
    expect(s.artifacts[0].fileName).toBe("x.md");
  });

  it("TITLE_CHANGED updates the session title", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, { type: "TITLE_CHANGED", title: "New Title" });
    expect(s.title).toBe("New Title");
  });

  it("ERROR flips top-level status to error", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, { type: "ERROR", message: "network" });
    expect(s.status).toBe("error");
  });

  it("PLAN_UPDATE stores the plan path", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, { type: "PLAN_UPDATE", planPath: "/p.md" });
    expect(s.planPath).toBe("/p.md");
  });
});
