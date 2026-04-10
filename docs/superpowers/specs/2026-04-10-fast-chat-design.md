# Fast Chat Mode — Design Spec

## Problem

The current chat takes 2-5 seconds before the agent starts responding — intent analysis LLM call, resource indexing, embedding searches. For simple questions ("what's in this file?", "run this command", "explain this code"), the overhead is unnecessary and frustrating.

## Solution

Add a `/chat` route — a fast, simple chat mode that skips intent analysis, resource indexing, and the planning pipeline. Direct conversation with the agent using the same tools, memory, and delegation capabilities, but without the orchestration overhead.

## Design

### Two Modes

| | `/` (Goal Mode) | `/chat` (Fast Mode) |
|---|---|---|
| Intent analysis | Yes (1-3s LLM call) | **No** |
| Resource indexing | Yes (100-300ms) | **No** |
| Execution strategy | Yes (planning, graph) | **No** |
| Memory recall | Yes | Yes |
| Tools | All | All |
| Delegation | Full orchestration | Direct delegation |
| Skills | Auto-recommended | Load on demand |
| System prompt | Full (SOUL + INSTRUCTIONS + OS + 4 shards + intent injection) | Lean (SOUL + INSTRUCTIONS + OS + tooling_skills only) |
| UI | Intelligence feed sidebar, phase indicators, subagent panels | **Clean chat only** — no sidebar |
| Session persistence | Yes | Yes |
| Attachments | Yes | Yes |
| Thinking mode | Configurable | **Off by default** (speed) |

### Backend: Skip Intent Analysis

Add `skip_intent_analysis: bool` to `ExecutionConfig`. When true:
- Skip the intent analysis LLM call (runner.rs line ~1451)
- Skip resource indexing (runner.rs line ~1454)
- Skip intent memory recall
- Skip intent injection into system prompt
- Still run first-message memory recall (useful context)

The root agent runs the same executor, same tools, same LLM — just without the middleware overhead.

### Backend: Lean System Prompt

For fast chat sessions, assemble a shorter system prompt:
- SOUL.md (identity)
- INSTRUCTIONS.md (execution rules)
- OS.md (platform)
- tooling_skills.md shard only (tool reference)
- Skip: planning_autonomy, memory_learning, first_turn_protocol shards

This reduces the system prompt from ~15-23KB to ~8-12KB, saving input tokens on every turn.

### Backend: Trigger via Invoke

The WebSocket `Invoke` message already has metadata. Add a `mode: "fast" | "deep"` field (default "deep" for backward compat). When `mode: "fast"`:
- Create session with `skip_intent_analysis: true`
- Use lean system prompt
- Set `thinking_enabled: false` by default (overridable)

### Frontend: `/chat` Route

A new page with a clean, minimal UI:

```
┌─────────────────────────────────────────────────┐
│  z-Bot  ●  Fast Chat              [Go Deep →]   │
├─────────────────────────────────────────────────┤
│                                                  │
│  User: What's in src/auth.rs?                   │
│                                                  │
│  Agent: Here's the contents of src/auth.rs:     │
│  [code block with file contents]                │
│                                                  │
│  User: Add a rate limiter middleware             │
│                                                  │
│  Agent: I'll create the middleware...            │
│  [tool calls: edit, shell cargo test]            │
│                                                  │
├─────────────────────────────────────────────────┤
│ Type a message...                    [📎] [Send] │
└─────────────────────────────────────────────────┘
```

**No sidebar.** No phase indicators. No intelligence feed. Just messages and tool execution blocks (same ToolExecutionBlock component from current chat).

**Header:** z-Bot logo + "Fast Chat" label + "Go Deep →" button (stretch goal).

**Chat input:** Same ChatInput component with attachments support.

### Frontend: Navigation

Add "Chat" to the sidebar navigation alongside existing links:
- Chat (fast) — `/chat`
- Home (goal-oriented) — `/`
- Dashboard — `/dashboard`
- Logs — `/logs`
- Settings — `/settings`

### Stretch Goal: "Go Deep" Handoff

A button in the fast chat header that:
1. Takes the current conversation context (user messages + key findings)
2. Creates a new goal-oriented session on `/`
3. Injects the context as the first message
4. Navigates to `/` with the new session

This lets users start fast and escalate when they realize the task needs full orchestration.

Not required for v1 — just the button placeholder.

## Scope

### In Scope
- `skip_intent_analysis` flag in ExecutionConfig
- Lean system prompt assembly (fewer shards)
- `mode` field in WebSocket Invoke message
- `/chat` route with minimal UI
- Navigation link
- Same tools, memory, delegation, streaming

### Out of Scope
- Auto-detection of fast vs deep (user chooses)
- "Go Deep" handoff (stretch — just button placeholder)
- Separate session history for fast chat (uses same sessions table)
- Different model for fast chat (uses same orchestrator model)
- Mobile-specific layout

## Files to Modify/Create

### Backend (Rust)
| File | Change |
|------|--------|
| `gateway/gateway-ws-protocol/src/messages.rs` | Add `mode` field to `Invoke` message |
| `gateway/gateway-execution/src/runner.rs` | Honor `skip_intent_analysis` flag |
| `gateway/gateway-templates/src/lib.rs` | Add lean prompt assembly function |
| `gateway/src/websocket/handler.rs` | Pass mode through to execution config |

### Frontend (TypeScript/React)
| File | Change |
|------|--------|
| `apps/ui/src/features/chat/FastChat.tsx` | New: minimal chat page |
| `apps/ui/src/features/chat/fast-chat-hooks.ts` | New: simplified hooks (no intelligence feed) |
| `apps/ui/src/App.tsx` | Add `/chat` route |
| `apps/ui/src/services/transport/http.ts` | Pass `mode: "fast"` in executeAgent |
