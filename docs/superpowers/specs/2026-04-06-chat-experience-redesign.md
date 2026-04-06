# Chat Experience Redesign

**Date:** 2026-04-06
**Status:** Draft
**Scope:** Redesign the chat page for consistent session rendering, reliable reconnection, and clean separation of conversation (center) from execution context (sidebar).

---

## Problem

The current chat experience is inconsistent:

1. **Session reconnect is broken** — reopening a running or completed session loses user messages, renders tool calls as raw text, and misses the final `respond` output until you navigate away and back.
2. **`loadSession()` is fragile** — 270 lines of heuristic log parsing tries to reconstruct rich UI state from flat execution logs. It uses regex matching, reverse-scanning for "unmatched" blocks, and falls back to session titles when user messages are missing.
3. **Center panel is cluttered** — tool calls, delegations, recall blocks, intent analysis, and responses all mix into one scrollable stream. Hard to tell what's happening at a glance.
4. **Sidebar state is derived from blocks** — instead of being populated directly from events or a snapshot, sidebar content is extracted by scanning the narrative blocks array. If blocks are wrong, sidebar is wrong.

## Solution

Two changes:

1. **Session State API** (`GET /api/sessions/:id/state`) — backend assembles a structured snapshot of any session's current state. Frontend renders it directly. No log parsing.
2. **UI redesign** — center panel shows only user messages, phase indicators, and agent responses. All execution detail (subagents, tool calls, facts, intent, plan) moves to the sidebar.

---

## 1. Session State API

### Endpoint

`GET /api/sessions/:id/state`

Returns a complete, renderable snapshot of the session.

### Response Shape

```typescript
interface SessionState {
  session: {
    id: string;
    title: string | null;
    status: "running" | "completed" | "error" | "stopped";
    startedAt: string;
    durationMs: number;
    tokenCount: number;
    model: string | null;
  };

  // Center panel
  userMessage: string | null;
  phase: "intent" | "planning" | "executing" | "responding" | "completed" | "error";
  response: string | null;

  // Sidebar
  intentAnalysis: IntentAnalysis | null;
  ward: { name: string; content: string } | null;
  recalledFacts: RecalledFact[];
  plan: PlanStep[];
  subagents: SubagentState[];

  // Reconnection
  isLive: boolean;
}

interface SubagentState {
  agentId: string;
  executionId: string;
  task: string;
  status: "queued" | "running" | "completed" | "error";
  durationMs: number | null;
  tokenCount: number | null;
  toolCalls: ToolCallEntry[];
}

interface ToolCallEntry {
  toolName: string;
  status: "running" | "completed" | "error";
  durationMs: number | null;
  summary: string | null;
}
```

### How the Backend Builds This

All data comes from existing tables — no schema changes.

| Field | Source |
|-------|--------|
| `session` | `agent_executions` table (root execution for session_id) |
| `userMessage` | First `role=user` row in `messages` table for the session's conversation_id |
| `phase` | Derived: has intent log? → has plan/delegation? → has respond? → status=completed? |
| `response` | `respond` tool_call args.message from `execution_logs`; fallback: last assistant message from `messages` |
| `intentAnalysis` | `execution_logs` where category=intent, parse metadata JSON |
| `ward` | `execution_logs` where tool=ward/enter_ward, or from intent analysis ward_recommendation |
| `recalledFacts` | `execution_logs` where tool=memory+recall, parse tool_result JSON |
| `plan` | Latest `update_plan` tool_call args from `execution_logs` |
| `subagents` | Child rows in `agent_executions` for this session + their `execution_logs` tool_calls |
| `isLive` | `session.status === "running"` |

### Phase Derivation Logic

```
if session.status in ("completed", "stopped"):
    phase = "completed"
elif session.status == "error":
    phase = "error"
elif has respond tool_call or has assistant message:
    phase = "responding"
elif has delegation_started or has tool_calls beyond intent/plan:
    phase = "executing"
elif has update_plan tool_call:
    phase = "planning"
elif has intent log:
    phase = "intent"
else:
    phase = "intent"  // just started
```

---

## 2. UI Redesign

### Layout

3-panel layout (unchanged structure, redesigned content):

```
┌─────────────────────────────────────────────────────────────┐
│ Session Bar: [+ New] Title          Status  Tokens  Duration │
├──────────────────────────────────┬──────────────────────────┤
│                                  │                          │
│  CENTER PANEL                    │  RIGHT SIDEBAR           │
│                                  │                          │
│  User Message                    │  Intent Analysis  [▶]    │
│                                  │  ─────────────────────   │
│  Phase Indicators                │  Active Ward             │
│    ✓ Analyzing intent            │  ─────────────────────   │
│    ✓ Planning (3 agents)         │  Recalled Facts    [5]   │
│    ⟳ Executing (1/3 complete)    │  ─────────────────────   │
│    ○ Generating response         │  Subagents      2 active │
│                                  │   ⟳ research-agent       │
│  [Response appears here]         │     ✓ web_search  0.9s   │
│                                  │     ⟳ python  running    │
│                                  │   ✓ data-analyst   8.7s  │
│                                  │  ─────────────────────   │
│                                  │  Execution Plan     2/5  │
│                                  │    ✓ Fetch positions     │
│                                  │    ⟳ Run correlation     │
│                                  │    ○ Calculate VaR       │
├──────────────────────────────────┤                          │
│  [Chat input - disabled while    │                          │
│   agent is working]              │                          │
└──────────────────────────────────┴──────────────────────────┘
```

### Center Panel — Phase State Machine

Phases advance based on WebSocket events:

```
idle → intent → planning → executing → responding → completed
                                                  → error
```

| Event | Phase Transition |
|-------|-----------------|
| User sends message | `idle` → `intent` |
| `intent_analysis_complete` | `intent` → `planning` |
| `tool_call` where tool=`update_plan` or `delegate_to_agent` | `planning` → `executing` |
| `tool_call` where tool=`respond` OR first token stream | `executing` → `responding` |
| `agent_completed` or `turn_complete` | → `completed` |
| `error` | → `error` |

**Center renders per phase:**

| Phase | Content |
|-------|---------|
| `idle` | HeroInput (landing) |
| `intent` | User message + ⟳ Analyzing intent |
| `planning` | User message + ✓ Intent, ⟳ Planning |
| `executing` | User message + ✓ Intent, ✓ Planning, ⟳ Executing (n/m agents) |
| `responding` | User message + all phases ✓ + streaming response |
| `completed` | User message + all phases ✓ + final response |
| `error` | User message + last phase ✗ + error message |

**Multi-turn:** After completion, input re-enables. New message starts a new phase cycle below the previous response. Center becomes a scrollable conversation of `[user → phases → response]` blocks.

### Sidebar Sections

5 collapsible sections, each populated independently:

1. **Intent Analysis** (collapsed by default) — from `intent_analysis_complete` event or snapshot
2. **Active Ward** (open) — from `ward_changed` event or intent analysis
3. **Recalled Facts** (collapsed, badge with count) — from memory recall tool result
4. **Subagents** (open when active) — each subagent is a card:
   - Active: expanded, blue border, pulsing dot, live tool call list
   - Completed: collapsed to one line with duration + tool count
   - Error: red border with error message
   - Tool calls appear inline under their subagent, ordered chronologically
5. **Execution Plan** (open) — task checklist from `update_plan` tool, shows progress badge

### Sidebar Data Flow

**Live (staying on page):** Events update sidebar sections directly.

| Section | Updated by |
|---------|-----------|
| Intent Analysis | `intent_analysis_complete` event |
| Ward | `ward_changed` event |
| Recalled Facts | `tool_result` where tool=memory+recall |
| Subagents | `delegation_started/completed/error` + `tool_call/tool_result` with child execution_id |
| Plan | `tool_call` where tool=`update_plan` |

**Reconnect (reopen browser):** All sections populated from `GET /api/sessions/:id/state` snapshot.

**Subagent tool call routing:** Frontend maintains `Map<executionId, agentId>` to route tool_call events to the correct subagent card. On reconnect, snapshot pre-groups tool calls under each subagent.

---

## 3. Session Lifecycle

### New Session

```
1. User types message, hits send
2. POST /api/agents/:id/execute → { sessionId }
3. Store sessionId in localStorage
4. Subscribe to WebSocket with sessionId
5. Events → update phases (center) + sidebar
6. respond tool or token stream → response in center
7. agent_completed → completed, enable input
```

### Reconnect (Reopen Browser)

```
1. Page loads, find sessionId in localStorage
2. GET /api/sessions/:id/state → SessionState
3. Render center: user message + phases + response (if exists)
4. Render sidebar: all 5 sections from snapshot
5. If isLive: subscribe to WebSocket, new events append to state
6. If !isLive: static render, no WebSocket
```

### Switch to Past Session

```
1. User picks session from SessionBar history
2. Store sessionId, GET /api/sessions/:id/state
3. Render static completed session
```

### Multi-turn Continuation

```
1. User sends follow-up on completed session
2. POST execute with existing sessionId
3. Append new user message + fresh phases below previous response
4. Reset subagents and intent analysis (new delegation cycle), keep ward/facts/plan
5. Stream events as normal
```

---

## 4. Files Changed

### Backend (new)

| File | Change |
|------|--------|
| `gateway/src/http/sessions.rs` | Add `GET /api/sessions/:id/state` handler |
| `gateway/src/http/mod.rs` | Register route |
| `gateway/gateway-execution/src/lib.rs` | Add `SessionStateBuilder` — assembles SessionState from existing DB tables |

No new tables, no schema changes.

### Frontend (modify)

| File | Change |
|------|--------|
| `services/transport/types.ts` | Add `SessionState`, `SubagentState`, `ToolCallEntry` types |
| `services/transport/interface.ts` | Add `getSessionState(id)` method |
| `services/transport/http.ts` | Implement `getSessionState` |
| `features/chat/mission-hooks.ts` | Replace `loadSession()` with snapshot-based hydration. Separate sidebar state from blocks. Add phase state machine for live events. |
| `features/chat/MissionControl.tsx` | Render center as user message → PhaseIndicators → response. Pass sidebar props independently. |
| `features/chat/IntelligenceFeed.tsx` | Accept `SubagentState[]` with tool calls for inline rendering |

### Frontend (new)

| File | Purpose |
|------|---------|
| `features/chat/PhaseIndicators.tsx` | 4-phase progress component for center panel |

### Frontend (removed from center, sidebar-only or deprecated)

| File | Status |
|------|--------|
| `ToolExecutionBlock.tsx` | No longer in center. Tool calls render inline in sidebar subagent cards. |
| `DelegationBlock.tsx` | Replaced by subagent cards in sidebar. |
| `RecallBlock.tsx` | Recalled facts only in sidebar. |
| `ExecutionNarrative.tsx` | Simplified — renders user messages, phase indicators, and responses only. |

### Frontend (unchanged)

- `SessionBar.tsx`, `ChatInput.tsx`, `HeroInput.tsx` — same
- `AgentResponse.tsx`, `UserMessage.tsx` — same
- `PlanBlock.tsx`, `IntentAnalysisBlock.tsx` — same (sidebar)
