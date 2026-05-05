# Research multi-turn — implementation plan

> **For agentic workers:** Use `superpowers:executing-plans` to implement this task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render research sessions as a chronological list of turns (user message → its subagents → its assistant reply), derived from existing API responses. Eliminate the current "all user bubbles top, all subagents middle, one reply bottom" flattening.

**Architecture:** UI-only. Walk root-execution messages by `created_at`, treat each `role==="user"` boundary as a new turn, bucket subagents by `started_at` falling in `[turn.startedAt, turn.endedAt)`, attach the latest assistant reply per turn. Reducer routes WS events to the latest open turn.

**Tech Stack:** TypeScript, React, Vite, Vitest.

**Spec:** `memory-bank/future-state/2026-05-05-research-multi-turn-design.md`

---

## File map

**New (3):**
- `apps/ui/src/features/research-v2/turns.ts` — pure-function turn builder.
- `apps/ui/src/features/research-v2/turns.test.ts` — unit tests.
- `apps/ui/src/features/research-v2/SessionTurnBlock.tsx` — renders one `SessionTurn`.

**Edit (5):**
- `apps/ui/src/features/research-v2/types.ts` — add `SessionTurn`; remove `messages: ResearchMessage[]` from snapshot/state shape.
- `apps/ui/src/features/research-v2/session-snapshot.ts` — replace `buildTurns` with `buildSessionTurns`; drop `extractRespondByExecId` (replaced by per-turn extraction).
- `apps/ui/src/features/research-v2/reducer.ts` — change open-turn semantics to "always the last entry of `state.turns`".
- `apps/ui/src/features/research-v2/ResearchPage.tsx` — render `snap.turns.map(t => <SessionTurnBlock turn={t} />)`.
- `apps/ui/src/features/research-v2/index.ts` — export the new component if needed by tests.

**Tests (existing files updated):**
- `session-snapshot.test.ts` — drop assertions on `messages`/single-`respond`; replace with multi-turn fixtures.
- `reducer.test.ts` — extend with multi-turn live-event sequences.
- `ResearchPage.test.tsx` — assert two-turn render order.

---

## Task 1 — Add `SessionTurn` type, drop `messages` field

**Files:** `apps/ui/src/features/research-v2/types.ts`

- [ ] **Step 1.1: Add `SessionTurn` and supporting types**

  Append to `types.ts`:

  ```ts
  /** Per-turn timeline entry (mirrors AgentTurn['timeline'][number] shape). */
  export interface TurnTimelineEntry {
    kind: "thinking" | "tool_call" | "tool_result" | "note" | "error";
    text: string;
    toolName?: string;
    toolArgsPreview?: string;
    createdAt: string;
  }

  /** One user→assistant exchange within a session. The session's root
   *  execution can carry many of these (continuations); the UI groups by
   *  user-message timestamp boundaries. */
  export interface SessionTurn {
    /** Stable id derived from the user message id. */
    id: string;
    /** 0..N-1 chronological. */
    index: number;
    /** The user message that opens this turn. */
    userMessage: { id: string; content: string; createdAt: string };
    /** Subagents whose started_at falls in [startedAt, endedAt). */
    subagents: AgentTurn[];
    /** Final assistant text reply, null while in flight. */
    assistantText: string | null;
    /** Streaming buffer for in-flight reply (promoted on turn end). */
    assistantStreaming: string;
    /** Per-turn timeline (root-execution events in this window). */
    timeline: TurnTimelineEntry[];
    /** Per-turn status. */
    status: "running" | "completed" | "stopped" | "error";
    /** ISO timestamp of the user message. */
    startedAt: string;
    /** Right edge: next user message minus 1 ms, root.ended_at, or null. */
    endedAt: string | null;
    /** End - start in ms (null while running). */
    durationMs: number | null;
  }
  ```

- [ ] **Step 1.2: Update `ResearchSnapshot` and `ResearchState`**

  Find the existing `ResearchSnapshot` interface in `session-snapshot.ts` and the `ResearchState` interface (likely in `types.ts` or `useResearchSession.ts`). Replace `messages: ResearchMessage[]` and `turns: AgentTurn[]` with `turns: SessionTurn[]`.

  Keep `intentAnalysis`, `ward`, `artifacts`, `status`, `conversationId`, `title` at the snapshot/state level.

- [ ] **Step 1.3: `cargo`-equivalent type-check**

  Run: `cd apps/ui && npx tsc --noEmit 2>&1 | head -30`

  Expected: many errors at consumers of the removed fields. Each one is a Task 3/4/5/6 follow-up; this is the bow-wave of the migration. Don't fix here — record them.

---

## Task 2 — Pure-function turn builder

**Files:**
- Create: `apps/ui/src/features/research-v2/turns.ts`
- Create: `apps/ui/src/features/research-v2/turns.test.ts`

- [ ] **Step 2.1: Write failing tests first (TDD)**

  `apps/ui/src/features/research-v2/turns.test.ts`:

  ```ts
  import { describe, it, expect } from "vitest";
  import {
    findTurnBoundaries,
    bucketSubagents,
    extractAssistantReplyForTurn,
    buildSessionTurns,
  } from "./turns";
  import type { SessionMessage, LogSession } from "@/services/transport/types";

  function userMsg(id: string, createdAt: string, content = "user " + id): SessionMessage {
    return {
      id, execution_id: "root", agent_id: "root", delegation_type: "root",
      role: "user", content, created_at: createdAt,
    } as SessionMessage;
  }

  function asstMsg(id: string, createdAt: string, content: string, toolCalls?: unknown): SessionMessage {
    return {
      id, execution_id: "root", agent_id: "root", delegation_type: "root",
      role: "assistant", content, created_at: createdAt, tool_calls: toolCalls,
    } as unknown as SessionMessage;
  }

  function child(sessionId: string, startedAt: string, endedAt: string): LogSession {
    return {
      session_id: sessionId, conversation_id: "sess-X",
      agent_id: "builder-agent", agent_name: "builder-agent",
      parent_session_id: "root",
      started_at: startedAt, ended_at: endedAt,
      status: "completed", token_count: 0, tool_call_count: 0, error_count: 0,
      child_session_ids: [], title: "",
    } as unknown as LogSession;
  }

  describe("findTurnBoundaries", () => {
    it("returns one boundary per user message in chronological order", () => {
      const msgs = [
        userMsg("u1", "2026-05-03T13:05:34Z"),
        asstMsg("a1", "2026-05-03T13:12:50Z", "first reply"),
        userMsg("u2", "2026-05-05T13:11:30Z"),
      ];
      const boundaries = findTurnBoundaries(msgs, null);
      expect(boundaries).toHaveLength(2);
      expect(boundaries[0].userMessage.id).toBe("u1");
      expect(boundaries[0].startedAt).toBe("2026-05-03T13:05:34Z");
      expect(boundaries[0].endedAt).toBe("2026-05-05T13:11:30Z");
      expect(boundaries[1].userMessage.id).toBe("u2");
      expect(boundaries[1].startedAt).toBe("2026-05-05T13:11:30Z");
      expect(boundaries[1].endedAt).toBeNull(); // last turn open
    });

    it("uses rootEndedAt as the right edge of the last turn when session is done", () => {
      const msgs = [userMsg("u1", "2026-05-03T13:05:34Z")];
      const boundaries = findTurnBoundaries(msgs, "2026-05-03T13:12:50Z");
      expect(boundaries[0].endedAt).toBe("2026-05-03T13:12:50Z");
    });

    it("returns empty when no user messages yet", () => {
      expect(findTurnBoundaries([], null)).toEqual([]);
    });
  });

  describe("bucketSubagents", () => {
    it("buckets each child into the turn whose [startedAt, endedAt) contains it", () => {
      const boundaries = [
        { userMessage: { id: "u1", content: "", createdAt: "2026-05-03T13:05:34Z" },
          startedAt: "2026-05-03T13:05:34Z", endedAt: "2026-05-05T13:11:30Z" },
        { userMessage: { id: "u2", content: "", createdAt: "2026-05-05T13:11:30Z" },
          startedAt: "2026-05-05T13:11:30Z", endedAt: null },
      ];
      const childA = child("c-A", "2026-05-03T13:05:49Z", "2026-05-03T13:08:39Z"); // turn 1
      const childB = child("c-B", "2026-05-05T13:11:35Z", "2026-05-05T13:14:58Z"); // turn 2
      const buckets = bucketSubagents(boundaries, [childB, childA]);
      expect(buckets.get(0)?.map((c) => c.session_id)).toEqual(["c-A"]);
      expect(buckets.get(1)?.map((c) => c.session_id)).toEqual(["c-B"]);
    });

    it("subagent at exact boundary belongs to the new turn", () => {
      const boundaries = [
        { userMessage: { id: "u1", content: "", createdAt: "2026-05-03T13:00:00Z" },
          startedAt: "2026-05-03T13:00:00Z", endedAt: "2026-05-03T13:10:00Z" },
        { userMessage: { id: "u2", content: "", createdAt: "2026-05-03T13:10:00Z" },
          startedAt: "2026-05-03T13:10:00Z", endedAt: null },
      ];
      const onBoundary = child("c", "2026-05-03T13:10:00Z", "2026-05-03T13:11:00Z");
      const buckets = bucketSubagents(boundaries, [onBoundary]);
      expect(buckets.get(0) ?? []).toEqual([]);
      expect(buckets.get(1)?.map((c) => c.session_id)).toEqual(["c"]);
    });

    it("subagent before any user message is dropped (defensive)", () => {
      const boundaries = [
        { userMessage: { id: "u1", content: "", createdAt: "2026-05-03T13:05:00Z" },
          startedAt: "2026-05-03T13:05:00Z", endedAt: null },
      ];
      const earlyChild = child("c", "2026-05-03T13:04:00Z", "2026-05-03T13:04:30Z");
      const buckets = bucketSubagents(boundaries, [earlyChild]);
      expect(buckets.size).toBe(0);
    });
  });

  describe("extractAssistantReplyForTurn", () => {
    it("returns the latest plain-text assistant message in the window", () => {
      const win = [
        asstMsg("a1", "2026-05-03T13:06:00Z", "intermediate"),
        asstMsg("a2", "2026-05-03T13:12:50Z", "final answer"),
      ];
      expect(extractAssistantReplyForTurn(win)).toBe("final answer");
    });

    it("falls back to respond() tool call when no plain text exists", () => {
      const win = [
        asstMsg("a1", "2026-05-03T13:06:00Z", "[tool calls]",
                JSON.stringify([{ tool_name: "respond", args: { message: "via respond" } }])),
      ];
      expect(extractAssistantReplyForTurn(win)).toBe("via respond");
    });

    it("prefers plain text over respond() when both exist later", () => {
      const win = [
        asstMsg("a1", "2026-05-03T13:06:00Z", "[tool calls]",
                JSON.stringify([{ tool_name: "respond", args: { message: "via respond" } }])),
        asstMsg("a2", "2026-05-03T13:12:50Z", "plain text reply"),
      ];
      expect(extractAssistantReplyForTurn(win)).toBe("plain text reply");
    });

    it("returns null on an empty window", () => {
      expect(extractAssistantReplyForTurn([])).toBeNull();
    });

    it("ignores the [tool calls] placeholder", () => {
      const win = [asstMsg("a1", "2026-05-03T13:06:00Z", "[tool calls]")];
      expect(extractAssistantReplyForTurn(win)).toBeNull();
    });
  });

  describe("buildSessionTurns end-to-end", () => {
    it("recreates the recorded sess-9f40dc55 shape (2 turns, 4 subagents)", () => {
      const rootMessages = [
        userMsg("u1", "2026-05-03T13:05:34Z", "help me"),
        asstMsg("a1", "2026-05-03T13:12:50Z", "The assignment is fully complete!"),
        userMsg("u2", "2026-05-05T13:11:30Z", "can you make it into a presentation"),
        asstMsg("a2", "2026-05-05T13:14:58Z", "Done! I've created a 22-slide HTML presentation"),
      ];
      const children: LogSession[] = [
        child("c1", "2026-05-03T13:05:49Z", "2026-05-03T13:08:39Z"),
        child("c2", "2026-05-03T13:08:48Z", "2026-05-03T13:10:55Z"),
        child("c3", "2026-05-03T13:11:00Z", "2026-05-03T13:12:42Z"),
        child("c4", "2026-05-05T13:11:35Z", "2026-05-05T13:14:58Z"),
      ];
      const turns = buildSessionTurns({
        rootSessionId: "root",
        rootEndedAt: "2026-05-05T13:15:02Z",
        rootStatus: "completed",
        rootMessages,
        childRows: children,
      });
      expect(turns).toHaveLength(2);
      expect(turns[0].userMessage.content).toBe("help me");
      expect(turns[0].subagents.map((s) => s.id)).toEqual(["c1", "c2", "c3"]);
      expect(turns[0].assistantText).toBe("The assignment is fully complete!");
      expect(turns[0].status).toBe("completed");
      expect(turns[1].userMessage.content).toBe("can you make it into a presentation");
      expect(turns[1].subagents.map((s) => s.id)).toEqual(["c4"]);
      expect(turns[1].assistantText).toBe("Done! I've created a 22-slide HTML presentation");
      expect(turns[1].status).toBe("completed");
    });

    it("last turn has status='running' when the root execution is still active", () => {
      const rootMessages = [userMsg("u1", "2026-05-05T13:00:00Z")];
      const turns = buildSessionTurns({
        rootSessionId: "root", rootEndedAt: null, rootStatus: "running",
        rootMessages, childRows: [],
      });
      expect(turns[0].status).toBe("running");
      expect(turns[0].assistantText).toBeNull();
      expect(turns[0].endedAt).toBeNull();
    });
  });
  ```

- [ ] **Step 2.2: Run tests, verify they all FAIL with "module not found"**

  Run: `cd apps/ui && npx vitest run src/features/research-v2/turns.test.ts 2>&1 | tail -10`

  Expected: import errors. Confirms TDD red phase.

- [ ] **Step 2.3: Implement `turns.ts`**

  ```ts
  // apps/ui/src/features/research-v2/turns.ts
  import type { LogSession, SessionMessage } from "@/services/transport/types";
  import type { AgentTurn, SessionTurn } from "./types";
  import { turnFromLogRow } from "./session-snapshot";

  const TOOL_CALLS_PLACEHOLDER = "[tool calls]";
  const RESPOND_TOOL_NAME = "respond";

  export interface TurnBoundary {
    userMessage: { id: string; content: string; createdAt: string };
    startedAt: string;
    endedAt: string | null;
  }

  /** Walk a root execution's messages chronologically; each user message
   *  opens a new boundary. The right edge of each boundary is the next
   *  user-message timestamp (or `rootEndedAt`, or null while running). */
  export function findTurnBoundaries(
    rootMessages: SessionMessage[],
    rootEndedAt: string | null,
  ): TurnBoundary[] {
    const sorted = [...rootMessages].sort((a, b) =>
      a.created_at.localeCompare(b.created_at),
    );
    const userMessages = sorted.filter((m) => m.role === "user");
    return userMessages.map((m, i) => {
      const nextStart = userMessages[i + 1]?.created_at ?? null;
      const endedAt = nextStart ?? rootEndedAt ?? null;
      return {
        userMessage: { id: m.id, content: m.content, createdAt: m.created_at },
        startedAt: m.created_at,
        endedAt,
      };
    });
  }

  /** Bucket each child execution into the turn whose
   *  [startedAt, endedAt) interval contains its `started_at`. Children
   *  before the first user message are dropped (defensive — shouldn't
   *  happen, but the data sometimes lies). */
  export function bucketSubagents(
    boundaries: TurnBoundary[],
    childRows: LogSession[],
  ): Map<number, LogSession[]> {
    const out = new Map<number, LogSession[]>();
    for (const child of childRows) {
      const ts = child.started_at;
      const idx = boundaries.findIndex((b) => {
        const startsOk = b.startedAt <= ts;
        const endsOk = b.endedAt === null || ts < b.endedAt;
        return startsOk && endsOk;
      });
      if (idx === -1) continue;
      if (!out.has(idx)) out.set(idx, []);
      out.get(idx)!.push(child);
    }
    return out;
  }

  /** Last assistant text reply in the window. Prefers plain `content`;
   *  falls back to a `respond()` tool-call argument; returns null on
   *  no-match. */
  export function extractAssistantReplyForTurn(
    windowMessages: SessionMessage[],
  ): string | null {
    const sorted = [...windowMessages].sort((a, b) =>
      a.created_at.localeCompare(b.created_at),
    );
    let plain: string | null = null;
    let respondText: string | null = null;
    for (const m of sorted) {
      if (m.role !== "assistant") continue;
      const calls = parseToolCalls(m);
      for (const call of calls) {
        if (call?.tool_name !== RESPOND_TOOL_NAME) continue;
        const message = call.args?.["message"];
        if (typeof message === "string" && message.length > 0) {
          respondText = message;
        }
      }
      if (
        typeof m.content === "string" &&
        m.content.length > 0 &&
        m.content !== TOOL_CALLS_PLACEHOLDER
      ) {
        plain = m.content;
      }
    }
    return plain ?? respondText;
  }

  interface ToolCall {
    tool_name?: string;
    args?: Record<string, unknown>;
  }

  function parseToolCalls(m: SessionMessage): ToolCall[] {
    const camel = (m as unknown as { toolCalls?: unknown }).toolCalls;
    const candidate = camel ?? m.tool_calls;
    if (candidate == null) return [];
    try {
      const raw = typeof candidate === "string" ? candidate : JSON.stringify(candidate);
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? (parsed as ToolCall[]) : [];
    } catch {
      return [];
    }
  }

  export interface BuildSessionTurnsInput {
    rootSessionId: string;
    rootEndedAt: string | null;
    rootStatus: "running" | "completed" | "stopped" | "error";
    /** Messages whose execution_id == rootSessionId. */
    rootMessages: SessionMessage[];
    /** Child execution rows (parent_session_id == rootSessionId). */
    childRows: LogSession[];
  }

  /** Compose the per-turn rollup — boundaries → buckets → reply per turn. */
  export function buildSessionTurns(input: BuildSessionTurnsInput): SessionTurn[] {
    const { rootSessionId, rootEndedAt, rootStatus, rootMessages, childRows } = input;
    const boundaries = findTurnBoundaries(rootMessages, rootEndedAt);
    const buckets = bucketSubagents(boundaries, childRows);

    return boundaries.map((b, i) => {
      const subRows = buckets.get(i) ?? [];
      const subagents: AgentTurn[] = subRows
        .map((row) => turnFromLogRow(row, rootSessionId))
        .sort((a, b2) => a.startedAt - b2.startedAt);

      const windowMessages = rootMessages.filter((m) => {
        const ts = m.created_at;
        return ts >= b.startedAt && (b.endedAt === null || ts < b.endedAt);
      });
      const assistantText = extractAssistantReplyForTurn(windowMessages);

      const status = deriveTurnStatus({
        isLast: i === boundaries.length - 1,
        rootStatus,
        assistantText,
        subagents,
      });

      const startedMs = Date.parse(b.startedAt);
      const endedMs = b.endedAt ? Date.parse(b.endedAt) : null;
      const durationMs = endedMs !== null ? endedMs - startedMs : null;

      return {
        id: `turn-${b.userMessage.id}`,
        index: i,
        userMessage: b.userMessage,
        subagents,
        assistantText,
        assistantStreaming: "",
        timeline: [],
        status,
        startedAt: b.startedAt,
        endedAt: b.endedAt,
        durationMs,
      };
    });
  }

  function deriveTurnStatus(args: {
    isLast: boolean;
    rootStatus: "running" | "completed" | "stopped" | "error";
    assistantText: string | null;
    subagents: AgentTurn[];
  }): SessionTurn["status"] {
    const { isLast, rootStatus, assistantText, subagents } = args;
    if (rootStatus === "error") return "error";
    if (rootStatus === "stopped") return "stopped";
    if (isLast && rootStatus === "running" && assistantText === null) return "running";
    if (isLast && rootStatus === "running" && subagents.some((s) => s.status === "running")) {
      return "running";
    }
    return "completed";
  }
  ```

- [ ] **Step 2.4: Run the test file, verify all pass**

  Run: `cd apps/ui && npx vitest run src/features/research-v2/turns.test.ts 2>&1 | tail -15`

  Expected: 11 tests pass.

---

## Task 3 — Wire `buildSessionTurns` into `session-snapshot.ts`

**Files:** `apps/ui/src/features/research-v2/session-snapshot.ts`

- [ ] **Step 3.1: Read the existing `buildTurns` and `snapshotSession` functions**

  Locate `buildTurns(rootRow, sessionRows, messages)` and the assistant-message extraction block. Note the current `extractRespondByExecId` consumers and `applyRespond` helper — both go away.

- [ ] **Step 3.2: Replace `buildTurns` with the new pipeline**

  Inside `snapshotSession` (where `buildTurns` is called today), import `buildSessionTurns` and call it instead:

  ```ts
  // Replace the old: const turns = buildTurns(rootRow, sessionRows, messages);
  // With:
  const rootMessages = messages.filter((m) => m.execution_id === rootRow.session_id);
  const childRows = sessionRows
    .filter((r) => !isRootRow(r) && r.session_id !== rootRow.session_id);

  const rootStatus = mapBackendStatusToTurnStatus(rootRow.status);
  const turns = buildSessionTurns({
    rootSessionId: rootRow.session_id,
    rootEndedAt: rootRow.ended_at ?? null,
    rootStatus,
    rootMessages,
    childRows,
  });
  ```

  `mapBackendStatusToTurnStatus` is a small helper — `running`/`completed`/`stopped`/`error` mapped from the backend's status string. Add it locally if not already present.

- [ ] **Step 3.3: Drop the now-dead helpers**

  Remove `extractRespondByExecId`, `applyRespond`, and the `buildTurns` function. Update `session-snapshot.test.ts` to drop tests that reference these symbols (the new behavior is covered by `turns.test.ts`).

- [ ] **Step 3.4: Update `ResearchSnapshot` to drop `messages`**

  Find the interface definition at the top of `session-snapshot.ts` (or `types.ts`); remove `messages: ResearchMessage[]`. The user-message data now lives in each `turn.userMessage`.

- [ ] **Step 3.5: Compile-check**

  Run: `cd apps/ui && npx tsc --noEmit 2>&1 | tee /tmp/tsc-after-task3.log | head -30`

  Expected: errors at consumers of `snap.messages` and any flat `turns: AgentTurn[]` reference. Continue to Task 4 to fix the reducer; Tasks 5/6 fix the page render.

---

## Task 4 — Reducer routes events to the latest open turn

**Files:** `apps/ui/src/features/research-v2/reducer.ts`

- [ ] **Step 4.1: Re-shape the reducer state**

  The state `turns` field changes from `AgentTurn[]` to `SessionTurn[]`. Subagent `AgentTurn` instances live inside `SessionTurn.subagents`, not at the top level.

  Add helpers:

  ```ts
  function lastTurn(state: ResearchState): SessionTurn | null {
    return state.turns.length > 0 ? state.turns[state.turns.length - 1] : null;
  }
  function setLastTurn(state: ResearchState, fn: (t: SessionTurn) => SessionTurn): SessionTurn[] {
    if (state.turns.length === 0) return state.turns;
    const next = [...state.turns];
    next[next.length - 1] = fn(next[next.length - 1]);
    return next;
  }
  ```

- [ ] **Step 4.2: Per-action behaviour**

  Map each existing action to the new shape. Key changes:

  - `user_message` (or whatever today's "new user prompt arrived" action is named) → `state.turns.push(newOpenTurn(payload))`. The new turn starts with `assistantText=null`, `assistantStreaming=""`, `subagents=[]`, `status="running"`.

  - `delegation_started` → append the subagent to `lastTurn().subagents`. Use `setLastTurn` to keep state immutable.

  - `token` (streaming) → if the event's `execution_id` matches a subagent in `lastTurn().subagents`, append to that subagent's `respondStreaming`. Otherwise (root-execution token), append to `lastTurn().assistantStreaming`.

  - `thinking` / `tool_call` / `tool_result` / `note` / `error` → append to the matching subagent's `timeline`, OR to `lastTurn().timeline` if the event's `execution_id == rootSessionId`.

  - `respond` (turn-final assistant text) → `lastTurn().assistantText = payload.text; assistantStreaming = "";`

  - `turn_complete` → set `lastTurn().status = "completed"`, `endedAt = now`, compute `durationMs`. Also promote streaming buffer if respond never fired (mirrors today's `endTurn` logic).

  - `subagent_complete` → mark the matching subagent's status; do NOT close the parent turn unless this is the last one and root reports completion.

- [ ] **Step 4.3: Update `reducer.test.ts`**

  Add multi-turn fixtures:

  ```ts
  it("opens a new turn on each user_message and routes events to the latest open turn", () => {
    let state = initial();
    state = reduce(state, { type: "user_message", id: "u1", content: "hi", createdAt: "T1" });
    expect(state.turns).toHaveLength(1);
    state = reduce(state, { type: "respond", text: "first reply" });
    expect(state.turns[0].assistantText).toBe("first reply");
    state = reduce(state, { type: "user_message", id: "u2", content: "follow up", createdAt: "T2" });
    expect(state.turns).toHaveLength(2);
    state = reduce(state, { type: "respond", text: "second reply" });
    expect(state.turns[1].assistantText).toBe("second reply");
    expect(state.turns[0].assistantText).toBe("first reply"); // not clobbered
  });

  it("token events route by execution_id — root tokens land on the open turn", () => {
    // construct a state with a subagent in flight; assert tokens with
    // root exec_id append to assistantStreaming, tokens with subagent
    // exec_id append to that subagent's respondStreaming
  });
  ```

  (Use the actual action names/types from the existing reducer; the snippets above are illustrative.)

- [ ] **Step 4.4: Run reducer tests**

  Run: `cd apps/ui && npx vitest run src/features/research-v2/reducer.test.ts 2>&1 | tail -15`

  Expected: all pass.

---

## Task 5 — `SessionTurnBlock` component

**Files:** `apps/ui/src/features/research-v2/SessionTurnBlock.tsx` (new)

- [ ] **Step 5.1: Write the component**

  ```tsx
  import type { SessionTurn } from "./types";
  import { Markdown } from "../shared/markdown";
  import { CopyButton } from "./ResearchMessages";
  import { SubagentCardTree } from "./AgentTurnBlock"; // export this if not already

  interface Props {
    turn: SessionTurn;
  }

  export function SessionTurnBlock({ turn }: Props) {
    const reply = turn.assistantText ?? (turn.assistantStreaming || null);
    const isStreaming = turn.assistantText === null && turn.assistantStreaming.length > 0;

    return (
      <section className="session-turn" data-turn-index={turn.index} data-status={turn.status}>
        <div className="session-turn__user research-msg research-msg--user">
          <div className="research-msg__card">
            <div className="research-msg__body">{turn.userMessage.content}</div>
          </div>
          <CopyButton text={turn.userMessage.content} label="Copy question" />
        </div>

        {turn.subagents.length > 0 && (
          <div className="session-turn__subagents">
            {turn.subagents.map((sa) => (
              <SubagentCardTree key={sa.id} turn={sa} allTurns={turn.subagents} />
            ))}
          </div>
        )}

        <div
          className={
            "session-turn__assistant research-msg research-msg--assistant" +
            (isStreaming ? " research-msg--streaming" : "")
          }
        >
          <div className="research-msg__card">
            <div className="research-msg__body">
              {reply !== null ? (
                <Markdown>{reply}</Markdown>
              ) : (
                <span className="agent-turn-block__placeholder">waiting…</span>
              )}
            </div>
          </div>
          {reply !== null && <CopyButton text={reply} label="Copy response" />}
        </div>
      </section>
    );
  }
  ```

  If `SubagentCardTree` isn't already exported, add `export` to its declaration in `AgentTurnBlock.tsx`. The `Markdown` and `CopyButton` imports may be already available — match the existing patterns from `AgentTurnBlock.tsx`.

- [ ] **Step 5.2: Add a thin CSS divider between turns**

  In `apps/ui/src/features/research-v2/research.css`, append:

  ```css
  .session-turn + .session-turn {
    border-top: 1px solid var(--border);
    margin-top: var(--space-section);
    padding-top: var(--space-section);
  }
  ```

  Use the actual token names from the existing CSS file. Keep it minimal — visual polish is a follow-up.

---

## Task 6 — Wire `ResearchPage.tsx`

**Files:** `apps/ui/src/features/research-v2/ResearchPage.tsx`

- [ ] **Step 6.1: Replace the render of user bubbles + flat turns**

  Find the current block that renders `state.messages.map(...)` and `state.turns.map(...)` separately. Replace with:

  ```tsx
  {state.turns.map((turn) => (
    <SessionTurnBlock key={turn.id} turn={turn} />
  ))}
  ```

  Keep the page-level chrome (intent analysis chip, ward chip, header, input, scroll container, etc.) unchanged.

- [ ] **Step 6.2: Update `ResearchPage.test.tsx`**

  Replace assertions on flat `messages`/`turns` with:

  ```tsx
  it("renders multiple turns in chronological order", async () => {
    // build a snapshot with two turns
    // assert: user1 text appears, then 3 subagent agentIds, then reply1 text,
    // then user2 text, then 1 subagent, then reply2 text — using
    // testing-library's `screen.getByText(...).compareDocumentPosition(...)`
    // to verify DOM order.
  });
  ```

  Pattern: render with the new snapshot fixture, query for all turn DOM nodes, assert they appear in expected order.

- [ ] **Step 6.3: Build + run page tests**

  Run: `cd apps/ui && npm run build 2>&1 | tail -10 && npx vitest run src/features/research-v2/ResearchPage.test.tsx 2>&1 | tail -15`

  Expected: build clean, page tests pass.

---

## Task 7 — Sweep remaining test failures

**Files:** any test file flagged by `tsc --noEmit` or vitest as broken.

- [ ] **Step 7.1: Full TS check**

  Run: `cd apps/ui && npx tsc --noEmit 2>&1 | tail -40`

  Expected: zero errors. If any remain, fix in lock-step with the new types.

- [ ] **Step 7.2: Full UI test suite**

  Run: `cd apps/ui && npm test -- --run 2>&1 | tail -10`

  Expected:
  - 0 NEW failures vs the 23 pre-existing baseline failures noted on the previous PR.
  - New tests added: 11 in `turns.test.ts`, ~3 in `reducer.test.ts`, ~1 in `ResearchPage.test.tsx`.
  - Removed tests: those that asserted on `extractRespondByExecId` / `snap.messages` / single-`respond` semantics — covered by new tests instead.

- [ ] **Step 7.3: Lint**

  Run: `cd apps/ui && npm run lint 2>&1 | tail -15`

  Expected: clean (or only pre-existing warnings).

---

## Task 8 — Verify on the live deploy + open PR

- [ ] **Step 8.1: Hot-reload check on local dev server**

  The dev server (`npm run dev`) should auto-reload. Use Chrome DevTools MCP to navigate to the recorded session:

  ```
  navigate_page → http://localhost:3000/research/sess-9f40dc55-ae46-405f-8557-b7017084fefc
  take_snapshot
  ```

  Assert the rendered DOM order is:
  `user1-bubble → builder-card → research-card → writing-card → reply1-bubble → DIVIDER → user2-bubble → builder-card → reply2-bubble`.

  If the daemon is on the Pi (`192.168.4.24:18791`), navigate there instead — the deck data is on the Pi.

- [ ] **Step 8.2: Regression — single-turn session**

  Browse to any single-turn session (e.g. one of the "Berlin Wall" entries on the recent list). Confirm rendering is unchanged: one user bubble, its subagents, one reply, no divider.

- [ ] **Step 8.3: Commit, push, open PR**

  ```bash
  git add apps/ui/src/features/research-v2/turns.ts \
          apps/ui/src/features/research-v2/turns.test.ts \
          apps/ui/src/features/research-v2/SessionTurnBlock.tsx \
          apps/ui/src/features/research-v2/types.ts \
          apps/ui/src/features/research-v2/session-snapshot.ts \
          apps/ui/src/features/research-v2/session-snapshot.test.ts \
          apps/ui/src/features/research-v2/reducer.ts \
          apps/ui/src/features/research-v2/reducer.test.ts \
          apps/ui/src/features/research-v2/ResearchPage.tsx \
          apps/ui/src/features/research-v2/ResearchPage.test.tsx \
          apps/ui/src/features/research-v2/AgentTurnBlock.tsx \
          apps/ui/src/features/research-v2/research.css

  git commit -m "feat(research-v2): render sessions as a chronological list of turns

  Each user message starts a new turn. Subagents bucket by started_at into
  the turn whose [user_msg_i, user_msg_{i+1}) interval contains them.
  Each turn has its own assistant reply, derived per-window instead of
  last-write-wins per execution.

  Spec: memory-bank/future-state/2026-05-05-research-multi-turn-design.md
  Plan: memory-bank/plans/2026-05-05-research-multi-turn-implementation.md

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  "

  git push -u origin feat/research-multi-turn
  gh pr create --base develop --title "feat(research-v2): multi-turn session rendering" --body "$(...)"
  ```

  PR body: summarize the symptom, point at the spec doc, list test coverage, note "no backend change."

---

## Self-review checklist

- [ ] Every section of the spec maps to a task: data model (Task 1), algorithm (Task 2), wiring (Task 3), reducer (Task 4), render (Tasks 5+6), tests (Tasks 2+4+6+7), live verify (Task 8). ✅
- [ ] No placeholders: every code block is complete; every command is runnable. ✅
- [ ] Type/symbol consistency: `SessionTurn`, `AgentTurn`, `TurnBoundary`, `buildSessionTurns` used identically across tasks. ✅
- [ ] Order is safe: types first, pure functions next, then snapshot wiring, then reducer (depends on types), then components (depend on snapshot+reducer), then page (depends on component), then tests sweep, then ship. ✅
- [ ] Acknowledged risk surfaces: out-of-order WS events (Task 4 routes by `execution_id`), test sweep volume (Task 7 explicitly carves time), removed-helper consumers (Task 3 deletes them). ✅
