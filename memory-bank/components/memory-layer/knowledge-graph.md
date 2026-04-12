# Knowledge Graph — Architecture After Phase 6

Transforms z-Bot's graph from an activity log (orchestration metadata only) into a true knowledge base (domain entities with provenance, temporal awareness, and resolution).

Grounded in: Graphiti/Zep (2025), MAGMA (2026), A-MEM (NeurIPS 2025), CIDOC CRM.

---

## What Changed in Phase 6

**Before**: Graph captured ~9 entities per rich research session — tickers, agents, wards, and files. No provenance. No entity resolution. No temporal awareness. Domain content (people, places, events) from structured ward files never reached the graph.

**After**: 100+ entities per rich research session. Every entity and relationship traces to a source episode. Name variants merge automatically. Facts carry epistemic classification that governs lifecycle. Real-time updates during execution. Four query modes.

---

## Architecture

```
┌────────────────────── INGESTION ──────────────────────────┐
│                                                            │
│  1. Distillation LLM         (session-end)                 │
│  2. Ward Artifact Indexer    (post-distillation, JSON→KG)  │
│  3. Tool Result Extractor    (real-time, per tool result)  │
│  4. Agent graph_query tool   (on-demand, agent-initiated)  │
│                                                            │
│      All extractions produce a kg_episodes record          │
│      with content_hash dedup.                              │
│                                                            │
└──────────────────────┬─────────────────────────────────────┘
                       │
          ┌────────────▼────────────┐
          │   EntityResolver        │
          │ 1. exact normalized     │
          │ 2. fuzzy (Levenshtein)  │
          │ 3. embedding cosine     │
          │ → Merge or Create       │
          └────────────┬────────────┘
                       │
     ┌─────────────────┼─────────────────┐
     │                 │                 │
┌────▼──────┐  ┌───────▼──────┐  ┌──────▼─────────┐
│ kg_       │  │ kg_          │  │ kg_causal_     │
│ entities  │  │ relationships│  │ edges          │
│ + aliases │  │ + valid_at   │  │ + confidence   │
│ + class   │  │ + invalid_at │  │ + relationship │
│ + episodes│  │ + class      │  │   (causes/     │
│ + confid. │  │ + episodes   │  │    prevents/   │
│           │  │              │  │    requires/   │
│           │  │              │  │    enables)    │
└────┬──────┘  └───────┬──────┘  └──────┬─────────┘
     │                 │                 │
     └─────────┬───────┴─────────────────┘
               │
     ┌─────────▼──────────────┐
     │  MAGMA QUERY VIEWS      │
     │  - semantic             │
     │  - temporal             │
     │  - entity (connections) │
     │  - hybrid (reranked)    │
     └────────────────────────┘
```

---

## Data Model

### `kg_entities`

```sql
CREATE TABLE kg_entities (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,      -- see expanded enum below
    name TEXT NOT NULL,
    properties TEXT,                -- JSON with type-specific schema
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    mention_count INTEGER DEFAULT 1,

    -- Phase 6 additions
    aliases TEXT,                   -- JSON array of name variants
    epistemic_class TEXT DEFAULT 'current',  -- archival | current | convention | procedural
    source_episode_ids TEXT,        -- JSON array of kg_episodes.id
    valid_from TEXT,                -- when entity came into existence
    valid_until TEXT,               -- when it ceased
    confidence REAL DEFAULT 0.8
);
CREATE INDEX idx_entities_class ON kg_entities(agent_id, epistemic_class);
```

### `kg_relationships`

```sql
CREATE TABLE kg_relationships (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    source_entity_id TEXT NOT NULL,
    target_entity_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL,  -- see expanded vocab below
    properties TEXT,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    mention_count INTEGER DEFAULT 1,

    -- Phase 6 additions
    valid_at TEXT,                    -- when relationship became true
    invalidated_at TEXT,              -- when it ceased
    epistemic_class TEXT DEFAULT 'current',
    source_episode_ids TEXT,
    confidence REAL DEFAULT 0.8,

    FOREIGN KEY (source_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
);
CREATE INDEX idx_rels_valid_at ON kg_relationships(valid_at);
```

### `kg_causal_edges` (Phase 1)

```sql
CREATE TABLE kg_causal_edges (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    cause_entity_id TEXT NOT NULL,
    effect_entity_id TEXT NOT NULL,
    relationship TEXT NOT NULL,       -- causes | prevents | requires | enables
    confidence REAL DEFAULT 0.7,
    session_id TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (cause_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (effect_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
);
```

### `kg_episodes` (Phase 6a — in conversations.db)

```sql
CREATE TABLE kg_episodes (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,         -- tool_result | ward_file | session | distillation | user_input
    source_ref TEXT NOT NULL,          -- tool_call_id | file_path | session_id | message_id
    content_hash TEXT NOT NULL,        -- SHA-256, prevents re-extracting unchanged content
    session_id TEXT,
    agent_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(content_hash, source_type)
);
```

---

## Entity Types (13)

| Type | Properties Schema | Example |
|------|-------------------|---------|
| `person` | birth_date, death_date, nationality, occupation | "V.D. Savarkar" |
| `organization` | founding_date, dissolution_date, type, location | "Hindu Mahasabha" |
| `location` | country, region, coordinates, type | "Ahmedabad" |
| `event` | start_date, end_date, location, participants | "Ahmedabad Session 1937" |
| `time_period` | start, end, era | "Pre-Independence Era" |
| `document` | author, publisher, publication_date, source_url | A book / PDF / URL |
| `role` | organization, start_date, end_date, held_by | "President of X, 1937–1938" |
| `artifact` | format, generator | Generated reports/outputs |
| `concept` | domain | Abstract ideas |
| `tool` | version, language | yfinance, pandas |
| `project` | language, framework | portfolio-analysis |
| `file` | path, exports, purpose | Ward files |
| `ward` | purpose | Workspace container |

## Relationship Vocabulary (27)

Grouped with directional examples:

**Temporal**: `before`, `after`, `during`, `concurrent_with`, `succeeded_by`, `preceded_by`

**Role-based**: `president_of`, `founder_of`, `member_of`, `author_of`, `held_role`, `employed_by`

**Spatial**: `located_in`, `held_at`, `born_in`, `died_in`

**Causal**: `caused`, `enabled`, `prevented`, `triggered_by`

**Hierarchical**: `part_of`, `contains`, `instance_of`, `subtype_of`

**Generic (fallback)**: `uses`, `created`, `related_to`, `exports`, `has_module`, `analyzed_by`, `prefers`, `mentions`

All relationships are **directional** — `source --type--> target`. Distillation prompt includes explicit direction examples per type.

---

## Ingestion Pipelines

### 1. Distillation (session-end, LLM)

Post-session, the distillation LLM extracts entities + relationships from the root transcript. Each entity/relationship is tagged with the session's `epistemic_class` guidance (default: `current`, or `archival` if the LLM recognizes a historical/document source).

### 2. Ward Artifact Indexer (Phase 6a)

Post-distillation, scans the ward directory for structured files (`.json`, `.csv`, `.yaml`):

- Array-of-objects with `name` field → `NamedObjectArray` → Person/Location entities
- Array-of-objects with `date`/`year` field → `DatedObjectArray` → Event entities
- `{key: {...}, key: {...}}` map → `NamedObjectMap` → typed by filename heuristic

Content hash dedup via `kg_episodes` — unchanged files are skipped on re-runs.

All artifacts → `epistemic_class = archival`, `source_ref = file_path`.

#### Relationship Extraction (Pack A, 2026-04-12)

In addition to entities, the indexer emits relationships from well-known field names. Rules live in `gateway/gateway-execution/src/indexer/relationship_rules.rs`.

| JSON field | Direction | Relationship | Target type |
|---|---|---|---|
| `location` (on Event) | forward | `held_at` | Location |
| `location` (on other) | forward | `located_in` | Location |
| `organization` | forward | `member_of` | Organization |
| `role` | forward | `held_role` | Role |
| `founder` | **reversed** (person → org) | `founder_of` | (source org) |
| `founded_in` | forward | `located_in` | Location |
| `participants[]` | **reversed** (person → event) | `participant` (Custom) | (source event) |
| `date` or `year` | forward | `during` | TimePeriod |
| `author` | **reversed** (person → doc) | `author_of` | (source doc) |
| `born_in` | forward | `born_in` | Location |
| `died_in` | forward | `died_in` | Location |

Target entities are synthesized and run through the same `EntityResolver` cascade as primary entities, so name variants collapse. Relationship writes are idempotent via `UNIQUE(source_entity_id, target_entity_id, relationship_type)` — re-runs bump `mention_count` instead of duplicating rows.

#### Force Re-index (Pack A)

`POST /api/graph/reindex` force-re-indexes every ward, bypassing the `kg_episodes` content-hash dedup. Safe to re-run; entity and relationship writes are idempotent. Returns `{wards_processed, entities_created}`.

### 3. Tool Result Extractor (Phase 6d)

Real-time, fires after every tool result in a non-blocking `tokio::spawn`:

- `web_fetch` result → `Document` entity with url, title, publish_date, description
- `shell` result (success only) → `File` entities for each file path in stdout (capped at 10)
- `multimodal_analyze` → pass-through of `entities` array in result

Zero LLM cost. All produce `epistemic_class = archival` with `source_ref = tool_call:{id}`.

### 4. Agent graph_query Tool

Agents can explore the graph on demand via 3 actions:

- `search(query, entity_type?, limit?)` — LIKE-based entity name search
- `neighbors(entity_name, direction?, depth?)` — 1–2 hop traversal
- `context(topic)` — semantic search + subgraph extraction

All search actions accept a `view` parameter (Phase 6d): `semantic` | `temporal` | `entity` | `hybrid`.

---

## Entity Resolution Cascade (Phase 6b)

On every entity write, `EntityResolver::resolve()` runs a 3-stage cascade. First match wins.

### Stage 1: Exact Normalized

```rust
fn normalize_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let stripped = strip_honorifics(&lower);  // "Dr.", "Mrs.", "Shri", etc.
    stripped.replace(['.', ','], "")
}
```

`V.D. Savarkar` and `vd savarkar` both normalize to `vd savarkar`. If an existing entity (same agent + type) matches the normalized name OR has the candidate name in its aliases, merge.

### Stage 2: Fuzzy Name (Levenshtein)

Only applied to names ≥6 chars (prevents false matches on short strings). Compares candidate against the top-100 most-mentioned existing entities of the same type. Merge if Levenshtein distance ≤3.

Catches: `Savarkar` ↔ `Savarker`, `Mahasabha` ↔ `Mahashabha` (common transcription variants).

### Stage 3: Embedding Similarity (optional)

If candidate embedding is provided, computes cosine similarity against existing entities' `_name_embedding` property. Merges if similarity ≥0.87 within same type.

Catches semantic variants that aren't string-close: `The First World War` ↔ `WWI`.

### Merge Behavior

On merge:
- `aliases` accumulated: `["V.D. Savarkar", "Vinayak Damodar Savarkar"]`
- `mention_count` incremented
- `last_seen_at` updated
- Any new properties from the candidate are NOT merged (existing entity wins — conservative)

### Match Provenance

The `ResolveOutcome::Merge` result includes a `MatchReason` (ExactNormalized / FuzzyName / EmbeddingSimilarity) — useful for observability and testing.

---

## Epistemic Classes

Every fact, entity, and relationship carries `epistemic_class` that governs its lifecycle:

| Class | Lifecycle | Recall Penalty | Examples |
|-------|-----------|----------------|----------|
| `archival` | Never decays. Corrected → mild penalty but stays retrievable. | 0.3x if `invalidated_at` set, else none | Birth dates, historical events, PDF contents |
| `current` | Decays sharply when superseded by newer observation. | 0.1x if `invalidated_at` set, else none | Stock prices, rates, current state |
| `convention` | Stable. Replaced only on explicit policy change. | None | User preferences, coding standards |
| `procedural` | Evolves via success/failure counts, not time. | None | Learned action sequences |

**Key insight**: Archival facts preserve primary-source records. A fact extracted from a 1937 historical document has the same recall weight in 2026 as in 2030 — it's a record of what the document says, not a statement about current reality.

### Correction vs Supersession

- **Archival correction**: "We had 1936 but it was actually 1937." Old archival fact keeps `epistemic_class = archival` but gets `invalidated_at` set. Both versions remain retrievable; new one ranks higher.
- **Current supersession**: "Price is now $524." Old current fact gets `invalidated_at`, gets heavy 0.1x penalty, effectively invisible in normal recall.
- **Convention change**: Explicit user/agent action required. Not auto-superseded.
- **Procedural evolution**: Success/failure counters update; no temporal mechanism.

---

## MAGMA Multi-View Queries (Phase 6d)

Same graph, four different lenses. The `view` parameter on `graph_query` routes to the right query:

### Semantic View (default)
Order by `mention_count` DESC — most-discussed entities first.

**Use when**: "What do I know about X?" (breadth)

### Temporal View
Order by `last_seen_at` DESC — most-recently-active entities first.

**Use when**: "What's been happening recently around X?"

### Entity View
Order by relationship count — most-connected entities first.

**Use when**: "Who/what is central to this domain?"

### Hybrid View
Reciprocal rank fusion across semantic + temporal + entity (k=60 RRF constant).

**Use when**: "Give me the best answer, don't make me pick a lens."

### Hybrid Query Implementation

```rust
async fn search_entities_hybrid(...) -> Vec<Entity> {
    let wide = limit * 2;
    let semantic = search_entities(wide).await?;
    let temporal = search_entities_temporal(wide).await?;
    let by_conn = search_entities_by_connections(wide).await?;

    merge_by_reciprocal_rank(&[semantic, temporal, by_conn])
        .into_iter().take(limit).collect()
}

fn merge_by_reciprocal_rank(lists: &[Vec<Entity>]) -> Vec<Entity> {
    let k = 60.0;
    let mut scores: HashMap<Id, (f64, Entity)> = HashMap::new();
    for list in lists {
        for (rank, entity) in list.iter().enumerate() {
            let score = 1.0 / (k + rank as f64 + 1.0);
            scores.entry(entity.id.clone())
                .and_modify(|(s, _)| *s += score)
                .or_insert_with(|| (score, entity.clone()));
        }
    }
    // sort by score desc, return entities
}
```

---

## Provenance: From Graph to Evidence

Every entity and relationship in the graph has `source_episode_ids` pointing to records in `kg_episodes`. Each episode has:

- `source_type`: `tool_result` | `ward_file` | `session` | `distillation` | `user_input`
- `source_ref`: the exact identifier (tool call ID, file path, session ID, message ID)
- `content_hash`: SHA-256 of the source content
- `session_id`: originating session

This means any claim in the graph can be traced:

```
Entity "V.D. Savarkar" → source_episode_ids: ["ep-42"]
  ep-42.source_type = "ward_file"
  ep-42.source_ref = "/wards/political-research/india-history/data/people.json"
  ep-42.session_id = "sess-abc"
  ep-42.content_hash = "a3f5..."
```

The agent can drill from "what do you know about Savarkar?" to the exact PDF page or research tool result that produced that knowledge.

---

## Query Scoring Formula (Current State)

```
For each candidate entity/relationship:
  score = base_similarity
        × mention_boost       (1.0 + log2(mention_count))
        × class_penalty       (from epistemic_class rules)
        × confidence          (stored on entity/relationship)

For graph queries with view:
  final_order = view.ordering(scored_results)
  where view ∈ {semantic(by score), temporal(by last_seen), entity(by connection_count), hybrid(RRF)}
```

---

## Key Files

| File | Purpose |
|------|---------|
| `services/knowledge-graph/src/types.rs` | EntityType (13 variants) + RelationshipType (27 variants) |
| `services/knowledge-graph/src/storage.rs` | CRUD + resolver-integrated writes + view-ordered searches |
| `services/knowledge-graph/src/service.rs` | `GraphView` enum + `search_entities_view` dispatcher + `merge_by_reciprocal_rank` |
| `services/knowledge-graph/src/resolver.rs` | Entity resolution cascade |
| `services/knowledge-graph/src/causal.rs` | Causal edge CRUD |
| `services/knowledge-graph/src/traversal.rs` | Multi-hop BFS (from Phase 1) |
| `gateway/gateway-database/src/kg_episode_repository.rs` | Episodes table CRUD |
| `gateway/gateway-execution/src/ward_artifact_indexer.rs` | JSON collection parser |
| `gateway/gateway-execution/src/tool_result_extractor.rs` | Real-time per-tool parsers |
| `gateway/gateway-execution/src/invoke/graph_adapter.rs` | Adapter: GraphStorageAccess trait ↔ GraphService |
| `runtime/agent-tools/src/tools/graph_query.rs` | Agent-facing tool with view parameter |
