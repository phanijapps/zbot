import { describe, it, expect } from "vitest";
import type { AgentTurn } from "./types";
import { rootTurns, childrenOf } from "./turn-tree";

function makeTurn(overrides: Partial<AgentTurn>): AgentTurn {
  return {
    id: "t",
    agentId: "root",
    parentExecutionId: null,
    startedAt: 0,
    completedAt: null,
    status: "running",
    wardId: null,
    timeline: [],
    tokenCount: 0,
    respond: null,
    respondStreaming: "",
    thinkingExpanded: false,
    errorMessage: null, request: null,
    ...overrides,
  };
}

describe("rootTurns", () => {
  it("returns only turns with parentExecutionId === null, sorted ascending by startedAt", () => {
    const r1 = makeTurn({ id: "r1", parentExecutionId: null, startedAt: 200 });
    const r2 = makeTurn({ id: "r2", parentExecutionId: null, startedAt: 100 });
    const c1 = makeTurn({ id: "c1", parentExecutionId: "r1", startedAt: 50 });
    const result = rootTurns([r1, c1, r2]);
    expect(result.map((t) => t.id)).toEqual(["r2", "r1"]);
  });

  it("returns [] when given an empty array", () => {
    expect(rootTurns([])).toEqual([]);
  });
});

describe("childrenOf", () => {
  it("finds direct children only (not grandchildren)", () => {
    const root = makeTurn({ id: "root", parentExecutionId: null, startedAt: 0 });
    const child = makeTurn({ id: "child", parentExecutionId: "root", startedAt: 100 });
    const grandchild = makeTurn({ id: "grand", parentExecutionId: "child", startedAt: 200 });
    const result = childrenOf(root, [root, child, grandchild]);
    expect(result.map((t) => t.id)).toEqual(["child"]);
  });

  it("sorts children by startedAt ascending", () => {
    const root = makeTurn({ id: "root", parentExecutionId: null, startedAt: 0 });
    const a = makeTurn({ id: "a", parentExecutionId: "root", startedAt: 300 });
    const b = makeTurn({ id: "b", parentExecutionId: "root", startedAt: 100 });
    const c = makeTurn({ id: "c", parentExecutionId: "root", startedAt: 200 });
    const result = childrenOf(root, [root, a, b, c]);
    expect(result.map((t) => t.id)).toEqual(["b", "c", "a"]);
  });

  it("returns [] when the turn has no children", () => {
    const root = makeTurn({ id: "root", parentExecutionId: null });
    const sibling = makeTurn({ id: "sib", parentExecutionId: null });
    expect(childrenOf(root, [root, sibling])).toEqual([]);
  });
});
