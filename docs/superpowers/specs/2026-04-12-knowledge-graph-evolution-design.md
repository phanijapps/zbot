# Knowledge Graph Evolution — From Activity Log to Knowledge Base

**Date**: 2026-04-12
**Status**: Draft
**Branch**: `feature/sentient` (continues Phase 5)
**Phase**: 6 (builds on Phases 1–5 of cognitive-memory-system)

---

## 1. Problem Statement

AgentZero's knowledge graph currently captures **orchestration metadata**, not domain knowledge. Across all sessions to date:

- Entities are tickers, agent IDs, workspace names, and file names
- Relationships are `uses`, `analyzed_by`, `part_of` — process-level
- Properties are nearly empty — the `properties` JSON column typically `{}`
- Rich domain content (people, places, events, dates) produced by child agents and saved to ward files never reaches the graph

Concrete example: a session that extracted **195 historical events, 31 people, 23 organizations, and 41 places** from a Hindu Mahasabha PDF produced **9 total graph entities**, none of them historical figures, places, or events. All the richness lived in `timeline.json`, `people.json`, etc. — structured files the graph never saw.

The graph today answers "which agent did what in which session?" It cannot answer "what do I know about Savarkar?" or "who was president of Hindu Mahasabha in 1937?"

### Contributing Causes

1. **Distillation is the only writer**, and it only sees the root agent's transcript
2. **Entity type system is too narrow** — no Event, Place, TimePeriod, Document, Role types
3. **Properties schema is undefined** — no prescriptive structure per entity type
4. **Relationship vocabulary is generic** — missing temporal, spatial, causal, role-based types
5. **No entity resolution** — `Savarkar`, `V.D. Savarkar`, `Vinayak Damodar Savarkar` become three separate entities
6. **No provenance** — entities don't reference the facts/sources that support them
7. **Ward artifacts are invisible** — structured JSON files in the ward are never parsed
8. **Temporal flatness** — no bitemporal tracking; cannot answer "what was true at time T?"
9. **Epistemic flatness** — historical (archival) facts and volatile (current) state are treated identically

## 2. Research Foundation

### Graphiti / Zep (arXiv:2501.13956)

Zep's Graphiti engine is purpose-built for AI-agent knowledge graphs. Its core contributions directly address our pain points:

- **Episodes as atomic ingestion unit** — every fact traces back to a source episode (tool call, document, session)
- **Incremental bitemporal ingestion** — graph updates in real-time, not batched at session end
- **Entity resolution built-in** — embedding + fuzzy name matching merges variants on write
- **Bitemporal edges** — `valid_at` + `invalidated_at` per relationship enables time-aware queries
- **Hybrid search** — semantic + keyword + graph traversal combined

Benchmark: 94.8% on Deep Memory Retrieval vs MemGPT 93.4%, with 90% lower latency.

### MAGMA (arXiv:2601.03236)

Multi-graph architecture: same memory indexed across four orthogonal views (semantic, temporal, causal, entity). Queries activate the view(s) that match the question type. 18–45% improvement over monolithic graphs.

### A-MEM (NeurIPS 2025, arXiv:2502.12110)

Zettelkasten-style self-organization: each memory is a "note" with keywords, tags, context, embedding, and dynamic links to related memories. New notes auto-link via shared attributes.

### Karpathy's LLM Wiki (2026)

Compile-once, query-many pattern: raw data is source code, structured knowledge is the compiled artifact. Already applied to wards in Phase 3; extended here to the graph itself.

### CIDOC CRM / Wikidata Conceptual Model

For ontology and qualifiers: the distinction between **archival** (historical record) and **current** (volatile state) facts, and the practice of attaching temporal qualifiers to relationships.

## 3. Epistemic Classes — A First-Class Concept

Facts, entities, and relationships are classified into four epistemic classes that govern their lifecycle:

| Class | Meaning | Decay Behavior | Examples |
|-------|---------|----------------|----------|
| `archival` | Record of what happened or was stated in a primary source | **Never decays.** Can be corrected, never expires. | "Savarkar was Hindu Mahasabha president 1937–38"; Gandhi's birth date; content of a published document |
| `current` | Observed state at a point in time | Decays; superseded by newer observations | "AAPL = $523"; "INR/USD = 85.2"; "API rate limit = 1/sec" |
| `convention` | Rules, preferences, standing orders | Stable but replaceable on explicit policy change | "Use plotly not matplotlib"; "Always enter ward first" |
| `procedural` | Learned action sequences reinforced by outcomes | Evolves via success/failure counts | "stock_analysis procedure, 87% success" |

### Default Classification

When epistemic class is not explicit, the extractor follows these rules:

- Extracted from a PDF, document, book, or URL → `archival`
- Extracted from a live API, shell probe, or current observation → `current`
- Extracted from user corrections or preferences → `convention`
- Extracted from a completed multi-step session's action trace → `procedural`

### Impact on Recall Scoring

```
match fact.epistemic_class {
    Archival => {
        // No age penalty. A 1937 archival fact retrieved in 2030 is
        // just as relevant as in 2026. Only corrected facts get a
        // penalty, and they remain retrievable for provenance queries.
        if fact.invalidated_at.is_some() { score *= 0.3; }
    }
    Current => {
        // Strong decay for superseded current state.
        if fact.invalidated_at.is_some() { score *= 0.1; }
    }
    Convention | Procedural => {
        // Confidence-based; no temporal penalty.
    }
}
```

### Impact on Correction Semantics

- **Archival correction**: "We had 1936 but it was actually 1937." Old fact is marked `invalidated_at = NOW()` and `superseded_by` new fact. Both remain retrievable, with old scored lower.
- **Current state update**: Same mechanism as Phase 1 — old state superseded, new takes precedence in recall.
- **Convention change**: Explicit user/agent action required. Convention facts are not auto-superseded by new facts on the same topic.
- **Procedural evolution**: Uses the `success_count` / `failure_count` model from Phase 4.

## 4. Goals

1. Graph captures real domain knowledge (people, places, events, concepts), not just orchestration
2. Entities are resolved across variant names (no duplicate "Savarkar" problem)
3. Every entity, relationship, and fact traces to its source episode (provenance)
4. Historical/archival knowledge does not decay inappropriately
5. Bitemporal queries supported ("what was true at time T?")
6. Ward-written structured artifacts are indexed into the graph without LLM cost
7. Real-time extraction on tool results, not batched at session end
8. Backward compatible with existing schema — additive only

## 5. Architecture Overview

### Five Layers

```
Layer 1: Episodes + Provenance
  - Every extraction produces an Episode record
  - Facts/entities/relationships reference their source episode
  - Enables drill-down from graph to evidence

Layer 2: Ward Artifact Indexer
  - Post-session, scan ward for structured files (JSON, CSV, YAML)
  - Parse collection-of-objects schemas into entities + relationships
  - Tagged epistemic_class = archival with source_ref = file path
  - Zero LLM cost, high fidelity

Layer 3: Expanded Ontology + Entity Resolution
  - EntityType expanded: Person, Organization, Place, Event, TimePeriod,
    Document, Role, Concept, Artifact, Tool, Project, File
  - Properties schema per type (prescribed)
  - Entity resolution on write: normalize, fuzzy match, embedding match,
    type constraint
  - aliases JSON column on kg_entities

Layer 4: Bitemporal Edges + Epistemic Classes
  - valid_at, invalidated_at on relationships
  - epistemic_class column on facts, entities, relationships
  - Recall scoring class-aware (see Section 3)
  - Correction vs supersession paths

Layer 5: MAGMA-Style Views + Real-Time Tool Extraction
  - Query modes: semantic, temporal, causal, entity, hybrid
  - Tool result extractors: web_fetch, shell output, structured returns
  - Incremental graph updates during execution
```

Layers are independently deployable; each builds on previous but does not require them.

### Data Flow

```
┌─────────────── INGESTION ────────────────┐
│ ToolResult ─┐                            │
│ WardFile  ──┼─► EpisodeExtractor ──┐     │
│ Session   ──┤                      │     │
│ UserInput ──┘                      ▼     │
│              EntityResolver ─► Kg Write  │
│                                    │     │
└────────────────────────────────────┼─────┘
                                     │
                           ┌─────────▼─────────┐
                           │  memory_facts     │  primary
                           │  kg_entities      │  (with
                           │  kg_relationships │  episodes)
                           │  kg_episodes      │
                           └─────────┬─────────┘
                                     │
                           ┌─────────▼─────────┐
                           │  Query Views:     │
                           │  semantic |       │
                           │  temporal |       │
                           │  causal   |       │
                           │  entity   |       │
                           │  hybrid           │
                           └───────────────────┘
```

## 6. Data Model

### 6.1 New Table: `kg_episodes`

```sql
CREATE TABLE kg_episodes (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,      -- tool_result | ward_file | session | distillation | user_input
    source_ref TEXT NOT NULL,       -- tool_call_id | file_path | session_id | message_id
    content_hash TEXT NOT NULL,     -- SHA-256 of source content, for dedup
    session_id TEXT,                -- originating session (nullable)
    agent_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(content_hash, source_type)
);
CREATE INDEX idx_episodes_session ON kg_episodes(session_id);
CREATE INDEX idx_episodes_source ON kg_episodes(source_type, source_ref);
```

The `content_hash` UNIQUE prevents re-extracting the same content (e.g., re-scanning an unchanged `timeline.json`).

### 6.2 Modified `kg_entities`

```sql
ALTER TABLE kg_entities ADD COLUMN aliases TEXT;           -- JSON array
ALTER TABLE kg_entities ADD COLUMN epistemic_class TEXT DEFAULT 'current';
ALTER TABLE kg_entities ADD COLUMN source_episode_ids TEXT; -- JSON array
ALTER TABLE kg_entities ADD COLUMN valid_from TEXT;         -- when this entity came into existence
ALTER TABLE kg_entities ADD COLUMN valid_until TEXT;        -- when it ceased
ALTER TABLE kg_entities ADD COLUMN confidence REAL DEFAULT 0.8;
CREATE INDEX idx_entities_class ON kg_entities(agent_id, epistemic_class);
```

### 6.3 Modified `kg_relationships`

```sql
ALTER TABLE kg_relationships ADD COLUMN valid_at TEXT;         -- when relationship became true
ALTER TABLE kg_relationships ADD COLUMN invalidated_at TEXT;   -- when it stopped
ALTER TABLE kg_relationships ADD COLUMN epistemic_class TEXT DEFAULT 'current';
ALTER TABLE kg_relationships ADD COLUMN source_episode_ids TEXT; -- JSON array
ALTER TABLE kg_relationships ADD COLUMN confidence REAL DEFAULT 0.8;
CREATE INDEX idx_rels_valid_at ON kg_relationships(valid_at);
```

### 6.4 Modified `memory_facts`

```sql
ALTER TABLE memory_facts ADD COLUMN epistemic_class TEXT DEFAULT 'current';
ALTER TABLE memory_facts ADD COLUMN source_episode_id TEXT;
ALTER TABLE memory_facts ADD COLUMN source_ref TEXT;            -- e.g., "hindu_mahasabha.pdf:page_42"
CREATE INDEX idx_facts_class ON memory_facts(agent_id, epistemic_class);
```

### 6.5 Expanded EntityType Enum

```rust
enum EntityType {
    // Existing
    Person,
    Organization,
    Project,
    Tool,
    Concept,
    File,
    // New in Phase 6
    Place,         // countries, cities, regions, coordinates
    Event,         // historical events, meetings, sessions
    TimePeriod,    // years, eras, date ranges
    Document,      // books, articles, PDFs, URLs
    Role,          // "president", "CEO", role held by a person at a time
    Artifact,      // generated files, reports, data outputs
    Ward,          // workspace entity (made explicit)
}
```

### 6.6 Expanded Relationship Vocabulary

Grouped for clarity in the prompt:

**Temporal**: `before`, `after`, `during`, `concurrent_with`, `succeeded_by`, `preceded_by`

**Role-based**: `president_of`, `founder_of`, `member_of`, `author_of`, `held_role`, `employed_by`

**Spatial**: `located_in`, `held_at`, `born_in`, `died_in`

**Causal**: `caused`, `enabled`, `prevented`, `triggered_by`

**Hierarchical**: `part_of`, `contains`, `instance_of`, `subtype_of`

**Generic** (existing): `uses`, `created`, `related_to`, `exports`, `has_module`, `analyzed_by`, `prefers`

### 6.7 Properties Schema per Type

Prescribed in the extraction prompt so the LLM produces consistent structure:

```
Person:       { birth_date, death_date, nationality, role, occupation }
Organization: { founding_date, dissolution_date, type, location, founder }
Place:        { country, region, coordinates, type }
Event:        { start_date, end_date, location, participants, outcome }
TimePeriod:   { start, end, era, significance }
Document:     { author, publisher, publication_date, source_url, page_range }
Role:         { organization, start_date, end_date, held_by }
Artifact:     { file_path, format, size, generator }
```

## 7. Implementation Phases

Each sub-phase is independently deployable. Ordered by ROI.

### 7.1 Phase 6a: Episodes + Ward Artifact Indexer (2 weeks)

**Highest ROI. Start here.**

1. Migration v21: `kg_episodes` table + new columns on existing tables
2. `EpisodeRepository` CRUD
3. `WardArtifactIndexer`:
   - Post-session scan ward directory
   - For each structured file (`.json`, `.csv`, `.yaml`):
     - Detect schema (array-of-objects with `name`/`date`/`place` fields = entity collection)
     - Extract entities with `epistemic_class = archival`
     - Record `source_ref = file_path`
     - Create episode with `content_hash` for dedup
4. Wire into runner: call indexer after distillation spawn

Impact: the Hindu Mahasabha session would go from 9 graph entities to ~100+ overnight.

### 7.2 Phase 6b: Expanded Ontology + Entity Resolution (1.5 weeks)

1. Expand `EntityType` enum and `RelationshipType` vocabulary
2. Update distillation prompt with:
   - All entity types + example properties per type
   - All relationship types + directional examples per group
   - Epistemic class classification instructions
   - 2–3 full few-shot examples
3. `EntityResolver`:
   - Normalize name (lowercase, trim, strip honorifics like "Dr.", "V.D.")
   - Exact match within same agent + type
   - Fuzzy name match (Levenshtein ≤ 3)
   - Embedding similarity ≥ 0.87 on name+description
   - On match: update aliases, bump mention count
4. Wire resolver into ingestion path

### 7.3 Phase 6c: Bitemporal + Epistemic Classes (1 week)

1. Migration already done in 6a (columns added); now populate them
2. Extraction prompt asks for `epistemic_class` + `valid_at`/`invalidated_at` per fact/relationship
3. Update `recall.rs` scoring logic:
   - Class-aware penalty branching (archival vs current vs convention vs procedural)
4. Correction vs supersession paths:
   - Archival correction updates old fact, preserves archival status
   - Current supersession uses existing Phase 1 logic

### 7.4 Phase 6d: MAGMA Views + Real-Time Tool Extraction (2 weeks)

1. `ToolResultExtractor` trait + implementations:
   - `WebFetchExtractor` — extract URL, title, date, mentioned entities from response
   - `ShellExtractor` — extract file paths, line numbers from grep-like output
   - `StructuredReturnExtractor` — parse known schemas from tool return values
2. Wire into `spawn_execution_task` — called per tool result, async, non-blocking
3. Query view APIs:
   - `graph.semantic_view(entity_id)` — embedding-ordered neighbors
   - `graph.temporal_view(entity_id)` — time-ordered events
   - `graph.causal_view(entity_id)` — follow causal edges
   - `graph.entity_view(entity_id)` — n-hop neighborhood (existing)
   - `graph.hybrid_search(query)` — combine all three + rerank
4. Expose views via `graph_query` tool additions

## 8. Non-Functional Requirements

### 8.1 Rust Backend Quality

| Requirement | Standard | Reference |
|-------------|----------|-----------|
| Formatting | `cargo fmt --all --check` clean | — |
| Linting | `cargo clippy --all-targets -- -D warnings` clean | — |
| Cognitive Complexity | Every function ≤ **15** (SonarQube threshold) | `rust:S3776` |
| No `unwrap()` in production | Use `?`, `unwrap_or`, `unwrap_or_else` | — |
| UTF-8 safety | All byte-index slicing via char-boundary helpers | internal |
| Error handling | `Result<T, String>` matching existing patterns | — |
| Nesting depth | Max 4 levels of nested control flow | SonarQube S2004 |

**Cognitive Complexity (`rust:S3776`)** — per SonarQube's definition:

> Cognitive Complexity is a measure of how hard it is to understand the control flow of a unit of code. It is incremented each time the code breaks the normal linear reading flow (loops, conditionals, catches, switches, jumps, multi-operator conditions) and each time nesting depth increases. Method calls do not increment complexity — a well-named method is a free summary. Recursive calls, however, do increment.
>
> Threshold: **15**. Above this, functions must be refactored into smaller pieces. Severity: CRITICAL code smell with HIGH maintainability impact.
>
> Reference: [Sonar Cognitive Complexity paper](https://www.sonarsource.com/docs/CognitiveComplexity.pdf)

Enforcement: CI runs `sonar-scanner` on push to main. Any new function exceeding 15 blocks merge. For this phase:
- Extract handlers into named functions rather than inlining branches
- Prefer early returns over deep nesting
- Use `match` over chained `if-else` when branching on an enum

### 8.2 Tests per Sub-Phase

**6a Episodes + Ward Indexer**:
- `EpisodeRepository` CRUD tests (in-memory SQLite)
- Content hash dedup test
- JSON collection schema detection tests (array-of-objects heuristic)
- End-to-end test: given a sample `timeline.json`, produce expected entities

**6b Ontology + Entity Resolution**:
- `EntityResolver::resolve_or_create` with exact match
- Fuzzy name match test (`Savarkar` ↔ `V.D. Savarkar`)
- Embedding similarity merge test
- Type constraint isolation test (Person `Savarkar` doesn't merge with Place `Savarkar`)
- Alias accumulation test

**6c Bitemporal + Epistemic**:
- Class-aware recall scoring (archival no-decay, current strong-decay)
- Correction path test (archival fact corrected, both retrievable)
- `valid_at` range query test

**6d Views + Real-Time**:
- Each `ToolResultExtractor` with representative tool outputs
- View query tests (fixtures: small graph, assert correct entities returned per view)

### 8.3 Performance

| Operation | Target |
|-----------|--------|
| Ward artifact indexing (100-event JSON) | < 500ms |
| Entity resolution on write | < 50ms per entity |
| Episode hash dedup check | < 10ms |
| Tool result extraction | < 100ms (non-blocking) |
| Hybrid graph query | < 300ms |

### 8.4 Database

- Additive migrations only (new columns, new tables, default values preserve existing rows)
- Schema version bump: v21
- Every new FK column indexed
- Backward compatibility: existing graph records work unchanged; new columns default to `epistemic_class = current`, `confidence = 0.8`, etc.

### 8.5 UI

No UI changes required for Phase 6. Future work may add graph visualization enhancements (color by epistemic class, timeline view), but this spec is backend-only.

## 9. Out of Scope

- Federated queries across external knowledge bases (future work)
- Full ontology alignment to Wikidata / schema.org (future work, mentioned as direction)
- Graph visualization redesign (future work)
- Automatic ontology learning (user-proposed types becoming first-class)
- Multi-agent graph (currently per-agent; shared graph across agents is future work)

## 10. Success Criteria

After Phase 6a deployment:
- A session like "Hindu Mahasabha fact extraction" produces ≥ 50 graph entities (vs 9 today)
- Every graph entity has a `source_episode_id` traceable to a tool result, ward file, or session
- Graph entities include at least one `Place`, `Event`, or `Person` type (not just tools/projects)

After Phase 6b:
- Entity variants (`Savarkar`, `V.D. Savarkar`) merge to single canonical entity
- Relationship vocabulary usage shifts: ≥ 30% of relationships use typed vocab (not `related_to`)
- Properties populated on ≥ 70% of domain entities

After Phase 6c:
- Archival facts from past sessions retain full recall score even months later
- Bitemporal query "what was true at time T?" returns correct historical state

After Phase 6d:
- Graph updates during execution, visible in real-time via `graph_query` tool
- Four query views selectable by agents

## 11. Files Modified/Created

### Phase 6a

| Action | File |
|--------|------|
| MODIFY | `gateway/gateway-database/src/schema.rs` — migration v21 |
| CREATE | `gateway/gateway-database/src/episode_repository.rs` (or reuse existing) |
| CREATE | `gateway/gateway-execution/src/ward_artifact_indexer.rs` |
| MODIFY | `gateway/gateway-execution/src/runner.rs` — call indexer post-distillation |
| MODIFY | `services/knowledge-graph/src/storage.rs` — support new columns |

### Phase 6b

| Action | File |
|--------|------|
| MODIFY | `services/knowledge-graph/src/types.rs` — expand EntityType, RelationshipType |
| CREATE | `services/knowledge-graph/src/resolver.rs` — EntityResolver |
| MODIFY | `gateway/gateway-execution/src/distillation.rs` — expanded prompt with few-shot |
| MODIFY | `services/knowledge-graph/src/storage.rs` — use resolver on write |

### Phase 6c

| Action | File |
|--------|------|
| MODIFY | `gateway/gateway-execution/src/distillation.rs` — extract epistemic_class |
| MODIFY | `gateway/gateway-execution/src/recall.rs` — class-aware scoring |
| MODIFY | `gateway/gateway-database/src/memory_repository.rs` — class field in struct |

### Phase 6d

| Action | File |
|--------|------|
| CREATE | `gateway/gateway-execution/src/tool_result_extractor.rs` — trait + impls |
| MODIFY | `gateway/gateway-execution/src/runner.rs` — wire extractor in tool result path |
| MODIFY | `runtime/agent-tools/src/tools/graph_query.rs` — add view modes |
| MODIFY | `services/knowledge-graph/src/service.rs` — view query methods |

## 12. Migration + Backward Compatibility

- **Existing graph entities**: get `epistemic_class = 'current'` by default (neutral — no behavior change for old data)
- **Existing relationships**: get same default
- **Existing memory_facts**: get `epistemic_class = 'current'` by default; recall behavior unchanged from Phase 1 for existing facts
- **New extractions**: properly classified from Phase 6c onward
- **No data loss**: every migration is additive; rollback is schema-version decrement only (columns remain, treated as nullable)

## 13. Why This Is The Right Design

**Research convergence**: Graphiti (2025), MAGMA (2026), A-MEM (NeurIPS 2025) independently arrived at the episodes + resolution + bitemporal + multi-view pattern. When three research lines converge, it's not a trend — it's the shape of the problem.

**Benchmarked baseline**: Graphiti's specific approach (the closest match to this design) shows 94.8% DMR accuracy, 90% latency reduction. Not speculative.

**Composable layers**: Each sub-phase ships independently and adds value without the others. Phase 6a alone transforms the graph richness. Each subsequent layer compounds.

**Archival preservation baked in**: the epistemic class model — which you (the user) correctly identified — is woven into storage, recall scoring, and correction semantics. Not a flag added later, but a first-class dimension.

**Provenance end-to-end**: every fact, entity, relationship ties back to a source episode. Audit trail. Debuggability. Trustworthiness.

---

## Appendix A: Example Extraction Transformation

### Before (today's output, Hindu Mahasabha session)

9 entities, all orchestration-level:
```
PTON, NVDA, TSLA, AAPL, SPY, portfolio-analysis,
planner-agent, code-agent, research-agent
```

### After (Phase 6a+b applied to the same session)

100+ entities, domain-rich:

```
Persons (epistemic: archival)
  V.D. Savarkar {birth: 1883, death: 1966, role: president}
    aliases: ["Savarkar", "Vinayak Damodar Savarkar"]
  Bhai Parmanand {aliases: ["Parmanand", "Parmananda"]}
  ... 29 more

Organizations (archival)
  Hindu Mahasabha {founded: 1915, type: political}
  ... 22 more

Places (archival)
  Ahmedabad {country: India, type: city}
  Maharashtra {country: India, type: state}
  ... 39 more

Events (archival)
  Ahmedabad Session 1937 {date: 1937, location: Ahmedabad, type: meeting}
  ... 194 more (from timeline.json)

TimePeriods (archival)
  Pre-Independence Era {start: 1915, end: 1947}

Documents (archival)
  Hindu Mahasabha History PDF {source_ref: hindu_mahasabha.pdf}
```

Relationships (bitemporal, archival):
```
V.D. Savarkar --president_of--> Hindu Mahasabha
  {valid_at: "1937", invalidated_at: "1938", source_episodes: [ep_42]}

Ahmedabad Session 1937 --held_at--> Ahmedabad
  {valid_at: "1937", source_episodes: [ep_42]}

V.D. Savarkar --participant_in--> Ahmedabad Session 1937
  {source_episodes: [ep_42]}
```

Every fact traceable to PDF page 42 via episode provenance. Archival class means no decay. Name aliases resolved. Rich properties. Queryable by time.

This is a knowledge graph.
