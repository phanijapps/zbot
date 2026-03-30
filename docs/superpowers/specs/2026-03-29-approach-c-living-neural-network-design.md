# Approach C: Living Neural Network — Design Spec

**Date:** 2026-03-29
**Status:** Approved
**Prerequisite:** Approach B (Cognitive Memory & Knowledge Graph) — complete
**Constraint:** Must run on Raspberry Pi 4 (4GB RAM, quad-core Cortex-A72). Design for Pi floor, benefit from desktop ceiling.

## Problem Statement

Approach B built the memory infrastructure: distillation, recall, episodes, knowledge graph, Observatory. But the agent still:

- **Misses graph connections** — "analyze MSFT" recalls MSFT domain facts but not the PowerShell correction 2 hops away via `MSFT → financial-analysis → PowerShell → no_bash_syntax`
- **Treats old facts same as new** — SPY price from March 22 has the same weight as a correction from yesterday
- **Doesn't learn from success patterns** — 5 successful financial analyses all used the same 3 corrections, but the agent doesn't know that
- **Grows unboundedly** — `conversations.db` accumulates raw transcripts that are never queried after distillation
- **Ward knowledge isn't portable** — facts live in SQLite, not in the ward folder

## Design Principles

1. **Pi 4 floor** — no in-memory graph loading, bounded SQLite queries, lazy computation
2. **SQLite stays** — the hot store for distilled knowledge. Future option to swap graph backend via trait.
3. **Lazy over eager** — compute on demand at recall time, cache if reused, never precompute what you might not need
4. **Config-driven** — all features togglable and tunable in `recall_config.json`

## Storage Architecture

```
Session transcript (cold → archive after 7 days)
       ↓ distillation
Facts + Episodes (hot → SQLite forever, pruned when decayed)
Entities + Relationships (hot → knowledge_graph.db forever)
Ward summary (warm → ward/memory/ward.md, portable)
Archive (cold → data/archive/{session_id}.jsonl.gz)
```

Nothing that feeds the graph or recall gets offloaded. The archive only takes raw messages and logs — already distilled.

---

## Section 1: Session Offload (Hot/Cold Storage)

### Problem
`conversations.db` grows unboundedly with messages and logs. After distillation extracts knowledge, raw transcripts are dead weight.

### Design

**Hot store (SQLite):** Facts, episodes, entities, relationships, distillation_runs, session metadata (with `archived` flag). Always in `conversations.db` and `knowledge_graph.db`.

**Cold store (filesystem):** Session transcripts (messages) + execution logs. Compressed to `.jsonl.gz` per session at `~/Documents/zbot/data/archive/{session_id}.jsonl.gz`.

**Offload criteria:** Session must be:
- `completed_at < now - offload_after_days` (default 7)
- Has a `distillation_runs` entry with `status = 'success'` (knowledge extracted)

**Offload process:**
1. Export messages + execution_logs for the session to `{archive_path}/{session_id}.jsonl.gz`
2. DELETE messages and execution_logs rows from SQLite
3. UPDATE session row: set `archived = 1`
4. KEEP: session metadata, all facts, episodes, entities, relationships

**Archive format:** One `.jsonl.gz` per session:
```jsonl
{"type":"session","id":"sess-abc123","agent_id":"root","status":"completed",...}
{"type":"message","id":"msg-001","role":"user","content":"Analyze SPY..."}
{"type":"log","id":"log-001","category":"tool_call","message":"shell: ls -la"}
```

**Restore:** `zero sessions restore {session_id}` — decompress and re-insert into SQLite.

**CLI:** `zero sessions archive --older-than 7d`
**Auto:** Configurable cron schedule.

**Config:**
```json
{
  "session_offload": {
    "enabled": true,
    "offload_after_days": 7,
    "keep_session_metadata": true,
    "archive_path": "data/archive"
  }
}
```

### Schema Change

```sql
ALTER TABLE sessions ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
```

### Pi Impact
After 6 months / 5,000 sessions: SQLite ~50MB (distilled knowledge only). Archive ~500MB on filesystem (compressed). Without offload: SQLite ~2GB+.

---

## Section 2: Graph-Driven Recall

### Problem
Current recall uses cosine similarity + FTS5. When you ask "analyze MSFT," it finds MSFT facts but misses corrections connected via graph relationships 2 hops away.

### Design

**GraphTraversal trait (future-proof for Neo4j):**

```rust
#[async_trait]
pub trait GraphTraversal: Send + Sync {
    async fn traverse(&self, entity_id: &str, max_hops: u8) -> Result<Vec<TraversalNode>, String>;
    async fn connected_entities(&self, names: &[&str], max_hops: u8) -> Result<Vec<TraversalNode>, String>;
}

pub struct TraversalNode {
    pub entity: Entity,
    pub hop_distance: u8,
    pub path: Vec<String>,      // relationship types traversed
    pub relevance: f64,          // decays per hop
}
```

**SQLite implementation (Pi-safe):**

2-hop BFS using `WITH RECURSIVE` CTE, bounded by `max_hops` and `LIMIT`. With indexes on `kg_relationships(source_entity_id)` and `kg_relationships(target_entity_id)`, runs in <10ms on Pi 4 for graphs up to 10K entities.

```sql
WITH RECURSIVE graph_walk(entity_id, hop, path) AS (
    SELECT ?1, 0, ?1
    UNION ALL
    SELECT
        CASE WHEN r.source_entity_id = gw.entity_id
             THEN r.target_entity_id ELSE r.source_entity_id END,
        gw.hop + 1,
        gw.path || ',' || r.relationship_type
    FROM graph_walk gw
    JOIN kg_relationships r ON r.source_entity_id = gw.entity_id
                            OR r.target_entity_id = gw.entity_id
    WHERE gw.hop < ?2
)
SELECT DISTINCT e.*, gw.hop, gw.path
FROM graph_walk gw
JOIN kg_entities e ON e.id = gw.entity_id
ORDER BY gw.hop ASC, e.mention_count DESC
LIMIT ?3
```

**Integration with recall pipeline:**

Current: `embed query → hybrid search → score → format`
New: `embed query → hybrid search → **graph expansion** → score → format`

After hybrid search returns top facts, extract entity names from them. Feed into `connected_entities(names, max_hops)`. The graph returns related entities within N hops. Look up facts connected to those entities (by matching entity name in fact content or key). Merge into result set with hop decay.

**Hop decay:**
```
hop 0 (direct match): score × 1.0
hop 1 (1 neighbor away): score × hop_decay
hop 2 (2 neighbors away): score × hop_decay²
```

Default `hop_decay = 0.6`:
- Hop 0: 1.0
- Hop 1: 0.6
- Hop 2: 0.36

**Config:**
```json
{
  "graph_traversal": {
    "enabled": true,
    "max_hops": 2,
    "hop_decay": 0.6,
    "max_graph_facts": 5
  }
}
```

**Example:**
```
Query: "analyze MSFT"

Hybrid search finds:
  - domain.finance.msft.fundamentals (score 0.92)
  - skill:yf-data (score 0.85)

Graph expansion from [MSFT, yf-data]:
  MSFT → part_of → financial-analysis (hop 1, relevance 0.6)
  financial-analysis → uses → PowerShell (hop 2, relevance 0.36)

Facts connected to "PowerShell":
  - correction.shell.powershell_syntax (0.95 × 0.36 = 0.34)

Merged: PowerShell correction now in result set — found via graph, not cosine.
```

### Files Changed
- New: `services/knowledge-graph/src/traversal.rs` — `GraphTraversal` trait + `SqliteGraphTraversal` impl
- Modify: `services/knowledge-graph/src/lib.rs` — export traversal module
- Modify: `gateway/gateway-execution/src/recall.rs` — graph expansion step after hybrid search
- Modify: `gateway/src/state.rs` — wire traversal into recall

---

## Section 3: Temporal Decay

### Problem
All facts are scored equally regardless of age. SPY price data from 7 days ago competes with a correction from yesterday.

### Design

**Decay formula (applied in Rust during recall scoring):**

```rust
fn temporal_decay(last_seen: DateTime<Utc>, half_life_days: f64) -> f64 {
    let age_days = (Utc::now() - last_seen).num_days() as f64;
    1.0 / (1.0 + (age_days / half_life_days))
}
```

**Per-category half-lives:**

| Category | Half-life | Rationale |
|---|---|---|
| correction | 90 days | Mistakes stay relevant |
| instruction | 120 days | Standing orders are durable |
| user | 180 days | Preferences change slowly |
| strategy | 60 days | Approaches evolve |
| pattern | 45 days | Workarounds may become obsolete |
| domain | 30 days | Data goes stale fastest |
| skill/agent | never | Re-indexed each session |

**Mention count counters decay:**
```
final_score = base_score × category_weight × ward_affinity × decay × (1 + log2(mention_count))
```

Frequently recalled facts resist decay. Unused facts fade naturally.

**Pruning:** Facts below `prune_threshold` (0.05) effective score for `prune_after_days` (30) get moved to `memory_facts_archive` table. Keeps SQLite lean.

```sql
CREATE TABLE memory_facts_archive (
    -- Same schema as memory_facts
    -- Moved here when decayed below threshold
    archived_at TEXT NOT NULL
);
```

**Config:**
```json
{
  "temporal_decay": {
    "enabled": true,
    "half_life_days": {
      "correction": 90,
      "strategy": 60,
      "domain": 30,
      "user": 180,
      "pattern": 45,
      "instruction": 120
    },
    "prune_threshold": 0.05,
    "prune_after_days": 30
  }
}
```

### Files Changed
- Modify: `gateway/gateway-execution/src/recall.rs` — apply decay in scoring pipeline
- Modify: `gateway/gateway-services/src/recall_config.rs` — add decay config
- Modify: `gateway/gateway-database/src/schema.rs` — migration v13: `memory_facts_archive` table, `sessions.archived` column
- New: `gateway/gateway-execution/src/pruning.rs` — fact pruning logic (CLI + cron callable)

---

## Section 4: Predictive Recall

### Problem
The agent doesn't learn from success patterns. 5 successful financial analyses all used the same corrections, but the agent doesn't know that correlation.

### Design

**New table — recall_log:**

```sql
CREATE TABLE recall_log (
    session_id TEXT NOT NULL,
    fact_key TEXT NOT NULL,
    recalled_at TEXT NOT NULL,
    PRIMARY KEY (session_id, fact_key)
);
CREATE INDEX idx_recall_log_session ON recall_log(session_id);
```

When the `memory.recall` tool runs, log which fact keys were returned. Lightweight — key + session_id only, no content duplication.

**Predictive scoring (during recall):**

After normal recall, before final scoring:

1. Find similar successful episodes:
   ```
   episodes = episode_repo.search_by_similarity(agent_id, query_embedding, 0.5, max_episodes)
       .filter(outcome == "success")
   ```

2. Get fact keys recalled in those sessions:
   ```
   predictive_keys = recall_log.get_keys_for_sessions(episode_session_ids)
   ```

3. Count occurrences — facts recalled in 2+ successful sessions get boosted:
   ```
   if predictive_keys.count(fact.key) >= min_similar_successes:
       fact.score *= predictive_boost
   ```

**The flywheel:** More sessions → better predictions → better recall → more success → stronger predictions.

**Config:**
```json
{
  "predictive_recall": {
    "enabled": true,
    "min_similar_successes": 2,
    "predictive_boost": 1.3,
    "max_episodes_to_check": 5
  }
}
```

### Pi Performance
One extra SQLite query per recall: fetch fact keys for 3-5 sessions (~30-50 rows). Negligible.

### Files Changed
- Modify: `gateway/gateway-database/src/schema.rs` — migration v13: `recall_log` table
- New: `gateway/gateway-database/src/recall_log_repository.rs` — CRUD for recall_log
- Modify: `gateway/gateway-execution/src/recall.rs` — predictive boost in scoring
- Modify: `runtime/agent-tools/src/tools/memory.rs` — log recalled keys after recall tool executes
- Modify: `gateway/gateway-database/src/memory_fact_store.rs` — log recalled keys in prioritized recall

---

## Section 5: Ward File Sync

### Problem
Ward knowledge lives only in SQLite. If you copy a ward folder, the knowledge doesn't come along.

### Design

After distillation, write a human-readable summary to `{ward_path}/memory/ward.md`:

```markdown
# Ward Knowledge: financial-analysis
*Auto-generated from knowledge graph. Last updated: 2026-03-29*

## Corrections (ALWAYS follow)
- PowerShell: no bash syntax (||, &&), use try/catch
- apply_patch: strict '*** Begin Patch' format, no inline HTML
- shell: no file creation via redirect, use apply_patch

## Patterns
- yfinance MultiIndex columns: flatten with `[c[0] for c in df.columns]`
- PowerShell multiline strings: use Python scripts, not inline

## Domain
- SPY: P/E 25.73, oversold RSI 25.8 (as of March 2026)
- MSFT: P/E 22.33, strong buy conviction (as of March 2026)

## Key Entities
- SPY (82 mentions), MSFT (15 mentions), yfinance (50 mentions)
- data-analyst (primary subagent for financial analysis)
```

**Generation trigger:** Post-distillation, if the session was in a ward.
**Not a source of truth** — just a readable projection. If deleted, regenerated on next distillation.
**Portable** — copy the ward folder, knowledge comes along.

### Files Changed
- New: `gateway/gateway-execution/src/ward_sync.rs` — generate ward.md from facts + graph
- Modify: `gateway/gateway-execution/src/distillation.rs` — call ward sync after successful distillation

---

## Schema Changes Summary (Migration v13)

### New Tables
1. `recall_log` — tracks which facts were recalled per session (predictive recall)
2. `memory_facts_archive` — cold store for pruned facts

### Altered Tables
1. `sessions` — add `archived INTEGER NOT NULL DEFAULT 0`

### New Config Fields in `recall_config.json`

```json
{
  "session_offload": {
    "enabled": true,
    "offload_after_days": 7,
    "archive_path": "data/archive"
  },
  "graph_traversal": {
    "enabled": true,
    "max_hops": 2,
    "hop_decay": 0.6,
    "max_graph_facts": 5
  },
  "temporal_decay": {
    "enabled": true,
    "half_life_days": { "correction": 90, "strategy": 60, "domain": 30, "user": 180, "pattern": 45, "instruction": 120 },
    "prune_threshold": 0.05,
    "prune_after_days": 30
  },
  "predictive_recall": {
    "enabled": true,
    "min_similar_successes": 2,
    "predictive_boost": 1.3,
    "max_episodes_to_check": 5
  }
}
```

### New CLI Commands
1. `zero sessions archive --older-than 7d` — offload old session transcripts
2. `zero sessions restore {session_id}` — restore archived session

### New Rust Trait
1. `GraphTraversal` — abstract graph backend (SQLite today, Neo4j tomorrow)

---

## Dependencies

### No new Rust crates
All operations use existing `rusqlite`, `flate2` (already in tree for gzip), `serde_json`. The `WITH RECURSIVE` CTE is standard SQLite 3.8.3+.

### No new npm packages
Observatory already has D3. No UI changes in this spec.

---

## What Stays the Same

- Memory.recall tool interface (agent calls it the same way)
- Recall nudges (session start, ward entry, post-delegation)
- Distillation pipeline (extended with ward sync, not replaced)
- Observatory UI (reads from same graph/facts tables)
- Corrections-as-rules formatting
- Capability gap detection
- Contradiction detection
- Failure clustering
