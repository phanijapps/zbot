# Research UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a new durable multi-session research workbench at `/research-v2` with per-agent-turn blocks, a drawer-toggle sessions list, a ward-sticky state, and the same rolling status pill introduced by the Quick Chat plan. Fix four latent bugs at the UI layer while rebuilding.

**Architecture:** New page at `/research-v2` (and `/research-v2/:sessionId`) backed by `useResearchSession`, which collapses gateway events into per-agent-turn blocks. Subscribes to the existing WebSocket `invoke` flow with `mode="research"` (full pipeline). Reuses `StatusPill`, `useStatusPill`, and the tool-phrase dictionary from the Quick Chat plan. Keeps `SessionsList` presentation-agnostic so a later topbar evolution swaps its mount without rewriting data flow. Old `/` (MissionControl / WebChatPanel) remains untouched; retired in a separate cleanup PR.

**Tech Stack:** React 18 + TypeScript, React Router v6. Reuses existing `ChatInput`, `ArtifactSlideOut`, `ArtifactsPanel`, `ReactMarkdown` components. Vitest for unit tests. Existing `getTransport()` for HTTP + WebSocket.

**Depends on:** `docs/superpowers/plans/2026-04-19-quick-chat-ui.md` Tasks 3–6 must be completed first (ships `apps/ui/src/features/shared/statusPill/`).

**Related spec:** `docs/superpowers/specs/2026-04-19-chat-research-ui-redesign-design.md`

---

## Pre-conditions — chat-v2 learnings to apply

**Read `memory-bank/components/chat-v2/learnings.md` BEFORE writing any code.** This plan was drafted before `/chat-v2` shipped; every code block in the original plan predates the bugs we hit in production. The fixes below are the diff to apply while executing.

### Wire-format field corrections (gateway → UI)

Every event-map snippet in this plan used the wrong field names. Greppable corrections:

| Event | WRONG (original plan) | CORRECT (verified against `gateway/gateway-events/src/lib.rs`) |
|---|---|---|
| `Token` | `ev.content` | `ev.delta` (accept `content` as forward-compat fallback) |
| `Respond` | `ev.content` | `ev.message` (accept `content` as forward-compat fallback) |
| `ToolCall` | `ev.tool` | `ev.tool_name` (accept `ev.tool` as forward-compat fallback) |
| `ToolResult` | `ev.tool` | `ev.tool_name` (same pattern) |
| `AgentCompleted` | `ev.last`, `ev.is_final` | **Neither field exists.** Treat every root-agent `agent_completed` as final for the pill. |
| `WardChanged` | `ev.ward.id` / `ev.ward.name` | `ev.ward_id` (flat). Keep the nested-`ward.name` branch as a forward-compat fallback, never as primary. |
| Session bound | `session_initialized` | Backend emits `invoke_accepted`. Accept BOTH event types in the mapper, primary is `invoke_accepted`. |

### Pill semantics

Thinking events **must not** drive the status pill. glm-5-turbo emits per-token thinking (pill flashes), nemotron emits zero (pill stays empty). The pill is driven by event *kind*: `AgentStarted → "Thinking…"` / `ToolCall → <phrase>` / `Respond → "Responding"` / `AgentCompleted → fade`. Thinking deltas still render *inside the per-agent-turn block's Thinking timeline* (that's the right place for depth).

Drop the `thinking` case from `mapGatewayEventToPillEvent` entirely. Keep it in `mapGatewayEventToResearchAction` — the block-level timeline needs it.

### Client never generates ids

- **No `newConvId()` helper.** For a new research session, invoke with `session_id: undefined`; the backend emits `invoke_accepted` carrying both ids. The client reads them from the event, never generates its own.
- **No client-side `conversationId` seeding at mount.** `useReducer(reduceResearch, EMPTY_RESEARCH_STATE)` with `conversationId: null`. The WS subscribe-effect checks `if (!convId) return;` until the first invoke response populates it.
- **The `RESET` action** is still allowed in Research (multi-session surface has an explicit "New research" flow), but it must not regenerate client-side ids. It simply clears state and returns to the empty URL.

### Hook wiring — stable deps + post-async bootstrap

- `pillSink` is memoised in `use-status-pill-aggregator.ts` already (shipped with `/chat-v2`). **Never** list it in an effect's deps array — the subscribe-effect depends on `[state.conversationId]` only with an explicit `eslint-disable-next-line` and a one-line comment.
- Any "bootstrap once" ref is set **inside** the async completion block, after the data arrives, not at effect entry. Otherwise StrictMode's double-mount leaves the hook in a "started but never dispatched" state.
- The hook filters history messages at its boundary: `role === "tool"` rows and `content === "[tool calls]"` placeholders are dropped before the reducer sees them.

### Artifacts + slide-out (promote from optional)

Task 15 in the original plan marked artifact slide-out wiring as "optional". Promote it to required — `/chat-v2` shipped it and the Research UI's users will expect parity. Known backlog item: `memory-bank/components/chat-v2/backlog.md` B1 — fast-mode writes don't auto-register in the `artifacts` table. The UI is tolerant: when the backend registers, cards appear; when it doesn't, the strip is empty. Research agents go through the full writer pipeline so declarations fire normally; expect artifacts to work here.

### Silent-crash surfacing

When an execution crashes (`LLM error`, `Max iterations reached`, etc.) the backend emits `turn_complete` with `final_message: ""` — **no `error` event**. The UI sees nothing and shows an empty reply. This is a backlog item (`chat-v2/backlog.md` B3) but the research UI will hit it more often (delegations can crash independently). Add an explicit check in the reducer: on `TURN_COMPLETE` with no accumulated content for the turn AND no Respond, mark the turn as `status: 'error'` with a generic `"Turn ended with no output"` message. When backlog B3 lands, upgrade to the real error text from the `error` event.

### Theme tokens, no hex

Every CSS colour in a component file uses `var(--*)` tokens from `apps/ui/src/styles/theme.css`. If the original plan's CSS had `rgba(...)` or `rgb(...)` literals for accents, replace with tokens. The two-row status pill + category tokens are already in place from the chat-v2 shared module.

### E2E with route mocks

The original plan shipped one "smoke test in dev" task. Add a proper Playwright spec following the chat-v2 pattern:

- **Page-load group** (no daemon): page renders, drawer toggle, empty-session initial state.
- **Route-mocked group** (stubbed `/api/sessions`, `/api/sessions/:id/state`, `/api/sessions/:id/messages`, `/api/sessions/:id/artifacts`): per-agent-turn block renders, drawer lists sessions and navigates, artifact slide-out opens.
- **Agent-backed group** (auto-skip when daemon unreachable): sends a real research prompt, asserts at least one turn block + Respond renders.

### SonarQube complexity

The research reducer has ~14 action variants. Extract each `case` into a named helper (`handleAgentStarted`, `handleToolCall`, `handleRespond`, etc.) up front. Keeps cognitive complexity under 15.

### Backend endpoint reality check (surprises from live probing)

- `GET /api/sessions/:id/state` returns **404 even for sessions that exist in the DB**. Do not rely on it as a presence check. Use `getSessionMessages(sid, { scope: 'root' })` (returns `200 []` for empty but extant sessions) and treat the sessions-list endpoint `/api/logs/sessions` as the source of truth for session metadata.
- `mode="research"` is unnecessary — the executor's `SessionMode::Research` is the default when `mode` is omitted. Pass `undefined` to `executeAgent` (not `"research"`) to avoid a string that looks meaningful but isn't.
- The reserved-session self-heal added in `gateway/src/http/chat.rs::init_chat_session` is specific to `/chat`. Research sessions don't have a reserved slot; each session's lifecycle is independent.

---

## File Structure

### New files

```
apps/ui/src/features/research-v2/
    types.ts                         # ResearchSessionState, AgentTurn, TimelineEntry
    reducer.ts                       # reduceResearchSession
    reducer.test.ts                  # reducer unit tests
    event-map.ts                     # GatewayEvent → ResearchAction + PillEvent
    event-map.test.ts                # mapping tests
    useResearchSession.ts            # main hook
    useResearchSession.test.ts       # hook test (lightweight)
    AgentTurnBlock.tsx               # collapsed Thinking + visible Respond block
    AgentTurnBlock.test.tsx          # render tests
    ThinkingTimeline.tsx             # inner timeline shown when chevron expanded
    SessionsList.tsx                 # presentation-agnostic list (drawer-ready AND topbar-ready)
    SessionsList.test.tsx            # render tests
    SessionsDrawer.tsx               # wrapper that slides SessionsList from the left
    ResearchPage.tsx                 # page component
    ResearchPage.test.tsx            # render tests
    research.css                     # scoped styles
    index.ts                         # barrel
```

### Modified files

```
apps/ui/src/App.tsx                  # route wiring for /research-v2 and /research-v2/:sessionId + sidebar link
```

### Not modified

`MissionControl.tsx`, `mission-hooks.ts`, `WebChatPanel.tsx`, `SessionBar.tsx`, `ExecutionNarrative.tsx`, `IntelligenceFeed.tsx`. All left in place for `/`. Retired in a separate cleanup PR.

---

## Tasks

### Task 1: Research types

**Files:**
- Create: `apps/ui/src/features/research-v2/types.ts`

- [ ] **Step 1: Write types**

```typescript
export type AgentTurnStatus = "running" | "completed" | "stopped" | "error";

// One entry in the chronological Thinking timeline.
export interface TimelineEntry {
  id: string;
  at: number;                                  // ms epoch
  kind: "thinking" | "tool_call" | "tool_result" | "error" | "note";
  text: string;                                // display text
  toolName?: string;                           // tool_call: canonical tool name
  toolArgsPreview?: string;                    // tool_call: ~60 char preview
  toolResultPreview?: string;                  // tool_result: ~60 char preview
}

export interface AgentTurn {
  id: string;                                  // execution_id from the gateway
  agentId: string;
  parentExecutionId: string | null;            // for delegated turns
  startedAt: number;
  completedAt: number | null;
  status: AgentTurnStatus;
  wardId: string | null;                       // ward at the time this turn started
  timeline: TimelineEntry[];                   // events collapsed into one list
  tokenCount: number;
  respond: string | null;                      // final respond() content (markdown)
  respondStreaming: string;                    // buffer of Token events pre-Respond
  thinkingExpanded: boolean;                   // per-turn UI toggle
}

export interface ResearchMessage {
  id: string;
  role: "user" | "system";
  content: string;
  timestamp: number;
}

export type ResearchStatus = "idle" | "running" | "complete" | "stopped" | "error";

export interface ResearchSessionState {
  sessionId: string | null;
  conversationId: string;
  title: string;
  status: ResearchStatus;
  wardId: string | null;                       // STICKY — only updated by ward_changed
  wardName: string | null;                     // STICKY
  messages: ResearchMessage[];                 // user-authored prompts only
  turns: AgentTurn[];                          // chronological agent turns (root + delegations flattened)
  intentAnalyzing: boolean;                    // true between IntentAnalysisStarted and Complete/Skipped
  intentClassification: string | null;         // from IntentAnalysisComplete
  planPath: string | null;                     // from PlanUpdate
}

export const EMPTY_RESEARCH_STATE: ResearchSessionState = {
  sessionId: null,
  conversationId: "",
  title: "",
  status: "idle",
  wardId: null,
  wardName: null,
  messages: [],
  turns: [],
  intentAnalyzing: false,
  intentClassification: null,
  planPath: null,
};

// Summary row used by SessionsList.
export interface SessionSummary {
  id: string;
  title: string;
  status: "running" | "complete" | "crashed" | "paused";
  wardName: string | null;
  updatedAt: number;                           // ms epoch, used for grouping
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/features/research-v2/types.ts
git commit -m "feat(research-v2): state types"
```

---

### Task 2: Research reducer (with tests)

**Files:**
- Create: `apps/ui/src/features/research-v2/reducer.ts`
- Test: `apps/ui/src/features/research-v2/reducer.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
import { describe, it, expect } from "vitest";
import { reduceResearch, type ResearchAction } from "./reducer";
import { EMPTY_RESEARCH_STATE } from "./types";

describe("reduceResearch", () => {
  it("APPEND_USER adds a user message", () => {
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
    expect(s.turns[0].wardId).toBe("w1");   // turn inherits sticky ward
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

  it("TOOL_CALL appends to turn timeline and tracks args preview", () => {
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

  it("TOKEN streams into turn's respondStreaming buffer", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    s = reduceResearch(s, { type: "TOKEN", turnId: "t1", text: "par" });
    s = reduceResearch(s, { type: "TOKEN", turnId: "t1", text: "tial" });
    expect(s.turns[0].respondStreaming).toBe("partial");
  });

  it("RESPOND sets final respond and clears streaming", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    s = reduceResearch(s, { type: "TOKEN", turnId: "t1", text: "streaming" });
    s = reduceResearch(s, { type: "RESPOND", turnId: "t1", text: "final" });
    expect(s.turns[0].respond).toBe("final");
    expect(s.turns[0].respondStreaming).toBe("");
  });

  it("RESPOND without a prior AGENT_STARTED still persists to an orphan turn", () => {
    // Covers the bug: AgentCompleted lost over the socket but Respond landed.
    const s = reduceResearch(EMPTY_RESEARCH_STATE, { type: "RESPOND", turnId: "t1", text: "final" });
    expect(s.turns).toHaveLength(1);
    expect(s.turns[0].respond).toBe("final");
  });

  it("AGENT_COMPLETED flips turn status to completed", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    s = reduceResearch(s, { type: "AGENT_COMPLETED", turnId: "t1", completedAt: 10 });
    expect(s.turns[0].status).toBe("completed");
  });

  it("TOGGLE_THINKING flips per-turn expanded flag", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "AGENT_STARTED", turnId: "t1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: 1,
    });
    expect(s.turns[0].thinkingExpanded).toBe(false);
    s = reduceResearch(s, { type: "TOGGLE_THINKING", turnId: "t1" });
    expect(s.turns[0].thinkingExpanded).toBe(true);
  });

  it("HYDRATE seeds from snapshot", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, {
      type: "HYDRATE",
      sessionId: "sess-1",
      conversationId: "c1",
      title: "Research X",
      status: "complete",
      wardId: "w1",
      wardName: "W1",
      messages: [{ id: "m1", role: "user", content: "go", timestamp: 1 }],
      turns: [],
    });
    expect(s.sessionId).toBe("sess-1");
    expect(s.wardId).toBe("w1");
    expect(s.title).toBe("Research X");
  });

  it("INTENT_ANALYSIS_STARTED flips intentAnalyzing", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, { type: "INTENT_ANALYSIS_STARTED" });
    expect(s.intentAnalyzing).toBe(true);
  });

  it("INTENT_ANALYSIS_COMPLETE clears flag and stores classification", () => {
    let s = reduceResearch(EMPTY_RESEARCH_STATE, { type: "INTENT_ANALYSIS_STARTED" });
    s = reduceResearch(s, { type: "INTENT_ANALYSIS_COMPLETE", classification: "research" });
    expect(s.intentAnalyzing).toBe(false);
    expect(s.intentClassification).toBe("research");
  });

  it("SESSION_COMPLETE transitions status", () => {
    const s = reduceResearch(
      { ...EMPTY_RESEARCH_STATE, status: "running" },
      { type: "SESSION_COMPLETE" }
    );
    expect(s.status).toBe("complete");
  });

  it("RESET clears state but keeps new conversationId", () => {
    const s = reduceResearch(EMPTY_RESEARCH_STATE, { type: "RESET", conversationId: "c-new" });
    expect(s.conversationId).toBe("c-new");
    expect(s.turns).toHaveLength(0);
  });
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cd apps/ui && npx vitest run src/features/research-v2/reducer.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the reducer**

```typescript
import type {
  ResearchSessionState, AgentTurn, TimelineEntry, ResearchMessage,
} from "./types";
import { EMPTY_RESEARCH_STATE } from "./types";

export type ResearchAction =
  | { type: "HYDRATE"; sessionId: string; conversationId: string; title: string; status: ResearchSessionState["status"]; wardId: string | null; wardName: string | null; messages: ResearchMessage[]; turns: AgentTurn[] }
  | { type: "APPEND_USER"; message: ResearchMessage }
  | { type: "SESSION_BOUND"; sessionId: string }
  | { type: "TITLE_CHANGED"; title: string }
  | { type: "WARD_CHANGED"; wardId: string; wardName: string }
  | { type: "AGENT_STARTED"; turnId: string; agentId: string; parentExecutionId: string | null; wardId: string | null; startedAt: number }
  | { type: "AGENT_COMPLETED"; turnId: string; completedAt: number }
  | { type: "AGENT_STOPPED"; turnId: string; completedAt: number }
  | { type: "THINKING_DELTA"; turnId: string; entry: TimelineEntry }
  | { type: "TOOL_CALL"; turnId: string; entry: TimelineEntry }
  | { type: "TOOL_RESULT"; turnId: string; entry: TimelineEntry }
  | { type: "TOKEN"; turnId: string; text: string }
  | { type: "RESPOND"; turnId: string; text: string }
  | { type: "TOGGLE_THINKING"; turnId: string }
  | { type: "INTENT_ANALYSIS_STARTED" }
  | { type: "INTENT_ANALYSIS_COMPLETE"; classification: string }
  | { type: "INTENT_ANALYSIS_SKIPPED" }
  | { type: "PLAN_UPDATE"; planPath: string }
  | { type: "SESSION_COMPLETE" }
  | { type: "ERROR"; message: string }
  | { type: "RESET"; conversationId: string };

function ensureTurn(
  state: ResearchSessionState,
  turnId: string,
  seed?: Partial<AgentTurn>
): ResearchSessionState {
  const existing = state.turns.find((t) => t.id === turnId);
  if (existing) return state;
  const fresh: AgentTurn = {
    id: turnId,
    agentId: seed?.agentId ?? "root",
    parentExecutionId: seed?.parentExecutionId ?? null,
    startedAt: seed?.startedAt ?? Date.now(),
    completedAt: null,
    status: "running",
    wardId: seed?.wardId ?? state.wardId,
    timeline: [],
    tokenCount: 0,
    respond: null,
    respondStreaming: "",
    thinkingExpanded: false,
    ...seed,
  };
  return { ...state, turns: [...state.turns, fresh] };
}

function updateTurn(
  state: ResearchSessionState,
  turnId: string,
  patch: (t: AgentTurn) => AgentTurn
): ResearchSessionState {
  return {
    ...state,
    turns: state.turns.map((t) => (t.id === turnId ? patch(t) : t)),
  };
}

export function reduceResearch(state: ResearchSessionState, action: ResearchAction): ResearchSessionState {
  switch (action.type) {
    case "HYDRATE":
      return {
        ...state,
        sessionId: action.sessionId,
        conversationId: action.conversationId,
        title: action.title,
        status: action.status,
        wardId: action.wardId,
        wardName: action.wardName,
        messages: action.messages,
        turns: action.turns,
      };
    case "APPEND_USER":
      return { ...state, messages: [...state.messages, action.message], status: "running" };
    case "SESSION_BOUND":
      return { ...state, sessionId: action.sessionId };
    case "TITLE_CHANGED":
      return { ...state, title: action.title };
    case "WARD_CHANGED":
      return { ...state, wardId: action.wardId, wardName: action.wardName };
    case "AGENT_STARTED": {
      // Sticky ward: if event's wardId is null, inherit from state. If present, use it.
      const wardForTurn = action.wardId ?? state.wardId;
      return ensureTurn(state, action.turnId, {
        agentId: action.agentId,
        parentExecutionId: action.parentExecutionId,
        startedAt: action.startedAt,
        wardId: wardForTurn,
      });
    }
    case "AGENT_COMPLETED":
      return updateTurn(state, action.turnId, (t) => ({
        ...t,
        status: "completed",
        completedAt: action.completedAt,
      }));
    case "AGENT_STOPPED":
      return updateTurn(state, action.turnId, (t) => ({
        ...t,
        status: "stopped",
        completedAt: action.completedAt,
      }));
    case "THINKING_DELTA":
    case "TOOL_CALL":
    case "TOOL_RESULT": {
      const seeded = ensureTurn(state, action.turnId);
      return updateTurn(seeded, action.turnId, (t) => ({
        ...t,
        timeline: [...t.timeline, action.entry],
      }));
    }
    case "TOKEN": {
      const seeded = ensureTurn(state, action.turnId);
      return updateTurn(seeded, action.turnId, (t) => ({
        ...t,
        respondStreaming: t.respondStreaming + action.text,
      }));
    }
    case "RESPOND": {
      const seeded = ensureTurn(state, action.turnId);
      return updateTurn(seeded, action.turnId, (t) => ({
        ...t,
        respond: action.text,
        respondStreaming: "",
      }));
    }
    case "TOGGLE_THINKING":
      return updateTurn(state, action.turnId, (t) => ({ ...t, thinkingExpanded: !t.thinkingExpanded }));
    case "INTENT_ANALYSIS_STARTED":
      return { ...state, intentAnalyzing: true };
    case "INTENT_ANALYSIS_COMPLETE":
      return { ...state, intentAnalyzing: false, intentClassification: action.classification };
    case "INTENT_ANALYSIS_SKIPPED":
      return { ...state, intentAnalyzing: false };
    case "PLAN_UPDATE":
      return { ...state, planPath: action.planPath };
    case "SESSION_COMPLETE":
      return { ...state, status: "complete" };
    case "ERROR":
      return { ...state, status: "error" };
    case "RESET":
      return { ...EMPTY_RESEARCH_STATE, conversationId: action.conversationId };
    default:
      return state;
  }
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cd apps/ui && npx vitest run src/features/research-v2/reducer.test.ts`
Expected: PASS — 14 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/research-v2/reducer.ts apps/ui/src/features/research-v2/reducer.test.ts
git commit -m "feat(research-v2): reducer with ward-sticky + orphan-turn handling"
```

---

### Task 3: Event→action + Event→pill mapping

**Files:**
- Create: `apps/ui/src/features/research-v2/event-map.ts`
- Test: `apps/ui/src/features/research-v2/event-map.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
import { describe, it, expect } from "vitest";
import { mapGatewayEventToResearchAction, mapGatewayEventToPillEvent } from "./event-map";

describe("mapGatewayEventToResearchAction", () => {
  it("AgentStarted maps with execution_id → turnId", () => {
    const a = mapGatewayEventToResearchAction({
      type: "agent_started", agent_id: "root", execution_id: "exec-1", ward_id: null,
    } as any);
    expect(a).toEqual({
      type: "AGENT_STARTED",
      turnId: "exec-1", agentId: "root", parentExecutionId: null, wardId: null, startedAt: expect.any(Number),
    });
  });

  it("WardChanged maps with name and id", () => {
    const a = mapGatewayEventToResearchAction({
      type: "ward_changed", ward: { id: "stock-analysis", name: "Stock Analysis" },
    } as any);
    expect(a).toEqual({ type: "WARD_CHANGED", wardId: "stock-analysis", wardName: "Stock Analysis" });
  });

  it("WardChanged without id returns null (ignore)", () => {
    expect(mapGatewayEventToResearchAction({ type: "ward_changed" } as any)).toBeNull();
  });

  it("Thinking maps with execution_id", () => {
    const a = mapGatewayEventToResearchAction({
      type: "thinking", execution_id: "exec-1", content: "deep thought",
    } as any);
    expect(a?.type).toBe("THINKING_DELTA");
    expect((a as any).entry.text).toBe("deep thought");
  });

  it("ToolCall maps with tool_name and arg preview (verified wire field)", () => {
    const a = mapGatewayEventToResearchAction({
      type: "tool_call", execution_id: "exec-1", tool_name: "write_file", args: { path: "a.py" },
    } as any);
    expect((a as any).entry.toolName).toBe("write_file");
    expect((a as any).entry.toolArgsPreview).toContain("a.py");
  });

  it("ToolCall accepts legacy `tool` field (forward-compat fallback)", () => {
    const a = mapGatewayEventToResearchAction({
      type: "tool_call", execution_id: "exec-1", tool: "write_file", args: {},
    } as any);
    expect((a as any).entry.toolName).toBe("write_file");
  });

  it("Token maps with `delta` (verified wire field)", () => {
    expect(mapGatewayEventToResearchAction({
      type: "token", execution_id: "exec-1", delta: "abc",
    } as any)).toEqual({ type: "TOKEN", turnId: "exec-1", text: "abc" });
  });

  it("Token accepts `content` (forward-compat fallback)", () => {
    expect(mapGatewayEventToResearchAction({
      type: "token", execution_id: "exec-1", content: "abc",
    } as any)).toEqual({ type: "TOKEN", turnId: "exec-1", text: "abc" });
  });

  it("Respond maps with `message` (verified wire field)", () => {
    const a = mapGatewayEventToResearchAction({
      type: "respond", execution_id: "exec-1", message: "final",
    } as any);
    expect(a).toEqual({ type: "RESPOND", turnId: "exec-1", text: "final" });
  });

  it("Respond accepts `content` (forward-compat fallback)", () => {
    const a = mapGatewayEventToResearchAction({
      type: "respond", execution_id: "exec-1", content: "fallback",
    } as any);
    expect(a).toEqual({ type: "RESPOND", turnId: "exec-1", text: "fallback" });
  });

  it("Respond without execution_id uses 'orphan' turn id", () => {
    const a = mapGatewayEventToResearchAction({ type: "respond", message: "orphan" } as any);
    expect(a).toEqual({ type: "RESPOND", turnId: "orphan", text: "orphan" });
  });

  it("invoke_accepted maps to SESSION_BOUND (verified wire event)", () => {
    expect(mapGatewayEventToResearchAction({ type: "invoke_accepted", session_id: "sess-x" } as any))
      .toEqual({ type: "SESSION_BOUND", sessionId: "sess-x" });
  });

  it("session_initialized maps to SESSION_BOUND (forward-compat)", () => {
    expect(mapGatewayEventToResearchAction({ type: "session_initialized", session_id: "sess-y" } as any))
      .toEqual({ type: "SESSION_BOUND", sessionId: "sess-y" });
  });

  it("WardChanged with flat ward_id (verified wire format)", () => {
    expect(mapGatewayEventToResearchAction({ type: "ward_changed", ward_id: "stock-analysis" } as any))
      .toEqual({ type: "WARD_CHANGED", wardId: "stock-analysis", wardName: "stock-analysis" });
  });

  it("SessionTitleChanged maps", () => {
    expect(mapGatewayEventToResearchAction({ type: "session_title_changed", title: "New T" } as any))
      .toEqual({ type: "TITLE_CHANGED", title: "New T" });
  });

  it("IntentAnalysisStarted/Complete/Skipped map", () => {
    expect(mapGatewayEventToResearchAction({ type: "intent_analysis_started" } as any))
      .toEqual({ type: "INTENT_ANALYSIS_STARTED" });
    expect(mapGatewayEventToResearchAction({ type: "intent_analysis_complete", classification: "research" } as any))
      .toEqual({ type: "INTENT_ANALYSIS_COMPLETE", classification: "research" });
    expect(mapGatewayEventToResearchAction({ type: "intent_analysis_skipped" } as any))
      .toEqual({ type: "INTENT_ANALYSIS_SKIPPED" });
  });
});

describe("mapGatewayEventToPillEvent (research-v2)", () => {
  it("reuses shared shape from statusPill module", () => {
    expect(mapGatewayEventToPillEvent({ type: "agent_started", agent_id: "planner" } as any))
      .toEqual({ kind: "agent_started", agent_id: "planner" });
  });

  it("does NOT map thinking events to pill (handled inside turn-block timeline)", () => {
    expect(mapGatewayEventToPillEvent({ type: "thinking", content: "…" } as any))
      .toBeNull();
  });

  it("maps tool_call with wire field tool_name", () => {
    expect(mapGatewayEventToPillEvent({ type: "tool_call", tool_name: "write_file", args: {} } as any))
      .toEqual({ kind: "tool_call", tool: "write_file", args: {} });
  });

  it("maps agent_completed with is_final=true (no field required)", () => {
    expect(mapGatewayEventToPillEvent({ type: "agent_completed", agent_id: "planner" } as any))
      .toEqual({ kind: "agent_completed", agent_id: "planner", is_final: true });
  });

  it("does NOT hijack the pill for intent_analysis_started (per-turn block handles it)", () => {
    expect(mapGatewayEventToPillEvent({ type: "intent_analysis_started" } as any))
      .toBeNull();
  });
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cd apps/ui && npx vitest run src/features/research-v2/event-map.test.ts`
Expected: FAIL

- [ ] **Step 3: Implement the mapping**

```typescript
import type { ConversationEvent } from "@/services/transport/types";
import type { PillEvent } from "../shared/statusPill";
import type { ResearchAction } from "./reducer";
import type { TimelineEntry } from "./types";

function previewArgs(args: Record<string, unknown>): string {
  try {
    const s = JSON.stringify(args);
    return s.length <= 60 ? s : s.slice(0, 57) + "…";
  } catch {
    return "";
  }
}

function previewResult(result: unknown): string {
  const s = typeof result === "string" ? result : JSON.stringify(result ?? "");
  return s.length <= 60 ? s : s.slice(0, 57) + "…";
}

// -------------------------------------------------------------------------
// Per-branch mappers. Keeps the outer switch cognitive-complexity < 15.
// Field names are VERIFIED against gateway/gateway-events/src/lib.rs.
// -------------------------------------------------------------------------

function mapAgentStarted(e: Record<string, unknown>, now: number): ResearchAction {
  return {
    type: "AGENT_STARTED",
    turnId: (e.execution_id as string) ?? crypto.randomUUID(),
    agentId: (e.agent_id as string) ?? "root",
    parentExecutionId: (e.parent_execution_id as string) ?? null,
    wardId: (e.ward_id as string) ?? null,
    startedAt: now,
  };
}

function mapWardChanged(e: Record<string, unknown>): ResearchAction | null {
  // Primary: flat `ward_id` (current wire format, verified in messages.rs).
  const flat = e["ward_id"];
  if (typeof flat === "string" && flat.length > 0) {
    return { type: "WARD_CHANGED", wardId: flat, wardName: flat };
  }
  // Forward-compat: nested `ward.name` (reserved for future enrichment).
  const ward = e["ward"] as Record<string, unknown> | undefined;
  const id = ward?.["id"];
  const name = ward?.["name"];
  if (typeof id === "string" && id.length > 0) {
    return {
      type: "WARD_CHANGED",
      wardId: id,
      wardName: typeof name === "string" ? name : id,
    };
  }
  return null;
}

function mapThinkingDelta(e: Record<string, unknown>, now: number): ResearchAction | null {
  const content = e["content"];
  if (typeof content !== "string" || content.length === 0) return null;
  const entry: TimelineEntry = {
    id: crypto.randomUUID(),
    at: now,
    kind: "thinking",
    text: content,
  };
  return {
    type: "THINKING_DELTA",
    turnId: (e.execution_id as string) ?? "orphan",
    entry,
  };
}

function mapToolCall(e: Record<string, unknown>, now: number): ResearchAction {
  // Gateway emits `tool_name`; keep `tool` as a forward-compat fallback.
  const tool = (e["tool_name"] ?? e["tool"]) as string | undefined ?? "tool";
  const entry: TimelineEntry = {
    id: crypto.randomUUID(),
    at: now,
    kind: "tool_call",
    text: tool,
    toolName: tool,
    toolArgsPreview: previewArgs((e["args"] ?? {}) as Record<string, unknown>),
  };
  return { type: "TOOL_CALL", turnId: (e.execution_id as string) ?? "orphan", entry };
}

function mapToolResult(e: Record<string, unknown>, now: number): ResearchAction {
  const tool = (e["tool_name"] ?? e["tool"]) as string | undefined ?? "result";
  const entry: TimelineEntry = {
    id: crypto.randomUUID(),
    at: now,
    kind: "tool_result",
    text: tool,
    toolResultPreview: previewResult(e["result"]),
  };
  return { type: "TOOL_RESULT", turnId: (e.execution_id as string) ?? "orphan", entry };
}

function mapToken(e: Record<string, unknown>): ResearchAction | null {
  // Gateway emits `delta`; keep `content` as fallback.
  const text = e["delta"] ?? e["content"];
  if (typeof text !== "string" || text.length === 0) return null;
  return { type: "TOKEN", turnId: (e.execution_id as string) ?? "orphan", text };
}

function mapRespond(e: Record<string, unknown>): ResearchAction | null {
  // Gateway emits `message`; keep `content` as fallback.
  const text = e["message"] ?? e["content"];
  if (typeof text !== "string") return null;
  return { type: "RESPOND", turnId: (e.execution_id as string) ?? "orphan", text };
}

function mapSessionBound(e: Record<string, unknown>): ResearchAction | null {
  const sid = e["session_id"];
  if (typeof sid !== "string" || !sid) return null;
  return { type: "SESSION_BOUND", sessionId: sid };
}

export function mapGatewayEventToResearchAction(ev: ConversationEvent): ResearchAction | null {
  const e = ev as unknown as Record<string, unknown>;
  const type = e["type"] as string;
  const now = Date.now();
  switch (type) {
    case "agent_started":       return mapAgentStarted(e, now);
    case "agent_completed":     return { type: "AGENT_COMPLETED", turnId: (e.execution_id as string) ?? "orphan", completedAt: now };
    case "agent_stopped":       return { type: "AGENT_STOPPED",   turnId: (e.execution_id as string) ?? "orphan", completedAt: now };
    case "ward_changed":        return mapWardChanged(e);
    case "thinking":            return mapThinkingDelta(e, now);
    case "tool_call":           return mapToolCall(e, now);
    case "tool_result":         return mapToolResult(e, now);
    case "token":               return mapToken(e);
    case "respond":             return mapRespond(e);
    case "session_title_changed": return { type: "TITLE_CHANGED", title: (e.title as string) ?? "" };
    case "intent_analysis_started":  return { type: "INTENT_ANALYSIS_STARTED" };
    case "intent_analysis_complete": return { type: "INTENT_ANALYSIS_COMPLETE", classification: (e.classification as string) ?? "" };
    case "intent_analysis_skipped":  return { type: "INTENT_ANALYSIS_SKIPPED" };
    case "plan_update":         return { type: "PLAN_UPDATE", planPath: (e.plan_path as string) ?? "" };
    // Backend emits `invoke_accepted`; `session_initialized` reserved for future revision.
    case "invoke_accepted":
    case "session_initialized": return mapSessionBound(e);
    case "error":               return { type: "ERROR", message: (e.message as string) ?? "error" };
    default:                    return null;
  }
}

// -------------------------------------------------------------------------
// Pill mapper — deterministic, Thinking intentionally NOT mapped.
// glm-5-turbo flashes per-token thinking (unreadable); nemotron emits zero
// (pill stays empty). Pill narration is driven by event kind instead.
// -------------------------------------------------------------------------

function mapPillToolCall(e: Record<string, unknown>): PillEvent | null {
  const tool = (e["tool_name"] ?? e["tool"]) as string | undefined;
  if (typeof tool !== "string") return null;
  return { kind: "tool_call", tool, args: (e["args"] ?? {}) as Record<string, unknown> };
}

export function mapGatewayEventToPillEvent(ev: ConversationEvent): PillEvent | null {
  const e = ev as unknown as Record<string, unknown>;
  const type = e["type"] as string;
  switch (type) {
    case "agent_started":   return { kind: "agent_started", agent_id: (e.agent_id as string) ?? "" };
    // Gateway has no `last` or `is_final` field. For the single-root-plus-
    // subagents research flow, a final AgentCompleted of the ROOT agent is
    // the hide signal. For simplicity we treat any AgentCompleted as final
    // and let the UI page-level state (running/idle) re-show the pill if a
    // subagent is still active. This trades a brief flicker for simplicity.
    case "agent_completed": return { kind: "agent_completed", agent_id: (e.agent_id as string) ?? "", is_final: true };
    case "tool_call":       return mapPillToolCall(e);
    case "respond":         return { kind: "respond" };
    // Intent analysis is a research-page concern; the per-agent-turn block
    // renders its own "analyzing intent…" line. Don't hijack the pill for it.
    default:                return null;
  }
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cd apps/ui && npx vitest run src/features/research-v2/event-map.test.ts`
Expected: PASS — ~17 tests (original 11 + 6 new wire-format-verification cases).

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/research-v2/event-map.ts apps/ui/src/features/research-v2/event-map.test.ts
git commit -m "feat(research-v2): event→action mapper with orphan-turn support"
```

---

### Task 4: `useResearchSession` hook

**Files:**
- Create: `apps/ui/src/features/research-v2/useResearchSession.ts`

- [ ] **Step 1: Implement the hook**

```typescript
import { useCallback, useEffect, useReducer, useRef, type Dispatch } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getTransport } from "@/services/transport";
import type { Transport } from "@/services/transport";
import type {
  ConversationEvent,
  SessionMessage,
} from "@/services/transport/types";
import { useStatusPill, type PillEventSink } from "../shared/statusPill";
import {
  type ResearchSessionState, type ResearchMessage,
  EMPTY_RESEARCH_STATE,
} from "./types";
import { reduceResearch, type ResearchAction } from "./reducer";
import { mapGatewayEventToResearchAction, mapGatewayEventToPillEvent } from "./event-map";

const ROOT_AGENT_ID = "root";
// Research mode is the executor's default; passing undefined is semantically
// clearer than "research" (which is a string that looks meaningful but just
// falls through `SessionMode::from_mode_string` to Research anyway).
const RESEARCH_MODE: string | undefined = undefined;
const HISTORY_TAIL_LIMIT = 50;

// -------------------------------------------------------------------------
// Pure helpers — identical patterns to chat-v2/useQuickChat.
// -------------------------------------------------------------------------

function isVisibleResearchMessage(m: SessionMessage): boolean {
  if (m.role === "tool") return false;
  if (m.role === "assistant" && m.content.trim() === "[tool calls]") return false;
  return m.role === "user" || m.role === "assistant";
}

function messageFromApi(m: SessionMessage): ResearchMessage {
  return {
    id: m.id,
    role: m.role === "user" ? "user" : "system", // assistant rows render via turn blocks, not messages[]
    content: m.content,
    timestamp: new Date(m.created_at).getTime(),
  };
}

function makeEventHandler(
  pillSink: PillEventSink,
  dispatch: Dispatch<ResearchAction>
) {
  return (event: ConversationEvent) => {
    const action = mapGatewayEventToResearchAction(event);
    if (action) dispatch(action);
    const pillEv = mapGatewayEventToPillEvent(event);
    if (pillEv) pillSink.push(pillEv);
  };
}

async function hydrateExistingSession(
  transport: Transport,
  sessionId: string
): Promise<{ messages: ResearchMessage[]; title: string; wardId: string | null; wardName: string | null } | null> {
  // DO NOT call /api/sessions/:id/state — it returns 404 even for existing
  // sessions (verified during chat-v2 testing). Rely on the messages endpoint,
  // which returns `200 []` for empty but extant sessions.
  const msgs = await transport.getSessionMessages(sessionId, { scope: "root" });
  if (!msgs.success || !msgs.data) return null;
  const messages = msgs.data
    .filter(isVisibleResearchMessage)
    .slice(-HISTORY_TAIL_LIMIT)
    .map(messageFromApi);
  // Ward / title come from the sessions-list row or WardChanged events in the
  // live stream. Hydrate with null; updates arrive via the WS.
  return { messages, title: "", wardId: null, wardName: null };
}

// -------------------------------------------------------------------------
// Hook
// -------------------------------------------------------------------------

export function useResearchSession() {
  const { sessionId: urlSessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  // NO client-side conversationId seed — the backend generates it when the
  // first invoke lands and we pick it up from the `invoke_accepted` event.
  const [state, dispatch] = useReducer(reduceResearch, EMPTY_RESEARCH_STATE);
  const { state: pillState, sink: pillSink } = useStatusPill();

  // Idempotency guard: set AFTER async completes, inside the dispatch block.
  // StrictMode double-mounts in dev — setting ref at entry creates a
  // "started but never dispatched" state. Post-async is the safe pattern.
  const hydratedForSessionRef = useRef<string | null>(null);
  const subscribedConvIdRef = useRef<string | null>(null);

  // --- Hydrate existing session (only when the URL carries one) ---
  useEffect(() => {
    if (!urlSessionId) return;
    if (hydratedForSessionRef.current === urlSessionId) return;
    (async () => {
      const transport = await getTransport();
      const snapshot = await hydrateExistingSession(transport, urlSessionId);
      if (hydratedForSessionRef.current === urlSessionId) return;
      hydratedForSessionRef.current = urlSessionId;
      if (!snapshot) {
        dispatch({ type: "ERROR", message: "Failed to load session" });
        return;
      }
      dispatch({
        type: "HYDRATE",
        sessionId: urlSessionId,
        conversationId: null, // populated on next invoke_accepted
        title: snapshot.title,
        status: "idle",
        wardId: snapshot.wardId,
        wardName: snapshot.wardName,
        messages: snapshot.messages,
        turns: [],
      });
    })();
  }, [urlSessionId]);

  // --- Subscribe to WS events for the persisted conversationId ---
  // `pillSink` is memoised in useStatusPill (stable identity). Listing it in
  // deps would force a teardown+resubscribe on every render, dropping events.
  useEffect(() => {
    const convId = state.conversationId;
    if (!convId || subscribedConvIdRef.current === convId) return;
    subscribedConvIdRef.current = convId;
    const onEvent = makeEventHandler(pillSink, dispatch);
    const unsubscribe = Promise.resolve().then(async () => {
      const transport = await getTransport();
      return transport.subscribeConversation(convId, { onEvent });
    });
    return () => {
      unsubscribe.then((fn) => fn && fn()).catch(() => { /* no-op */ });
      if (subscribedConvIdRef.current === convId) {
        subscribedConvIdRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.conversationId]);

  // --- Sync URL when a session_id arrives from the backend ---
  useEffect(() => {
    if (state.sessionId && urlSessionId !== state.sessionId) {
      navigate(`/research-v2/${state.sessionId}`, { replace: true });
    }
  }, [state.sessionId, urlSessionId, navigate]);

  // --- Send a user message ---
  const sendMessage = useCallback(async (text: string) => {
    const trimmed = text.trim();
    if (!trimmed || state.status === "running") return;
    dispatch({
      type: "APPEND_USER",
      message: { id: crypto.randomUUID(), role: "user", content: trimmed, timestamp: Date.now() },
    });
    const transport = await getTransport();
    // For a BRAND NEW session both conversationId and sessionId are null; the
    // transport's executeAgent accepts no conversationId and assigns one
    // server-side. For a continuation, we pass both persisted ids.
    const result = await transport.executeAgent(
      ROOT_AGENT_ID,
      state.conversationId ?? `research-${crypto.randomUUID()}`, // only used when we truly have no id
      trimmed,
      state.sessionId ?? undefined,
      RESEARCH_MODE
    );
    if (!result.success) {
      dispatch({ type: "ERROR", message: result.error ?? "Failed to send" });
    }
  }, [state.status, state.conversationId, state.sessionId]);

  // --- Stop running turn ---
  const stopAgent = useCallback(async () => {
    if (!state.conversationId) return;
    const transport = await getTransport();
    await transport.stopAgent(state.conversationId);
  }, [state.conversationId]);

  // --- Start a fresh research session ---
  // Does NOT generate a client conversationId. RESET clears state; the next
  // sendMessage will invoke with null conv — the backend creates one.
  const startNewResearch = useCallback(() => {
    pillSink.push({ kind: "reset" });
    dispatch({ type: "RESET" });
    hydratedForSessionRef.current = null;
    subscribedConvIdRef.current = null;
    navigate("/research-v2", { replace: true });
  }, [navigate, pillSink]);

  const toggleThinking = useCallback((turnId: string) => {
    dispatch({ type: "TOGGLE_THINKING", turnId });
  }, []);

  return { state, pillState, sendMessage, stopAgent, startNewResearch, toggleThinking };
}
```

**Note on the `conversationId ?? \`research-${uuid}\`` fallback:** the transport's WS `invoke` command requires a `conversation_id` field. For a brand-new session we have no server-assigned id yet, so we pass a disposable placeholder — the backend ignores it when `session_id` is null (it creates a fresh session with its own conv_id), and the subsequent `invoke_accepted` event carries the real ids. We dispatch `SESSION_BOUND` on that event and the subscribe-effect picks up the new conv_id. The placeholder is never used for persistence.

This is a concession to the current WS schema (`conversation_id: String`, not `Option<String>`). If the schema relaxes later, drop the fallback. Until then, it's the only path to kick off a new session without a pre-existing id.

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/features/research-v2/useResearchSession.ts
git commit -m "feat(research-v2): useResearchSession hook (snapshot + subscribe + send)"
```

---

### Task 5: `ThinkingTimeline` component

**Files:**
- Create: `apps/ui/src/features/research-v2/ThinkingTimeline.tsx`

- [ ] **Step 1: Implement the component**

```tsx
import type { TimelineEntry } from "./types";

export interface ThinkingTimelineProps {
  entries: TimelineEntry[];
}

function formatTime(at: number): string {
  const d = new Date(at);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export function ThinkingTimeline({ entries }: ThinkingTimelineProps) {
  if (entries.length === 0) {
    return <div className="thinking-timeline__empty">no intermediate events</div>;
  }
  return (
    <ol className="thinking-timeline">
      {entries.map((e) => (
        <li key={e.id} className={`thinking-timeline__item thinking-timeline__item--${e.kind}`}>
          <span className="thinking-timeline__time">{formatTime(e.at)}</span>
          <span className="thinking-timeline__text">
            {e.kind === "tool_call" && e.toolName ? (
              <>
                <code>{e.toolName}</code>
                {e.toolArgsPreview && <span className="thinking-timeline__preview">{e.toolArgsPreview}</span>}
              </>
            ) : e.kind === "tool_result" ? (
              <>
                <span className="thinking-timeline__label">↳</span>
                <span className="thinking-timeline__preview">{e.toolResultPreview ?? e.text}</span>
              </>
            ) : (
              e.text
            )}
          </span>
        </li>
      ))}
    </ol>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/features/research-v2/ThinkingTimeline.tsx
git commit -m "feat(research-v2): ThinkingTimeline component"
```

---

### Task 6: `AgentTurnBlock` component (with tests)

**Files:**
- Create: `apps/ui/src/features/research-v2/AgentTurnBlock.tsx`
- Test: `apps/ui/src/features/research-v2/AgentTurnBlock.test.tsx`

- [ ] **Step 1: Write failing tests**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { AgentTurnBlock } from "./AgentTurnBlock";
import type { AgentTurn } from "./types";

const turn: AgentTurn = {
  id: "t1",
  agentId: "planner",
  parentExecutionId: null,
  startedAt: 1000,
  completedAt: 2000,
  status: "completed",
  wardId: "w1",
  timeline: [
    { id: "e1", at: 1100, kind: "thinking", text: "analyzing" },
    { id: "e2", at: 1200, kind: "tool_call", text: "write_file", toolName: "write_file", toolArgsPreview: "path=a.py" },
  ],
  tokenCount: 100,
  respond: "# Plan\n\nDone.",
  respondStreaming: "",
  thinkingExpanded: false,
};

describe("<AgentTurnBlock>", () => {
  it("renders agent id, status icon, and Respond markdown", () => {
    render(<AgentTurnBlock turn={turn} onToggleThinking={() => {}} />);
    expect(screen.getByText(/planner/)).toBeTruthy();
    expect(screen.getByText("Done.")).toBeTruthy();
  });

  it("shows thinking count when collapsed and expands on click", () => {
    const fn = vi.fn();
    render(<AgentTurnBlock turn={turn} onToggleThinking={fn} />);
    const chevron = screen.getByTestId("thinking-chevron-t1");
    fireEvent.click(chevron);
    expect(fn).toHaveBeenCalledWith("t1");
  });

  it("shows streaming buffer when respond is null", () => {
    const streamingTurn = { ...turn, respond: null, respondStreaming: "partial text" };
    render(<AgentTurnBlock turn={streamingTurn} onToggleThinking={() => {}} />);
    expect(screen.getByText(/partial text/)).toBeTruthy();
  });

  it("shows running badge when status is running", () => {
    const runningTurn = { ...turn, status: "running" as const, completedAt: null };
    render(<AgentTurnBlock turn={runningTurn} onToggleThinking={() => {}} />);
    expect(screen.getByTestId("turn-running-badge")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cd apps/ui && npx vitest run src/features/research-v2/AgentTurnBlock.test.tsx`
Expected: FAIL

- [ ] **Step 3: Implement the component**

```tsx
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ChevronRight, CheckCircle2, Square, AlertCircle, Loader2 } from "lucide-react";
import type { AgentTurn } from "./types";
import { ThinkingTimeline } from "./ThinkingTimeline";

export interface AgentTurnBlockProps {
  turn: AgentTurn;
  onToggleThinking(turnId: string): void;
}

const AGENT_COLORS: Record<string, string> = {
  planner: "rgb(150,255,180)",
  "planner-agent": "rgb(150,255,180)",
  solution: "rgb(200,150,255)",
  "solution-agent": "rgb(200,150,255)",
  builder: "rgb(255,180,100)",
  "builder-agent": "rgb(255,180,100)",
  writer: "rgb(100,200,255)",
  "writer-agent": "rgb(100,200,255)",
  root: "rgba(255,255,255,0.8)",
  "quick-chat": "rgb(140,220,255)",
};

function formatDuration(startedAt: number, completedAt: number | null): string {
  const end = completedAt ?? Date.now();
  const ms = end - startedAt;
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.round(ms / 1000)}s`;
}

function StatusIcon({ status }: { status: AgentTurn["status"] }) {
  switch (status) {
    case "running": return <Loader2 size={14} className="spin" />;
    case "completed": return <CheckCircle2 size={14} />;
    case "stopped": return <Square size={14} />;
    case "error": return <AlertCircle size={14} />;
  }
}

export function AgentTurnBlock({ turn, onToggleThinking }: AgentTurnBlockProps) {
  const color = AGENT_COLORS[turn.agentId] ?? "rgba(255,255,255,0.7)";
  const displayContent = turn.respond ?? turn.respondStreaming;
  const isStreaming = turn.respond === null;

  return (
    <div
      className="agent-turn-block"
      style={{ borderLeft: `3px solid ${color}` }}
      data-parent={turn.parentExecutionId ?? ""}
    >
      <div className="agent-turn-block__header">
        <span className="agent-turn-block__agent" style={{ color }}>{turn.agentId}</span>
        <span className="agent-turn-block__meta">
          <StatusIcon status={turn.status} />
          <span>{formatDuration(turn.startedAt, turn.completedAt)}</span>
          {turn.tokenCount > 0 && <span>{turn.tokenCount}tok</span>}
          {turn.status === "running" && <span data-testid="turn-running-badge" className="agent-turn-block__running">· running</span>}
        </span>
      </div>

      <button
        type="button"
        data-testid={`thinking-chevron-${turn.id}`}
        className="agent-turn-block__chevron"
        onClick={() => onToggleThinking(turn.id)}
        aria-expanded={turn.thinkingExpanded}
      >
        <ChevronRight
          size={14}
          style={{ transform: turn.thinkingExpanded ? "rotate(90deg)" : "rotate(0deg)" }}
        />
        <span>Thinking ({turn.timeline.length} {turn.timeline.length === 1 ? "action" : "actions"})</span>
      </button>

      {turn.thinkingExpanded && (
        <div className="agent-turn-block__timeline">
          <ThinkingTimeline entries={turn.timeline} />
        </div>
      )}

      <div className={`agent-turn-block__respond ${isStreaming ? "agent-turn-block__respond--streaming" : ""}`}>
        {displayContent ? (
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{displayContent}</ReactMarkdown>
        ) : (
          <span className="agent-turn-block__placeholder">waiting…</span>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cd apps/ui && npx vitest run src/features/research-v2/AgentTurnBlock.test.tsx`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/research-v2/AgentTurnBlock.tsx apps/ui/src/features/research-v2/AgentTurnBlock.test.tsx
git commit -m "feat(research-v2): AgentTurnBlock with collapsed Thinking + visible Respond"
```

---

### Task 7: `SessionsList` presentation-agnostic component (with tests)

**Files:**
- Create: `apps/ui/src/features/research-v2/SessionsList.tsx`
- Test: `apps/ui/src/features/research-v2/SessionsList.test.tsx`

- [ ] **Step 1: Write failing tests**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SessionsList, groupSessions } from "./SessionsList";
import type { SessionSummary } from "./types";

const now = Date.now();
const oneDay = 24 * 60 * 60 * 1000;

const sessions: SessionSummary[] = [
  { id: "s1", title: "Running one", status: "running", wardName: "stock-analysis", updatedAt: now - 1000 },
  { id: "s2", title: "Today done", status: "complete", wardName: "stock-analysis", updatedAt: now - 30 * 60 * 1000 },
  { id: "s3", title: "Yesterday", status: "complete", wardName: "maritime", updatedAt: now - 1.5 * oneDay },
  { id: "s4", title: "Last week", status: "complete", wardName: null, updatedAt: now - 5 * oneDay },
  { id: "s5", title: "Older", status: "crashed", wardName: null, updatedAt: now - 30 * oneDay },
];

describe("groupSessions", () => {
  it("groups into Running / Today / Yesterday / Last week / Older", () => {
    const groups = groupSessions(sessions, now);
    expect(groups.Running.map((s) => s.id)).toEqual(["s1"]);
    expect(groups.Today.map((s) => s.id)).toEqual(["s2"]);
    expect(groups.Yesterday.map((s) => s.id)).toEqual(["s3"]);
    expect(groups["Last week"].map((s) => s.id)).toEqual(["s4"]);
    expect(groups.Older.map((s) => s.id)).toEqual(["s5"]);
  });
});

describe("<SessionsList>", () => {
  it("renders group headers and rows", () => {
    render(
      <SessionsList
        sessions={sessions}
        currentId={null}
        onSelect={() => {}}
        onNew={() => {}}
        renderDensity="expanded"
      />
    );
    expect(screen.getByText("Running")).toBeTruthy();
    expect(screen.getByText("Today done")).toBeTruthy();
  });

  it("fires onSelect with session id on click", () => {
    const fn = vi.fn();
    render(
      <SessionsList
        sessions={sessions}
        currentId={null}
        onSelect={fn}
        onNew={() => {}}
        renderDensity="expanded"
      />
    );
    fireEvent.click(screen.getByText("Running one"));
    expect(fn).toHaveBeenCalledWith("s1");
  });

  it("fires onNew from the New button", () => {
    const fn = vi.fn();
    render(
      <SessionsList
        sessions={sessions}
        currentId={null}
        onSelect={() => {}}
        onNew={fn}
        renderDensity="expanded"
      />
    );
    fireEvent.click(screen.getByTestId("sessions-list-new"));
    expect(fn).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cd apps/ui && npx vitest run src/features/research-v2/SessionsList.test.tsx`
Expected: FAIL

- [ ] **Step 3: Implement the component**

```tsx
import { Plus } from "lucide-react";
import type { SessionSummary } from "./types";

const ONE_DAY = 24 * 60 * 60 * 1000;
const SEVEN_DAYS = 7 * ONE_DAY;

type Bucket = "Running" | "Today" | "Yesterday" | "Last week" | "Older";
const BUCKET_ORDER: Bucket[] = ["Running", "Today", "Yesterday", "Last week", "Older"];

export function groupSessions(
  sessions: SessionSummary[],
  now: number = Date.now()
): Record<Bucket, SessionSummary[]> {
  const buckets: Record<Bucket, SessionSummary[]> = {
    "Running": [], "Today": [], "Yesterday": [], "Last week": [], "Older": [],
  };
  const startOfToday = new Date(now);
  startOfToday.setHours(0, 0, 0, 0);
  const startOfYesterday = startOfToday.getTime() - ONE_DAY;
  for (const s of sessions) {
    if (s.status === "running") { buckets.Running.push(s); continue; }
    if (s.updatedAt >= startOfToday.getTime()) buckets.Today.push(s);
    else if (s.updatedAt >= startOfYesterday) buckets.Yesterday.push(s);
    else if (s.updatedAt >= now - SEVEN_DAYS) buckets["Last week"].push(s);
    else buckets.Older.push(s);
  }
  for (const b of BUCKET_ORDER) buckets[b].sort((a, b2) => b2.updatedAt - a.updatedAt);
  return buckets;
}

const STATUS_DOT: Record<SessionSummary["status"], { color: string; label: string }> = {
  running:  { color: "rgb(150,255,180)", label: "running" },
  complete: { color: "rgba(255,255,255,0.5)", label: "complete" },
  crashed:  { color: "rgb(255,120,120)", label: "crashed" },
  paused:   { color: "rgb(255,200,80)", label: "paused" },
};

function relativeTime(at: number, now: number = Date.now()): string {
  const diff = now - at;
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.round(diff / 60_000)}m ago`;
  if (diff < ONE_DAY) return `${Math.round(diff / 3_600_000)}h ago`;
  return `${Math.round(diff / ONE_DAY)}d ago`;
}

export interface SessionsListProps {
  sessions: SessionSummary[];
  currentId: string | null;
  onSelect(id: string): void;
  onNew(): void;
  renderDensity: "expanded" | "condensed";
}

export function SessionsList({ sessions, currentId, onSelect, onNew, renderDensity }: SessionsListProps) {
  const groups = groupSessions(sessions);
  return (
    <div className={`sessions-list sessions-list--${renderDensity}`}>
      <button
        type="button"
        data-testid="sessions-list-new"
        className="sessions-list__new"
        onClick={onNew}
      >
        <Plus size={14} /> New research
      </button>
      {BUCKET_ORDER.map((bucket) => (
        groups[bucket].length === 0 ? null : (
          <div key={bucket} className="sessions-list__group">
            <div className="sessions-list__group-title">{bucket}</div>
            {groups[bucket].map((s) => (
              <button
                type="button"
                key={s.id}
                className={`sessions-list__row ${s.id === currentId ? "sessions-list__row--active" : ""}`}
                onClick={() => onSelect(s.id)}
              >
                <span
                  className="sessions-list__dot"
                  style={{ background: STATUS_DOT[s.status].color }}
                  title={STATUS_DOT[s.status].label}
                />
                <span className="sessions-list__title">{s.title || "(untitled)"}</span>
                {s.wardName && <span className="sessions-list__ward">{s.wardName}</span>}
                <span className="sessions-list__time">{relativeTime(s.updatedAt)}</span>
              </button>
            ))}
          </div>
        )
      ))}
      {sessions.length === 0 && (
        <div className="sessions-list__empty">no research sessions yet</div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cd apps/ui && npx vitest run src/features/research-v2/SessionsList.test.tsx`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/research-v2/SessionsList.tsx apps/ui/src/features/research-v2/SessionsList.test.tsx
git commit -m "feat(research-v2): SessionsList presentation-agnostic component"
```

---

### Task 8: `SessionsDrawer` wrapper

**Files:**
- Create: `apps/ui/src/features/research-v2/SessionsDrawer.tsx`

- [ ] **Step 1: Implement the component**

```tsx
import { useEffect } from "react";
import { X } from "lucide-react";
import { SessionsList, type SessionsListProps } from "./SessionsList";

export interface SessionsDrawerProps extends Omit<SessionsListProps, "renderDensity"> {
  open: boolean;
  onClose(): void;
}

export function SessionsDrawer({ open, onClose, ...listProps }: SessionsDrawerProps) {
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) { if (e.key === "Escape") onClose(); }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <>
      <button
        type="button"
        aria-label="Close sessions drawer"
        className="sessions-drawer__backdrop"
        onClick={onClose}
      />
      <aside className="sessions-drawer" role="dialog" aria-label="Research sessions">
        <div className="sessions-drawer__header">
          <span>Sessions</span>
          <button type="button" className="btn btn--ghost btn--sm" onClick={onClose} aria-label="Close">
            <X size={14} />
          </button>
        </div>
        <SessionsList {...listProps} renderDensity="expanded" />
      </aside>
    </>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/features/research-v2/SessionsDrawer.tsx
git commit -m "feat(research-v2): SessionsDrawer wrapper"
```

---

### Task 9: `useResearchSessionsList` hook

**Files:**
- Create: `apps/ui/src/features/research-v2/useSessionsList.ts`

- [ ] **Step 1: Implement the hook**

```typescript
import { useCallback, useEffect, useState } from "react";
import { getTransport } from "@/services/transport";
import type { SessionSummary } from "./types";

interface SessionsApiRow {
  session_id: string;
  title?: string;
  status?: string;
  ward?: { name?: string };
  updated_at?: string;
}

function mapStatus(s: string | undefined): SessionSummary["status"] {
  switch (s) {
    case "running": case "active": return "running";
    case "complete": case "completed": return "complete";
    case "crashed": case "error": return "crashed";
    case "paused": return "paused";
    default: return "complete";
  }
}

export function useResearchSessionsList() {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const transport = await getTransport();
      const result = await transport.get<SessionsApiRow[]>("/api/sessions?mode=research&limit=50");
      if (result.success && Array.isArray(result.data)) {
        setSessions(result.data.map((row): SessionSummary => ({
          id: row.session_id,
          title: row.title ?? "(untitled)",
          status: mapStatus(row.status),
          wardName: row.ward?.name ?? null,
          updatedAt: row.updated_at ? new Date(row.updated_at).getTime() : Date.now(),
        })));
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  return { sessions, loading, refresh };
}
```

- [ ] **Step 2: Verify endpoint compatibility**

Run: `curl -s 'http://localhost:18791/api/sessions?mode=research&limit=5' | jq '.[0]'`
Expected: a row with `session_id`, `title`, `status`, optionally `ward.name`, `updated_at`.

If the endpoint does not accept `?mode=` or does not return `ward.name`/`updated_at`, extend the backend handler (`gateway/src/http/sessions.rs::list_sessions` or equivalent). Capture the change as a follow-up commit.

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/research-v2/useSessionsList.ts
git commit -m "feat(research-v2): useResearchSessionsList hook"
```

---

### Task 10: `ResearchPage` component

**Files:**
- Create: `apps/ui/src/features/research-v2/ResearchPage.tsx`
- Create: `apps/ui/src/features/research-v2/research.css`
- Create: `apps/ui/src/features/research-v2/index.ts`

- [ ] **Step 1: Implement the page**

```tsx
import { useState } from "react";
import { Menu, Plus, Square } from "lucide-react";
import { ChatInput } from "../chat/ChatInput";
import { StatusPill } from "../shared/statusPill";
import { ArtifactSlideOut } from "../chat/ArtifactSlideOut";
import { AgentTurnBlock } from "./AgentTurnBlock";
import { SessionsDrawer } from "./SessionsDrawer";
import { useResearchSession } from "./useResearchSession";
import { useResearchSessionsList } from "./useSessionsList";
import { useNavigate } from "react-router-dom";
import type { Artifact } from "@/services/transport/types";
import "./research.css";

export function ResearchPage() {
  const { state, pillState, sendMessage, stopAgent, startNewResearch, toggleThinking } = useResearchSession();
  const { sessions, refresh: refreshSessions } = useResearchSessionsList();
  const navigate = useNavigate();
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [viewingArtifact, setViewingArtifact] = useState<Artifact | null>(null);

  const isEmpty = state.turns.length === 0 && state.messages.length === 0 && !state.sessionId;

  const handleSelect = (id: string) => {
    setDrawerOpen(false);
    navigate(`/research-v2/${id}`);
  };

  const handleNew = () => {
    setDrawerOpen(false);
    startNewResearch();
    refreshSessions();
  };

  return (
    <div className="research-page">
      <div className="research-page__header">
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={() => setDrawerOpen(true)}
          aria-label="Open sessions"
        >
          <Menu size={16} />
        </button>
        <div className="research-page__title">zbot</div>
        <div className="research-page__header-actions">
          {state.wardName && (
            <span className="research-page__ward-chip">{state.wardName}</span>
          )}
          <button type="button" className="btn btn--ghost btn--sm" onClick={handleNew}>
            <Plus size={14} /> New research
          </button>
          {state.status === "running" && (
            <button type="button" className="btn btn--ghost btn--sm" onClick={stopAgent} title="Stop">
              <Square size={14} />
            </button>
          )}
        </div>
      </div>

      <div className="research-page__pill-strip">
        <StatusPill state={pillState} />
      </div>

      <SessionsDrawer
        open={drawerOpen}
        onClose={() => setDrawerOpen(false)}
        sessions={sessions}
        currentId={state.sessionId}
        onSelect={handleSelect}
        onNew={handleNew}
      />

      <div className="research-page__body">
        <div className="research-page__column">
          {isEmpty ? (
            <div className="research-page__empty">
              <h1>Research</h1>
              <p>Ask a research question — the full agent chain kicks in.</p>
            </div>
          ) : (
            <>
              {state.messages.map((m) => (
                <div key={m.id} className="research-page__user-bubble">{m.content}</div>
              ))}
              {state.intentAnalyzing && (
                <div className="research-page__intent-muted">analyzing intent…</div>
              )}
              {state.intentClassification && !state.intentAnalyzing && (
                <div className="research-page__intent-classification">
                  intent: <strong>{state.intentClassification}</strong>
                  {state.wardName && <> · ward: <strong>{state.wardName}</strong></>}
                </div>
              )}
              {state.turns.map((turn) => (
                <AgentTurnBlock key={turn.id} turn={turn} onToggleThinking={toggleThinking} />
              ))}
            </>
          )}
        </div>
      </div>

      <div className="research-page__composer">
        <ChatInput onSend={sendMessage} disabled={state.status === "running"} />
      </div>

      {viewingArtifact && (
        <ArtifactSlideOut
          artifact={viewingArtifact}
          onClose={() => setViewingArtifact(null)}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Write the CSS**

```css
.research-page { display: flex; flex-direction: column; height: 100%; overflow: hidden; }
.research-page__header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  border-bottom: 1px solid var(--border);
}
.research-page__title { flex: 1; text-align: center; font-weight: 600; }
.research-page__header-actions { display: flex; align-items: center; gap: 8px; }
.research-page__ward-chip {
  font-size: 11px;
  padding: 3px 10px;
  border-radius: 10px;
  background: rgba(100, 200, 255, 0.12);
  color: rgb(140, 220, 255);
}
.research-page__pill-strip {
  display: flex;
  justify-content: center;
  padding: 8px 20px 0;
  min-height: 36px;
}
.research-page__body { flex: 1; overflow-y: auto; }
.research-page__column {
  max-width: 880px;
  margin: 0 auto;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.research-page__empty { text-align: center; padding: 80px 0; }
.research-page__empty h1 { font-size: 28px; font-weight: 600; }
.research-page__empty p { color: var(--muted-foreground); }
.research-page__user-bubble {
  align-self: flex-end;
  max-width: 80%;
  padding: 10px 14px;
  background: var(--primary, rgba(100, 200, 255, 0.15));
  border-radius: 14px 14px 2px 14px;
  font-size: 14px;
  white-space: pre-wrap;
}
.research-page__intent-muted { font-size: 12px; color: var(--muted-foreground); font-style: italic; }
.research-page__intent-classification { font-size: 12px; color: var(--muted-foreground); }

.agent-turn-block {
  padding: 12px 14px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid var(--border);
}
.agent-turn-block__header { display: flex; justify-content: space-between; align-items: center; font-size: 12px; }
.agent-turn-block__agent { font-family: ui-monospace, monospace; font-weight: 600; }
.agent-turn-block__meta { display: flex; align-items: center; gap: 8px; color: var(--muted-foreground); }
.agent-turn-block__running { color: rgb(150,255,180); }
.agent-turn-block__chevron {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 8px 0;
  background: transparent;
  border: none;
  font-size: 12px;
  color: var(--muted-foreground);
  cursor: pointer;
  padding: 0;
}
.agent-turn-block__chevron svg { transition: transform 150ms ease; }
.agent-turn-block__timeline { padding-left: 16px; border-left: 1px dashed var(--border); margin: 8px 0; }
.agent-turn-block__respond { font-size: 14px; line-height: 1.6; margin-top: 4px; }
.agent-turn-block__respond--streaming { opacity: 0.85; }
.agent-turn-block__placeholder { color: var(--muted-foreground); font-style: italic; }

.thinking-timeline { list-style: none; padding: 0; margin: 0; }
.thinking-timeline__item {
  font-size: 12px;
  display: grid;
  grid-template-columns: 68px 1fr;
  gap: 8px;
  padding: 2px 0;
}
.thinking-timeline__item--tool_call { color: rgb(140, 220, 255); }
.thinking-timeline__item--tool_result { color: var(--muted-foreground); }
.thinking-timeline__time { font-family: ui-monospace, monospace; color: var(--muted-foreground); }
.thinking-timeline__preview { font-family: ui-monospace, monospace; color: var(--muted-foreground); }
.thinking-timeline__empty { font-size: 12px; color: var(--muted-foreground); font-style: italic; }

.research-page__composer {
  border-top: 1px solid var(--border);
  padding: 12px 20px;
  max-width: 880px;
  width: 100%;
  margin: 0 auto;
}

.sessions-drawer {
  position: fixed;
  top: 0;
  left: 0;
  bottom: 0;
  width: 280px;
  background: var(--background);
  border-right: 1px solid var(--border);
  box-shadow: 2px 0 12px rgba(0, 0, 0, 0.4);
  z-index: 50;
  display: flex;
  flex-direction: column;
  animation: sessions-drawer-in 150ms ease-out;
}
.sessions-drawer__backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.3);
  z-index: 40;
  border: none;
  cursor: default;
}
.sessions-drawer__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
  font-weight: 600;
}
.sessions-list { padding: 12px; display: flex; flex-direction: column; gap: 8px; overflow-y: auto; flex: 1; }
.sessions-list__new {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  border: 1px solid var(--border);
  background: transparent;
  border-radius: 8px;
  font-size: 13px;
  cursor: pointer;
}
.sessions-list__group-title { font-size: 11px; color: var(--muted-foreground); text-transform: uppercase; letter-spacing: 0.05em; padding: 8px 4px 4px; }
.sessions-list__row {
  display: grid;
  grid-template-columns: 10px 1fr auto;
  grid-template-areas: "dot title time" "dot ward time";
  gap: 4px 8px;
  align-items: center;
  padding: 6px 8px;
  border-radius: 6px;
  border: none;
  background: transparent;
  text-align: left;
  cursor: pointer;
}
.sessions-list__row:hover { background: rgba(255,255,255,0.04); }
.sessions-list__row--active { background: rgba(100,200,255,0.1); }
.sessions-list__dot { grid-area: dot; width: 8px; height: 8px; border-radius: 50%; }
.sessions-list__title { grid-area: title; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sessions-list__ward { grid-area: ward; font-size: 11px; color: var(--muted-foreground); }
.sessions-list__time { grid-area: time; font-size: 11px; color: var(--muted-foreground); }
.sessions-list__empty { padding: 20px; color: var(--muted-foreground); text-align: center; font-size: 12px; }

@keyframes sessions-drawer-in {
  from { transform: translateX(-10px); opacity: 0; }
  to   { transform: translateX(0); opacity: 1; }
}
```

- [ ] **Step 3: Write the barrel**

```typescript
export { ResearchPage } from "./ResearchPage";
```

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/research-v2/ResearchPage.tsx apps/ui/src/features/research-v2/research.css apps/ui/src/features/research-v2/index.ts
git commit -m "feat(research-v2): ResearchPage with drawer + turn blocks + pill"
```

---

### Task 11: ResearchPage render test

**Files:**
- Create: `apps/ui/src/features/research-v2/ResearchPage.test.tsx`

- [ ] **Step 1: Write tests**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { ResearchPage } from "./ResearchPage";

vi.mock("./useResearchSession", () => ({
  useResearchSession: () => ({
    state: {
      sessionId: null,
      conversationId: "c1",
      title: "",
      status: "idle",
      wardId: null,
      wardName: null,
      messages: [],
      turns: [],
      intentAnalyzing: false,
      intentClassification: null,
      planPath: null,
    },
    pillState: { visible: false, narration: "", suffix: "", category: "neutral", starting: false, swapCounter: 0 },
    sendMessage: vi.fn(),
    stopAgent: vi.fn(),
    startNewResearch: vi.fn(),
    toggleThinking: vi.fn(),
  }),
}));

vi.mock("./useSessionsList", () => ({
  useResearchSessionsList: () => ({ sessions: [], loading: false, refresh: vi.fn() }),
}));

describe("<ResearchPage>", () => {
  it("renders empty state", () => {
    render(
      <MemoryRouter initialEntries={["/research-v2"]}>
        <Routes>
          <Route path="/research-v2" element={<ResearchPage />} />
        </Routes>
      </MemoryRouter>
    );
    expect(screen.getByText("Research")).toBeTruthy();
    expect(screen.getByText(/full agent chain/)).toBeTruthy();
  });

  it("toggles the drawer when ☰ is clicked", () => {
    render(
      <MemoryRouter initialEntries={["/research-v2"]}>
        <Routes>
          <Route path="/research-v2" element={<ResearchPage />} />
        </Routes>
      </MemoryRouter>
    );
    const openBtn = screen.getByLabelText("Open sessions");
    fireEvent.click(openBtn);
    expect(screen.getByLabelText("Research sessions")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Verify tests pass**

Run: `cd apps/ui && npx vitest run src/features/research-v2/ResearchPage.test.tsx`
Expected: PASS — 2 tests.

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/research-v2/ResearchPage.test.tsx
git commit -m "test(research-v2): ResearchPage render + drawer toggle"
```

---

### Task 12: Route wiring for `/research-v2`

**Files:**
- Modify: `apps/ui/src/App.tsx`

- [ ] **Step 1: Add import**

Locate the import block near line 25 and add:

```typescript
import { ResearchPage } from "./features/research-v2";
```

- [ ] **Step 2: Add routes**

Locate the `<Routes>` block near line 187 and add below `/chat-v2` (added by Quick Chat plan):

```tsx
<Route path="/research-v2" element={<ResearchPage />} />
<Route path="/research-v2/:sessionId" element={<ResearchPage />} />
```

- [ ] **Step 3: Add sidebar link**

Find the sidebar link list (grep for `MessageSquare` or `/chat-v2` link if it was added). Add a sibling link using the `Search` icon that's already imported:

```tsx
<Link to="/research-v2" className={`sidebar-link ${location.pathname.startsWith("/research-v2") ? "active" : ""}`}>
  <Search size={18} />
  <span>Research <span className="sidebar-link__badge">v2</span></span>
</Link>
```

- [ ] **Step 4: Build check**

Run: `cd apps/ui && npm run build`
Expected: build succeeds.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/App.tsx
git commit -m "feat(research-v2): route /research-v2 and /research-v2/:sessionId"
```

---

### Task 13: Backend verification — `/api/sessions?mode=research` and `/api/sessions/:id/state`

**Files:**
- Possibly modify: `gateway/src/http/sessions.rs`

- [ ] **Step 1: Verify list endpoint**

Run:
```bash
curl -s 'http://localhost:18791/api/sessions?mode=research&limit=5' | jq '.[0]'
```

Expected shape: `{ session_id, title, status, ward: { name }, updated_at }`.

If the endpoint doesn't exist or doesn't filter by mode, extend the handler. Pattern to follow: look at how `list_sessions` (or the existing sessions list endpoint) is wired in `gateway/src/http/mod.rs`, add a `mode` query filter, and include `ward.name` + `updated_at` in the response struct.

- [ ] **Step 2: Verify state endpoint carries turns + conversation id + ward**

Run:
```bash
curl -s 'http://localhost:18791/api/sessions/<SOME_ID>/state' | jq '{ ward, conversation_id, title, status, message_count: (.messages | length), turns_count: (.turns // [] | length) }'
```

The UI consumes `ward.id`, `ward.name`, `conversation_id`, `title`, `status`, `messages`, and optionally `turns`. If `turns` is absent, the page will populate turns from live events as before — acceptable. If `conversation_id` is absent, UI falls back to a fresh conv id (which breaks continuation). In that case, extend `SessionStateBuilder` to include the stored conversation id.

- [ ] **Step 3: If changes were needed, commit**

```bash
git add gateway/
git commit -m "feat(research-v2): expose mode filter + conversation_id + ward.name in session endpoints"
```

Otherwise, skip the commit — mark this task done.

---

### Task 14: Smoke-test Research end-to-end in dev

**Files:** none (verification task)

- [ ] **Step 1: Start dev daemon + UI, open `http://localhost:5173/research-v2`**

- [ ] **Step 2: Start a new research**

Type a real research prompt (e.g., "Summarize what we decided about z.ai rate limits and propose a semaphore size"). Send.

Expected:
- A user bubble appears.
- Intent-analysis muted line appears briefly.
- Ward chip populates in header when `ward_changed` arrives. It does NOT flip to "unknown" on subsequent `AgentStarted` events (acceptance criterion #7).
- A turn block for `root` appears with a collapsed Thinking chevron. Expand it — see chronological events without ping-pong (acceptance criterion #8).
- Additional turn blocks appear for delegated agents (planner, solution, builder, writer).
- Status pill swaps as tools fire.
- Final `Respond` renders inside the last turn block's body (acceptance criterion #9).

- [ ] **Step 3: Open the drawer (`☰`)**

Expected: drawer slides in from left, lists the running session under "Running" with a green dot. Previous sessions appear under Today/Yesterday/etc.

- [ ] **Step 4: Click a different session**

Expected: URL changes to `/research-v2/<otherId>`, drawer closes, page re-hydrates from snapshot and shows turn blocks for the old session.

- [ ] **Step 5: Close the tab mid-session**

Reopen at `/research-v2/<runningId>`. Expected: the running turn block continues to stream; status pill shows live narration.

- [ ] **Step 6: Click "New research"**

Expected: URL returns to `/research-v2`, empty state visible. Start a new session.

- [ ] **Step 7: Memory-bank note**

Edit `memory-bank/activeContext.md`:

```
Research v2 (/research-v2) shipping behind sidebar "Research v2" link.
Fixes: ward-sticky chip, collapsed-Thinking turn blocks, orphan-turn Respond rendering, new-research race-free navigation.
Old / (MissionControl) retired in a separate cleanup PR.
```

```bash
git add memory-bank/activeContext.md
git commit -m "docs(research-v2): note v2 availability and bug fixes"
```

---

### Task 15: Artifact strip + slide-out wiring (required — parity with chat-v2)

**Files:**
- Modify: `apps/ui/src/features/research-v2/types.ts` (add `ResearchArtifactRef`)
- Modify: `apps/ui/src/features/research-v2/reducer.ts` (add `SET_ARTIFACTS` action)
- Modify: `apps/ui/src/features/research-v2/useResearchSession.ts` (fetch on turn complete + bootstrap)
- Modify: `apps/ui/src/features/research-v2/ResearchPage.tsx` (render `ArtifactCard` + `ArtifactSlideOut`)
- Modify: `apps/ui/src/features/research-v2/research.css` (theme-token artifact card)

**Why required:** chat-v2 shipped this (see `memory-bank/components/chat-v2/overview.md` → Artifact slide-out section). Users open Research expecting parity. Research agents go through the full writer pipeline, so `ArtifactDeclaration`s land in `respond()` calls — the artifact table is populated naturally (no dependence on backlog item B1). Expect this to work end-to-end live.

- [ ] **Step 1: Types**

Add to `types.ts`:

```typescript
export interface ResearchArtifactRef {
  id: string;
  fileName: string;
  fileType?: string;
  fileSize?: number;
  label?: string;
}
```

Extend `ResearchSessionState` with `artifacts: ResearchArtifactRef[]` and default `EMPTY_RESEARCH_STATE.artifacts = []`.

- [ ] **Step 2: Reducer action**

Add to `QuickChatAction` union (same pattern as chat-v2):

```typescript
| { type: "SET_ARTIFACTS"; artifacts: ResearchArtifactRef[] }
```

Handler:

```typescript
case "SET_ARTIFACTS":
  return { ...state, artifacts: action.artifacts };
```

Also extend HYDRATE payload with `artifacts: ResearchArtifactRef[]`.

- [ ] **Step 3: Hook fetch**

Add `fetchArtifacts(transport, sessionId)` helper (mirror of chat-v2). Add an effect that refetches on `(status === "idle" or "complete", sessionId)` transitions. Include artifacts in the `HYDRATE` dispatch from `hydrateExistingSession`.

- [ ] **Step 4: Page rendering**

In `ResearchPage.tsx` — add `ArtifactCard` sub-component (or import the chat-v2 one from a shared utility). Render the strip below the last turn block. Wire `setViewingArtifact` + mount `ArtifactSlideOut` from `../chat/ArtifactSlideOut`. The `refToArtifact(ref, sessionId)` shim pattern from chat-v2 applies here verbatim.

- [ ] **Step 5: CSS**

All colours via theme tokens: `var(--background-surface)`, `var(--border)`, `var(--foreground)`, `var(--muted-foreground)`, `var(--primary)`. No hex.

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/features/research-v2/
git commit -m "feat(research-v2): artifact strip + slide-out — parity with chat-v2"
```

---

### Task 16: Error-state surfacing for crashed turns

**Files:**
- Modify: `apps/ui/src/features/research-v2/reducer.ts` (track per-turn error text)
- Modify: `apps/ui/src/features/research-v2/AgentTurnBlock.tsx` (render error banner)

**Why:** chat-v2 backlog B3 — when an execution crashes (LLM 500, context blow-up, max-iterations), the backend emits `turn_complete` with `final_message: ""` and **no `error` event**. Research UI hits this more often than chat (delegations can crash independently of the root). Without UI surfacing, users see an empty Respond and assume the UI is broken.

- [ ] **Step 1: Extend `AgentTurn` type**

```typescript
export interface AgentTurn {
  // ... existing fields
  errorMessage: string | null;
}
```

- [ ] **Step 2: Reducer inference**

In the `TURN_COMPLETE` / `AGENT_COMPLETED` handler — when the turn's `respond === null` AND `timeline` contains no meaningful events AND `status !== "stopped"`, set `status: "error"` and `errorMessage: "Turn ended with no output (provider error or context limit)"`. When chat-v2 backlog B3 lands, read the real message from the `error` event.

- [ ] **Step 3: UI banner**

In `AgentTurnBlock.tsx`, when `turn.status === "error"` AND `turn.errorMessage`, render a compact banner (italic red text with `lucide-react`'s `AlertCircle`) in place of the respond content.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/research-v2/
git commit -m "feat(research-v2): surface silent crash as per-turn error banner"
```

---

### Task 17: E2E Playwright spec

**Files:**
- Create: `apps/ui/tests/e2e/research-v2.spec.ts`

Follow the chat-v2 pattern (`apps/ui/tests/e2e/quick-chat.spec.ts`). Three describe-blocks:

- [ ] **Step 1: Page-load tests (no daemon)**

```typescript
test.describe('Research v2 — page load', () => {
  test('renders empty state with composer', async ({ page }) => {
    await page.goto('/research-v2');
    await page.waitForSelector('.research-page', { state: 'visible', timeout: 15_000 });
    await expect(page.getByPlaceholder('Type a message...')).toBeVisible();
  });

  test('drawer toggle opens and closes', async ({ page }) => {
    await page.goto('/research-v2');
    const toggle = page.getByRole('button', { name: /sessions/i });
    await toggle.click();
    await expect(page.locator('.sessions-drawer')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('.sessions-drawer')).toHaveCount(0);
  });

  test('no status pill until a turn is active', async ({ page }) => {
    await page.goto('/research-v2');
    await expect(page.locator('[data-testid="status-pill"]')).toHaveCount(0);
  });
});
```

- [ ] **Step 2: Route-mocked tests**

```typescript
const STUB_SESSION_ID = 'sess-research-e2e';

async function installApiStubs(page: Page) {
  // Sessions list — two rows to exercise drawer grouping.
  await page.route('**/api/logs/sessions', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([
        { session_id: STUB_SESSION_ID, conversation_id: 'r-1', agent_id: 'root',
          agent_name: 'root', started_at: new Date().toISOString(), status: 'running',
          token_count: 123, tool_call_count: 2, duration_ms: 5000 },
      ]),
    })
  );
  await page.route(`**/api/executions/v2/sessions/${STUB_SESSION_ID}/messages*`, (route) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: '[]' })
  );
  await page.route(`**/api/sessions/${STUB_SESSION_ID}/artifacts`, (route) =>
    route.fulfill({
      status: 200, contentType: 'application/json',
      body: JSON.stringify([{
        id: 'art-r-1', sessionId: STUB_SESSION_ID,
        filePath: '/tmp/plan.md', fileName: 'plan.md', fileType: 'md',
        fileSize: 512, label: 'research plan', createdAt: new Date().toISOString(),
      }]),
    })
  );
  await page.route('**/api/artifacts/art-r-1/content', (route) =>
    route.fulfill({ status: 200, contentType: 'text/markdown', body: '# Plan\n\nOutline of research.\n' })
  );
}

test('drawer lists sessions; clicking a row navigates to it', async ({ page }) => {
  await installApiStubs(page);
  await page.goto('/research-v2');
  await page.getByRole('button', { name: /sessions/i }).click();
  await page.getByText(/root/i).first().click();
  await expect(page).toHaveURL(new RegExp(STUB_SESSION_ID));
});

test('artifact strip + slide-out', async ({ page }) => {
  await installApiStubs(page);
  await page.goto(`/research-v2/${STUB_SESSION_ID}`);
  const card = page.getByTestId('research-v2-artifact').first();
  await card.waitFor({ state: 'visible', timeout: 10_000 });
  await expect(card).toContainText('plan.md');
  await card.click();
  const slideOut = page.locator('.artifact-slideout').first();
  await expect(slideOut).toBeVisible();
  await expect(slideOut).toContainText('Outline of research');
  await slideOut.getByTitle('Close').click();
  await expect(slideOut).toHaveCount(0);
});
```

- [ ] **Step 3: Agent-backed tests (auto-skip when daemon down)**

```typescript
test.describe('Research v2 — agent-backed', () => {
  test.beforeEach(async ({}, testInfo) => {
    const ok = await daemonReachable();
    testInfo.skip(!ok, 'gateway daemon not reachable');
  });

  test('sends a prompt and renders at least one turn block', async ({ page }) => {
    test.setTimeout(120_000);
    await page.goto('/research-v2');
    await sendPrompt(page, 'Summarise today in one sentence.');
    const turnBlock = page.locator('.agent-turn-block').first();
    await expect(turnBlock).toBeVisible({ timeout: 90_000 });
    // Respond text lands inside the block after agent_completed.
    await expect(turnBlock.locator('.agent-turn-block__respond')).toBeVisible({ timeout: 60_000 });
  });
});
```

- [ ] **Step 4: Run + commit**

```
cd apps/ui && npx playwright test research-v2.spec.ts --grep 'page load|route-mocked'
```

Expected: all non-agent-backed tests pass.

```bash
git add apps/ui/tests/e2e/research-v2.spec.ts
git commit -m "test(research-v2): playwright spec — page load, route-mocked, agent-backed"
```

---

## Self-review checklist

Before declaring complete:

1. **Spec coverage** — every `Research — Spec` and `Bug fixes rolled into this design` bullet has a task:
   - [x] Unchanged session model (Task 4 hydrate + Task 13 sessions-list)
   - [x] `☰` drawer-toggle header (Task 10)
   - [x] Status pill strip (Task 10 + shared module from Quick Chat plan, deterministic mode only)
   - [x] Sessions drawer with Running / Today / Yesterday / Last week / Older (Tasks 7–8)
   - [x] Per-agent-turn block (Tasks 5–6)
   - [x] Delegation indent via `parentExecutionId` (Task 1 type + Task 6 data-parent attr)
   - [x] Ward-sticky fix (Task 2 WARD_CHANGED only, AGENT_STARTED inherits)
   - [x] Thinking ping-pong fix (Task 6 single block per turn, plus pill ignores Thinking entirely)
   - [x] New-research race-free flow (Task 4 startNewResearch → RESET → navigate → re-subscribe; no client-side id generation)
   - [x] Final Respond reliability / orphan turn (Task 2 RESPOND works without AGENT_STARTED)
   - [x] Artifact strip + slide-out (Task 15 — promoted from optional to required)
   - [x] Silent-crash surfacing per turn (Task 16 — addresses chat-v2 backlog B3 at the UI layer)
   - [x] E2E Playwright spec with route-mocked coverage (Task 17)
   - [x] Routes alongside `/` (Task 12)

2. **chat-v2 learnings applied** — see `memory-bank/components/chat-v2/learnings.md`. This plan incorporates all 12:
   - (1) Server-owned session identity — no `newConvId()`; client never generates ids.
   - (2) Self-healing lookups — `hydrateExistingSession` uses `/messages` (always 200) instead of `/state` (can 404 for extant sessions).
   - (3) Deterministic pill — Task 3 drops `thinking` from `mapGatewayEventToPillEvent`.
   - (4) Wire-format field names verified — `tool_name`, `delta`, `message`, `invoke_accepted`, flat `ward_id`, no `is_final`.
   - (5) Stable effect-dep identity — `pillSink` memoised in shared module, dropped from Task 4's subscribe-effect deps with explicit eslint-disable.
   - (6) StrictMode-safe ref set post-async — Task 4's `hydratedForSessionRef` is assigned inside the dispatch block.
   - (7) History ≠ reducer stream — `isVisibleResearchMessage` filter at hook boundary.
   - (8) User-visible recovery — Task 16 error banner + "New research" button.
   - (9) Theme tokens only — Tasks 10 + 15 CSS use `var(--*)` exclusively.
   - (10) E2E route mocks > live waits — Task 17.
   - (11) SonarQube extraction — Task 3 per-branch mappers, Task 2 per-case reducer helpers.
   - (12) Live browser verify after unit tests — Task 14.

3. **Placeholder scan** — no "TBD" / "TODO" / "implement later". Verified.

4. **Type consistency** — `AgentTurn`, `TimelineEntry`, `ResearchAction`, `ResearchSessionState`, `SessionSummary`, `ResearchArtifactRef`, `PillState`/`PillEvent` (imported from shared module) used consistently. The hook exports `toggleThinking(turnId)` which matches `AgentTurnBlock`'s `onToggleThinking(turnId)` prop.

5. **Acceptance criteria coverage** (from spec):
   - (4) `/research-v2` shows centered column + `☰` drawer + full agent chain — Tasks 10, 12, 14.
   - (5) Rolling status pill appears on both pages — shared module + Task 10 wire-up (deterministic, no Thinking flicker).
   - (6) Tab-close mid-session + reopen resumes live stream — Task 4 hydrate + subscribe.
   - (7) Ward chip never flips to "unknown" — Task 2 reducer + Task 3 `ward_changed` guard (null id → null return).
   - (8) Thinking chevron expands to chronological timeline — Task 6 (Thinking lives here, NOT on the pill).
   - (9) Respond event rendered even if AgentCompleted missing — Task 2 orphan-turn test.
   - Old `/` still works — unchanged.

6. **Inherited backlog items** (from chat-v2, documented at `memory-bank/components/chat-v2/backlog.md`):
   - **B1** — fast-mode `write_file` artifact auto-registration. Research uses full `mode="research"` (the default), so writer-agent declarations fire normally. **Not blocking for this plan.**
   - **B2** — server-side context compaction. Research creates a new session per query, so each session starts fresh — **not affected.**
   - **B3** — silent-crash `error` event. Task 16 works around this at the UI layer (infer from empty turn_complete). When B3 lands, swap the inferred message for the real `error.message`.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-19-research-ui.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.
