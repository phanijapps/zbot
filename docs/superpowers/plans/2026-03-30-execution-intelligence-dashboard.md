# Execution Intelligence Dashboard — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat logs page with a visual execution intelligence dashboard — KPI cards with sparklines, session list with inline waterfalls, expandable waterfall timelines, and real-time WebSocket updates.

**Architecture:** Pure frontend redesign. Replace monolithic `WebLogsPanel.tsx` (845 lines) with 7 focused components. All data from existing `GET /api/logs/sessions` and `GET /api/logs/sessions/{id}` APIs. Real-time via existing WebSocket event stream. SVG for all visualizations (sparklines, waterfalls) — no chart library.

**Tech Stack:** React 19, TypeScript, SVG (inline JSX), existing WebSocket transport, Vitest

**Spec:** `docs/superpowers/specs/2026-03-30-execution-intelligence-dashboard-design.md`

**UI patterns:** Read `apps/ui/ARCHITECTURE.md` before coding. Semantic BEM classes in `components.css`, design tokens from `theme.css`, no inline Tailwind.

---

## File Structure

### New Files
| File | Responsibility |
|---|---|
| `features/logs/ExecutionDashboard.tsx` | Main page: layout, state, event handling |
| `features/logs/KpiCards.tsx` | 4 metric cards with inline sparkline SVGs |
| `features/logs/SessionRow.tsx` | Single session row with inline mini waterfall |
| `features/logs/SessionWaterfall.tsx` | Expanded full waterfall timeline SVG |
| `features/logs/MiniWaterfall.tsx` | Compact 16px-height waterfall for session rows |
| `features/logs/ErrorCallout.tsx` | Compact error card for inline display |
| `features/logs/log-hooks.ts` | Data fetching + real-time event hooks |

### Modified Files
| File | Change |
|---|---|
| `features/logs/WebLogsPanel.tsx` | Replace entirely (or redirect to ExecutionDashboard) |
| `styles/components.css` | Add execution dashboard component classes |

### Reference Files (read before coding)
| File | Why |
|---|---|
| `apps/ui/ARCHITECTURE.md` | UI patterns, CSS conventions |
| `apps/ui/src/features/agent/WebChatPanel.tsx` | WebSocket event subscription pattern |
| `apps/ui/src/services/transport/` | Transport layer, event types |
| `apps/ui/src/styles/theme.css` | Design tokens (colors, spacing, fonts) |
| `services/api-logs/src/types.rs` | LogSession, ExecutionLog data structures |

---

## Chunk 1: Foundation Components

### Task 1: CSS Classes + ErrorCallout

**Files:**
- Modify: `apps/ui/src/styles/components.css`
- Create: `apps/ui/src/features/logs/ErrorCallout.tsx`

- [ ] **Step 1: Add dashboard CSS classes to components.css**

Read `apps/ui/ARCHITECTURE.md` and existing patterns in `components.css` first. Add:

```css
/* Execution Dashboard */
.exec-dashboard { display: flex; flex-direction: column; height: 100%; overflow: hidden; }
.exec-dashboard__kpis { display: flex; gap: 1px; background: var(--border); flex-shrink: 0; }
.exec-dashboard__filters { display: flex; align-items: center; gap: var(--spacing-3); padding: var(--spacing-2) var(--spacing-4); border-bottom: 1px solid var(--border); flex-shrink: 0; }
.exec-dashboard__sessions { flex: 1; overflow-y: auto; }

/* KPI Card */
.kpi-card { flex: 1; background: var(--card); padding: var(--spacing-3) var(--spacing-4); }
.kpi-card__header { display: flex; justify-content: space-between; align-items: center; }
.kpi-card__label { font-size: var(--text-xs); color: var(--muted-foreground); margin-bottom: var(--spacing-1); }
.kpi-card__value { font-size: var(--text-xl); font-weight: 700; }
.kpi-card__value--success { color: var(--success); }
.kpi-card__value--warning { color: var(--warning); }
.kpi-card__value--error { color: var(--destructive); }
.kpi-card__detail { font-size: 9px; color: var(--muted-foreground); margin-top: var(--spacing-1); }

/* Session Row */
.session-row { display: flex; align-items: center; padding: var(--spacing-2) var(--spacing-4); gap: var(--spacing-3); border-bottom: 1px solid var(--border); cursor: pointer; transition: background-color 0.1s; }
.session-row:hover { background-color: var(--muted); }
.session-row--expanded { background-color: var(--card); }

.session-row__status { width: 8px; height: 8px; border-radius: var(--radius-full); flex-shrink: 0; }
.session-row__status--completed { background-color: var(--success); }
.session-row__status--running { background-color: var(--primary); animation: pulse 2s ease-in-out infinite; }
.session-row__status--error { background-color: var(--destructive); }
.session-row__status--crashed { background-color: var(--destructive); }

.session-row__title { font-weight: 600; font-size: var(--text-sm); min-width: 120px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 200px; }
.session-row__agent { color: var(--muted-foreground); font-size: var(--text-xs); min-width: 80px; }
.session-row__waterfall { flex: 1; height: 16px; min-width: 100px; }
.session-row__metric { color: var(--muted-foreground); font-size: var(--text-xs); min-width: 55px; text-align: right; font-family: var(--font-mono); }
.session-row__metric--error { color: var(--destructive); }

/* Waterfall (expanded) */
.waterfall { padding: var(--spacing-3) var(--spacing-4) var(--spacing-3) calc(var(--spacing-4) + 20px); background: var(--background); border-top: 1px solid var(--border); }
.waterfall svg { width: 100%; }

/* Error Callout */
.error-callout { padding: var(--spacing-1-5) var(--spacing-3); background: rgba(239, 68, 68, 0.06); border-left: 2px solid var(--destructive); border-radius: 0 var(--radius-sm) var(--radius-sm) 0; margin-top: var(--spacing-1-5); font-size: var(--text-xs); }
.error-callout__time { color: var(--destructive); font-weight: 500; font-family: var(--font-mono); }
.error-callout__message { color: var(--muted-foreground); margin-left: var(--spacing-2); }

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
```

- [ ] **Step 2: Create ErrorCallout component**

```typescript
// features/logs/ErrorCallout.tsx
interface ErrorCalloutProps {
  timestamp: string;  // RFC3339
  message: string;
}

export function ErrorCallout({ timestamp, message }: ErrorCalloutProps) {
  const time = new Date(timestamp).toLocaleTimeString('en-US', { hour12: false });
  return (
    <div className="error-callout">
      <span className="error-callout__time">{time}</span>
      <span className="error-callout__message">{message}</span>
    </div>
  );
}
```

- [ ] **Step 3: Build UI**

Run: `cd apps/ui && npm run build`

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/styles/components.css apps/ui/src/features/logs/ErrorCallout.tsx
git commit -m "feat(ui): add execution dashboard CSS classes and ErrorCallout component"
```

---

### Task 2: MiniWaterfall Component

**Files:**
- Create: `apps/ui/src/features/logs/MiniWaterfall.tsx`

- [ ] **Step 1: Implement MiniWaterfall**

A compact 16px-height SVG showing execution shape for session rows.

```typescript
// features/logs/MiniWaterfall.tsx
interface MiniWaterfallProps {
  session: LogSession;
  childSessions?: LogSession[];
}
```

The component:
1. Takes session start/end timestamps to compute total duration
2. Renders root span as a thin bar (top 6px, primary color)
3. Renders each child session as a shorter bar (bottom 5px, success color) proportional to its duration and position
4. If session is "running", the root bar extends to current time with pulse animation

Read the `LogSession` type from `apps/ui/src/services/transport/types.ts` to understand available fields. The key fields are: `started_at`, `ended_at`, `duration_ms`, `status`, `child_session_ids`.

For child session positioning: if child session timestamps are not available inline, use a simple heuristic — distribute children evenly across the root span.

Use `viewBox="0 0 300 16"` with `preserveAspectRatio="none"` so it stretches to fill the container.

- [ ] **Step 2: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/logs/MiniWaterfall.tsx
git commit -m "feat(ui): add MiniWaterfall — compact execution shape for session rows"
```

---

### Task 3: KpiCards Component

**Files:**
- Create: `apps/ui/src/features/logs/KpiCards.tsx`

- [ ] **Step 1: Implement KpiCards**

```typescript
interface KpiCardsProps {
  sessions: LogSession[];
}
```

Computes from the sessions array:
- **Success rate:** `completed.length / total.length * 100`, colored green
- **Total tokens:** sum of `token_count` across sessions, with avg per session
- **Tool calls:** sum of `tool_call_count`, with top 3 tool types if available from metadata
- **Avg duration:** average `duration_ms` formatted, with min/max

Each card includes a **sparkline SVG** (60x24px): a simple `<polyline>` connecting the last 20 sessions' values, scaled to fit.

```typescript
function Sparkline({ data, color }: { data: number[]; color: string }) {
  if (data.length < 2) return null;
  const max = Math.max(...data);
  const min = Math.min(...data);
  const range = max - min || 1;
  const points = data.map((v, i) =>
    `${(i / (data.length - 1)) * 56 + 2},${22 - ((v - min) / range) * 18}`
  ).join(' ');
  return (
    <svg viewBox="0 0 60 24" style={{ width: 60, height: 24 }}>
      <polyline points={points} fill="none" stroke={color} strokeWidth="1.5" opacity="0.7" />
    </svg>
  );
}
```

- [ ] **Step 2: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/logs/KpiCards.tsx
git commit -m "feat(ui): add KpiCards — 4 metric cards with sparkline trends"
```

---

## Chunk 2: Waterfall + Session Row

### Task 4: SessionWaterfall Component (Expanded View)

**Files:**
- Create: `apps/ui/src/features/logs/SessionWaterfall.tsx`

- [ ] **Step 1: Implement SessionWaterfall**

```typescript
interface SessionWaterfallProps {
  session: LogSession;
  childSessions: LogSession[];
  logs: ExecutionLog[];
}
```

Renders a full SVG (100% width, dynamic height based on agent count):

1. **Time axis:** Bottom line with labeled markers (0s, 5m, 10m, etc.)
2. **Agent lanes:** One row per agent:
   - Root: full-width translucent background + opaque active segments
   - Children: colored spans positioned by start/end time, labeled with task + duration
3. **Tool dots:** Bottom lane, positioned by timestamp:
   - Amber circle = shell tool call
   - Purple = memory
   - Green = delegation
   - Red (larger) = error
4. **Time-to-X mapping:**
   ```typescript
   const timeToX = (ts: Date) => {
     const elapsed = ts.getTime() - start.getTime();
     return labelWidth + (elapsed / totalDuration) * barWidth;
   };
   ```

Read the `ExecutionLog` type to understand available fields: `timestamp`, `category`, `level`, `message`, `duration_ms`, `metadata`.

Tool type colors: derive from `category` field or from `message` content (e.g., "shell:" prefix).

- [ ] **Step 2: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/logs/SessionWaterfall.tsx
git commit -m "feat(ui): add SessionWaterfall — full execution timeline with delegation spans"
```

---

### Task 5: SessionRow Component

**Files:**
- Create: `apps/ui/src/features/logs/SessionRow.tsx`

- [ ] **Step 1: Implement SessionRow**

```typescript
interface SessionRowProps {
  session: LogSession;
  childSessions?: LogSession[];
  isExpanded: boolean;
  onToggle: () => void;
  logs?: ExecutionLog[];  // only loaded when expanded
}
```

The component renders:
1. **Collapsed:** status dot + title + agent + MiniWaterfall + duration + delegation count + token count + error count. Click toggles expanded.
2. **Expanded:** collapsed row + SessionWaterfall below + ErrorCallout for each error log.

Session title: derive from first log message that looks like a user message, or fall back to session_id. Truncate to ~30 chars.

Fetch session detail (`GET /api/logs/sessions/{id}`) on first expand. Cache the result.

- [ ] **Step 2: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/logs/SessionRow.tsx
git commit -m "feat(ui): add SessionRow — expandable row with inline waterfall and error callouts"
```

---

## Chunk 3: Data Hooks + Main Page

### Task 6: Data Hooks

**Files:**
- Create: `apps/ui/src/features/logs/log-hooks.ts`

- [ ] **Step 1: Implement hooks**

Read `apps/ui/src/services/transport/` first to understand how the transport layer works, and `WebChatPanel.tsx` to understand the WebSocket event subscription pattern.

```typescript
// Fetch sessions with optional filters
export function useLogSessions(filters?: LogFilter) {
  // Fetches GET /api/logs/sessions via transport
  // Returns { sessions, loading, error, refetch }
}

// Fetch session detail (logs + child sessions) — called on expand
export function useSessionDetail(sessionId: string | null) {
  // Fetches GET /api/logs/sessions/{id} via transport
  // Returns { detail, loading, error }
}

// Real-time event subscription
export function useLogEvents(onEvent: (event: StreamEvent) => void) {
  // Subscribe to WebSocket events via transport
  // Filter for: agent_started, agent_completed, tool_call, tool_result,
  //             delegation_started, delegation_completed
  // Call onEvent for each
  // Return unsubscribe function
}
```

For `useLogEvents`: check the transport interface for a method like `subscribeToAllEvents` or `onStreamEvent`. If it doesn't exist, implement polling as fallback: refetch sessions every 5 seconds when any session has `status === 'running'`.

- [ ] **Step 2: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/logs/log-hooks.ts
git commit -m "feat(ui): add log-hooks — session fetching, detail loading, real-time events"
```

---

### Task 7: ExecutionDashboard Main Page

**Files:**
- Create: `apps/ui/src/features/logs/ExecutionDashboard.tsx`
- Modify: `apps/ui/src/features/logs/WebLogsPanel.tsx`

- [ ] **Step 1: Implement ExecutionDashboard**

Main page that composes all components:

```typescript
export function ExecutionDashboard() {
  const [agentFilter, setAgentFilter] = useState<string | undefined>();
  const [levelFilter, setLevelFilter] = useState<string | undefined>();
  const [searchTerm, setSearchTerm] = useState('');
  const [expandedSessionId, setExpandedSessionId] = useState<string | null>(null);

  const { sessions, loading, refetch } = useLogSessions({ agent_id: agentFilter });
  const { detail } = useSessionDetail(expandedSessionId);

  // Filter sessions by level and search
  const filteredSessions = useMemo(() => {
    return sessions.filter(s => {
      if (levelFilter === 'error' && s.error_count === 0) return false;
      if (searchTerm && !matchesSearch(s, searchTerm)) return false;
      return true;
    });
  }, [sessions, levelFilter, searchTerm]);

  // Real-time updates
  useLogEvents((event) => {
    // Update sessions list based on event type
    refetch(); // Simple approach: refetch on any event
  });

  // Extract unique agents for filter pills
  const agents = useMemo(() =>
    [...new Set(sessions.map(s => s.agent_id))],
    [sessions]
  );

  return (
    <div className="exec-dashboard">
      <KpiCards sessions={sessions} />

      <div className="exec-dashboard__filters">
        {/* Agent pills */}
        {/* Level toggles */}
        {/* Search */}
      </div>

      <div className="exec-dashboard__sessions">
        {filteredSessions.map(session => (
          <SessionRow
            key={session.session_id}
            session={session}
            isExpanded={expandedSessionId === session.session_id}
            onToggle={() => setExpandedSessionId(
              expandedSessionId === session.session_id ? null : session.session_id
            )}
            logs={detail?.logs}
            childSessions={/* resolve from sessions list */}
          />
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Replace WebLogsPanel**

In `WebLogsPanel.tsx`, replace the component body to render `ExecutionDashboard`:

```typescript
import { ExecutionDashboard } from './ExecutionDashboard';

export function WebLogsPanel() {
  return <ExecutionDashboard />;
}
```

This preserves the existing import/export contract while delegating to the new implementation.

- [ ] **Step 3: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/logs/ExecutionDashboard.tsx apps/ui/src/features/logs/WebLogsPanel.tsx
git commit -m "feat(ui): add ExecutionDashboard — visual observability replaces flat log list"
```

---

## Chunk 4: Polish + Verification

### Task 8: Real-Time Polish

**Files:**
- Modify: `apps/ui/src/features/logs/log-hooks.ts`
- Modify: `apps/ui/src/features/logs/ExecutionDashboard.tsx`

- [ ] **Step 1: Implement smart refetch**

Instead of refetching the full session list on every WebSocket event, implement targeted updates:

- `agent_started` → prepend new session to list
- `agent_completed` → update session status + duration in place
- `tool_call` → increment tool_call_count in place
- For running sessions: poll session detail every 3 seconds to update the waterfall

If WebSocket subscription isn't available on the transport, implement auto-refresh: refetch every 5 seconds while any session is running, stop when all are completed.

- [ ] **Step 2: Add loading and empty states**

- Loading: centered spinner (same pattern as Observatory)
- Empty: "No execution logs yet" with icon
- Error: retry button

- [ ] **Step 3: Build and verify**

Run: `cd apps/ui && npm run build`

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/logs/
git commit -m "feat(ui): add real-time updates and loading states to execution dashboard"
```

---

### Task 9: End-to-End Verification

- [ ] **Step 1: Build UI**

Run: `cd apps/ui && npm run build`
Expected: Success, no errors

- [ ] **Step 2: Run UI tests**

Run: `cd apps/ui && npm test -- --run`
Expected: All pass (existing tests should still work since we preserved the WebLogsPanel export)

- [ ] **Step 3: Manual smoke test**

1. Start daemon
2. Open UI, navigate to Logs page
3. Verify: KPI cards show correct totals from existing sessions
4. Verify: session list shows sessions with inline mini waterfalls
5. Click a session — verify full waterfall expands with delegation spans and tool dots
6. Start a new agent session — verify the logs page updates (new row appears, waterfall grows)

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(ui): Execution Intelligence Dashboard — complete visual observability"
```
