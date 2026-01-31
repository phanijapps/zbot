# Logs Dashboard V2 — Dedicated Monitoring Page

## Problem Statement

The current logs panel is embedded in the chat view. While functional, it's constrained by:
- Limited screen real estate
- Context-switching between chat and logs
- No persistent monitoring view
- Hard to compare multiple sessions

## Vision

A dedicated `/logs` page that serves as a **command center** for monitoring agent activity:
- Full-screen monitoring experience
- Multiple viewing modes (timeline, tree, metrics)
- Real-time updates with live tailing
- Session comparison and analysis

---

## Page Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  [Filters]  Agent ▼  |  Status ▼  |  Time Range ▼  |  🔍 Search...  │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────┐  ┌─────────────────────────────────┐  │
│  │      VIEW MODE TABS             │  │     METRICS SUMMARY             │  │
│  │  [Timeline] [Tree] [Table]      │  │  ● 3 Running  ✓ 12 Done  ✗ 1   │  │
│  └─────────────────────────────────┘  └─────────────────────────────────┘  │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │                         MAIN VIEW AREA                              │   │
│  │                                                                     │   │
│  │    (Changes based on selected view mode)                           │   │
│  │                                                                     │   │
│  │                                                                     │   │
│  │                                                                     │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      DETAIL PANEL (Collapsible)                     │   │
│  │  Selected session/log details, JSON viewer, tool args/results      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## View Modes

### 1. Timeline View (Default)

Chronological stream of all activity across sessions.

```
┌─────────────────────────────────────────────────────────────────────┐
│  TODAY                                                              │
│  ─────────────────────────────────────────────────────────────────  │
│                                                                     │
│  14:32:05  ● root-agent started                          [Running]  │
│            ├─ "Help me refactor the auth module"                    │
│            │                                                        │
│  14:32:08  │  ◆ tool: list_skills                                   │
│            │  └─ Found 12 skills                                    │
│            │                                                        │
│  14:32:12  │  → Delegated to code-reviewer                         │
│            │    ├─ ● code-reviewer started              [Running]   │
│            │    │                                                   │
│  14:32:15  │    │  ◆ tool: read_file                                │
│            │    │  └─ auth/service.rs (234 lines)                   │
│            │    │                                                   │
│  14:32:18  │    │  ◆ tool: grep                                     │
│            │    │  └─ Found 8 matches                               │
│            │    │                                                   │
│  14:32:45  │    └─ ✓ code-reviewer completed            [Done]      │
│            │       Result: "Found 3 issues..."                      │
│            │                                                        │
│  14:32:47  └─ ✓ root-agent completed                    [Done]      │
│                                                                     │
│  ─────────────────────────────────────────────────────────────────  │
│  YESTERDAY                                                          │
│  ...                                                                │
└─────────────────────────────────────────────────────────────────────┘
```

**Features:**
- Collapsible session groups
- Inline tool call details
- Color-coded status indicators
- Live tailing with auto-scroll
- Click to expand full details

### 2. Tree View

Hierarchical view focused on agent delegation structure.

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│  ▼ ● root-agent                                    14:32 - 14:33    │
│    │ Status: Completed                                              │
│    │ Duration: 42s                                                  │
│    │ Tools: 3 calls                                                 │
│    │                                                                │
│    ├─▼ ● code-reviewer                             14:32 - 14:32    │
│    │   │ Status: Completed                                          │
│    │   │ Duration: 33s                                              │
│    │   │ Tools: 5 calls                                             │
│    │   │                                                            │
│    │   └─▶ ● file-analyzer                         14:32 - 14:32    │
│    │       Status: Completed                                        │
│    │       Duration: 8s                                             │
│    │                                                                │
│    └─▶ ● test-runner                               14:32 - 14:33    │
│        Status: Completed                                            │
│        Duration: 12s                                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Features:**
- Expand/collapse subtrees
- Quick metrics per agent
- Visual parent-child relationships
- Click to see full activity stream

### 3. Table View

Sortable, filterable table for power users.

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Session ID    │ Agent         │ Status    │ Duration │ Tools │ Started     │
├───────────────┼───────────────┼───────────┼──────────┼───────┼─────────────┤
│ ses-abc123    │ root-agent    │ ✓ Done    │ 42s      │ 3     │ 14:32:05    │
│ ses-def456    │ code-reviewer │ ✓ Done    │ 33s      │ 5     │ 14:32:12    │
│ ses-ghi789    │ test-runner   │ ● Running │ 1m 23s   │ 8     │ 14:33:01    │
│ ses-jkl012    │ root-agent    │ ⏸ Paused  │ 5m 12s   │ 15    │ 14:28:00    │
│ ses-mno345    │ analyzer      │ ✗ Crashed │ 2m 01s   │ 4     │ 14:25:00    │
└────────────────────────────────────────────────────────────────────────────┘
```

**Features:**
- Column sorting
- Multi-select for bulk actions
- Export to CSV
- Quick filters per column

---

## Detail Panel

When a session or log entry is selected:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  SESSION: ses-abc123                                              [✕ Close] │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Agent: code-reviewer                                                       │
│  Status: Completed                                                          │
│  Duration: 33 seconds                                                       │
│  Parent: ses-xyz789 (root-agent)                                           │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ACTIVITY STREAM                                     [Expand All ▼] │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │                                                                     │   │
│  │  14:32:12  Session started                                          │   │
│  │                                                                     │   │
│  │  14:32:14  ▶ Tool: read_file                                        │   │
│  │            Args: { "path": "src/auth/service.rs" }                  │   │
│  │            Result: (234 lines) [View Full ▼]                        │   │
│  │            Duration: 45ms                                           │   │
│  │                                                                     │   │
│  │  14:32:18  ▶ Tool: grep                                             │   │
│  │            Args: { "pattern": "TODO|FIXME", "path": "src/" }        │   │
│  │            Result: 8 matches [View Full ▼]                          │   │
│  │            Duration: 123ms                                          │   │
│  │                                                                     │   │
│  │  14:32:45  Session completed                                        │   │
│  │            Final response: "Found 3 issues that need attention..."  │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  [Resume] [Cancel] [Delete] [Export JSON]                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Metrics Bar

Always visible summary at the top:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  ● 3 Running    ⏸ 1 Paused    ✓ 47 Completed    ✗ 2 Crashed    Today: 53   │
│                                                                             │
│  Avg Duration: 34s    Tool Calls: 234    Delegations: 18                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Filters

### Agent Filter
- Dropdown with all agents
- Multi-select supported
- "Root agents only" toggle

### Status Filter
- Running
- Paused
- Completed
- Crashed
- Cancelled

### Time Range
- Last hour
- Today
- Yesterday
- Last 7 days
- Custom range picker

### Search
- Full-text search across:
  - Session IDs
  - Agent names
  - Tool names
  - Log messages
  - Tool arguments (JSON)

---

## Real-time Updates

### WebSocket Events

```typescript
// New session started
{ type: "session_created", session: {...} }

// Session status changed
{ type: "session_status", session_id: string, status: string }

// New log entry
{ type: "log_entry", session_id: string, log: {...} }

// Session completed
{ type: "session_completed", session_id: string, result: string }
```

### Live Tailing

- Auto-scroll to new entries (toggleable)
- Visual indicator for new entries when scrolled up
- "Jump to latest" button
- Pause live updates while inspecting

---

## Implementation Plan

### Phase 1: Page Structure

**File:** `apps/ui/src/features/logs/LogsDashboard.tsx`

1. Create route `/logs`
2. Basic layout with header, filters, main area
3. View mode tabs (Timeline/Tree/Table)
4. Connect to existing log data

### Phase 2: Timeline View

**File:** `apps/ui/src/features/logs/views/TimelineView.tsx`

1. Chronological list component
2. Session grouping by day
3. Collapsible session entries
4. Inline tool call display
5. Status badges and indicators

### Phase 3: Tree View

**File:** `apps/ui/src/features/logs/views/TreeView.tsx`

1. Hierarchical tree component
2. Parent-child session relationships
3. Expand/collapse functionality
4. Summary metrics per node

### Phase 4: Table View

**File:** `apps/ui/src/features/logs/views/TableView.tsx`

1. Sortable table component
2. Column configuration
3. Pagination
4. Bulk selection
5. Export functionality

### Phase 5: Detail Panel

**File:** `apps/ui/src/features/logs/DetailPanel.tsx`

1. Slide-out panel component
2. Activity stream display
3. Tool call inspection
4. JSON viewer for args/results
5. Action buttons (Resume/Cancel/Delete)

### Phase 6: Real-time Updates

1. WebSocket subscription to log events
2. Live update integration
3. Auto-scroll behavior
4. Optimistic UI updates

### Phase 7: Metrics & Polish

1. Metrics summary bar
2. Time range picker
3. Advanced search
4. Keyboard shortcuts
5. URL state for filters (shareable links)

---

## Files to Create/Modify

| File | Description |
|------|-------------|
| `apps/ui/src/features/logs/LogsDashboard.tsx` | Main dashboard page |
| `apps/ui/src/features/logs/components/FilterBar.tsx` | Filter controls |
| `apps/ui/src/features/logs/components/MetricsSummary.tsx` | Metrics bar |
| `apps/ui/src/features/logs/components/ViewModeSelector.tsx` | Tab switcher |
| `apps/ui/src/features/logs/views/TimelineView.tsx` | Timeline view |
| `apps/ui/src/features/logs/views/TreeView.tsx` | Tree view |
| `apps/ui/src/features/logs/views/TableView.tsx` | Table view |
| `apps/ui/src/features/logs/DetailPanel.tsx` | Session detail panel |
| `apps/ui/src/App.tsx` | Add `/logs` route |
| `gateway/src/http/logs.rs` | Enhanced log API endpoints |

---

## API Enhancements

### New Endpoints

```
GET /api/logs/sessions
  ?agent_id=...
  &status=running,paused
  &from=2024-01-01T00:00:00Z
  &to=2024-01-31T23:59:59Z
  &search=...
  &page=1
  &limit=50

GET /api/logs/sessions/:id/stream
  → WebSocket upgrade for live tailing

GET /api/logs/metrics
  → { running: 3, paused: 1, completed: 47, crashed: 2, ... }

GET /api/logs/sessions/:id/export
  → JSON download of full session data
```

---

## Design Tokens

### Status Colors

| Status | Color | Badge |
|--------|-------|-------|
| Running | Blue | `bg-blue-500` |
| Paused | Yellow | `bg-yellow-500` |
| Completed | Green | `bg-green-500` |
| Crashed | Red | `bg-red-500` |
| Cancelled | Gray | `bg-gray-500` |

### Typography

- Session titles: `text-base font-medium`
- Timestamps: `text-xs text-muted-foreground font-mono`
- Tool names: `text-sm font-mono`
- Log messages: `text-sm`

---

## Verification

### Functional Tests

1. All three view modes render correctly
2. Filters work independently and combined
3. Real-time updates appear without refresh
4. Detail panel shows complete information
5. Actions (Resume/Cancel/Delete) work
6. Search finds entries in all fields

### Performance Tests

1. 1000+ sessions render smoothly
2. Live updates don't cause re-render storms
3. Large tool results don't freeze UI
4. Virtualized lists for long timelines

### UX Tests

1. Intuitive navigation between views
2. Clear status indicators
3. Responsive on different screen sizes
4. Keyboard accessible
