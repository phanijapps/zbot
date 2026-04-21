// =============================================================================
// session-snapshot — R14f unit tests.
//
// Coverage targets:
// - null returns on listLogSessions failure and missing root row
// - completed session: title, turns, respond per turn from tool_calls
// - running session: status + conversationId:null documented limitation
// - turnFromLogRow status mapping
// - extractDelegationTasks walks only root messages in timestamp order
// - children sorted by startedAt; leftover children get request:null
// - /artifacts endpoint wins over respond.args.artifacts fallback
// - last-respond-per-exec wins when an agent fires multiple
// - isRootRow ignores empty/undefined parent_session_id
// =============================================================================

import { describe, it, expect, vi, beforeEach } from "vitest";
import type { Transport } from "@/services/transport";
import type {
  Artifact,
  LogSession,
  SessionMessage,
  SessionStatus,
} from "@/services/transport/types";

const listLogSessions = vi.fn<Transport["listLogSessions"]>();
const getSessionMessages = vi.fn<Transport["getSessionMessages"]>();
const listSessionArtifacts = vi.fn<Transport["listSessionArtifacts"]>();
const getSessionState = vi.fn<Transport["getSessionState"]>();

function makeTransport(): Transport {
  // getSessionState defaults to a benign no-ward response; individual
  // tests can override to assert ward-bearing snapshots.
  getSessionState.mockImplementation(async () => ({
    success: true,
    data: {
      session: { id: "", title: null, status: "completed", startedAt: "", durationMs: 0, tokenCount: 0, model: null },
      userMessage: null,
      phase: "completed",
      response: null,
      intentAnalysis: null,
      ward: null,
      recalledFacts: [],
      plan: [],
      subagents: [],
      isLive: false,
    },
  }));
  return {
    listLogSessions,
    getSessionMessages,
    listSessionArtifacts,
    getSessionState,
  } as unknown as Transport;
}

import {
  extractDelegationTasks,
  extractRespondByExecId,
  isRootRow,
  snapshotSession,
  turnFromLogRow,
} from "./session-snapshot";

// -----------------------------------------------------------------------------
// Fixture factories
// -----------------------------------------------------------------------------

const SESSION_ID = "sess-ABC";
const ROOT_EXEC = "exec-root";
const CHILD_EXEC_1 = "exec-child-1";
const CHILD_EXEC_2 = "exec-child-2";

function makeRow(overrides: Partial<LogSession> = {}): LogSession {
  return {
    session_id: ROOT_EXEC,
    conversation_id: SESSION_ID,
    agent_id: "root",
    agent_name: "root",
    started_at: "2026-04-19T00:00:00.000Z",
    ended_at: "2026-04-19T00:01:00.000Z",
    status: "completed" as SessionStatus,
    token_count: 0,
    tool_call_count: 0,
    error_count: 0,
    child_session_ids: [],
    title: "",
    ...overrides,
  };
}

function makeMessage(overrides: Partial<SessionMessage> = {}): SessionMessage {
  return {
    id: `msg-${Math.random().toString(36).slice(2)}`,
    execution_id: ROOT_EXEC,
    agent_id: "root",
    delegation_type: "root",
    role: "assistant",
    content: "",
    created_at: "2026-04-19T00:00:00.000Z",
    ...overrides,
  };
}

function makeToolCallMessage(
  execId: string,
  toolCalls: Array<{ tool_name: string; args: Record<string, unknown>; tool_id?: string }>,
  createdAt: string = "2026-04-19T00:00:30.000Z",
): SessionMessage {
  return makeMessage({
    execution_id: execId,
    role: "assistant",
    content: "[tool calls]",
    created_at: createdAt,
    // Wire uses camelCase for this column; our parser accepts both.
    tool_calls: JSON.stringify(toolCalls),
  });
}

function makeArtifact(id: string, overrides: Partial<Artifact> = {}): Artifact {
  return {
    id,
    sessionId: SESSION_ID,
    filePath: `/tmp/${id}.md`,
    fileName: `${id}.md`,
    fileType: "md",
    fileSize: 100,
    createdAt: "2026-04-19T00:00:00Z",
    ...overrides,
  };
}

// -----------------------------------------------------------------------------
// Setup
// -----------------------------------------------------------------------------

beforeEach(() => {
  listLogSessions.mockReset();
  getSessionMessages.mockReset();
  listSessionArtifacts.mockReset();
  getSessionState.mockReset();
});

// -----------------------------------------------------------------------------
// Null paths
// -----------------------------------------------------------------------------

describe("snapshotSession — null returns", () => {
  it("returns null when listLogSessions fails", async () => {
    listLogSessions.mockResolvedValueOnce({ success: false, error: "offline" });
    getSessionMessages.mockResolvedValueOnce({ success: true, data: [] });
    listSessionArtifacts.mockResolvedValueOnce({ success: true, data: [] });

    const result = await snapshotSession(makeTransport(), SESSION_ID);
    expect(result).toBeNull();
  });

  it("returns null when no root row matches the sessionId", async () => {
    const unrelated = makeRow({
      conversation_id: "sess-DIFFERENT",
      parent_session_id: undefined,
    });
    listLogSessions.mockResolvedValueOnce({ success: true, data: [unrelated] });
    getSessionMessages.mockResolvedValueOnce({ success: true, data: [] });
    listSessionArtifacts.mockResolvedValueOnce({ success: true, data: [] });

    const result = await snapshotSession(makeTransport(), SESSION_ID);
    expect(result).toBeNull();
  });
});

// -----------------------------------------------------------------------------
// Completed-session happy path
// -----------------------------------------------------------------------------

describe("snapshotSession — completed session", () => {
  it("builds title + root + child turns with per-turn respond", async () => {
    const rootRow = makeRow({
      session_id: ROOT_EXEC,
      title: "Q4 market analysis",
      status: "completed" as SessionStatus,
      parent_session_id: undefined,
    });
    const childRow1 = makeRow({
      session_id: CHILD_EXEC_1,
      agent_id: "planner-agent",
      parent_session_id: ROOT_EXEC,
      started_at: "2026-04-19T00:00:10.000Z",
      status: "completed" as SessionStatus,
    });
    const childRow2 = makeRow({
      session_id: CHILD_EXEC_2,
      agent_id: "writer-agent",
      parent_session_id: ROOT_EXEC,
      started_at: "2026-04-19T00:00:20.000Z",
      status: "completed" as SessionStatus,
    });

    const userMsg = makeMessage({
      execution_id: ROOT_EXEC,
      role: "user",
      content: "What's the Q4 outlook?",
      created_at: "2026-04-19T00:00:00.000Z",
    });
    const rootDelegate1 = makeToolCallMessage(
      ROOT_EXEC,
      [{ tool_name: "delegate_to_agent", args: { agent_id: "planner-agent", task: "Plan the analysis." } }],
      "2026-04-19T00:00:05.000Z",
    );
    const rootDelegate2 = makeToolCallMessage(
      ROOT_EXEC,
      [{ tool_name: "delegate_to_agent", args: { agent_id: "writer-agent", task: "Draft the response." } }],
      "2026-04-19T00:00:15.000Z",
    );
    const rootRespond = makeToolCallMessage(
      ROOT_EXEC,
      [{ tool_name: "respond", args: { message: "Final answer." } }],
      "2026-04-19T00:00:50.000Z",
    );
    const child1Respond = makeToolCallMessage(
      CHILD_EXEC_1,
      [{ tool_name: "respond", args: { message: "Plan done." } }],
      "2026-04-19T00:00:12.000Z",
    );
    const child2Respond = makeToolCallMessage(
      CHILD_EXEC_2,
      [{ tool_name: "respond", args: { message: "Draft ready." } }],
      "2026-04-19T00:00:22.000Z",
    );

    listLogSessions.mockResolvedValueOnce({
      success: true,
      data: [rootRow, childRow1, childRow2],
    });
    getSessionMessages.mockResolvedValueOnce({
      success: true,
      data: [userMsg, rootDelegate1, child1Respond, rootDelegate2, child2Respond, rootRespond],
    });
    listSessionArtifacts.mockResolvedValueOnce({ success: true, data: [] });

    const snap = await snapshotSession(makeTransport(), SESSION_ID);
    expect(snap).not.toBeNull();
    expect(snap!.title).toBe("Q4 market analysis");
    expect(snap!.status).toBe("complete");

    expect(snap!.turns).toHaveLength(3);
    const [root, c1, c2] = snap!.turns;
    expect(root.id).toBe(ROOT_EXEC);
    expect(root.parentExecutionId).toBeNull();
    expect(root.respond).toBe("Final answer.");
    expect(c1.id).toBe(CHILD_EXEC_1);
    expect(c1.parentExecutionId).toBe(ROOT_EXEC);
    expect(c1.respond).toBe("Plan done.");
    expect(c1.request).toBe("Plan the analysis.");
    expect(c2.id).toBe(CHILD_EXEC_2);
    expect(c2.respond).toBe("Draft ready.");
    expect(c2.request).toBe("Draft the response.");

    // User bubble preserved, no assistants in state.messages.
    expect(snap!.messages).toHaveLength(1);
    expect(snap!.messages[0]).toMatchObject({ role: "user", content: "What's the Q4 outlook?" });
  });

  it("returns conversationId:null — documented limitation for re-attach", async () => {
    const runningRow = makeRow({
      session_id: ROOT_EXEC,
      status: "running" as SessionStatus,
      parent_session_id: undefined,
      ended_at: undefined,
    });
    listLogSessions.mockResolvedValueOnce({ success: true, data: [runningRow] });
    getSessionMessages.mockResolvedValueOnce({ success: true, data: [] });
    listSessionArtifacts.mockResolvedValueOnce({ success: true, data: [] });

    const snap = await snapshotSession(makeTransport(), SESSION_ID);
    expect(snap).not.toBeNull();
    expect(snap!.status).toBe("running");
    expect(snap!.conversationId).toBeNull();
  });
});

// -----------------------------------------------------------------------------
// turnFromLogRow — status mapping
// -----------------------------------------------------------------------------

describe("turnFromLogRow", () => {
  it("maps backend status strings into AgentTurnStatus", () => {
    const completed = turnFromLogRow(makeRow({ status: "completed" as SessionStatus }), null);
    expect(completed.status).toBe("completed");

    const running = turnFromLogRow(makeRow({ status: "running" as SessionStatus, ended_at: undefined }), null);
    expect(running.status).toBe("running");
    expect(running.completedAt).toBeNull();

    const stopped = turnFromLogRow(makeRow({ status: "stopped" as SessionStatus }), null);
    expect(stopped.status).toBe("stopped");

    const crashed = turnFromLogRow(makeRow({ status: "error" as SessionStatus }), null);
    expect(crashed.status).toBe("error");
  });

  it("carries parent execution id through", () => {
    const t = turnFromLogRow(makeRow({ session_id: CHILD_EXEC_1 }), ROOT_EXEC);
    expect(t.parentExecutionId).toBe(ROOT_EXEC);
    expect(t.id).toBe(CHILD_EXEC_1);
  });
});

// -----------------------------------------------------------------------------
// extractDelegationTasks
// -----------------------------------------------------------------------------

describe("extractDelegationTasks", () => {
  it("walks only ROOT's messages in timestamp order and returns tasks in order", () => {
    const earlier = makeToolCallMessage(
      ROOT_EXEC,
      [{ tool_name: "delegate_to_agent", args: { task: "first" } }],
      "2026-04-19T00:00:05.000Z",
    );
    const later = makeToolCallMessage(
      ROOT_EXEC,
      [{ tool_name: "delegate_to_agent", args: { task: "second" } }],
      "2026-04-19T00:00:15.000Z",
    );
    // Child execution's delegation-like calls must NOT pollute the root task
    // list (subagents don't spawn subagents, but this guards the filter).
    const childNoise = makeToolCallMessage(
      CHILD_EXEC_1,
      [{ tool_name: "delegate_to_agent", args: { task: "noise" } }],
      "2026-04-19T00:00:10.000Z",
    );

    // Intentional reverse insert to prove the function sorts.
    const tasks = extractDelegationTasks([later, childNoise, earlier], ROOT_EXEC);
    expect(tasks).toEqual(["first", "second"]);
  });
});

// -----------------------------------------------------------------------------
// Zip tasks → children
// -----------------------------------------------------------------------------

describe("snapshotSession — children zip + sort", () => {
  it("leftover children get request:null when delegation count < children count", async () => {
    const rootRow = makeRow({ session_id: ROOT_EXEC, parent_session_id: undefined });
    const childA = makeRow({
      session_id: "child-A",
      parent_session_id: ROOT_EXEC,
      started_at: "2026-04-19T00:00:10.000Z",
    });
    const childB = makeRow({
      session_id: "child-B",
      parent_session_id: ROOT_EXEC,
      started_at: "2026-04-19T00:00:20.000Z",
    });
    // Only one delegation task, two children — second child is orphan.
    const delegate = makeToolCallMessage(
      ROOT_EXEC,
      [{ tool_name: "delegate_to_agent", args: { task: "only task" } }],
      "2026-04-19T00:00:05.000Z",
    );

    listLogSessions.mockResolvedValueOnce({ success: true, data: [rootRow, childB, childA] });
    getSessionMessages.mockResolvedValueOnce({ success: true, data: [delegate] });
    listSessionArtifacts.mockResolvedValueOnce({ success: true, data: [] });

    const snap = await snapshotSession(makeTransport(), SESSION_ID);
    const [, first, second] = snap!.turns;
    // Children sorted by started_at ascending.
    expect(first.id).toBe("child-A");
    expect(second.id).toBe("child-B");
    expect(first.request).toBe("only task");
    expect(second.request).toBeNull();
  });
});

// -----------------------------------------------------------------------------
// Artifacts merge
// -----------------------------------------------------------------------------

describe("snapshotSession — artifacts", () => {
  it("/artifacts endpoint wins when it returns data", async () => {
    const rootRow = makeRow({ session_id: ROOT_EXEC, parent_session_id: undefined });
    const respondWithHints = makeToolCallMessage(ROOT_EXEC, [
      {
        tool_name: "respond",
        args: { message: "done", artifacts: [{ path: "/tmp/hint.md", label: "Hint" }] },
      },
    ]);
    const realArtifact = makeArtifact("real-1", { fileName: "real.md" });

    listLogSessions.mockResolvedValueOnce({ success: true, data: [rootRow] });
    getSessionMessages.mockResolvedValueOnce({ success: true, data: [respondWithHints] });
    listSessionArtifacts.mockResolvedValueOnce({ success: true, data: [realArtifact] });

    const snap = await snapshotSession(makeTransport(), SESSION_ID);
    expect(snap!.artifacts).toHaveLength(1);
    expect(snap!.artifacts[0].id).toBe("real-1");
    expect(snap!.artifacts[0].fileName).toBe("real.md");
  });

  it("falls back to respond.args.artifacts when /artifacts endpoint is empty", async () => {
    const rootRow = makeRow({ session_id: ROOT_EXEC, parent_session_id: undefined });
    const respondWithHints = makeToolCallMessage(ROOT_EXEC, [
      {
        tool_name: "respond",
        args: { message: "done", artifacts: [{ path: "/tmp/hint.md", label: "Plan" }] },
      },
    ]);

    listLogSessions.mockResolvedValueOnce({ success: true, data: [rootRow] });
    getSessionMessages.mockResolvedValueOnce({ success: true, data: [respondWithHints] });
    listSessionArtifacts.mockResolvedValueOnce({ success: true, data: [] });

    const snap = await snapshotSession(makeTransport(), SESSION_ID);
    expect(snap!.artifacts).toHaveLength(1);
    expect(snap!.artifacts[0].id).toBe("/tmp/hint.md");
    expect(snap!.artifacts[0].fileName).toBe("hint.md");
    expect(snap!.artifacts[0].label).toBe("Plan");
  });
});

// -----------------------------------------------------------------------------
// Last-respond-wins semantics
// -----------------------------------------------------------------------------

describe("extractRespondByExecId", () => {
  it("returns the LAST respond() per execution id", () => {
    const first = makeToolCallMessage(
      ROOT_EXEC,
      [{ tool_name: "respond", args: { message: "first" } }],
      "2026-04-19T00:00:05.000Z",
    );
    const second = makeToolCallMessage(
      ROOT_EXEC,
      [{ tool_name: "respond", args: { message: "second" } }],
      "2026-04-19T00:00:15.000Z",
    );
    const map = extractRespondByExecId([first, second]);
    expect(map.get(ROOT_EXEC)).toBe("second");
  });
});

// -----------------------------------------------------------------------------
// isRootRow
// -----------------------------------------------------------------------------

describe("isRootRow", () => {
  it("true when parent_session_id is undefined", () => {
    expect(isRootRow(makeRow({ parent_session_id: undefined }))).toBe(true);
  });
  it("true when parent_session_id is empty string", () => {
    expect(isRootRow(makeRow({ parent_session_id: "" }))).toBe(true);
  });
  it("false when parent_session_id is non-empty", () => {
    expect(isRootRow(makeRow({ parent_session_id: "exec-parent" }))).toBe(false);
  });
});
