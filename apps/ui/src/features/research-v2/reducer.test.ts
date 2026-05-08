import { describe, it, expect } from "vitest";
import { reduceResearch } from "./reducer";
import { EMPTY_RESEARCH_STATE, type ResearchSessionState } from "./types";

// -----------------------------------------------------------------------------
// Fixture helpers
//
// Most reducer tests in the new multi-turn shape follow the same opening
// sequence: APPEND_USER (opens a SessionTurn) → AGENT_STARTED with
// parentExecutionId=null (stamps rootExecutionId). After that, root-keyed
// events (TOKEN/RESPOND/AGENT_*/timeline) target `state.turns[last]` and
// subagent-keyed events target a located AgentTurn inside that turn's
// `subagents[]`.
// -----------------------------------------------------------------------------

const ROOT_EXEC = "exec-root";
const SUB_EXEC = "exec-sub-1";

function withRootTurn(rootExecId = ROOT_EXEC): ResearchSessionState {
  let s = reduceResearch(EMPTY_RESEARCH_STATE, {
    type: "APPEND_USER",
    message: { id: "u1", content: "go", createdAt: "2026-04-19T00:00:00.000Z" },
  });
  s = reduceResearch(s, {
    type: "AGENT_STARTED",
    turnId: rootExecId,
    agentId: "root",
    parentExecutionId: null,
    wardId: null,
    startedAt: 1,
  });
  return s;
}

function withSubagent(
  state: ResearchSessionState,
  args: {
    turnId: string;
    agentId?: string;
    parentExecutionId?: string;
    wardId?: string | null;
    startedAt?: number;
    request?: string | null;
  },
): ResearchSessionState {
  return reduceResearch(state, {
    type: "AGENT_STARTED",
    turnId: args.turnId,
    agentId: args.agentId ?? "planner",
    parentExecutionId: args.parentExecutionId ?? ROOT_EXEC,
    wardId: args.wardId ?? null,
    startedAt: args.startedAt ?? 2,
    request: args.request ?? null,
  });
}

describe("reduceResearch", () => {
  it("APPEND_USER opens a SessionTurn and flips status to running", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "APPEND_USER",
      message: { id: "m1", content: "go", createdAt: "2026-04-19T00:00:00.000Z" },
    });
    expect(s.turns).toHaveLength(1);
    expect(s.turns[0].userMessage.content).toBe("go");
    expect(s.turns[0].status).toBe("running");
    expect(s.status).toBe("running");
  });

  it("WARD_CHANGED sets sticky ward", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "WARD_CHANGED",
      wardId: "stock-analysis",
      wardName: "Stock Analysis",
    });
    expect(s.wardId).toBe("stock-analysis");
    expect(s.wardName).toBe("Stock Analysis");
  });

  it("AGENT_STARTED root stamps rootExecutionId without opening an extra SessionTurn", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "APPEND_USER",
      message: { id: "u1", content: "go", createdAt: "2026-04-19T00:00:00.000Z" },
    });
    s = reduceResearch(s, {
      type: "AGENT_STARTED",
      turnId: ROOT_EXEC,
      agentId: "root",
      parentExecutionId: null,
      wardId: null,
      startedAt: 2,
    });
    expect(s.rootExecutionId).toBe(ROOT_EXEC);
    expect(s.turns).toHaveLength(1);
  });

  it("AGENT_STARTED subagent without wardId inherits sticky ward onto the subagent", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "WARD_CHANGED",
      wardId: "w1",
      wardName: "W1",
    });
    s = reduceResearch(s, {
      type: "APPEND_USER",
      message: { id: "u1", content: "go", createdAt: "2026-04-19T00:00:00.000Z" },
    });
    s = reduceResearch(s, {
      type: "AGENT_STARTED",
      turnId: ROOT_EXEC,
      agentId: "root",
      parentExecutionId: null,
      wardId: null,
      startedAt: 2,
    });
    s = withSubagent(s, { turnId: SUB_EXEC });
    expect(s.wardId).toBe("w1");
    expect(s.turns[0].subagents).toHaveLength(1);
    expect(s.turns[0].subagents[0].wardId).toBe("w1"); // subagent inherits sticky
  });

  it("THINKING_DELTA on the root execution appends to the SessionTurn timeline", () => {
    let s = withRootTurn();
    s = reduceResearch(s, {
      type: "THINKING_DELTA",
      turnId: ROOT_EXEC,
      entry: { id: "e1", at: 2, kind: "thinking", text: "thinking…" },
    });
    expect(s.turns[0].timeline).toHaveLength(1);
  });

  it("TOOL_CALL on a subagent appends to that subagent's timeline with metadata", () => {
    let s = withRootTurn();
    s = withSubagent(s, { turnId: SUB_EXEC });
    s = reduceResearch(s, {
      type: "TOOL_CALL",
      turnId: SUB_EXEC,
      entry: {
        id: "e1",
        at: 2,
        kind: "tool_call",
        text: "write_file",
        toolName: "write_file",
        toolArgsPreview: "path=a.py",
      },
    });
    const sub = s.turns[0].subagents[0];
    expect(sub.timeline).toHaveLength(1);
    expect(sub.timeline[0].toolName).toBe("write_file");
  });

  it("TOKEN on the root execution streams into assistantStreaming", () => {
    let s = withRootTurn();
    s = reduceResearch(s, { type: "TOKEN", turnId: ROOT_EXEC, text: "par" });
    s = reduceResearch(s, { type: "TOKEN", turnId: ROOT_EXEC, text: "tial" });
    expect(s.turns[0].assistantStreaming).toBe("partial");
  });

  it("TOKEN on a subagent streams into that subagent's respondStreaming", () => {
    let s = withRootTurn();
    s = withSubagent(s, { turnId: SUB_EXEC });
    s = reduceResearch(s, { type: "TOKEN", turnId: SUB_EXEC, text: "sub-" });
    s = reduceResearch(s, { type: "TOKEN", turnId: SUB_EXEC, text: "stream" });
    expect(s.turns[0].subagents[0].respondStreaming).toBe("sub-stream");
    // Root's own buffer should remain empty.
    expect(s.turns[0].assistantStreaming).toBe("");
  });

  it("RESPOND on the root execution sets assistantText and clears the streaming buffer", () => {
    let s = withRootTurn();
    s = reduceResearch(s, { type: "TOKEN", turnId: ROOT_EXEC, text: "streaming" });
    s = reduceResearch(s, { type: "RESPOND", turnId: ROOT_EXEC, text: "final" });
    expect(s.turns[0].assistantText).toBe("final");
    expect(s.turns[0].assistantStreaming).toBe("");
  });

  it("RESPOND on a subagent sets the subagent's respond and clears its streaming buffer", () => {
    let s = withRootTurn();
    s = withSubagent(s, { turnId: SUB_EXEC });
    s = reduceResearch(s, { type: "TOKEN", turnId: SUB_EXEC, text: "draft" });
    s = reduceResearch(s, { type: "RESPOND", turnId: SUB_EXEC, text: "final-sub" });
    const sub = s.turns[0].subagents[0];
    expect(sub.respond).toBe("final-sub");
    expect(sub.respondStreaming).toBe("");
  });

  it("AGENT_COMPLETED on the root flips the SessionTurn to completed when there's content", () => {
    let s = withRootTurn();
    s = reduceResearch(s, { type: "RESPOND", turnId: ROOT_EXEC, text: "done" });
    s = reduceResearch(s, { type: "AGENT_COMPLETED", turnId: ROOT_EXEC, completedAt: 10 });
    expect(s.turns[0].status).toBe("completed");
    expect(s.turns[0].endedAt).not.toBeNull();
  });

  it("AGENT_COMPLETED on an empty root SessionTurn infers an error (silent crash)", () => {
    let s = withRootTurn();
    s = reduceResearch(s, { type: "AGENT_COMPLETED", turnId: ROOT_EXEC, completedAt: 2 });
    expect(s.turns[0].status).toBe("error");
    // Silent-crash workaround uses the assistantText slot as the error display.
    expect(s.turns[0].assistantText ?? "").toMatch(/no output/i);
  });

  it("AGENT_COMPLETED on an empty subagent infers an error onto errorMessage", () => {
    let s = withRootTurn();
    s = withSubagent(s, { turnId: SUB_EXEC });
    s = reduceResearch(s, { type: "AGENT_COMPLETED", turnId: SUB_EXEC, completedAt: 2 });
    const sub = s.turns[0].subagents[0];
    expect(sub.status).toBe("error");
    expect(sub.errorMessage).toContain("no output");
  });

  it("AGENT_STOPPED on the root marks the SessionTurn stopped without inferring error", () => {
    let s = withRootTurn();
    s = reduceResearch(s, { type: "AGENT_STOPPED", turnId: ROOT_EXEC, completedAt: 2 });
    expect(s.turns[0].status).toBe("stopped");
  });

  it("AGENT_STOPPED on a subagent marks it stopped and leaves errorMessage null", () => {
    let s = withRootTurn();
    s = withSubagent(s, { turnId: SUB_EXEC });
    s = reduceResearch(s, { type: "AGENT_STOPPED", turnId: SUB_EXEC, completedAt: 2 });
    const sub = s.turns[0].subagents[0];
    expect(sub.status).toBe("stopped");
    expect(sub.errorMessage).toBeNull();
  });

  it("TOGGLE_THINKING flips the per-subagent expanded flag (root has no chevron)", () => {
    let s = withRootTurn();
    s = withSubagent(s, { turnId: SUB_EXEC });
    expect(s.turns[0].subagents[0].thinkingExpanded).toBe(false);
    s = reduceResearch(s, { type: "TOGGLE_THINKING", turnId: SUB_EXEC });
    expect(s.turns[0].subagents[0].thinkingExpanded).toBe(true);
  });

  it("HYDRATE seeds state from snapshot — turns + rootExecutionId + artifacts", () => {
    const turn = {
      id: "turn-u1",
      index: 0,
      userMessage: { id: "u1", content: "go", createdAt: "2026-04-19T00:00:00.000Z" },
      subagents: [],
      assistantText: "answer",
      assistantStreaming: "",
      timeline: [],
      status: "completed" as const,
      startedAt: "2026-04-19T00:00:00.000Z",
      endedAt: "2026-04-19T00:01:00.000Z",
      durationMs: 60_000,
    };
    const s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "HYDRATE",
      sessionId: "sess-1",
      conversationId: null,
      title: "Research X",
      status: "complete",
      wardId: "w1",
      wardName: "W1",
      rootExecutionId: ROOT_EXEC,
      turns: [turn],
      artifacts: [{ id: "art-1", fileName: "plan.md", fileType: "md" }],
    });
    expect(s.sessionId).toBe("sess-1");
    expect(s.wardId).toBe("w1");
    expect(s.title).toBe("Research X");
    expect(s.rootExecutionId).toBe(ROOT_EXEC);
    expect(s.turns).toHaveLength(1);
    expect(s.turns[0].userMessage.content).toBe("go");
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
      { type: "SESSION_COMPLETE" },
    );
    expect(s.status).toBe("complete");
  });

  it("RESET clears state fully", () => {
    const seeded = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "HYDRATE",
      sessionId: "sess-x",
      conversationId: "c1",
      title: "t",
      status: "idle",
      wardId: "w",
      wardName: "W",
      rootExecutionId: null,
      turns: [],
      artifacts: [],
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

  it("duplicate AGENT_STARTED root is idempotent — does not overwrite rootExecutionId", () => {
    let s = withRootTurn();
    expect(s.rootExecutionId).toBe(ROOT_EXEC);
    s = reduceResearch(s, {
      type: "AGENT_STARTED",
      turnId: "exec-other",
      agentId: "planner",
      parentExecutionId: null,
      wardId: "w-other",
      startedAt: 999,
    });
    // Sticky: first root execution wins.
    expect(s.rootExecutionId).toBe(ROOT_EXEC);
  });

  it("duplicate AGENT_STARTED subagent is idempotent — does not overwrite the existing subagent", () => {
    let s = withRootTurn();
    s = withSubagent(s, { turnId: SUB_EXEC, agentId: "planner", startedAt: 2 });
    s = withSubagent(s, { turnId: SUB_EXEC, agentId: "writer", startedAt: 999 });
    expect(s.turns[0].subagents).toHaveLength(1);
    expect(s.turns[0].subagents[0].agentId).toBe("planner");
    expect(s.turns[0].subagents[0].startedAt).toBe(2);
  });

  it("WARD_CHANGED after a subagent exists updates state ward but NOT the historical subagent's wardId", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "WARD_CHANGED",
      wardId: "w1",
      wardName: "W1",
    });
    s = reduceResearch(s, {
      type: "APPEND_USER",
      message: { id: "u1", content: "go", createdAt: "2026-04-19T00:00:00.000Z" },
    });
    s = reduceResearch(s, {
      type: "AGENT_STARTED",
      turnId: ROOT_EXEC,
      agentId: "root",
      parentExecutionId: null,
      wardId: null,
      startedAt: 1,
    });
    s = withSubagent(s, { turnId: SUB_EXEC });
    s = reduceResearch(s, {
      type: "WARD_CHANGED",
      wardId: "w2",
      wardName: "W2",
    });
    expect(s.wardId).toBe("w2");
    expect(s.wardName).toBe("W2");
    expect(s.turns[0].subagents[0].wardId).toBe("w1"); // historical — not retroactive
  });

  it("APPEND_USER mid-stream promotes the prior turn's streaming buffer to assistantText", () => {
    // Regression: a second user message arriving while the first turn is
    // still streaming would otherwise leave the prior turn's cursor
    // blinking forever. The reducer promotes the in-flight buffer into
    // assistantText and marks the prior turn completed before opening the
    // new one.
    let s = withRootTurn();
    s = reduceResearch(s, { type: "TOKEN", turnId: ROOT_EXEC, text: "in flight" });
    s = reduceResearch(s, {
      type: "APPEND_USER",
      message: { id: "u2", content: "next", createdAt: "2026-04-19T00:00:30.000Z" },
    });
    expect(s.turns).toHaveLength(2);
    expect(s.turns[0].status).toBe("completed");
    expect(s.turns[0].assistantText).toBe("in flight");
    expect(s.turns[0].assistantStreaming).toBe("");
    expect(s.turns[1].userMessage.content).toBe("next");
    expect(s.turns[1].status).toBe("running");
  });
});
