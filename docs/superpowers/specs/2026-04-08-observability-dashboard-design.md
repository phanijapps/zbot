# Observability Dashboard — Design Spec

## Problem

The current Logs page (`/logs`) doesn't show tool calls in subagents. It uses a waterfall visualization that only renders root-level activity. Users can't see the narrative flow of what happened: root agent → delegated to subagent → subagent called tools → results. This makes it hard to understand what the agent did and why.

## Solution

Replace the Logs page with a full observability dashboard using a **List + Detail Split** layout and a **Timeline Tree** for session detail. The timeline tree renders the complete execution hierarchy: root agent → subagents → tool calls, as an indented vertical timeline like a git log.

## Audience

End users of z-Bot who want to understand what the agent did. Primary view is narrative (what happened, with tool calls visible). Token counts and timing are secondary, shown per-node.

## Page Structure

Three zones:

| Zone | Width | Content |
|------|-------|---------|
| KPI bar | full width, ~48px | Session count, success rate, total tokens, avg duration |
| Session list | 300px fixed, left | Filterable compact rows: title, agent count, status, duration |
| Timeline tree | remaining space, right | Hierarchical narrative: root → subagents → tool calls |

### KPI Bar

Single row at the top. Aggregates from the fetched session list:
- Session count (today or filtered period)
- Success rate (completed / total)
- Total tokens consumed
- Average session duration

### Session List (Left Panel)

Compact rows showing:
- Session title (or truncated first user message)
- Number of subagents delegated
- Status badge: completed (green), crashed (red), running (blue/pulse), paused (yellow)
- Duration
- Token count

Features:
- Text filter (searches title/agent name)
- Selected row highlighted
- Running sessions show a subtle pulse indicator
- Scrollable, loads most recent first

### Timeline Tree (Right Panel)

Vertical indented tree. Each line is a node. Four node types:

**Root node:**
```
● root — "Build the auth system"                    24.5s · 12,730 tokens
```

**Tool call node (indented under parent agent):**
```
  ● shell — cargo test                               1.8s
  ● edit — src/auth.rs                                0.1s
```

**Delegation node (indented under root, expandable):**
```
  ▲ code-agent — "Implement auth middleware"          14.2s · 8,100 tokens
    ● shell — cargo check                             3.2s
    ● edit — src/auth.rs                               0.1s
    ● shell — cargo test                               FAIL 1.8s
```

**Error node (red):**
```
    ● shell — cargo test                               FAIL 1.8s
```

**Interaction:**
- Nodes are collapsed by default (single line each)
- Click a node to expand and show detail: full arguments, full result text, error trace
- Delegation nodes expand/collapse their child tool calls
- Internal tools filtered out: `analyze_intent`, `update_plan`, `set_session_title`

**Color scheme:**
- Green: completed successfully
- Red: error/crash/failed tool call
- Blue: running/in-progress
- Gray: pending/queued
- Agent-specific colors for delegation nodes (consistent per agent name)

## Data Flow

### Historical Sessions (completed/crashed)

1. Page load → `GET /api/logs/sessions` → populate session list
2. User clicks session → `GET /api/logs/sessions/{id}` → root session detail with logs
3. For each entry in `child_session_ids[]` → `GET /api/logs/sessions/{child_id}` → subagent logs
4. Build execution map: group log entries by `agent_id`, order by timestamp, nest tool calls under their agent
5. Render timeline tree from the execution map

### Real-time Sessions (running)

1. Same HTTP fetch for initial state (catches up on what already happened)
2. Subscribe WebSocket: `{ type: "subscribe", conversation_id: session_id, scope: "all" }`
3. Build execution lookup map: `execution_id → { agent_id, task }`
4. On `DelegationStarted` → register `child_execution_id → child_agent_id` in map, add delegation node
5. On `ToolCall` → look up `execution_id` in map → append tool call node under correct agent
6. On `ToolResult` → update matching tool call node with result/duration/status
7. On `DelegationCompleted` → mark delegation node as completed
8. On `AgentCompleted` / `Error` → update root node status

### Session Switching

- User clicks different session in left panel
- Unsubscribe from previous session's WebSocket (if any)
- HTTP fetch new session's detail
- Subscribe to new session's WebSocket (if running)

### No Backend Changes Needed

All events (`ToolCall`, `ToolResult`, `DelegationStarted`, `DelegationCompleted`) and endpoints (`/api/logs/sessions`, `/api/logs/sessions/{id}`) already exist. The `scope: "all"` subscription already delivers subagent events with `execution_id` for tagging. This is purely a UI change.

## Component Breakdown

| Component | Responsibility |
|-----------|---------------|
| `ObservabilityDashboard` | Page layout (KPI bar + split panels), coordinates list + detail |
| `SessionList` | Left panel: fetch sessions, filter input, selection state |
| `SessionListItem` | Single row: title, status badge, stats |
| `TraceTimeline` | Right panel: renders the timeline tree from execution map |
| `TraceNode` | Single node line (root, tool call, delegation, or error) |
| `TraceNodeDetail` | Expanded detail view (full args, result, error trace) |
| `useSessionTrace` | Hook: merges HTTP history + WebSocket real-time into unified tree |
| `useTraceSubscription` | Hook: WebSocket subscribe/unsubscribe lifecycle per selected session |

### Styling

Follow existing CSS-first patterns from `ARCHITECTURE.md`:
- Component classes: `.trace-timeline`, `.trace-node`, `.trace-node--tool`, `.trace-node--delegation`, `.trace-node--error`, `.trace-node--expanded`
- BEM naming convention
- Design tokens from `theme.css` for colors, spacing, typography
- No inline styles except dynamic indentation depth (`padding-left: ${depth * 20}px`)

## Files to Create/Modify

| File | Change |
|------|--------|
| `apps/ui/src/features/logs/ObservabilityDashboard.tsx` | New: main page component |
| `apps/ui/src/features/logs/SessionList.tsx` | New: left panel session list |
| `apps/ui/src/features/logs/SessionListItem.tsx` | New: single session row |
| `apps/ui/src/features/logs/TraceTimeline.tsx` | New: timeline tree renderer |
| `apps/ui/src/features/logs/TraceNode.tsx` | New: single tree node |
| `apps/ui/src/features/logs/TraceNodeDetail.tsx` | New: expanded node detail |
| `apps/ui/src/features/logs/useSessionTrace.ts` | New: data merging hook |
| `apps/ui/src/features/logs/useTraceSubscription.ts` | New: WebSocket lifecycle hook |
| `apps/ui/src/features/logs/WebLogsPanel.tsx` | Modify: point to ObservabilityDashboard |
| `apps/ui/src/features/logs/observability.css` | New: component styles |
| `apps/ui/src/features/logs/log-hooks.ts` | Modify: may reuse `useLogSessions` for list |

## Decisions

- **`scope: "all"` for WebSocket** — simpler than per-execution subscriptions. Every event includes `execution_id` for tagging to the correct agent.
- **No backend changes** — all data/events exist. Pure UI feature.
- **Replace, don't add** — replaces the current Logs page rather than adding a new route. The current waterfall/KPI view is superseded.
- **Internal tools filtered** — `analyze_intent`, `update_plan`, `set_session_title` hidden from tree to reduce noise.
- **Single WebSocket subscription per selected session** — subscribe only to the session the user is viewing, not all sessions.

## Out of Scope

- Cost estimation (dollar amounts)
- Flamegraph/gantt chart visualization
- Log search/grep across sessions
- Export/download logs
- Session comparison
- Custom time range filtering (use simple text filter for now)
