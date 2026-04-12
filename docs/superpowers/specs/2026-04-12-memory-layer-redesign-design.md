# Memory Layer Redesign — Design Spec

**Date:** 2026-04-12
**Branch target:** `feature/memory-layer-v2` (one long-lived branch; phases land as focused PRs)
**Replaces:** `2026-04-12-kg-activation-pack-a-design.md` (Pack A ships as Phase 0; this spec is the umbrella)

---

## Executive Summary

Redesign AgentZero's memory subsystem to support a **goal-oriented professional agent** that can ingest long documents in real time, scale to 100–200 MB of persistent knowledge over 3–4 months of daily use, and make that knowledge actually influence agent reasoning — not just accumulate in a log.

Research foundation (primary sources cited in Appendix A):

- **Zep/Graphiti** (arXiv:2501.13956, 2025) — bitemporal edges, episode-based ingestion, 94.8% DMR
- **A-MEM** (NeurIPS 2025, arXiv:2502.12110) — Zettelkasten self-organization, write-time linking
- **Letta/MemGPT v1** (2025) — memory blocks + sleep-time compute for consolidation
- **iText2KG** (Neo4j Nodes 2025, arXiv:2409.03284) — reuse-aware extraction prompt
- **MemGuide** (arXiv:2505.20231) — intent-aware retrieval for goal-oriented agents
- **MemOS** (arXiv, Jul 2025) — Consolidate/Index/Update/Forget memory operations
- **sqlite-vec** (asg017, stable 2025) — embedded vector ANN

Audit of the current codebase (Appendix B) reveals five load-bearing gaps:

1. Ingestion is session-end batch only — books/PDFs wait hours for extraction
2. EntityResolver is O(N) fuzzy + O(N) cosine scan — degrades past ~5k entities
3. Graph context is a 2000-char free-text tail, not a scored recall lane
4. No pruning/compaction — memory_facts and kg_entities grow unbounded
5. Goal-oriented primitives (intents, intent-aware retrieval) are absent

This spec fixes all five with one cohesive design shipped in five phases. Each phase is independently valuable and independently shippable.

---

## Design Principles (Invariants)

Five rules every component obeys. These are non-negotiable and drive all downstream decisions.

1. **Everything is an episode.** Every byte entering the graph attaches to a `kg_episodes` row with `source_type`, `source_ref`, `content_hash`. Provenance is mandatory.
2. **Extraction is streaming, not batch.** Content → chunker → episode → extraction queue → resolver → storage. Per-chunk. Non-blocking. Observable.
3. **Write-time linking, sleep-time consolidation.** New knowledge links to existing neighbors the moment it's written (A-MEM pattern). Heavy work — community summaries, compaction, cross-session synthesis — runs off-thread (Letta sleep-time pattern).
4. **Never hard-delete.** Contradictions set `invalidated_at`. Compaction sets `compressed_into`. Originals remain queryable; scoring filters them by default.
5. **Retrieval is unified and scored.** memory_facts, wiki articles, procedures, graph neighborhoods, goals — all enter a single scored pool, merged via Reciprocal Rank Fusion. No second-class free-text tails.

---

## Architecture

```
┌────────────────────── SOURCES ──────────────────────────────┐
│  Session turns · Tool results · Ward files · Documents ·    │
│  User corrections · Agent-saved facts · External APIs       │
└──────────────────────────┬──────────────────────────────────┘
                           │
                   ┌───────▼────────┐
                   │    Chunker     │  paragraph-aware, 800–1200 tok
                   │                │  10–15% overlap
                   └───────┬────────┘
                           │
                   ┌───────▼────────┐
                   │ Episode        │  content_hash dedup
                   │ Registry       │  status: pending/running/done/failed
                   │                │  retry_count, error
                   └───────┬────────┘
                           │
                   ┌───────▼────────┐
                   │ Extraction     │  tokio mpsc queue
                   │ Queue          │  N workers (default 2)
                   │                │
                   │ Two-pass LLM:  │  pass 1: entities + aliases
                   │  • entities    │  pass 2: relations (conditioned
                   │  • relations   │          on pass-1 entity list)
                   │                │  JSON-schema constrained output
                   │                │  conditioned on nearest existing
                   │                │  entities (iText2KG trick)
                   └───────┬────────┘
                           │
                   ┌───────▼────────┐
                   │ Resolver +     │  3 stages, O(log N):
                   │ Writer         │   1. normalized_hash (O(1))
                   │                │   2. ANN blocking (sqlite-vec)
                   │                │   3. LLM pairwise verify (on top-k)
                   │                │
                   │                │  Merges append to alias table
                   │                │  Writes are bitemporal
                   └───────┬────────┘
                           │
STORAGE            ┌───────▼─────────────────────────┐
                   │  kg_entities        kg_relationships │
                   │  kg_episodes        kg_aliases        │ ← new
                   │  kg_name_index      kg_goals          │ ← new
                   │  (sqlite-vec)       kg_compactions    │ ← new
                   │                                       │
                   │  memory_facts       ward_wiki         │
                   │  procedures         session_episodes  │
                   └───────┬───────────────────────────────┘
                           │
SLEEP-TIME WORKER ┌────────▼─────────────────────────┐
                   │  Runs hourly / on-idle:           │
                   │   • Community summaries (Leiden)  │
                   │   • Compaction (duplicate merge)  │
                   │   • Decay scoring (access-based)  │
                   │   • Cross-session synthesis       │
                   │   • Archival rotation             │
                   └──────────────────────────────────┘

RETRIEVAL          ┌──────────────────────────────────┐
                   │  Unified Recall Pool (RRF):        │
                   │   • memory_facts (scored)          │
                   │   • wiki articles (scored)         │
                   │   • procedures (intent-matched)    │
                   │   • graph neighborhoods (scored)   │
                   │   • active goals (lifecycle-filt)  │
                   │                                    │
                   │  Intent-aware filter (MemGuide):    │
                   │   boost items aligned with active  │
                   │   goal slots.                      │
                   └──────────────────────────────────┘

AGENT-FACING TOOLS (promoted in prompt shards)
  • ingest(path|url|text, source_id?)   — enqueue for streaming ingestion
  • graph_query(action, ...)            — existing, taught
  • memory(action, ...)                  — existing, taught
  • goal(action, ...)                    — NEW: create/update/complete goals
```

---

## Schema (v22, clean slate)

Both `conversations.db` and `knowledge_graph.db` are deleted before rollout. Schema v22 is the initial schema — no ALTER TABLEs, no backfills, no rollback. On daemon boot, if either DB is missing, the full v22 schema is created. Everything below defines v22 directly.

### New tables

**`kg_aliases`** — forever alias table (entity resolution short-circuit)

```sql
CREATE TABLE kg_aliases (
    id TEXT PRIMARY KEY,
    entity_id TEXT NOT NULL,
    surface_form TEXT NOT NULL,
    normalized_form TEXT NOT NULL,
    source TEXT NOT NULL,           -- 'extraction' | 'merge' | 'user'
    confidence REAL DEFAULT 1.0,
    first_seen_at TEXT NOT NULL,
    FOREIGN KEY (entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    UNIQUE(normalized_form, entity_id)
);
CREATE INDEX idx_aliases_normalized ON kg_aliases(normalized_form);
CREATE INDEX idx_aliases_entity ON kg_aliases(entity_id);
```

Every entity gets one self-alias row on creation (name → itself). Merges append loser-side surface forms. A future lookup of "Savarker" short-circuits to the existing entity without running resolver stages.

**`kg_name_index`** — sqlite-vec virtual table for ANN lookup on entity name embeddings

```sql
CREATE VIRTUAL TABLE kg_name_index USING vec0(
    entity_id TEXT PRIMARY KEY,
    name_embedding FLOAT[384]
);
```

One row per entity. Dimension 384 = `bge-small-en-v1.5` or equivalent lightweight embedder. Binary quantization optional at >1M rows.

**`kg_goals`** — first-class goals for goal-oriented retrieval

```sql
CREATE TABLE kg_goals (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    ward_id TEXT,
    title TEXT NOT NULL,
    description TEXT,
    state TEXT NOT NULL,              -- 'active' | 'blocked' | 'satisfied' | 'abandoned'
    parent_goal_id TEXT,              -- decomposition edges
    slots TEXT,                       -- JSON: required inputs/outputs
    filled_slots TEXT,                -- JSON: what we have so far
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    embedding BLOB,
    FOREIGN KEY (parent_goal_id) REFERENCES kg_goals(id)
);
CREATE INDEX idx_goals_agent_state ON kg_goals(agent_id, state);
CREATE INDEX idx_goals_ward ON kg_goals(ward_id);
```

**`kg_compactions`** — audit trail for merges and prunes

```sql
CREATE TABLE kg_compactions (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    operation TEXT NOT NULL,          -- 'merge' | 'prune' | 'invalidate'
    entity_id TEXT,
    relationship_id TEXT,
    merged_into TEXT,                 -- for 'merge' ops, points to surviving entity
    reason TEXT,                      -- e.g. 'cosine_0.94_same_type'
    created_at TEXT NOT NULL
);
CREATE INDEX idx_compactions_run ON kg_compactions(run_id);
```

No foreign keys — we preserve history even after referenced rows are deleted.

### Core tables defined at v22

**`kg_entities`** (full definition, replaces all prior schemas):

```sql
CREATE TABLE kg_entities (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    normalized_hash TEXT NOT NULL,
    properties TEXT,                    -- JSON
    epistemic_class TEXT NOT NULL DEFAULT 'current',  -- archival|current|convention|procedural
    confidence REAL NOT NULL DEFAULT 0.8,
    mention_count INTEGER NOT NULL DEFAULT 1,
    access_count INTEGER NOT NULL DEFAULT 0,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    last_accessed_at TEXT,
    t_valid_from TEXT,                  -- bitemporal
    t_valid_to TEXT,
    t_invalidated_by TEXT,              -- episode or entity that invalidated this
    compressed_into TEXT,               -- set when compactor merges into another entity
    source_episode_ids TEXT             -- JSON array
);
CREATE INDEX idx_entities_normalized_hash ON kg_entities(agent_id, entity_type, normalized_hash);
CREATE INDEX idx_entities_agent_type ON kg_entities(agent_id, entity_type);
CREATE INDEX idx_entities_name ON kg_entities(name);
CREATE INDEX idx_entities_last_accessed ON kg_entities(last_accessed_at);
CREATE INDEX idx_entities_epistemic ON kg_entities(agent_id, epistemic_class);
```

**`kg_relationships`** (full definition):

```sql
CREATE TABLE kg_relationships (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    source_entity_id TEXT NOT NULL,
    target_entity_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL,
    properties TEXT,                    -- JSON
    epistemic_class TEXT NOT NULL DEFAULT 'current',
    confidence REAL NOT NULL DEFAULT 0.8,
    mention_count INTEGER NOT NULL DEFAULT 1,
    access_count INTEGER NOT NULL DEFAULT 0,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    last_accessed_at TEXT,
    valid_at TEXT,                      -- bitemporal
    invalidated_at TEXT,
    t_invalidated_by TEXT,
    source_episode_ids TEXT,
    UNIQUE(source_entity_id, target_entity_id, relationship_type),
    FOREIGN KEY (source_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (target_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
);
CREATE INDEX idx_rels_source ON kg_relationships(source_entity_id);
CREATE INDEX idx_rels_target ON kg_relationships(target_entity_id);
CREATE INDEX idx_rels_agent ON kg_relationships(agent_id);
CREATE INDEX idx_rels_valid ON kg_relationships(valid_at);
```

**`kg_episodes`** (full definition, with queue lifecycle):

```sql
CREATE TABLE kg_episodes (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,          -- session|tool_result|ward_file|document|user_input
    source_ref TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    session_id TEXT,
    agent_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending|running|done|failed
    retry_count INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    UNIQUE(content_hash, source_type)
);
CREATE INDEX idx_episodes_status ON kg_episodes(status);
CREATE INDEX idx_episodes_source_ref ON kg_episodes(source_ref);
CREATE INDEX idx_episodes_session ON kg_episodes(session_id);
```

**`memory_facts`** (full definition, includes Phase 6 additions from day one):

```sql
CREATE TABLE memory_facts (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    agent_id TEXT NOT NULL,
    scope TEXT NOT NULL,                -- agent|shared|ward
    category TEXT NOT NULL,             -- correction|strategy|domain|instruction|pattern|user|skill|agent|ward
    key TEXT NOT NULL,
    content TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.8,
    mention_count INTEGER NOT NULL DEFAULT 1,
    source_summary TEXT,
    source_episode_id TEXT,
    source_ref TEXT,
    embedding BLOB,
    ward_id TEXT NOT NULL DEFAULT '__global__',
    epistemic_class TEXT NOT NULL DEFAULT 'current',
    contradicted_by TEXT,
    t_valid_from TEXT,
    t_valid_to TEXT,
    superseded_by TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT,
    UNIQUE(agent_id, scope, ward_id, key)
);
CREATE INDEX idx_facts_agent_scope ON memory_facts(agent_id, scope);
CREATE INDEX idx_facts_category ON memory_facts(agent_id, category);
CREATE INDEX idx_facts_ward ON memory_facts(ward_id);
CREATE INDEX idx_facts_epistemic ON memory_facts(epistemic_class);
CREATE VIRTUAL TABLE memory_facts_fts USING fts5(key, content, category, content=memory_facts);
```

All other tables (`sessions`, `agent_executions`, `messages`, `ward_wiki_articles`, `procedures`, `session_episodes`, `recall_log`, `distillation_runs`, `embedding_cache`, `artifacts`) carry forward unchanged from v21.

---

## Components

### Chunker (new: `gateway/gateway-execution/src/ingest/chunker.rs`)

Pure-function API:

```rust
pub fn chunk_text(text: &str, opts: ChunkOptions) -> Vec<Chunk>;

pub struct ChunkOptions {
    pub target_tokens: usize,    // default 1000
    pub overlap_tokens: usize,   // default 100
    pub prefer_paragraph: bool,  // default true
}

pub struct Chunk {
    pub index: usize,
    pub text: String,
    pub char_range: (usize, usize),   // for source-ref citations
}
```

Paragraph-aware splitting: prefer boundary at `\n\n`, fall back to sentence `. ?!`, fall back to character count. Token counting via `tiktoken-rs` (cl100k_base — same as gpt-4o family; adequate estimate for llama models).

### Episode Registry (extended)

`KgEpisodeRepository` gains three methods:

```rust
pub fn create_pending(source_type, source_ref, content_hash, session_id, agent_id) -> EpisodeId;
pub fn update_status(id, status, error?) -> Result<()>;
pub fn list_by_source(source_id) -> Vec<Episode>;   // for progress endpoint
```

All existing call sites default to `status='done'` (backwards compatible).

### Extraction Queue (new: `gateway/gateway-execution/src/ingest/queue.rs`)

```rust
pub struct IngestionQueue {
    tx: mpsc::Sender<IngestionJob>,
    // workers drain rx in tokio::spawn loops
}

pub struct IngestionJob {
    pub episode_id: EpisodeId,
    pub chunk_text: String,
    pub neighborhood_hints: Vec<String>,   // pre-computed nearest existing entities
    pub session_id: Option<String>,
    pub agent_id: String,
}
```

Workers: N (default 2) concurrent extractors. Each worker:

1. Claim episode (mark running)
2. Run pass 1 — entity extraction
3. Run pass 2 — relationship extraction
4. Call resolver+writer
5. Mark done OR retry (up to 3x) OR failed

Failures are loud: emit event on `gateway_bus`, log with tracing, update episode error.

### Structured Extractor (new: `gateway/gateway-execution/src/ingest/extractor.rs`)

Two-pass prompt, JSON-schema constrained outputs.

**Pass 1 — entities:**

```json
{
  "entities": [
    {
      "name": "string",
      "type": "person|organization|location|event|document|concept|tool|...",
      "aliases": ["string"],
      "description": "string (≤50 words)"
    }
  ]
}
```

Prompt conditioned on `neighborhood_hints` — "these entities already exist in the graph; prefer reusing their names if a new mention refers to the same thing."

**Pass 2 — relationships, conditioned on pass-1 entity list:**

```json
{
  "relationships": [
    {
      "source": "entity_name_from_pass_1",
      "target": "entity_name_from_pass_1",
      "type": "held_at|member_of|authored|...",
      "confidence": 0.0-1.0
    }
  ]
}
```

Both passes use the provider's structured output mode when available (OpenAI, Gemini, Anthropic); fall back to instructor-style retry on local models without constrained decoding.

Validation: pass-2 relationships referencing entities not in pass-1 output are dropped. Retry on drop rate >20%.

### Resolver v2 (rewrite: `services/knowledge-graph/src/resolver.rs`)

Three stages, each `O(log N)` or better:

```rust
pub async fn resolve(candidate: &EntityCandidate) -> ResolveOutcome {
    // Stage 1: alias table + normalized hash → O(1) index lookup
    if let Some(existing) = lookup_by_normalized_hash(candidate) {
        return ResolveOutcome::Merge(existing.id, MatchReason::ExactNormalized);
    }
    if let Some(existing) = lookup_alias(candidate.normalized) {
        return ResolveOutcome::Merge(existing.id, MatchReason::AliasMatch);
    }

    // Stage 2: ANN on name embeddings → O(log N) via sqlite-vec
    let candidates = ann_search(candidate.name_embedding, top_k=5)?;
    let filtered = candidates.into_iter()
        .filter(|c| c.entity_type == candidate.entity_type)
        .filter(|c| c.cosine >= 0.90)
        .collect();

    if filtered.is_empty() {
        return ResolveOutcome::Create(new_entity_with_self_alias(candidate));
    }

    // Stage 3: LLM pairwise verify on top-k → bounded cost
    let verified = llm_pairwise_verify(candidate, &filtered).await?;
    match verified {
        Some(id) => {
            append_alias(id, candidate.surface_form);
            ResolveOutcome::Merge(id, MatchReason::LlmVerified)
        }
        None => ResolveOutcome::Create(new_entity_with_self_alias(candidate)),
    }
}
```

All merges append to `kg_aliases`. Create paths also seed one self-alias row. After first use, repeat mentions of any surface form short-circuit at stage 1.

### Sleep-Time Worker (new: `gateway/gateway-execution/src/ingest/sleep_worker.rs`)

Tokio task started at daemon boot, runs every 60 min OR on explicit trigger (`POST /api/memory/consolidate`). Four ops:

**Compaction:** pairs of entities with cosine ≥ 0.92 AND same type AND neither is `archival` → LLM pairwise verify → merge loser into winner. Record in `kg_compactions`.

**Decay scoring:** for every non-archival entity, compute `decay_score = last_accessed_freshness × access_count × mention_boost`. Write to a view, not a column.

**Pruning candidates:** entities where `decay_score < threshold AND epistemic_class != 'archival' AND no_edges AND age > 30d` → move to archive table. Never hard-delete.

**Cross-session synthesis:** find strongly-connected subgraphs (≥3 edges among same top-K entities over ≥2 distinct sessions) → LLM writes a `memory_fact` with category=`strategy`, source=`graph_synthesis`. Feeds the cheaper fact-recall lane.

All operations non-blocking; if a pass takes >5 min, emit progress events.

### Unified Recall (rewrite: `gateway/gateway-execution/src/recall.rs`)

Replaces the current "facts first, graph as tail" pipeline. Single scored pool:

```rust
pub struct ScoredItem {
    pub kind: ItemKind,   // Fact | Wiki | Procedure | GraphNode | Goal
    pub id: String,
    pub content: String,
    pub score: f64,
    pub provenance: Provenance,
}

pub async fn recall(ctx: &RecallContext) -> Vec<ScoredItem> {
    let fact_items = score_facts(ctx).await?;
    let wiki_items = score_wiki(ctx).await?;
    let proc_items = score_procedures(ctx).await?;
    let graph_items = score_graph_neighborhoods(ctx).await?;
    let goal_items = score_active_goals(ctx).await?;

    let merged = rrf_merge(&[fact_items, wiki_items, proc_items, graph_items, goal_items], k=60);
    let intent_filtered = apply_intent_boost(merged, &ctx.active_goals);

    intent_filtered.truncate(ctx.budget.top_k);
    intent_filtered
}
```

Every item shares the same multipliers: category/kind weight × epistemic penalty × access decay × ward affinity × mention boost. Graph neighborhoods convert to items via: "entity name + 1-hop edge summary as content, cosine+centrality as score."

### Goal-Oriented Memory (new: `gateway/gateway-execution/src/goals/`)

Agent-facing tool:

```
goal(action="create", title, description?, parent?, slots=[{name, type, required}])
goal(action="update", id, filled_slots={...}, state?)
goal(action="complete", id)
goal(action="list", state?="active")
```

Every session start pulls active goals for the agent. MemGuide-style retrieval boost: when an active goal has unfilled slot `X`, every recall item that could plausibly fill `X` gets a 1.3× score multiplier.

### Observability (new: `gateway/src/http/memory_stats.rs`)

```
GET /api/memory/stats
  → { entities, relationships, facts, episodes, procedures, wiki_articles,
      goals_active, db_size_mb, orphan_ratio, recent_growth_per_day }

GET /api/memory/health
  → { ingestion_queue_depth, pending_episodes, failed_episodes_24h,
      avg_extraction_latency_ms, last_compaction_run, next_compaction_at }

GET /api/graph/ingest/:source_id/progress
  → { source_id, total_episodes, status_counts, throughput_per_min }
```

UI dashboard ships with the Observatory page gaining a Memory Health tab.

### Agent Tool Visibility

Shard `gateway/templates/shards/memory_learning.md` gains a "When to use which memory tool" section covering:

- `memory(recall)` — targeted fact lookup
- `graph_query(search|neighbors|context)` — entity/relationship exploration
- `goal(create|update|complete|list)` — intent lifecycle
- `ingest(path|url|text)` — add new source to graph

Explicit triggers and "don't call" guidance for each.

---

## Ingestion Pipeline (Walkthrough)

Example: user asks agent to index `Rise of Modern Indian Nationalism.pdf`.

1. Agent calls `ingest(path="/tmp/Rise.pdf", source_id="book-rise-2024")`.
2. `ingest` tool extracts text via `pdftotext` skill (existing), splits via `Chunker::chunk_text()` with paragraph mode → 247 chunks.
3. For each chunk, create `kg_episode` with `status='pending'`, `source_type='document'`, `source_ref='book-rise-2024#chunk-{N}'`, content_hash. Returns immediately with `source_id` and 247-chunk estimate.
4. Enqueue each episode into `IngestionQueue`. HTTP returns `202 Accepted` immediately.
5. Workers drain the queue (2 concurrent). Per job:
   - Mark episode `running`
   - Compute neighborhood hints: ANN search on chunk's first-100-word embedding, return top-5 existing entity names
   - Pass 1 — entities + aliases, conditioned on hints
   - Pass 2 — relationships
   - For each entity: resolver v2 → alias table or create
   - For each relationship: upsert with UNIQUE(source,target,type), bump mention_count
   - Mark episode `done`
6. User polls `GET /api/graph/ingest/book-rise-2024/progress` for live status. Each chunk's progress is observable.
7. Throughout, the agent continues interactive work. The graph grows in the background.
8. On sleep-time tick (hourly), compactor finds duplicate entities (e.g., "Savarkar" vs "V.D. Savarkar" that somehow escaped stage 2), merges them with alias preservation.

Typical throughput target: **3–5 chunks/second** with gpt-4o-mini, single API key. A 247-chunk book finishes in ~1 min. With local llama-3.1-8B on Ollama: ~1 chunk/second (~4 min for the book).

---

## Retrieval Pipeline (Walkthrough)

Example: user asks "what connects Savarkar to Hindu Mahasabha?"

1. `recall()` fires at session start:
   - Score facts (existing pipeline, unchanged primitives)
   - Score wiki articles (existing)
   - Score procedures (existing)
   - Score graph neighborhoods: ANN on query embedding → top-10 entity candidates, 2-hop expansion, subgraph serialization with edge labels
   - Score active goals (MemGuide filter)
2. RRF merges all five lists with k=60.
3. Intent boost: no active goal with matching slots → no boost.
4. Top-20 items returned, formatted into system message with provenance:
   - `[graph] V.D. Savarkar --president_of--> Hindu Mahasabha (src: book-rise-2024#chunk-42, episode ep-abc)`
5. Agent sees explicit graph edges as first-class context, uses them to answer.

---

## Clean-Slate Precondition

Before implementation begins:

1. Stop daemon.
2. `rm ~/Documents/zbot/data/conversations.db* ~/Documents/zbot/data/knowledge_graph.db*`
3. All of the above. No partial deletes, no backfills.
4. Wards on disk (`~/Documents/zbot/wards/`) stay — they're the source of truth for archival knowledge and will be re-ingested as needed.

Pack A's relationship extraction rules (`gateway/gateway-execution/src/indexer/relationship_rules.rs`) carry forward unchanged — they're part of the v22 solution, not a legacy.

## Roll-Out Phases

Each phase is one PR. Acceptance criteria are binary — merged only when all pass. No phase depends on historical data; each is validated on a fresh DB.

### Phase 1 — Schema v22 + Resolver v2 + sqlite-vec

**Deliverables:**
- `schema.rs` rewritten: schema v22 as the initial schema. Daemon boot creates it if DB missing.
- `sqlite-vec` wired into `gateway-database` via extension load at connection open.
- `EntityResolver` v2 (alias-first, ANN second, LLM-verify third) replacing current `resolver.rs`.
- Every entity create seeds one self-alias row; embedding computed inline (async, non-blocking).
- Integration test: ingest a synthetic 1000-entity ward, assert resolver p95 < 20 ms.

**Acceptance:** fresh DB comes up clean, 1000-entity synthetic ward indexes with <2% duplicate entities and resolver p95 < 20 ms.

### Phase 2 — Streaming Ingestion

**Deliverables:**
- `Chunker` module with unit tests
- `IngestionQueue` + 2 workers
- Two-pass `Extractor` with structured outputs
- `POST /api/graph/ingest` endpoint
- `GET /api/graph/ingest/:source_id/progress` endpoint
- `ingest` agent tool, registered in tool registry
- Shard edits teaching the tool
- End-to-end test: a 10-chunk synthetic document indexes to ≥50 entities and ≥80 relationships with no duplicates

**Acceptance:** index `Rise of Modern Indian Nationalism.pdf` in <2 min with gpt-4o-mini; resulting graph has ≥500 entities, ≥800 relationships, orphan ratio <15%.

### Phase 3 — Unified Recall + Goal-Oriented Retrieval

**Deliverables:**
- `recall.rs` rewrite with `ScoredItem` pool and RRF merge
- `goal` agent tool + `kg_goals` CRUD
- Intent-boost in recall when active goal slots match candidate items
- Shard edits teaching the goal tool
- Remove the legacy free-text graph section from system prompts (superseded by scored graph items)

**Acceptance:** A/B test on a fixed set of 10 research prompts — new recall produces ≥30% higher average mention of graph-derived facts in agent responses vs old recall.

### Phase 4 — Sleep-Time Worker + Compaction + Observability

**Deliverables:**
- Sleep-time worker (hourly) with the four ops (compaction, decay scoring, pruning candidates, cross-session synthesis)
- `POST /api/memory/consolidate` on-demand trigger
- `GET /api/memory/stats`, `GET /api/memory/health`
- Observatory UI tab for memory health
- Cross-session synthesis writes strategy facts with `source=graph_synthesis`

**Acceptance:** after 2 weeks of real use, orphan ratio stays ≤20%, db_size growth is sub-linear, no duplicate entities with cosine ≥0.95 survive past one compaction cycle.

### Phase 5 — Hardening + Docs

**Deliverables:**
- Failure-mode tests (worker crashes, LLM timeouts, malformed responses, SIGKILL mid-extraction)
- `memory-bank/components/memory-layer/` docs fully refreshed reflecting v22
- Architecture diagram (SVG) added to Observatory
- Performance baseline published: cold-boot time, resolver p50/p95/p99, ingestion throughput per provider

**Acceptance:** cold-boot to ready in <10 s with 10k entities; single worker sustains ingestion for 24h without crash; docs reviewed.

---

## Testing Strategy

| Layer | Tests |
|---|---|
| Unit | Chunker, resolver stages, scorer, goal slot matching |
| Integration | End-to-end ingest of synthetic ward, end-to-end recall with goal boost |
| Property | Resolver idempotency: running N times produces the same graph |
| Regression | Pack A tests still green; no existing session loses data |
| Performance | Resolver p95 < 20 ms at 10k entities; ingestion ≥ 3 chunks/sec on gpt-4o-mini |
| Chaos | Kill worker mid-extraction; assert retry path; assert no partial writes |

---

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| sqlite-vec unavailable on user's platform | Bundle the compiled extension in the daemon release; platform matrix verified in CI (Linux/macOS/Windows x86_64 + arm64) |
| Two-pass extraction doubles LLM cost | Worker count tunable; local-model path documented; cost surfaced in Observatory |
| Structured outputs not supported by all providers | Provider capability matrix at boot; fallback to instructor retry pattern (max 3x); skip extraction with visible warning on unsupported providers |
| Sleep-time compactor merges entities incorrectly | All merges write to `kg_compactions` with reason; admin UI shows recent merges with undo; conservative 0.92 cosine threshold |
| Goal schema bloats | Goals auto-archive after 30 days in terminal state; archival preserves history, excludes from live retrieval |
| Resolver latency spikes under load | Metric exported; on p95 > 50 ms, ingestion queue backpressures (producer waits); no silent degradation |
| Book ingestion overwhelms queue | Per-source rate limit: max 500 pending episodes per source; UI shows backpressure status |

---

## Open Questions

These need a call before Phase 1 lands. Each has a recommended default.

1. **Which embedding model for name embeddings?** Recommendation: `bge-small-en-v1.5` (384d, fast, CPU-runnable). Alternative: provider-native embeddings (costlier, better quality).
2. **Worker count default?** Recommendation: 2. User-tunable via `config.ingestion.workers`.
3. **Structured-outputs required or degraded-mode OK?** Recommendation: required for cloud providers (OpenAI, Gemini, Anthropic all support it); local models use instructor retry with a 3× max.
4. **Goal schema: fixed slot types or freeform?** Recommendation: start freeform strings; add typed slots only if retrieval quality demands it.
5. **Backfill strategy for existing entities missing embeddings?** Recommendation: Phase 1 backfill script runs in batches of 100, progress-reportable; takes ~5 min per 10k entities on a single CPU.

---

## Out of Scope (Explicitly)

To keep the umbrella focused:

- Multi-agent shared memory (cross-user). Each agent's memory is isolated.
- Encrypted-at-rest storage beyond SQLite's default.
- Federated graph queries across wards (queries are ward-scoped or global, not ward-to-ward-joined).
- Real-time embedding pipelines beyond name embeddings (we're not re-embedding every chunk for retrieval on the fly).
- UI for direct graph editing (admin endpoints only; UI is read + trigger-actions).

---

## Appendix A — Research Citations

- Rasmussen et al., *Zep: A Temporal Knowledge Graph Architecture for Agent Memory*, arXiv:2501.13956, Jan 2025.
- Xu et al., *A-Mem: Agentic Memory for LLM Agents*, NeurIPS 2025, arXiv:2502.12110.
- Letta v1 Agent Blog, 2025 — https://www.letta.com/blog/letta-v1-agent
- Memory Blocks, Letta, 2025 — https://www.letta.com/blog/memory-blocks
- *iText2KG: Incremental Knowledge Graphs Construction Using Large Language Models*, arXiv:2409.03284, Neo4j Nodes 2025.
- Cognee AI Memory Eval, Aug 2025 — https://www.cognee.ai/blog/deep-dives/ai-memory-evals-0825
- *MemGuide: Intent-Aware Retrieval for Goal-Oriented LLM Agents*, arXiv:2505.20231.
- MemOS: An Operating System for Memory-Augmented LLMs, Jul 2025 — https://statics.memtensor.com.cn/files/MemOS_0707.pdf
- OpenAI Structured Outputs — https://developers.openai.com/api/docs/guides/structured-outputs
- asg017/sqlite-vec — https://github.com/asg017/sqlite-vec
- *Memp: Exploring Agent Procedural Memory*, 2025.
- *LEGOMem: Modular Procedural Memory for Agents*, 2025.
- *Rethinking Memory in AI*, arXiv:2505.00675.
- *RAG vs. GraphRAG: A Systematic Evaluation*, arXiv:2502.11371.
- *When to use Graphs in RAG*, arXiv:2506.05690.

---

## Appendix B — Current-State Audit Summary

(Full audit in conversation transcript, 2026-04-12.)

- 4.8 MB `conversations.db` with memory_facts, episodes, procedures, wiki, distillation_runs, etc.
- 10.8 MB `knowledge_graph.db` with 561 entities / 28 relationships (544 orphans — fixed by Pack A).
- Ingestion: distillation (session-end), ward artifact indexer (post-distillation), tool result extractor (opportunistic). **No streaming prose path.**
- Resolver: 3 stages, all O(N) at scale. Stage 1 full scan same-type; stage 2 Levenshtein top-100; stage 3 cosine scan top-50.
- Recall: hybrid FTS+vector on facts; graph context appended as 2000-char free text, not scored.
- Pruning: function exists in `pruning.rs`, **never called**.
- Observability: none exposed; stats buried in repositories.

---

## Acceptance of This Spec

Before Phase 1 begins, user confirms:

1. This spec accurately describes the target system.
2. Phased roll-out order is acceptable.
3. Open questions above have accepted defaults OR specific instructions to change them.

Once accepted, implementation plans ship per-phase via the `writing-plans` skill.
