# Execution Intelligence Dashboard — Design Spec

**Date:** 2026-03-30
**Status:** Approved
**Replaces:** Current flat `WebLogsPanel.tsx` (845 lines)

## Problem Statement

The current logs page is a flat hierarchical text list. Sessions are expandable rows showing chronological log entries. No visualizations, no real-time updates, no performance insights. The data is rich (timestamps, durations, tool calls, delegations, errors, token counts) but the UI treats it as a text dump. Users must manually refresh to see updates.

## Design Principles

1. **One page, not tabs.** KPIs always visible at top. Sessions are the content. Click to expand detail.
2. **Inline waterfalls.** Every session row shows a mini execution waterfall — at a glance you see execution shape, delegation count, time distribution.
3. **Real-time.** WebSocket event streaming updates the page live. Running sessions grow as you watch.
4. **Errors surface automatically.** Red dots in waterfalls, error counts in rows, inline error callouts on expand.
5. **No new backend APIs.** All data exists in execution_logs and session summaries. One new WebSocket subscription on the frontend.

---

## Section 1: Page Layout

### Structure

```
┌─────────────────────────────────────────────────────────────┐
│ KPI Cards (4)                                               │
│ [Success Rate + sparkline] [Tokens + sparkline] [Tools] [Duration + sparkline] │
├─────────────────────────────────────────────────────────────┤
│ Filter Bar: [Agent pills] [Error/Warn toggles] [Search]    │
├─────────────────────────────────────────────────────────────┤
│ Session Row: ● MSFT Analysis  root  [===|||===]  26m  3del 180K  1err │
│   ▼ Expanded: full waterfall + tool dots + error callouts   │
│ Session Row: ● AMD Analysis   root  [==|====]    18m  1del 220K       │
│ Session Row: ● INR Query      root  [=]          45s       12K        │
│ Session Row: ✕ Spotify        root  [x]          12s       3K   fail  │
│ ...                                                         │
└─────────────────────────────────────────────────────────────┘
```

### KPI Cards (Top Row)

Four metric cards, always visible:

| Card | Primary Metric | Secondary | Sparkline |
|---|---|---|---|
| Success Rate | `92%` (green) | 85 success / 7 failed / 1 crashed | 7-day trend |
| Total Tokens | `19.3M` | avg 208K in / 14.6K out per session | 7-day trend |
| Tool Calls | `3,097` | shell: 2100, memory: 340, delegate: 75 | Tool type breakdown (inline text) |
| Avg Duration | `4m 12s` | fastest: 18s / slowest: 26m | 7-day trend |

**Sparklines:** Tiny SVG polylines (60x24px) showing the metric trend over the last 7 days or last 20 sessions. Rendered inline using simple SVG — no chart library needed.

**Data source:** Aggregate from existing `GET /api/logs/sessions` response. Compute on the frontend from the session list.

### Filter Bar

- **Agent filter pills:** All | root | data-analyst | code-agent | research-agent (from session list)
- **Level toggles:** "Errors Only" (red), "Warnings" (amber) — filter sessions by error/warning presence
- **Search:** Free text search on session title (derived from first user message), agent ID, session ID

### Session Rows

Each row shows:
- **Status dot:** green (completed), red (failed/crashed), blue pulsing (running)
- **Session title:** Derived from first user message (truncated to ~30 chars)
- **Agent:** root agent ID
- **Inline mini waterfall:** Small SVG (flex: 1, 16px height) showing execution shape:
  - Root span as a thin bar (primary color)
  - Delegation spans as shorter bars below (green)
  - Proportional to duration
- **Duration:** Monospace, right-aligned
- **Delegation count:** "3 deleg"
- **Token count:** "180K tok"
- **Error count:** Red, only if > 0

**Click to expand** → full waterfall detail view.

---

## Section 2: Session Waterfall (Expanded View)

When a session row is clicked, it expands to show a full execution waterfall:

### Waterfall Timeline

An SVG visualization (100% width, ~120px height) showing:

- **Time axis:** Bottom of the SVG, labeled with absolute time markers (0s, 5m, 10m, etc.)
- **Agent lanes:** Horizontal rows, one per agent involved:
  - Root agent: full-width translucent bar (background) + opaque segments for active periods (planning, synthesis)
  - Each delegated agent: colored bar positioned at start/end time, labeled with task name and duration
- **Tool call markers:** Dots along a "tools" lane at the bottom:
  - Amber = shell
  - Purple = memory
  - Green = delegate
  - Red = error (larger dot)
  - Clicking a dot shows tool details in a tooltip

### Inline Error Callouts

Below the waterfall SVG, any errors in the session are shown as compact cards:
```
15:53:42  apply_patch failed: Unicode encoding error in generate_report.py
14:22:10  shell: command not found — grep (bash command in PowerShell)
```

Styled with red left border, subtle red background, timestamp + message.

### Data Source

The waterfall is built from the existing `SessionDetail` API response:
- `session.started_at` / `session.ended_at` → time axis bounds
- `session.child_session_ids` → delegation spans (query child sessions for their start/end)
- `logs` array filtered by `category = 'tool_call'` → tool dots with timestamps
- `logs` filtered by `level = 'error'` → error callouts

### Implementation

Pure SVG rendered in React. No D3 needed (the waterfall is simpler than the knowledge graph — just positioned rectangles and circles). The waterfall component receives:

```typescript
interface WaterfallProps {
  session: LogSession;
  childSessions: LogSession[];
  logs: ExecutionLog[];
}
```

Positions are computed as: `x = (timestamp - session.started_at) / session.duration * width`.

---

## Section 3: Real-Time Updates via WebSocket

### Problem
Current page fetches once on load. Running sessions show stale data.

### Design

The gateway already broadcasts `GatewayEvent` over WebSocket. The chat panel (`WebChatPanel.tsx`) subscribes to events filtered by conversation_id. The logs page subscribes to ALL events (no conversation filter) and updates the session list in real-time.

### Events and Their Effect

| Gateway Event | Logs Page Update |
|---|---|
| `AgentStarted` | New session row appears with "running" status (blue pulsing dot), empty waterfall bar starts |
| `Token` | Token count in KPI card increments. Running session's token count updates. |
| `ToolCall` | New dot appears on running session's waterfall. Tool count increments. |
| `ToolResult` | If error: red dot, error count bumps, error callout appears. |
| `DelegationStarted` | New child span appears in waterfall. Delegation count increments. |
| `DelegationCompleted` | Child span completes (gets end position). |
| `AgentCompleted` | Session status flips to "completed". Duration finalizes. KPI success rate recalculates. |

### WebSocket Subscription

Use the existing transport layer's WebSocket connection. The chat panel already subscribes to events. The logs page adds a parallel subscription:

```typescript
// In the logs page effect:
const transport = await getTransport();
const unsubscribe = transport.subscribeToAllEvents((event: StreamEvent) => {
  switch (event.type) {
    case 'agent_started':
      addOrUpdateSession(event.session_id, { status: 'running', agent_id: event.agent_id });
      break;
    case 'tool_call':
      appendToolDot(event.session_id, event);
      break;
    case 'agent_completed':
      updateSession(event.session_id, { status: 'completed' });
      break;
    // ...
  }
});
```

If `subscribeToAllEvents` doesn't exist on the transport, the alternative is polling `/api/logs/sessions` every 3 seconds for running sessions only. Less elegant but zero WebSocket changes.

### Running Session Waterfall Animation

When a session is running and expanded:
- The root span bar grows rightward (width increases proportional to elapsed time)
- New tool dots appear with a subtle fade-in
- Delegation spans appear and grow
- A subtle pulse animation on the status dot indicates "live"

When the session completes, the animation stops and the waterfall freezes at final state.

---

## Section 4: Component Architecture

### New Files

| File | Responsibility |
|---|---|
| `features/logs/ExecutionDashboard.tsx` | Main page: KPIs + filter + session list + real-time updates |
| `features/logs/KpiCards.tsx` | 4 metric cards with sparklines |
| `features/logs/SessionRow.tsx` | Single session row with inline mini waterfall |
| `features/logs/SessionWaterfall.tsx` | Expanded full waterfall SVG |
| `features/logs/MiniWaterfall.tsx` | Inline compact waterfall for session rows (16px height) |
| `features/logs/ErrorCallout.tsx` | Compact error card for inline display |
| `features/logs/log-hooks.ts` | Data fetching hooks + real-time event handling |

### Removed/Replaced

`WebLogsPanel.tsx` (845 lines) → replaced by `ExecutionDashboard.tsx` + focused sub-components. Total should be ~600-700 lines across 7 files (smaller, focused units).

### CSS

Add to `components.css`:
- `.exec-dashboard` — main container
- `.kpi-card` — metric card with sparkline slot
- `.session-row` — clickable row with inline waterfall
- `.session-row--running` — blue pulse animation
- `.session-row--error` — red tint
- `.waterfall` — full SVG waterfall container
- `.waterfall__span` — agent span bar
- `.waterfall__dot` — tool call dot
- `.error-callout` — compact error card

Follow existing patterns from ARCHITECTURE.md: semantic BEM classes, design tokens from theme.css, no inline Tailwind.

---

## Section 5: KPI Sparkline Computation

Sparklines show trends over the last 20 sessions (or 7 days, whichever produces more data points).

**Computation (frontend, from session list):**

```typescript
// Success rate sparkline: group sessions by day, compute daily success %
const dailySuccess = groupByDay(sessions).map(day =>
  day.filter(s => s.status === 'completed').length / day.length
);

// Token sparkline: per-session token count
const tokenTrend = sessions.map(s => s.token_count);

// Duration sparkline: per-session duration
const durationTrend = sessions.map(s => s.duration_ms);
```

**SVG rendering:** Simple polyline connecting data points, scaled to fit 60x24px viewbox. No axes, no labels — just the trend shape. Color matches the KPI theme (green for success, amber for tokens, etc.).

---

## Section 6: Waterfall Position Computation

### Time-to-X Mapping

All waterfall elements (spans, dots) are positioned using:

```typescript
function timeToX(timestamp: Date, sessionStart: Date, sessionEnd: Date, svgWidth: number): number {
  const totalDuration = sessionEnd.getTime() - sessionStart.getTime();
  if (totalDuration === 0) return 0;
  const elapsed = timestamp.getTime() - sessionStart.getTime();
  return (elapsed / totalDuration) * svgWidth;
}
```

### Span Layout

- Root agent: y=0, full width background bar (translucent) + active segments
- Each child agent: y increments by 16px per delegation, positioned by child's start/end timestamps
- Tool dots: y = bottom lane, positioned by log timestamp

### Mini Waterfall (Session Row)

Same computation but compressed to 16px height:
- Root: top 6px
- Children: bottom 5px, stacked
- No labels, no dots — just colored bars showing execution shape

---

## What Changes

### Frontend
- Replace `WebLogsPanel.tsx` with 7 focused components
- Add WebSocket event subscription (or polling fallback)
- Add waterfall SVG rendering
- Add sparkline SVG rendering
- Update CSS with new component classes

### Backend
- **No changes.** All data comes from existing `GET /api/logs/sessions` and `GET /api/logs/sessions/{id}`.
- WebSocket events already broadcast by the gateway — the frontend just needs to subscribe.

### Data Flow

```
Initial load: GET /api/logs/sessions → KPI computation + session list render
Session expand: GET /api/logs/sessions/{id} → full waterfall render
Real-time: WebSocket events → incremental updates to session list + KPIs
```

---

## Dependencies

### No new npm packages
SVG is rendered directly in React JSX. Sparklines are simple polylines. No chart library needed.

### No new Rust crates
Backend is unchanged.

---

## What Stays the Same

- API endpoints (`/api/logs/sessions`, `/api/logs/sessions/{id}`)
- Data model (ExecutionLog, LogSession)
- Delete/cleanup functionality
- Log level filtering concept (now visual toggles instead of dropdown)
- Search functionality
