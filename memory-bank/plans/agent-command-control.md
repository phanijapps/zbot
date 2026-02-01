# Agent Command Control

## Vision

A real-time operational dashboard available as a **tab in the side panel** (alongside Chat, Logs, MCPs, etc.). Think DevOps for AI agents — see what's running, how much it's costing, and intervene when needed.

**Not a logs page.** This is mission control.

---

## Layout

```
┌──────────────────────────────────────────────────────────────────────────┐
│ AGENT COMMAND CONTROL                         ● Live   Tokens: Today ▾   │
├──────────────────────────────────────────────────────────────────────────┤
│ SYSTEM SNAPSHOT                                                          │
│ Agents: 4 Running   Subagents: 9 Active   ✓ Execs: 27   ✗ 2              │
│ Tokens Today: 1.82M IN   612K OUT   Ratio: 3.0x   Burn: HIGH             │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│ ┌──────────── LIVE AGENTS ────────────┐ ┌──── ACTIVE SUBAGENTS ──────┐  │
│ │                                     │ │                             │  │
│ │ root-agent        ● RUNNING         │ │ code-reviewer   ● RUNNING   │  │
│ │ ▓▓▓▓▓▓░░░ 72%     ⏱ 01:42           │ │ ▓▓▓░░░░░ 31%    ⏱ 00:21     │  │
│ │ IN: 148K  OUT: 21K                  │ │ IN: 42K  OUT: 6K            │  │
│ │ fan-out: 3        burn: MED         │ │ tool: read_file             │  │
│ │                                     │ │                             │  │
│ │ planner           ● RUNNING         │ │ analyzer        ✓ DONE      │  │
│ │ ▓▓▓▓░░░░░ 45%     ⏱ 00:58           │ │ IN: 18K OUT: 3K             │  │
│ │ IN: 63K  OUT: 9K                    │ │ result: OK                  │  │
│ │ fan-out: 2        burn: LOW         │ │                             │  │
│ │                                     │ │ retriever       ✗ FAILED    │  │
│ │ validator         ⏸ WAITING         │ │ IN: 9K  OUT: 1K             │  │
│ │ idle on input                       │ │ cause: timeout              │  │
│ └─────────────────────────────────────┘ └─────────────────────────────┘  │
│                                                                          │
├──────────────────────── TODAY'S EXECUTIONS (TOKEN SUMMARY) ──────────────┤
│                                                                          │
│ 14:41 ✓ root-agent   148K → 21K   42s   3 agents   burn: MED             │
│ 14:32 ✓ planner      63K → 9K     19s   1 agent    burn: LOW             │
│ 14:18 ✗ analyzer     18K → 3K     12s   timeout    burn: SPIKE           │
│ 13:55 ✓ root-agent   312K → 44K   1m02s 4 agents   burn: HIGH            │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Core Concepts

### 1. System Snapshot
Top-level health at a glance:
- **Agents Running** — Root agents currently executing
- **Subagents Active** — Delegated agents in progress
- **Executions** — Today's completed (✓) and failed (✗)
- **Token Burn** — Cost indicator based on consumption rate

### 2. Live Agents Panel
Root-level agents currently running:
- **Progress bar** — Estimated completion (based on avg duration or token budget)
- **Duration** — Time since start
- **Tokens IN/OUT** — Current session consumption
- **Fan-out** — Number of subagents spawned
- **Burn rate** — LOW/MED/HIGH based on token velocity

### 3. Active Subagents Panel
Delegated agents spawned by root agents:
- **Status** — RUNNING, DONE, FAILED, WAITING
- **Current tool** — What tool is currently executing
- **Token consumption** — Per-subagent tracking
- **Result/Cause** — Quick status or error reason

### 4. Execution History
Today's completed executions:
- **Time** — When it finished
- **Status** — ✓ success, ✗ failed, ⏸ paused
- **Tokens** — IN → OUT summary
- **Duration** — Total execution time
- **Fan-out** — How many subagents were used
- **Burn** — Cost indicator for that execution

---

## Token Economics

### Tracking
```rust
struct TokenMetrics {
    input_tokens: u64,      // Prompt tokens sent to LLM
    output_tokens: u64,     // Completion tokens received
    cached_tokens: u64,     // Tokens served from cache (if applicable)
}

struct SessionTokens {
    session_id: String,
    agent_id: String,
    parent_session_id: Option<String>,
    tokens: TokenMetrics,
    started_at: DateTime,
    duration_ms: u64,
}
```

### Burn Rate Calculation
```
burn_rate = (tokens_in + tokens_out * 3) / duration_seconds

LOW:   < 1000 tokens/sec
MED:   1000-5000 tokens/sec
HIGH:  5000-20000 tokens/sec
SPIKE: > 20000 tokens/sec
```

### Daily Aggregates
```sql
SELECT
    SUM(input_tokens) as total_in,
    SUM(output_tokens) as total_out,
    COUNT(*) as executions,
    COUNT(CASE WHEN status = 'failed' THEN 1 END) as failures
FROM session_tokens
WHERE DATE(started_at) = DATE('now')
```

---

## Real-time Updates

### WebSocket Events
```typescript
// Agent started
{ type: "agent_started", session_id, agent_id, parent_session_id }

// Token update (every N tokens or every M seconds)
{ type: "token_update", session_id, tokens_in, tokens_out }

// Tool execution
{ type: "tool_started", session_id, tool_name }
{ type: "tool_completed", session_id, tool_name, duration_ms }

// Status change
{ type: "agent_status", session_id, status, result?, error? }

// Subagent spawned
{ type: "subagent_spawned", parent_session_id, child_session_id, agent_id }
```

### Update Frequency
- Token counts: Every 1000 tokens or 2 seconds
- Status changes: Immediate
- Progress bar: Derived from token velocity vs historical average

---

## Progress Estimation

Since we can't know true completion %, estimate based on:

1. **Token budget** — If agent has a max_tokens setting
2. **Historical average** — Avg tokens for this agent type
3. **Time-based** — Avg duration for similar tasks
4. **Tool pattern** — Detect "wrapping up" tools like `respond`

```rust
fn estimate_progress(session: &Session) -> f32 {
    // Use whichever signal is most reliable
    if let Some(budget) = session.token_budget {
        return session.tokens_used as f32 / budget as f32;
    }

    if let Some(avg) = get_historical_avg_tokens(session.agent_id) {
        return (session.tokens_used as f32 / avg as f32).min(0.95);
    }

    // Fallback: time-based with cap
    let elapsed = session.duration();
    let avg_duration = get_historical_avg_duration(session.agent_id);
    (elapsed.as_secs_f32() / avg_duration.as_secs_f32()).min(0.95)
}
```

---

## Panel Placement

Command Control is a **tab in the side panel** alongside Chat, Logs, MCPs, etc:

```
┌────────────────────────────────────────────────────────────────────────────┐
│  HEADER                                                                    │
├────────┬───────────────────────────────────────────────────────────────────┤
│        │                                                                   │
│  NAV   │                      MAIN CONTENT AREA                            │
│        │                                                                   │
│ ┌────┐ │   (Shows selected panel content)                                  │
│ │💬  │ │                                                                   │
│ │Chat│ │   When "Command" is selected:                                     │
│ └────┘ │   ┌─────────────────────────────────────────────────────────────┐ │
│ ┌────┐ │   │ AGENT COMMAND CONTROL               ● Live  Tokens: Today ▾│ │
│ │⚡  │ │   ├─────────────────────────────────────────────────────────────┤ │
│ │Cmd │◄│   │ SYSTEM SNAPSHOT                                            │ │
│ └────┘ │   │ Agents: 4 Running  Subagents: 9  ✓ 27  ✗ 2                 │ │
│ ┌────┐ │   │ Tokens: 1.82M IN  612K OUT  Burn: HIGH                     │ │
│ │📋  │ │   ├─────────────────────────────────────────────────────────────┤ │
│ │Logs│ │   │ LIVE AGENTS          │  ACTIVE SUBAGENTS                   │ │
│ └────┘ │   │ ...                  │  ...                                │ │
│ ┌────┐ │   └─────────────────────────────────────────────────────────────┘ │
│ │🔌  │ │                                                                   │
│ │MCPs│ │                                                                   │
│ └────┘ │                                                                   │
│        │                                                                   │
└────────┴───────────────────────────────────────────────────────────────────┘
```

### Navigation Tabs
| Tab | Icon | Description |
|-----|------|-------------|
| Chat | 💬 | Conversations and chat interface |
| Command | ⚡ | Agent Command Control (this feature) |
| Logs | 📋 | Execution logs and history |
| MCPs | 🔌 | MCP server management |
| Skills | 📚 | Skill management |
| Settings | ⚙️ | Provider and app settings |

### Status Badge on Tab
When agents are running, show indicator on Command tab:
```
│ ⚡  │
│ Cmd │
│ ●4  │  ← "4 running" badge
```

---

## Data Model

### Database: `session_tokens` table
```sql
CREATE TABLE session_tokens (
    session_id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    parent_session_id TEXT,

    -- Status
    status TEXT NOT NULL,  -- running, completed, failed, paused

    -- Tokens
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,

    -- Timing
    started_at TEXT NOT NULL,
    completed_at TEXT,
    duration_ms INTEGER,

    -- Delegation
    subagent_count INTEGER DEFAULT 0,

    -- Outcome
    result TEXT,
    error TEXT,

    FOREIGN KEY (conversation_id) REFERENCES conversations(id),
    FOREIGN KEY (parent_session_id) REFERENCES session_tokens(session_id)
);

CREATE INDEX idx_tokens_date ON session_tokens(DATE(started_at));
CREATE INDEX idx_tokens_status ON session_tokens(status);
CREATE INDEX idx_tokens_parent ON session_tokens(parent_session_id);
```

---

## Architecture Placement

Following the layer structure in AGENTS.md:

```
┌─────────────────────────────────────────────────────────────────────────┐
│ apps/ui/src/features/command/        UI LAYER                           │
│   ├── CommandControl.tsx             Main panel component               │
│   ├── SystemSnapshot.tsx             Top metrics bar                    │
│   ├── LiveAgentCard.tsx              Running agent card                 │
│   ├── SubagentList.tsx               Active subagents                   │
│   ├── ExecutionHistory.tsx           Today's executions                 │
│   ├── TokenBadge.tsx                 Compact token display              │
│   └── BurnIndicator.tsx              Burn rate visual                   │
├─────────────────────────────────────────────────────────────────────────┤
│ gateway/                             GATEWAY LAYER                      │
│   ├── http/command.rs                HTTP API for command control       │
│   └── websocket/handler.rs           Real-time event streaming          │
├─────────────────────────────────────────────────────────────────────────┤
│ services/execution-state/            SERVICE LAYER (from other plan)   │
│   └── (provides session & token data)                                   │
└─────────────────────────────────────────────────────────────────────────┘
```

**Key principle:** Command Control is a **consumer** of the execution-state service. It doesn't add new backend services—it surfaces data that execution-state already tracks.

---

## Implementation Plan

### Phase 1: Backend API (Gateway)

**Depends on:** `services/execution-state/` from Execution State Management plan

**File:** `gateway/src/http/command.rs` (NEW)

```rust
// Snapshot endpoint
GET /api/command/snapshot
→ {
    agents_running: 4,
    subagents_active: 9,
    executions_today: 27,
    failures_today: 2,
    tokens_today: { in: 1820000, out: 612000 }
}

// Live agents
GET /api/command/live
→ {
    agents: [...],      // Root agents currently running
    subagents: [...]    // Their active subagents
}

// Execution history
GET /api/command/history?date=2024-01-31&limit=20
→ { executions: [...] }
```

**Files:**
- `gateway/src/http/command.rs` — NEW: Command Control HTTP handlers
- `gateway/src/http/mod.rs` — Register command routes

### Phase 2: Real-time Events (Gateway)

**File:** `gateway/src/websocket/handler.rs`

Add WebSocket event types (if not already from execution-state plan):

```typescript
// Server → Client events
{ type: "session_started", session: {...} }
{ type: "session_status", session_id, status }
{ type: "token_update", session_id, tokens_in, tokens_out }
{ type: "session_completed", session_id, result }
```

**Files:**
- `gateway/src/websocket/handler.rs` — Ensure events are broadcast
- `gateway/src/events/mod.rs` — Event type definitions

### Phase 3: UI Components (apps/ui)

**File:** `apps/ui/src/features/command/` (NEW directory)

```
command/
├── index.ts                    # Exports
├── CommandControl.tsx          # Main panel
├── components/
│   ├── SystemSnapshot.tsx      # Top metrics bar
│   ├── LiveAgentCard.tsx       # Agent card with progress
│   ├── SubagentRow.tsx         # Subagent list item
│   ├── ExecutionRow.tsx        # History row
│   ├── TokenDisplay.tsx        # "148K → 21K" format
│   └── BurnIndicator.tsx       # LOW/MED/HIGH/SPIKE
├── hooks/
│   ├── useCommandData.ts       # Fetch snapshot + live data
│   └── useCommandStream.ts     # WebSocket subscription
└── types.ts                    # TypeScript interfaces
```

### Phase 4: Navigation Integration (apps/ui)

**File:** `apps/ui/src/App.tsx`

Add Command tab to navigation:

```tsx
const NAV_ITEMS = [
  { id: 'chat', icon: MessageSquare, label: 'Chat' },
  { id: 'command', icon: Zap, label: 'Command', badge: runningCount },  // NEW
  { id: 'logs', icon: FileText, label: 'Logs' },
  { id: 'mcps', icon: Plug, label: 'MCPs' },
  // ...
];
```

**Files:**
- `apps/ui/src/App.tsx` — Add Command nav item
- `apps/ui/src/shared/types/index.ts` — Add command-related types

### Phase 5: Actions & Polish

1. Pause/Resume/Cancel buttons on agent cards
2. Click agent card → jump to conversation
3. Badge on nav showing running count
4. Time range selector
5. Auto-refresh toggle

---

## Files Summary

| Layer | File | Changes |
|-------|------|---------|
| **gateway** | `http/command.rs` | **NEW** - Command Control API |
| **gateway** | `http/mod.rs` | Register command routes |
| **gateway** | `websocket/handler.rs` | Ensure events broadcast |
| **apps/ui** | `features/command/` | **NEW** - All UI components |
| **apps/ui** | `App.tsx` | Add Command nav tab |
| **apps/ui** | `shared/types/` | Command-related types |

---

## Dependencies

This plan **depends on** Execution State Management plan:
- Session status tracking (RUNNING, PAUSED, etc.)
- Token metrics (IN/OUT per session)
- WebSocket events for real-time updates

Command Control is purely a **presentation layer** on top of execution-state data.

---

## UI Components

### LiveAgentCard
```tsx
interface LiveAgentCardProps {
  session: {
    id: string;
    agentId: string;
    status: 'running' | 'waiting' | 'paused';
    progress: number;        // 0-100
    duration: number;        // seconds
    tokensIn: number;
    tokensOut: number;
    fanOut: number;          // subagent count
    burnRate: 'low' | 'med' | 'high' | 'spike';
    currentTool?: string;
  };
  onPause: () => void;
  onCancel: () => void;
  onClick: () => void;       // Navigate to conversation
}
```

### SubagentRow
```tsx
interface SubagentRowProps {
  session: {
    id: string;
    agentId: string;
    status: 'running' | 'done' | 'failed' | 'waiting';
    tokensIn: number;
    tokensOut: number;
    currentTool?: string;
    result?: string;
    error?: string;
  };
}
```

### BurnIndicator
```tsx
// Visual indicator for token consumption rate
<BurnIndicator rate="high" />  // 🔥 or colored bar
```

### TokenDisplay
```tsx
// Compact token count with IN/OUT
<TokenDisplay in={148000} out={21000} />  // "148K → 21K"
```

---

## API Endpoints

```
GET /api/command/snapshot
  → { agents_running, subagents_active, executions_today, failures_today, tokens_today }

GET /api/command/live
  → { live_agents: [...], active_subagents: [...] }

GET /api/command/history?date=2024-01-31&limit=20
  → { executions: [...] }

GET /api/command/tokens/daily
  → { date, total_in, total_out, ratio, burn_rate }

WebSocket /ws
  → Subscribe to: token_update, agent_started, agent_status, subagent_spawned
```

---

## Design Tokens

### Status Colors
| Status | Color | Indicator |
|--------|-------|-----------|
| Running | Blue | ● |
| Waiting | Yellow | ⏸ |
| Done | Green | ✓ |
| Failed | Red | ✗ |
| Paused | Gray | ⏸ |

### Burn Rate Colors
| Rate | Color | Display |
|------|-------|---------|
| LOW | Green | `text-green-500` |
| MED | Yellow | `text-yellow-500` |
| HIGH | Orange | `text-orange-500` |
| SPIKE | Red | `text-red-500` + pulse animation |

### Typography
- Panel title: `text-xs font-bold uppercase tracking-wide`
- Agent names: `text-sm font-medium`
- Metrics: `text-xs font-mono`
- Status badges: `text-xs font-semibold`

---

## Verification

### Test Scenarios

1. **Single agent execution** — Card appears, progress updates, completes
2. **Multi-agent fan-out** — Root spawns subagents, tree visible
3. **Token tracking** — IN/OUT counts update in real-time
4. **Failure handling** — Failed agent shows error, burn rate shows SPIKE
5. **Pause/Resume** — Status changes, timer pauses
6. **Panel collapse** — Minimal bar shows key metrics
7. **Historical view** — Today's executions populate correctly

### Performance

- Panel should handle 20+ concurrent agents
- Token updates shouldn't cause render storms (batch/throttle)
- WebSocket reconnection on disconnect
- Graceful degradation if backend unavailable
