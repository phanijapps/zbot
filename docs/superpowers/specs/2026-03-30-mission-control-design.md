# Mission Control — Chat Reimagined

**Date:** 2026-03-30
**Status:** Approved
**Replaces:** `WebChatPanel.tsx` (900+ lines slide-out chat)

## Problem Statement

The current chat is a message list that hides the agent's real work. Intent analysis runs as invisible middleware. Tool calls are compressed into a rolling counter. Subagent activity is a side panel. Memory recall is hidden. The user can't see what's happening, must refresh to get updates, and every session is called "root."

## What This Builds

A full-page **Mission Control** — an execution theater where you watch your agent think, recall, plan, and act in real-time. Every agent action is a visible block in the conversation. An Intelligence Feed sidebar shows live ward state, recalled facts, active subagents, and the execution plan.

## Design Principles

1. **Every action visible.** No hidden middleware. Intent understanding, recall, tool calls, delegations — all rendered as distinct blocks.
2. **Intelligence persists.** Right sidebar keeps recalled facts and plan visible as the conversation scrolls.
3. **Sessions have identity.** Agent-generated titles, not "root."
4. **One page.** Not a slide-out. Full-page with session bar, narrative, sidebar, and input.

---

## Part 1: Backend Changes

### 1.1 Remove Intent Analysis Middleware

**Current:** `analyze_intent()` runs in `create_executor()` as middleware. Makes an LLM call, modifies system prompt in-place. Invisible.

**After:** Remove the middleware LLM call entirely. The agent's own first-turn reasoning replaces it.

**What stays:**
- `index_resources()` (skills/agents/wards upserted into memory) → moves to a daemon startup hook. Fast (<100ms), no LLM, just DB upserts. Re-runs on first session of the day or on skill/agent changes.

**What moves to agent instructions:**
- Hidden intent extraction → instruction shard: "Consider what the user explicitly asked AND what they would implicitly expect"
- Skill/agent selection → handled by `memory.recall` (skill/agent indices already in results)
- Ward recommendation → agent recalls ward facts, calls `ward` tool
- Execution strategy → agent calls `update_plan`

**Files changed:**
- `gateway/gateway-execution/src/runner.rs` — remove intent analysis call from `create_executor()`
- `gateway/gateway-execution/src/middleware/intent_analysis.rs` — keep `index_resources()`, remove `analyze_intent()` and `inject_intent_context()`
- `gateway/templates/shards/` — update instruction shards with first-turn protocol

**Agent instruction shard (new: `first_turn_protocol.md`):**
```markdown
## First Turn Protocol
On every new task from the user:
1. Call memory.recall to get relevant knowledge, corrections, past experiences, and available skills
2. Call set_session_title with a concise title for this task
3. Switch to the appropriate ward if needed
4. Call update_plan with your execution steps
5. Begin execution

Consider not just what the user explicitly asked, but what they would implicitly expect:
save results to the ward, follow established patterns, handle errors gracefully.
```

### 1.2 Session Title Tool

**New tool:** `set_session_title`

```json
{
  "name": "set_session_title",
  "description": "Set a human-readable title for the current session. Call this early so the UI shows a meaningful name.",
  "parameters": {
    "title": { "type": "string", "description": "Concise title (2-8 words) describing the task" }
  }
}
```

**Implementation:** Updates `sessions.title` column (already exists, currently NULL).

```rust
// In the tool's execute():
state_service.update_session_title(session_id, &title)?;
// Emit event so UI updates in real-time:
event_bus.broadcast(GatewayEvent::SessionTitleChanged { session_id, title });
```

**New WebSocket event:** `SessionTitleChanged { session_id, title }` — UI updates the session bar immediately.

**Files:**
- New: `runtime/agent-tools/src/tools/session_title.rs`
- Modify: `gateway/gateway-events/src/lib.rs` — add `SessionTitleChanged` event
- Modify: `gateway/gateway-execution/src/runner.rs` — register tool
- Modify: `services/execution-state/` — add `update_session_title()` method

### 1.3 Subagent Output Schemas

**Modify:** `delegate_to_agent` tool

Add optional `output_schema` parameter:

```json
{
  "name": "delegate_to_agent",
  "parameters": {
    "agent_id": "string (required)",
    "task": "string (required)",
    "context": "object (optional)",
    "output_schema": "object (optional) — JSON Schema the child must follow",
    "wait_for_result": "boolean (default: false)",
    "max_iterations": "integer (optional)"
  }
}
```

**When `output_schema` is provided:**
1. Inject into child agent's system prompt: "Your response MUST be a JSON object matching this schema: {schema}. Respond with ONLY the JSON object."
2. On child's `respond()`: validate response against schema
3. Valid JSON matching schema → pass to root as-is
4. Prose (not JSON) → wrap as `{ "summary": "<prose>", "_schema_valid": false }`
5. Invalid JSON → log warning, pass as-is with `_schema_valid: false`

**Validation is lenient.** Never blocks or discards the child's work. Failed matches are logged and will be distilled as corrections.

**Files:**
- Modify: `runtime/agent-tools/src/tools/delegate.rs` — add output_schema parameter
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs` — inject schema into child prompt
- Modify: `gateway/gateway-execution/src/delegation/callback.rs` — validate response

### 1.4 File Upload Endpoint

**New endpoint:** `POST /api/upload`

Accepts multipart form data. Stores file in `~/Documents/zbot/data/uploads/{uuid}.{ext}`.

Returns:
```json
{
  "id": "upload-uuid",
  "filename": "data.xlsx",
  "mime_type": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  "size": 45678,
  "path": "data/uploads/upload-uuid.xlsx"
}
```

The upload reference is included in the user message sent to the agent:
```json
{
  "message": "Analyze this spreadsheet",
  "attachments": [{ "id": "upload-uuid", "filename": "data.xlsx", "mime_type": "...", "path": "..." }]
}
```

The agent receives attachments as context. For files: the agent can `read` the file. For images: forward path to vision models (not in current scope — include the path, agent gets a text note: "[Image attached: chart.png, 1.2MB]").

**Files:**
- New: `gateway/src/http/upload.rs`
- Modify: `gateway/src/http/mod.rs` — register route
- Modify: WebSocket message handling — include attachments in user message

---

## Part 2: UI — Mission Control

### 2.1 Page Layout

```
┌──────────────────────────────────────────────────────────┐
│ SESSION BAR                                              │
├────────────────────────────────────┬─────────────────────┤
│ EXECUTION NARRATIVE (scrollable)   │ INTELLIGENCE FEED   │
│                                    │ (fixed sidebar)     │
├────────────────────────────────────┴─────────────────────┤
│ INPUT BAR                                                │
└──────────────────────────────────────────────────────────┘
```

### 2.2 Session Bar (Top)

- **Title** — from `set_session_title` tool call, or "New Session" until set. Updates via `SessionTitleChanged` WebSocket event.
- **Agent badge** — root agent name
- **Status dot** — green pulsing (running), green solid (completed), red (failed/crashed)
- **Metrics** — token count (live), duration (live), model name
- **Controls** — Stop button, Export

### 2.3 Execution Narrative (Center)

Scrollable message list. Each message rendered by type:

**UserMessage** — avatar (U), timestamp, text content, attachment chips (files) or inline thumbnails (images).

**RecallBlock** — purple left border. Triggered by `memory.recall` tool call. Parsed from tool result JSON:
- Corrections at top (red text, "⚠ NEVER...")
- Past episodes (outcome badges: ✓/✗)
- Domain facts below
- Collapsible if > 5 items

**ToolExecutionBlock** — dark background, expandable:
- Header: tool icon + name + command summary + duration + status badge
- Body (collapsed by default): full input/output, monospace
- Error state: red border + error message visible without expanding

**DelegationBlock** — green left border:
- Header: agent name + task description
- Live stats: tool call count, token count, duration (updating in real-time)
- Status: pulsing dot (active) → completed badge
- If `output_schema` was provided: structured result rendered as key-value card
- If no schema: result summary as text

**PlanBlock** — from `update_plan` tool result:
- Checklist: ✓ completed, ⟳ in progress, ○ pending
- Updates live as agent marks steps done

**AgentResponse** — avatar (Z), timestamp, markdown-rendered text. The final `respond()` output.

### 2.4 Intelligence Feed (Right Sidebar, 260px)

Four fixed sections, all data-driven from tool calls and WebSocket events:

**Active Ward** — ward name. Content populated from ward tool result (agents_md excerpt). Updates when agent calls `ward` tool.

**Recalled Facts** — mirrors RecallBlock but persists while scrolling. Corrections pinned at top with red border. Updates on each `memory.recall` tool call.

**Subagents** — from `DelegationStarted`/`DelegationCompleted` WebSocket events. Agent name, status dot, task summary. Click scrolls to the DelegationBlock in center.

**Execution Plan** — from `update_plan` tool results. Checklist synced with PlanBlock in center. Always visible for reference.

### 2.5 Input Bar (Bottom)

- **Attachment buttons:** 📎 file picker, 🖼️ image picker
- **Text input:** multiline, Enter to send, Shift+Enter for newline
- **Send button**
- Pending attachments shown as removable chips above the input
- File upload: calls `POST /api/upload`, gets reference, includes in message

### 2.6 Component Architecture

| Component | Responsibility |
|---|---|
| `MissionControl.tsx` | Page layout: session bar + narrative + sidebar + input |
| `SessionBar.tsx` | Title, agent, status, metrics, controls |
| `ExecutionNarrative.tsx` | Scrollable list, renders blocks by message type |
| `UserMessage.tsx` | Avatar, text, attachments |
| `RecallBlock.tsx` | Purple recall card parsed from memory.recall result |
| `ToolExecutionBlock.tsx` | Expandable dark tool card |
| `DelegationBlock.tsx` | Green delegation card with live stats |
| `PlanBlock.tsx` | Checklist from update_plan |
| `AgentResponse.tsx` | Avatar, markdown response |
| `IntelligenceFeed.tsx` | Right sidebar: ward + facts + subagents + plan |
| `ChatInput.tsx` | Text input + attachment buttons + file upload |
| `mission-hooks.ts` | WebSocket events, message parsing, state |

### 2.7 Message Type Detection

Tool calls from the agent arrive as WebSocket events. The UI determines block type from the tool name:

| Tool Name | Block Type |
|---|---|
| `memory` (action: recall) | RecallBlock |
| `shell`, `read`, `write`, `edit`, `glob`, `grep`, `python`, `web_fetch` | ToolExecutionBlock |
| `delegate_to_agent` | DelegationBlock |
| `update_plan` | PlanBlock |
| `set_session_title` | Updates SessionBar (no block) |
| `ward` | Updates Ward section in sidebar + compact tool block |
| `respond` | AgentResponse |
| Everything else | Generic ToolExecutionBlock |

### 2.8 Real-Time Event Mapping

| WebSocket Event | UI Update |
|---|---|
| `Token` | AgentResponse streams. Token counter ticks. |
| `ToolCall` | New block appears (type determined by tool name) |
| `ToolResult` | Block updates with result, duration finalizes |
| `DelegationStarted` | DelegationBlock appears + sidebar updates |
| `DelegationCompleted` | Block shows result + sidebar updates |
| `AgentCompleted` | Status → completed, duration finalizes |
| `SessionTitleChanged` | Session bar title updates |

---

## Part 3: Forward Path

### Vision Model Support (future)
When the model registry indicates vision capability:
- Uploaded images sent as base64 in LLM message content array
- Image thumbnails render inline in UserMessage blocks
- No changes to Mission Control layout — images just become part of the conversation

### Agent Apps (future, low priority)
Wards with established execution patterns become reusable "Agent Apps":
- A ward with a proven execution graph (e.g., "financial analysis" with fetch→analyze→report pipeline)
- Can be invoked with parameters ("run financial-analysis for AAPL")
- Has its own UI entry point derived from the ward's structure
- Managed by Mission Control as first-class entities

---

## Schema Changes

### New WebSocket Event
- `SessionTitleChanged { session_id: String, title: String }`

### New API Endpoint
- `POST /api/upload` — multipart file upload

### Modified Tool
- `delegate_to_agent` — add optional `output_schema` parameter

### New Tool
- `set_session_title` — sets session title in DB + emits event

### New Instruction Shard
- `gateway/templates/shards/first_turn_protocol.md`

### Removed
- Intent analysis LLM call from `create_executor()` middleware
- `analyze_intent()` function (keep `index_resources()`)
- `inject_intent_context()` function

---

## What Stays the Same

- WebSocket transport layer (same events, same subscription model)
- All existing tools (memory, ward, shell, delegate, etc.)
- Execution engine (runner, executor, delegation system)
- Memory/recall system (tool-call based recall, priority engine, graph traversal)
- Observatory and Execution Intelligence Dashboard (separate pages)
- Agent configurations and provider system
