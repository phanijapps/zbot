# Dashboard Implementation Plan

> **Reference:** `memory-bank/dashboard-technical-spec.md` contains the full architecture, data model, and design rationale.

## How to Use This Plan

Each part is self-contained. Copy-paste the prompt for the part you want to execute.

### Quick Start Prompts

**Part 1A - Add Execution Stats:**
```
Read memory-bank/dashboard-implementation-plan.md and execute Part 1A: Backend - Add Execution Stats.
The technical spec is in memory-bank/dashboard-technical-spec.md if you need context.
```

**Part 1B - Delete Legacy API:**
```
Read memory-bank/dashboard-implementation-plan.md and execute Part 1B: Backend - Delete Legacy API.
The technical spec is in memory-bank/dashboard-technical-spec.md if you need context.
```

**Part 1C - Delete Legacy Frontend:**
```
Read memory-bank/dashboard-implementation-plan.md and execute Part 1C: Frontend - Delete Legacy Types.
The technical spec is in memory-bank/dashboard-technical-spec.md if you need context.
```

**Part 1D - Rewrite Dashboard:**
```
Read memory-bank/dashboard-implementation-plan.md and execute Part 1D: Frontend - Rewrite Dashboard with V2 API.
The technical spec is in memory-bank/dashboard-technical-spec.md if you need context.
```

**Part 2A - Add Source Field:**
```
Read memory-bank/dashboard-implementation-plan.md and execute Part 2A: Add Source Field to Session.
The technical spec is in memory-bank/dashboard-technical-spec.md if you need context.
```

---

## Overview

Each part is self-contained and can be executed in a fresh context. Parts are ordered by dependency.

---

## PHASE 1: Fix Current Dashboard Bugs

### Part 1A: Backend - Add Execution Stats

**Scope:** Extend `DashboardStats` to include execution-level counts

**Files:**
- `services/execution-state/src/types.rs`
- `services/execution-state/src/repository.rs`

**Changes:**
1. Add to `DashboardStats` struct:
   ```rust
   pub executions_running: u64,
   pub executions_queued: u64,
   pub executions_completed: u64,
   pub executions_crashed: u64,
   ```

2. Update `get_dashboard_stats()` in repository to query `agent_executions` table

**Test:**
```bash
curl http://localhost:18791/api/executions/stats/counts
# Should return both session AND execution counts
```

**Done when:** Stats API returns execution counts

---

### Part 1B: Backend - Delete Legacy API

**Scope:** Remove all legacy execution session code

**Files:**
- `gateway/src/http/handlers.rs`

**Changes:**
1. Delete `LegacyExecutionSession` struct
2. Delete `impl From<AgentExecution> for LegacyExecutionSession`
3. Delete or update handlers that return `LegacyExecutionSession`
4. Keep only V2 endpoints (`/api/executions/v2/*`)

**Test:**
```bash
# This should 404 (deleted)
curl http://localhost:18791/api/executions/sessions

# This should work (kept)
curl http://localhost:18791/api/executions/v2/sessions/full
```

**Done when:** Legacy endpoint returns 404, V2 works

---

### Part 1C: Frontend - Delete Legacy Types

**Scope:** Remove legacy types and grouping logic

**Files:**
- `apps/ui/src/services/transport/types.ts`
- `apps/ui/src/features/ops/WebOpsDashboard.tsx`

**Changes in types.ts:**
1. Delete `ExecutionSession` interface (if it's the legacy one)
2. Add/verify `SessionWithExecutions` type matching V2 API:
   ```typescript
   interface SessionWithExecutions {
     session: Session;
     executions: AgentExecution[];
     subagent_count: number;
   }
   ```

**Changes in WebOpsDashboard.tsx:**
1. Delete `getRootConversationId()` function
2. Delete `buildConversationGroups()` function
3. Delete `ConversationGroup` interface
4. Delete any code referencing `-sub-` string manipulation

**Test:** Code compiles (will have errors until Part 1D)

**Done when:** No legacy grouping code remains

---

### Part 1D: Frontend - Rewrite Dashboard with V2 API

**Scope:** Update dashboard to use V2 API and display sessions properly

**Files:**
- `apps/ui/src/services/transport/http.ts`
- `apps/ui/src/services/transport/types.ts`
- `apps/ui/src/features/ops/WebOpsDashboard.tsx`

**Changes in http.ts:**
1. Update `listExecutionSessions()` to call `/api/executions/v2/sessions/full`
2. Update return type to `SessionWithExecutions[]`

**Changes in WebOpsDashboard.tsx:**
1. Update data fetching to expect `SessionWithExecutions[]`
2. Active Sessions: Group by session, show all executions within
3. Session History: Group by session, show turn count + subagent count
4. Stats: Display both session and execution counts

**Display Format:**
```
Active Sessions [2]
  ▼ sess-abc (running) [web]
    ├─ root (completed)
    └─ research-agent (running) ●

  ▼ sess-def (running) [cron]
    └─ root (running) ●

Stats: 2 sessions running | 2 executions running, 1 completed
```

**Test:**
1. Start an agent that delegates
2. Dashboard shows session with root + subagent hierarchically
3. Stats show correct counts

**Done when:** Dashboard displays sessions correctly with hierarchy

---

## PHASE 2: Data Model Extensions

### Part 2A: Add Source Field to Session

**Scope:** Track trigger source for each session

**Files:**
- `services/execution-state/src/types.rs`
- `services/execution-state/src/repository.rs`
- `gateway/src/http/handlers.rs` (session creation)

**Changes:**
1. Add `TriggerSource` enum:
   ```rust
   pub enum TriggerSource {
       Web,
       Cli,
       Cron,
       Api,
       Plugin,
   }
   ```

2. Add `source: TriggerSource` to `Session` struct

3. Update DB schema:
   ```sql
   ALTER TABLE sessions ADD COLUMN source TEXT DEFAULT 'web';
   ```

4. Update session creation to accept/set source

**Test:**
```bash
# Create session via API
curl -X POST http://localhost:18791/api/sessions -d '{"source": "api", ...}'

# Verify source in response
curl http://localhost:18791/api/executions/v2/sessions/full
# Should show source field
```

**Done when:** Sessions have source field persisted and returned

---

### Part 2B: Add Queued Status to Session

**Scope:** Allow sessions to be queued before starting

**Files:**
- `services/execution-state/src/types.rs`
- `services/execution-state/src/repository.rs`
- `services/execution-state/src/service.rs`

**Changes:**
1. Add `Queued` to `SessionStatus` enum
2. Update session creation logic to support queued state
3. Add `start_session()` to transition Queued → Running

**Test:**
```rust
// Create session in queued state
let session = service.create_session_queued(...)?;
assert_eq!(session.status, SessionStatus::Queued);

// Start it
service.start_session(&session.id)?;
assert_eq!(session.status, SessionStatus::Running);
```

**Done when:** Sessions can be created as Queued and transitioned to Running

---

### Part 2C: Stats by Source

**Scope:** Break down stats by trigger source

**Files:**
- `services/execution-state/src/types.rs`
- `services/execution-state/src/repository.rs`

**Changes:**
1. Add to `DashboardStats`:
   ```rust
   pub sessions_by_source: HashMap<String, u64>,
   ```

2. Update `get_dashboard_stats()`:
   ```sql
   SELECT source, COUNT(*) FROM sessions GROUP BY source
   ```

**Test:**
```bash
curl http://localhost:18791/api/executions/stats/counts
# Should return: { "sessions_by_source": { "web": 5, "cron": 2 }, ... }
```

**Done when:** Stats include breakdown by source

---

### Part 2D: Frontend - Source Filter and Display

**Scope:** Show source badge, add filter

**Files:**
- `apps/ui/src/features/ops/WebOpsDashboard.tsx`

**Changes:**
1. Display source badge on each session: `[web]` `[cron]` `[api]`
2. Add filter dropdown: "All Sources" / "Web" / "Cron" / etc.
3. Update stats panel to show breakdown by source

**Test:** Filter works, badges display correctly

**Done when:** Can filter sessions by source in UI

---

## PHASE 3: Gateway Bus (Future)

### Part 3A: Gateway Bus Interface

**Scope:** Create unified intake interface

**Files:**
- `framework/zero-gateway/` (new crate) or `gateway/src/bus/`

**Changes:**
1. Define `GatewayBus` trait
2. Define `SessionRequest` struct
3. Implement for current HTTP flow

**Done when:** Trait exists, HTTP handler uses it

---

### Part 3B: Foreign Plugin Endpoint

**Scope:** HTTP endpoint for non-Rust triggers

**Files:**
- `gateway/src/http/handlers.rs`

**Changes:**
1. Add `POST /api/gateway/submit` endpoint
2. Accepts `SessionRequest` JSON
3. Calls `bus.submit()` internally

**Done when:** Python/JS can submit sessions via HTTP

---

## Execution Order

```
Phase 1 (Fix bugs - do first):
  1A → 1B → 1C → 1D

Phase 2 (Extend model):
  2A → 2C → 2D  (source tracking)
  2B            (queuing - independent)

Phase 3 (Gateway bus - future):
  3A → 3B
```

## Context-Clear Friendly

Each part has:
- **Scope:** What to do
- **Files:** Where to change
- **Changes:** Specific code changes
- **Test:** How to verify
- **Done when:** Clear completion criteria

---

## All Prompts (Copy-Paste Ready)

### Phase 1: Fix Current Bugs

```
Read memory-bank/dashboard-implementation-plan.md and execute Part 1A: Backend - Add Execution Stats.
Reference: memory-bank/dashboard-technical-spec.md
```

```
Read memory-bank/dashboard-implementation-plan.md and execute Part 1B: Backend - Delete Legacy API.
Reference: memory-bank/dashboard-technical-spec.md
```

```
Read memory-bank/dashboard-implementation-plan.md and execute Part 1C: Frontend - Delete Legacy Types.
Reference: memory-bank/dashboard-technical-spec.md
```

```
Read memory-bank/dashboard-implementation-plan.md and execute Part 1D: Frontend - Rewrite Dashboard with V2 API.
Reference: memory-bank/dashboard-technical-spec.md
```

### Phase 2: Data Model Extensions

```
Read memory-bank/dashboard-implementation-plan.md and execute Part 2A: Add Source Field to Session.
Reference: memory-bank/dashboard-technical-spec.md
```

```
Read memory-bank/dashboard-implementation-plan.md and execute Part 2B: Add Queued Status to Session.
Reference: memory-bank/dashboard-technical-spec.md
```

```
Read memory-bank/dashboard-implementation-plan.md and execute Part 2C: Stats by Source.
Reference: memory-bank/dashboard-technical-spec.md
```

```
Read memory-bank/dashboard-implementation-plan.md and execute Part 2D: Frontend - Source Filter and Display.
Reference: memory-bank/dashboard-technical-spec.md
```

### Phase 3: Gateway Bus

```
Read memory-bank/dashboard-implementation-plan.md and execute Part 3A: Gateway Bus Interface.
Reference: memory-bank/dashboard-technical-spec.md
```

```
Read memory-bank/dashboard-implementation-plan.md and execute Part 3B: Foreign Plugin Endpoint.
Reference: memory-bank/dashboard-technical-spec.md
```

---

## Progress Tracking

Update this section as parts are completed:

| Part | Status | Notes |
|------|--------|-------|
| 1A | complete | Execution stats added to DashboardStats, repository, handlers |
| 1B | complete | Legacy API removed (LegacyExecutionSession, list_legacy_sessions, /sessions route) |
| 1C | complete | Legacy types removed, V2 types added (SessionWithExecutions, DashboardStats, AgentExecution) |
| 1D | complete | Dashboard rewritten to use V2 API, displays sessions with execution hierarchy |
| 2A | complete | TriggerSource enum added, source field on Session, DB schema updated |
| 2B | complete | SessionStatus::Queued added, create_session_queued(), start_session() methods, sessions_queued in stats |
| 2C | complete | sessions_by_source HashMap added to DashboardStats, repository queries by source |
| 2D | complete | Source filter dropdown, SourceBadge component, SourceStatsBar, badges on SessionCards |
| 3A | complete | GatewayBus trait, SessionRequest/SessionHandle types, HttpGatewayBus implementation |
| 3B | complete | POST /api/gateway/submit endpoint, status/cancel/pause/resume endpoints |
