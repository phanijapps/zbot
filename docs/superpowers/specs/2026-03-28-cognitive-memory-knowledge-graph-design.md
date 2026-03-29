# Cognitive Memory & Knowledge Graph Design

**Date:** 2026-03-28
**Status:** Approved
**Approach:** B (Cognitive Memory Architecture) with forward path to C (Living Neural Network)

## Problem Statement

z-Bot has a sophisticated memory and knowledge graph infrastructure that is architecturally complete but operationally disconnected:

- **Distillation fires but fails silently** — fire-and-forget pattern (runner.rs:869-878) means LLM extraction failures are logged as warnings and lost. Result: only 128 memory facts exist (mostly skill/agent/ward index entries from intent analysis), not learned knowledge.
- **Knowledge graph is empty** — `knowledge_graph.db` has 0 entities, 0 relationships. Entity extraction only happens inside distillation, which isn't completing successfully.
- **Recall works but has nothing useful to inject** — hybrid search runs at session start but finds only indexed resource entries, not domain knowledge or user preferences.
- **Delegated agents get no recall** — only root sessions receive memory injection.
- **No ward-scoped knowledge** — all facts are agent-scoped, no project-level expertise isolation.
- **No execution learning** — agent cannot track what strategies worked or failed across sessions.

93 sessions with 5,859 messages and 19.3M tokens of rich financial analysis work are evaporating after each session.

## Design Principles

1. **Simplicity** — extend existing infrastructure, don't replace it
2. **Explainability** — every behavior traceable to a config value or database record
3. **Forward-looking** — schema and APIs designed so Approach C features layer on without rewrites
4. **Functional** — each section delivers working value independently

## North Star

z-Bot is a goal-oriented autonomous agent that produces final results without asking follow-up questions. Memory and knowledge graph are the backbone enabling this autonomy — if the agent can't recall context, it asks questions; if it can't learn from failures, it repeats mistakes; if it doesn't know user preferences, it produces generic output.

---

## Section 1: Fix the Pipeline

### Problem
Distillation triggers after session completion but failures are invisible. The fire-and-forget `tokio::spawn` pattern means errors only appear as `tracing::warn` — no persistent record, no retry, no visibility.

### Solution

#### 1.1 Distillation Health Reporting

New table in `conversations.db`:

```sql
CREATE TABLE distillation_runs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL UNIQUE, -- one row per session, retries update in place
    status TEXT NOT NULL,            -- 'success', 'failed', 'skipped', 'permanently_failed'
    facts_extracted INTEGER DEFAULT 0,
    entities_extracted INTEGER DEFAULT 0,
    relationships_extracted INTEGER DEFAULT 0,
    episode_created INTEGER DEFAULT 0,  -- boolean: 0 or 1
    error TEXT,
    retry_count INTEGER DEFAULT 0,
    duration_ms INTEGER,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_distillation_runs_status ON distillation_runs(status);
```

Every distillation attempt (success, failure, or skip) writes a record. One row per session — retries UPDATE the existing row (incrementing `retry_count`, updating `status` and `error`). The UNIQUE constraint on `session_id` enforces this and prevents race conditions between cron retry and manual backfill. This table powers the Observatory's learning health bar and the backfill command's idempotency check.

**Status lifecycle:** `failed` (retryable, retry_count < 3) → `permanently_failed` (retry_count >= 3, no more attempts) → or `success` (retry succeeded).

#### 1.2 Distillation Fallback Chain

When the primary provider fails during distillation:
1. Use the agent's configured provider first (same as execution)
2. If that fails, iterate through remaining providers from `providers.json` in definition order, skipping any that lack a model supporting structured extraction (tool use or JSON mode)
3. Use each provider's `default_model()` on fallback
4. If all providers fail, mark the session as `failed` in `distillation_runs` with the error
5. Failed sessions are eligible for retry via cron (Section 1.3)

#### 1.3 Retry via Cron

A cron job (configurable, default: every 30 minutes) queries `distillation_runs` for `status = 'failed'` with `retry_count < 3`. Retries UPDATE the existing row: increment `retry_count`, attempt distillation via fallback chain, update `status` to `success` or keep as `failed`. After 3 failures, status becomes `permanently_failed` (no more automatic retries).

`retry_count` is included in the initial CREATE TABLE (Section 1.1) — no separate ALTER needed.

#### 1.4 API Endpoint

```
GET /api/distillation/status
```

Returns: total sessions, distilled count, failed count, skipped count, pending count (sessions without a distillation_runs entry).

### Files Changed
- `gateway/gateway-database/src/schema.rs` — new table + migration
- `gateway/gateway-execution/src/distillation.rs` — write distillation_runs records, fallback chain
- `gateway/gateway-execution/src/runner.rs` — pass provider list to distiller
- `gateway/src/http/graph.rs` — new status endpoint
- `gateway/gateway-cron/` — retry cron job

### Forward Path to C
The `distillation_runs` table becomes the foundation for execution replay — re-distilling sessions with a more mature graph for richer extraction.

---

## Section 2: Memory Tiers

### Problem
All 128 memory facts live in a flat structure with no distinction between knowledge types, no session outcome tracking, and no ward-level scoping.

### Solution

#### 2.1 Ward-Scoped Semantic Memory

Add `ward_id` column to `memory_facts` and update the UNIQUE constraint:

```sql
ALTER TABLE memory_facts ADD COLUMN ward_id TEXT NOT NULL DEFAULT '__global__';
CREATE INDEX idx_memory_facts_ward ON memory_facts(ward_id);
```

**UNIQUE constraint migration:** The existing constraint is `UNIQUE(agent_id, scope, key)`. This must change to `UNIQUE(agent_id, scope, ward_id, key)` so the same key can exist as both global and ward-local knowledge. SQLite doesn't support `ALTER CONSTRAINT`, so the migration recreates the table (standard SQLite migration pattern already used in schema.rs).

We use a sentinel value `'__global__'` instead of NULL for global facts because SQLite treats NULLs as distinct in UNIQUE constraints — two rows with `ward_id = NULL` and the same key would NOT conflict, which is undesirable.

- `ward_id = "__global__"` → global knowledge (user preferences, general corrections)
- `ward_id = "finance-ward"` → ward-local knowledge (project-specific patterns)

At recall time: query `WHERE ward_id = '__global__' OR ward_id = :current_ward`. Simple filter on existing hybrid search. The FTS5 index does NOT need `ward_id` added — ward filtering happens via JOIN back to the main table (same pattern as existing `search_memory_facts_fts` at memory_repository.rs:282-290).

#### 2.2 Episodic Memory

New table in `conversations.db` (managed by `gateway-database` crate, added to `schema.rs` migration). Lives in `conversations.db` because it references `session_id` (FK to sessions table) and is queried alongside `memory_facts` during recall — single database = single transaction, no cross-DB joins needed.

```sql
CREATE TABLE session_episodes (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    ward_id TEXT,                    -- nullable, same scoping as facts
    task_summary TEXT NOT NULL,       -- "Analyze SPY options chain for April expiry"
    outcome TEXT NOT NULL,            -- 'success', 'partial', 'failed', 'crashed'
    strategy_used TEXT,               -- "delegated to data-analyst for technicals, then research-agent for sentiment"
    key_learnings TEXT,               -- what went well, what didn't
    token_cost INTEGER,               -- total tokens consumed
    embedding BLOB,                   -- embedded task_summary for similarity search
    created_at TEXT NOT NULL
);

CREATE INDEX idx_session_episodes_agent ON session_episodes(agent_id);
CREATE INDEX idx_session_episodes_ward ON session_episodes(ward_id);
CREATE INDEX idx_session_episodes_outcome ON session_episodes(outcome);
```

Populated during distillation — same LLM call, extended extraction target. The distiller already loads the full transcript; we add: "Summarize what was attempted, the outcome, the strategy used, and key learnings."

#### 2.3 Procedural Memory (Strategy Facts)

No new table. Strategy knowledge stored as `memory_facts` with `category = 'strategy'`.

When the distiller sees patterns across successful episodes (same task type, same winning strategy), it writes strategy facts:
- Key: `strategy.financial_analysis`
- Content: `"Delegate to data-analyst with explicit sub-tasks. Avoid single-agent deep analysis beyond 200K tokens."`
- Confidence: 0.9+ (proven by outcomes)

Strategy facts are prioritized in recall via category weights (Section 3).

### How the Three Tiers Work Together

```
User sends: "Analyze AAPL options for next month"

Recall fires:
  1. Semantic: "user prefers technical + fundamental analysis" (correction fact)
  2. Episodic: "SPY options analysis succeeded with delegation strategy, 180K tokens"
  3. Procedural: "strategy.financial_analysis → delegate to data-analyst first"
  4. Ward-local: (if in finance ward) "this ward tracks options chains in CSV format"

All injected as system context → agent knows HOW to approach without asking.
```

### Files Changed
- `gateway/gateway-database/src/schema.rs` — ward_id column, session_episodes table
- `gateway/gateway-database/src/memory_repository.rs` — ward_id in queries, episode CRUD
- `gateway/gateway-execution/src/distillation.rs` — extended extraction (episode + strategy)
- `gateway/gateway-database/src/memory_repository.rs` — MemoryFact struct gets ward_id field (struct defined here, not in zero-core)

### Forward Path to C
Episodes with embeddings enable cross-agent knowledge transfer — "research-agent succeeded with this approach for a similar task, data-analyst should try it." The embedding on task_summary is the similarity signal.

---

## Section 3: Recall Priority Engine

### Problem
Current hybrid search scoring treats all facts equally. A user correction scores the same as a skill index entry. No ward-level boosting.

### Solution

#### 3.1 Category Priority Weights

Configurable via `~/Documents/zbot/config/recall_config.json`:

```json
{
  "category_weights": {
    "correction": 1.5,
    "strategy": 1.4,
    "user": 1.3,
    "instruction": 1.2,
    "domain": 1.0,
    "pattern": 0.9,
    "ward": 0.8,
    "skill": 0.7,
    "agent": 0.7
  },
  "ward_affinity_boost": 1.3,
  "max_recall_tokens": 3000,
  "vector_weight": 0.7,
  "bm25_weight": 0.3,
  "max_facts": 10,
  "max_episodes": 3,
  "high_confidence_threshold": 0.9,
  "mid_session_recall": {
    "enabled": true,
    "every_n_turns": 5,
    "min_novelty_score": 0.3
  }
}
```

#### 3.2 Scoring Formula

```
final_score = base_score × category_weight × ward_affinity
```

Where:
- `base_score` = existing `(vector_weight × cosine + bm25_weight × BM25) × confidence × recency × mention_boost`
- `category_weight` = lookup from config
- `ward_affinity` = 1.3 if fact's ward_id matches current ward, 1.0 otherwise

#### 3.3 Config Resilience

- **Compiled defaults** baked into the binary as `DEFAULT_RECALL_CONFIG`
- **File missing** → use compiled defaults, log info
- **File corrupted** → use compiled defaults, log warning
- **Partial file** → deep merge with compiled defaults (user values win per key)
- **File is never auto-created or auto-modified** by the system
- **`zero config recall --init`** generates a starter file from current defaults (refuses to overwrite unless `--force`)
- **Upgrades** add new keys to compiled defaults; user's file untouched, new keys get default values

#### 3.4 Expanded Recall Triggers

| Trigger | Current | After |
|---|---|---|
| New session start | Yes (runner.rs:571) | Yes, unchanged |
| Continuation | Hardcoded query | Uses actual continuation message |
| Delegation spawn | No | Yes — fresh recall using child's agent_id + delegation task as query, in `spawn_delegated_agent()` (delegation/spawn.rs), injected as system message in child's history before first LLM call |
| Post-delegation resume | No | Yes — after child completes, run recall using root's agent_id + child's result summary as query, inject any novel facts (key-based dedup against existing context) |
| Mid-session (automatic) | No | Yes — every N turns, novelty-filtered |
| Mid-session (agent-initiated) | Basic hybrid search | Full priority engine via upgraded memory.recall tool |

#### 3.5 Mid-Session Recall

**Automatic (middleware-driven):** Every `every_n_turns` turns, the middleware runs the recall priority engine with the most recent user message as query. Novelty filtering works in two stages:

1. **Key-based dedup:** The middleware maintains a `HashSet<String>` of fact keys already injected in this session (initialized from the session-start recall, updated on each mid-session injection). Facts with keys already in the set are excluded.
2. **Score threshold:** Among remaining (novel) facts, only those with `final_score >= min_novelty_score` (default 0.3) are injected. This prevents injecting marginally relevant facts just because they haven't been seen.

If no novel facts pass both filters, the middleware skips injection silently (no empty system message).

**Agent-initiated:** The existing `memory.recall()` tool is upgraded to use the full priority engine (category weights, ward affinity, episodic lookup) instead of basic hybrid search. Same tool name, same interface, better engine.

#### 3.6 Formatted Recall Output

```markdown
## Recalled Knowledge
### Corrections & Preferences
- [correction] Don't use single-agent for deep analysis (0.95)
- [user] Prefers technical + fundamental combined approach (0.90)

### Relevant Past Experiences
- SPY options analysis (2026-03-25): SUCCESS — delegated data-analyst → research-agent, 180K tokens
- PTON deep dive (2026-03-23): FAILED — single root agent, crashed at 632K tokens

### Domain Context
- [domain] Options chain analysis requires IV percentile comparison (0.85)
- [strategy] financial_analysis → delegate to data-analyst with explicit sub-tasks (0.92)
```

Token budget: capped at `max_recall_tokens` (default 3000). Trim from bottom (lowest-scoring) if exceeded. Budget is generous because episodes are verbose (~200 tokens each) and 10 facts + 3 episodes can approach 2000 tokens easily.

### Files Changed
- `gateway/gateway-execution/src/recall.rs` — priority weights, ward affinity, episode search, formatted output
- `gateway/gateway-services/` — new RecallConfig service (load + merge + fallback)
- `runtime/agent-runtime/src/middleware/` — mid-session recall middleware
- `runtime/agent-tools/src/tools/memory.rs` — upgrade recall tool to use priority engine
- `gateway/gateway-execution/src/runner.rs` — delegation recall injection, continuation query fix
- New file: `gateway/gateway-services/src/recall_config.rs`

### Forward Path to C
The category_weights map becomes the input to graph traversal edge weighting. The config file gains graph traversal parameters (max_hops, decay_per_hop) when C adds graph-driven reasoning.

---

## Section 4: Execution Scoring & Meta-Cognitive Loop

### Problem
The agent has no memory of what worked or failed. It cannot learn from past execution outcomes.

### Solution

#### 4.1 Outcome Assessment During Distillation

The distillation LLM prompt is extended with one additional extraction target:

```
Given the session transcript, assess:
1. Did the agent complete the user's goal? (success / partial / failed)
2. What strategy was used? (free text summary)
3. What went well and what didn't? (key learnings)
4. Was the token cost reasonable for the task complexity? (efficient / acceptable / wasteful)
```

This populates `session_episodes` (Section 2). No separate LLM call — one distillation pass extracts facts, entities, relationships, AND the episode.

#### 4.2 Feedback Loop

The meta-cognitive loop:

```
New task arrives
  → Recall fires (Section 3)
  → Episodic recall finds similar past tasks (cosine on embedded task_summary)
  → If past task FAILED: inject failure reason as warning
  → If past task SUCCEEDED: inject strategy as recommendation
  → Agent adjusts approach before first LLM call
```

No reinforcement learning. No gradient descent. The LLM does the reasoning — we give it the right context.

#### 4.3 Strategy Emergence

When the distiller processes a session and finds similar successful episodes:
1. Query `session_episodes` for same agent_id, outcome='success', cosine similarity > 0.7 on task_summary
2. If 2+ similar successes with the same strategy pattern → write/update a strategy fact
3. Strategy facts have high confidence (0.9+) and get priority boost in recall

This is conservative — strategies only emerge from repeated success, not from a single session.

**Episode similarity search implementation:** A new `search_episodes_by_similarity()` method in `MemoryRepository` (same crate as `search_memory_facts_hybrid`). Uses the same brute-force cosine approach: load all episode embeddings for the agent, compute cosine similarity in Rust, return top-K above threshold. This is viable because episode count grows slowly (~1 per root session). For the current 18 root sessions and projected hundreds, brute-force is fast (sub-millisecond for <10K rows). Same approach used for memory facts today (memory_repository.rs:393-429).

### Files Changed
- `gateway/gateway-execution/src/distillation.rs` — episode extraction, strategy emergence
- `gateway/gateway-database/src/memory_repository.rs` — episode CRUD, `search_episodes_by_similarity()` method (brute-force cosine, same pattern as fact vector search)
- Distillation prompt template — extended extraction targets, add `strategy` category to the allowed categories list (alongside existing: user, pattern, domain, instruction, correction)

### Forward Path to C
Execution scoring becomes the training signal for predictive recall — "given this task type, what knowledge was present in successful executions vs. failed ones?"

---

## Section 5: Retroactive Bootstrap

### Problem
93 existing sessions with rich conversation data are undistilled. The knowledge graph is empty. The observatory would show nothing.

### Solution

#### 5.1 CLI Command

```bash
zero distill --backfill
```

Process:
1. Query sessions with `status = 'completed'` that have no `distillation_runs` entry
2. Sort by `created_at` ascending (chronological — earlier entities exist when later sessions reference them)
3. For each session:
   - Load transcript (same as live distillation)
   - Skip if < 4 messages (same threshold)
   - Run full distillation: facts, entities, relationships, episode
   - Write to all tables
   - Record in `distillation_runs` (idempotent — safe to run twice)
4. Rate-limit: configurable concurrent LLM calls (default 2). Note: knowledge graph writes serialize on `GraphStorage`'s `Arc<Mutex<Connection>>`, so concurrency benefits LLM extraction time but graph writes are sequential. This is a correctness guarantee, not a bottleneck — the LLM call dominates latency.

#### 5.2 Progress Reporting

```
Backfill: 93 sessions found, 0 previously distilled
[  1/93] Session abc123 (2026-03-22) — 12 msgs — 6 facts, 4 entities, 3 rels ✓
[  2/93] Session def456 (2026-03-22) — 3 msgs — skipped (< 4 messages)
...
Complete: 78/93 distilled, 12 skipped, 3 failed
Facts: 342 | Entities: 89 | Relationships: 127 | Episodes: 78
```

#### 5.3 Idempotency

The `distillation_runs` table is the deduplication key. Re-running `--backfill` only processes sessions without an entry. This also means: if live distillation starts working after the fix (Section 1), backfill naturally skips already-processed sessions.

Can also be added to a startup hook or cron schedule to catch any sessions that slipped through live failures.

### Files Changed
- `apps/cli/` — new `distill --backfill` subcommand
- `gateway/gateway-execution/src/distillation.rs` — batch mode with rate limiting
- Reuses existing SessionDistiller — no new distillation logic

### Forward Path to C
Backfill becomes execution replay — re-distilling sessions with a more mature graph and improved extraction prompts to get richer knowledge from the same data.

---

## Section 6: Observatory UI

### Problem
No visibility into the agent's knowledge. No way to see what the agent knows, how entities connect, or whether the learning pipeline is healthy.

### Solution

#### 6.1 Force-Directed Graph (Main View)

- **Library:** D3-force (SVG-based, React-friendly, lightweight)
- **Node size:** proportional to `mention_count`
- **Node color:** by `entity_type` (person=indigo, concept=amber, agent/tool=green, project=red, strategy=purple)
- **Edge thickness:** proportional to relationship co-occurrence count
- **Interactions:** pan, zoom, drag nodes, click for detail
- **Scaling:** SVG rendering performs well up to ~500 nodes. Default view filters to top entities by mention_count (configurable, default 200). Full graph available via "Show all" toggle. If entity count grows beyond 1000, Canvas rendering is the forward path (same D3-force layout, different renderer).
- **Route:** `/observatory` alongside existing `/memory`, `/logs`, `/settings`

#### 6.2 Detail Sidebar (Slide-Over on Click)

Shows for the selected entity:
- Entity name, type, mention count
- All connections grouped by relationship type
- Related memory facts (matched by entity name in fact content)
- Timeline: first_seen_at, last_seen_at, session count

Read-only for now. The path to "command center" (Approach D) means adding edit/delete buttons here later.

#### 6.3 Learning Health Bar (Bottom)

Powered by `distillation_runs` table and aggregate queries:
- Sessions distilled / total
- Facts count, entities count, relationships count, episodes count
- Failed count, skipped count

#### 6.4 Filters

- **Agent filter pills:** All Agents | root | data-analyst | research-agent
- **Ward scope pills:** Global | finance-ward | agentzero-dev | ...
- **Search:** entity name filter with highlight
- Ward scope filter: `WHERE ward_id IS NULL OR ward_id = :selected_ward` + global entities with connections into that ward

#### 6.5 API Endpoints

```
GET /api/graph/:agent_id/entities               — entities for agent (filterable by ward_id, type)
GET /api/graph/:agent_id/entities/:id/connections — entity with 1-hop neighbors
GET /api/graph/:agent_id/subgraph?center=:id&depth=2 — BFS traversal for focus view
GET /api/graph/:agent_id/search?q=options       — entity name search
GET /api/graph/all/entities?ward_id=...          — cross-agent view (Observatory "All Agents" mode)
GET /api/graph/stats                             — aggregate counts for health bar
GET /api/distillation/status                     — distillation health from distillation_runs
```

Existing endpoints in `gateway/src/http/graph.rs` are scoped under `/api/graph/:agent_id/...`. We follow this pattern for agent-specific views. The new `/api/graph/all/entities` endpoint serves the Observatory's "All Agents" cross-agent mode. The `/api/graph/stats` and `/api/distillation/status` endpoints are global (not agent-scoped).

#### 6.6 UI Architecture Compliance

Follows existing patterns from `apps/ui/ARCHITECTURE.md`:
- Semantic CSS classes (BEM-style), not inline Tailwind
- Design tokens from `theme.css` — entity colors map to the token system
- Card grid + slide-over pattern for detail sidebar
- New feature module: `apps/ui/src/features/observatory/`

### Files Changed
- `apps/ui/src/features/observatory/` — new feature module
  - `ObservatoryPage.tsx` — main page with graph canvas
  - `GraphCanvas.tsx` — D3-force graph component
  - `EntityDetail.tsx` — slide-over sidebar
  - `LearningHealthBar.tsx` — bottom status bar
  - `graph-hooks.ts` — data fetching hooks
- `apps/ui/src/App.tsx` — add /observatory route
- `apps/ui/src/styles/components.css` — observatory component classes
- `gateway/src/http/graph.rs` — stats and distillation status endpoints
- `package.json` — add `d3-force`, `d3-selection`, `d3-zoom` dependencies

### Forward Path to C
D3's transition system handles the "living brain" animations natively — pulsing nodes during active execution, thought pathways lighting up during recall. Same component, add `d3.transition()` calls. No rewrite.

---

## Schema Changes Summary

### New Tables
1. `distillation_runs` — distillation health tracking and retry
2. `session_episodes` — episodic memory with outcomes and strategies

### Altered Tables
1. `memory_facts` — add `ward_id TEXT` nullable column

### New Config Files
1. `~/Documents/zbot/config/recall_config.json` — optional, compiled defaults as fallback

### New API Endpoints
1. `GET /api/distillation/status`
2. `GET /api/graph/stats`
3. Enhanced: `GET /api/graph/entities` with ward_id filter

### New UI Route
1. `/observatory` — knowledge graph visualization

### New CLI Command
1. `zero distill --backfill` — retroactive distillation
2. `zero config recall --init` — generate starter config

---

## Dependencies

### New npm packages
- `d3-force` — force-directed graph layout
- `d3-selection` — DOM manipulation for SVG
- `d3-zoom` — pan/zoom interactions

### No new Rust crates required
All database operations use existing `rusqlite`. All embedding operations use existing `fastembed`. No new external dependencies on the backend.

---

## What Stays the Same

- Hybrid search algorithm (FTS5 + vector cosine similarity)
- Embedding pipeline (fastembed, all-MiniLM-L6-v2, 384d)
- Embedding cache (SHA256 hash-based dedup)
- Knowledge graph schema (kg_entities, kg_relationships — unchanged)
- Memory tool API surface (save_fact, recall — same interface, better engine)
- Fire-and-forget distillation pattern (for happy path — retry handles failures)
- Middleware pipeline architecture
- WebSocket event system
- All existing UI pages and components
