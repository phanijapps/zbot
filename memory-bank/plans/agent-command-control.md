# Agent Command Control

## Vision

A real-time operational dashboard that sits **in front of chat** as a side panel. Think DevOps for AI agents — see what's running, how much it's costing, and intervene when needed.

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

The Command Control panel sits **to the left of chat**:

```
┌────────────────────────────────────────────────────────────────────────────┐
│                              HEADER                                         │
├─────────────────────────┬──────────────────────────────────────────────────┤
│                         │                                                  │
│   COMMAND CONTROL       │              CHAT PANEL                          │
│   (Collapsible)         │                                                  │
│                         │   [Conversation list]                            │
│   ● Live Status         │   [Message history]                              │
│   ● Token Metrics       │   [Input box]                                    │
│   ● Agent Cards         │                                                  │
│   ● Execution History   │                                                  │
│                         │                                                  │
│   [Collapse ◀]          │                                                  │
│                         │                                                  │
└─────────────────────────┴──────────────────────────────────────────────────┘
```

### Collapsed State
When collapsed, show minimal status bar:
```
│ ● 4 agents │ 1.82M tokens │ [Expand ▶]
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

## Implementation Plan

### Phase 1: Token Tracking Infrastructure

**Backend:**
1. Add `session_tokens` table to schema
2. Create `TokenTracker` service
3. Instrument executor to emit token events
4. Add token aggregation queries

**Files:**
- `gateway/src/database/schema.rs` — Add table
- `gateway/src/services/tokens.rs` — Token tracking service
- `gateway/src/execution/runner.rs` — Emit token events
- `runtime/agent-runtime/src/executor.rs` — Track LLM response tokens

### Phase 2: Real-time Events

**Backend:**
1. Add WebSocket events for token updates
2. Throttle token events (batch every 2 sec)
3. Add session status events

**Files:**
- `gateway/src/websocket/handler.rs` — New event types
- `gateway/src/events/mod.rs` — Token event definitions

### Phase 3: Command Control Panel UI

**Frontend:**
1. Create `CommandControl` component
2. System snapshot header
3. Live agents grid
4. Active subagents list
5. Execution history table
6. Collapsible panel behavior

**Files:**
- `apps/ui/src/features/command/CommandControl.tsx`
- `apps/ui/src/features/command/SystemSnapshot.tsx`
- `apps/ui/src/features/command/LiveAgentCard.tsx`
- `apps/ui/src/features/command/SubagentList.tsx`
- `apps/ui/src/features/command/ExecutionHistory.tsx`
- `apps/ui/src/features/command/TokenBadge.tsx`
- `apps/ui/src/features/command/BurnIndicator.tsx`

### Phase 4: Integration

1. Add panel to main layout
2. Connect WebSocket for real-time updates
3. Implement collapse/expand
4. Add click-through to chat context
5. Time range selector (Today/Week/All)

### Phase 5: Actions

1. Pause/Resume from panel
2. Cancel running agent
3. Retry failed execution
4. Quick-jump to conversation

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
