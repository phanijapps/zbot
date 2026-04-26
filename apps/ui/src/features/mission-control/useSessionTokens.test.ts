// ============================================================================
// useSessionTokens — pure buildIndex + sumExecutionTokensByAgent helpers
// ============================================================================

import { describe, it, expect } from "vitest";
import {
  buildIndex,
  sumExecutionTokensByAgent,
  normalizeV2Status,
  applyV2Status,
} from "./useSessionTokens";
import type { LogSession } from "@/services/transport/types";

describe("buildIndex", () => {
  it("returns empty maps for an empty input", () => {
    const idx = buildIndex([]);
    expect(idx.byRootExecId.size).toBe(0);
    expect(idx.executionsByRootExecId.size).toBe(0);
  });

  it("keys session totals by the ROOT execution id (not the parent session id)", () => {
    const idx = buildIndex([
      {
        id: "sess-A",
        total_tokens_in: 24174,
        total_tokens_out: 247,
        executions: [
          { id: "exec-root-A", agent_id: "root", tokens_in: 24174, tokens_out: 247, delegation_type: "root" },
          { id: "exec-child-A", agent_id: "research-agent", tokens_in: 0, tokens_out: 0, delegation_type: "sequential" },
        ],
      },
    ]);
    expect(idx.byRootExecId.get("exec-root-A")).toEqual({ in: 24174, out: 247, total: 24421 });
    expect(idx.byRootExecId.get("sess-A")).toBeUndefined();
  });

  it("falls back to executions[0] when no execution has delegation_type='root'", () => {
    const idx = buildIndex([
      {
        id: "sess-A",
        total_tokens_in: 100,
        total_tokens_out: 50,
        executions: [
          { id: "exec-first", agent_id: "root", tokens_in: 100, tokens_out: 50 },
        ],
      },
    ]);
    expect(idx.byRootExecId.get("exec-first")).toEqual({ in: 100, out: 50, total: 150 });
  });

  it("skips sessions that have no executions (defensive)", () => {
    const idx = buildIndex([
      { id: "sess-empty", total_tokens_in: 0, total_tokens_out: 0, executions: [] },
    ]);
    expect(idx.byRootExecId.size).toBe(0);
  });

  it("populates per-execution entries keyed by root exec id", () => {
    const idx = buildIndex([
      {
        id: "sess-X",
        total_tokens_in: 9000,
        total_tokens_out: 1000,
        executions: [
          { id: "root-X", agent_id: "root", tokens_in: 1000, tokens_out: 100, delegation_type: "root" },
          { id: "child-1", agent_id: "researcher", tokens_in: 4000, tokens_out: 400 },
          { id: "child-2", agent_id: "coder", tokens_in: 4000, tokens_out: 500 },
        ],
      },
    ]);
    const entries = idx.executionsByRootExecId.get("root-X");
    expect(entries).toHaveLength(3);
    expect(entries?.map((e) => e.agentId)).toEqual(["root", "researcher", "coder"]);
  });

  it("treats missing tokens_in/out as 0", () => {
    const idx = buildIndex([
      {
        id: "sess-zero",
        total_tokens_in: 0,
        total_tokens_out: 0,
        executions: [
          // Missing fields — fields default to undefined; helper should treat as 0.
          { id: "root-zero", agent_id: "root", tokens_in: undefined as unknown as number, tokens_out: undefined as unknown as number, delegation_type: "root" },
        ],
      },
    ]);
    const summary = idx.byRootExecId.get("root-zero");
    expect(summary).toEqual({ in: 0, out: 0, total: 0 });
  });
});

describe("buildIndex — preserves v2 status as canonical truth", () => {
  it("captures the v2 status on the index entry (so the UI can override stale logs status)", () => {
    const idx = buildIndex([
      {
        id: "sess-A",
        status: "running",
        total_tokens_in: 1000,
        total_tokens_out: 50,
        executions: [
          { id: "exec-root-A", agent_id: "root", tokens_in: 1000, tokens_out: 50, delegation_type: "root" },
        ],
      },
    ]);
    expect(idx.byRootExecId.get("exec-root-A")?.status).toBe("running");
  });
});

describe("normalizeV2Status", () => {
  it("maps completed → completed", () => expect(normalizeV2Status("completed")).toBe("completed"));
  it("maps crashed → error", () => expect(normalizeV2Status("crashed")).toBe("error"));
  it("maps running → running", () => expect(normalizeV2Status("running")).toBe("running"));
  // queued + paused have no LogSession equivalent — surface as "running" so
  // the UI keeps polling and the Live badge stays on.
  it("maps queued → running (no LogSession equivalent)", () => expect(normalizeV2Status("queued")).toBe("running"));
  it("maps paused → running (no LogSession equivalent)", () => expect(normalizeV2Status("paused")).toBe("running"));
});

describe("applyV2Status", () => {
  function makeSession(overrides: Partial<LogSession> = {}): LogSession {
    return {
      session_id: "sess-1",
      conversation_id: "conv-1",
      agent_id: "agent:root",
      agent_name: "root",
      started_at: "2026-04-26T00:00:00Z",
      status: "completed",
      token_count: 0,
      tool_call_count: 0,
      error_count: 0,
      child_session_ids: [],
      ...overrides,
    };
  }

  it("returns the original list untouched when the index is empty", () => {
    const sessions = [makeSession()];
    expect(applyV2Status(sessions, { byRootExecId: new Map(), executionsByRootExecId: new Map() })).toBe(sessions);
  });

  it("REGRESSION: overrides logs-API stale 'completed' with v2 'running'", () => {
    // Repro of the live bug: the logs API claimed a session was completed
    // while /api/executions/v2/sessions/full correctly reported it as still
    // running. Mission Control was lying to the user.
    const stale = makeSession({ session_id: "exec-running-1", status: "completed" });
    const idx = buildIndex([
      {
        id: "sess-X",
        status: "running",
        total_tokens_in: 200000,
        total_tokens_out: 6000,
        executions: [
          { id: "exec-running-1", agent_id: "root", tokens_in: 200000, tokens_out: 6000, delegation_type: "root" },
        ],
      },
    ]);
    const out = applyV2Status([stale], idx);
    expect(out[0].status).toBe("running");
    expect(out[0]).not.toBe(stale); // new object (so React detects the change)
  });

  it("does not allocate a new object when the status already matches", () => {
    const session = makeSession({ session_id: "exec-X", status: "running" });
    const idx = buildIndex([
      {
        id: "sess-Y",
        status: "running",
        total_tokens_in: 0,
        total_tokens_out: 0,
        executions: [
          { id: "exec-X", agent_id: "root", tokens_in: 0, tokens_out: 0, delegation_type: "root" },
        ],
      },
    ]);
    const out = applyV2Status([session], idx);
    expect(out[0]).toBe(session);
  });

  it("leaves untouched any session that the v2 index doesn't know about", () => {
    const orphan = makeSession({ session_id: "exec-orphan", status: "completed" });
    const known = makeSession({ session_id: "exec-known", status: "completed" });
    const idx = buildIndex([
      {
        id: "sess-Z",
        status: "running",
        total_tokens_in: 0,
        total_tokens_out: 0,
        executions: [{ id: "exec-known", agent_id: "root", tokens_in: 0, tokens_out: 0, delegation_type: "root" }],
      },
    ]);
    const out = applyV2Status([orphan, known], idx);
    expect(out[0]).toBe(orphan);
    expect(out[1].status).toBe("running");
  });
});

describe("sumExecutionTokensByAgent", () => {
  it("returns an empty map when entries is undefined", () => {
    expect(sumExecutionTokensByAgent(undefined).size).toBe(0);
  });

  it("returns one entry per unique agent_id when each appears once", () => {
    const out = sumExecutionTokensByAgent([
      { executionId: "1", agentId: "root", in: 100, out: 10 },
      { executionId: "2", agentId: "researcher", in: 500, out: 50 },
    ]);
    expect(out.get("root")).toEqual({ in: 100, out: 10 });
    expect(out.get("researcher")).toEqual({ in: 500, out: 50 });
  });

  it("sums input/output when an agent appears multiple times (re-delegated)", () => {
    const out = sumExecutionTokensByAgent([
      { executionId: "a", agentId: "builder-agent", in: 1000, out: 100 },
      { executionId: "b", agentId: "builder-agent", in: 2000, out: 200 },
      { executionId: "c", agentId: "builder-agent", in: 500, out: 50 },
    ]);
    expect(out.size).toBe(1);
    expect(out.get("builder-agent")).toEqual({ in: 3500, out: 350 });
  });
});
