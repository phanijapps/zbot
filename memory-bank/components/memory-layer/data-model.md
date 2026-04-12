# Memory Layer — Complete Data Model

Every table, every column, migration history.

Two SQLite databases at `~/Documents/zbot/data/`:

- `conversations.db` — sessions, facts, wiki, procedures, episodes
- `knowledge_graph.db` — entities, relationships, causal edges

Current schema version: **21**

---

## Schema Version History

| Version | Phase | What Was Added |
|---------|-------|----------------|
| 1–17 | Pre-Phase 1 | Base tables (sessions, agent_executions, messages, memory_facts, execution_logs, embedding_cache, session_episodes, recall_log, distillation_runs, memory_facts_archive, etc.) |
| 18 | Phase 1 | `memory_facts.valid_from`, `valid_until`, `superseded_by` columns + `kg_causal_edges` table |
| 19 | Phase 3 | `ward_wiki_articles` table |
| 20 | Phase 4 | `procedures` table |
| 21 | Phase 6a | `kg_episodes` table + `memory_facts.epistemic_class`, `source_episode_id`, `source_ref` columns |

Knowledge graph table columns added incrementally via `ALTER TABLE ... IF NOT EXISTS` in `services/knowledge-graph/src/storage.rs` init: `kg_entities` and `kg_relationships` gained `aliases`, `epistemic_class`, `source_episode_ids`, `valid_from`, `valid_until`, `confidence` (entities) and `valid_at`, `invalidated_at`, `epistemic_class`, `source_episode_ids`, `confidence` (relationships).

---

## `conversations.db` Tables

### `sessions`
Top-level conversation container.

| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | `sess-{uuid}` or `sess-chat-{uuid}` |
| status | TEXT | `queued`, `running`, `completed`, `crashed` |
| source | TEXT | `web`, `cli`, `cron`, `api`, `connector` |
| root_agent_id | TEXT | Usually `root` |
| title | TEXT | Set via `set_session_title` tool |
| created_at, started_at, completed_at | TEXT | ISO 8601 |
| total_tokens_in, total_tokens_out | INTEGER | Aggregated from executions |
| metadata | TEXT (JSON) | Flexible metadata |
| pending_delegations | INTEGER | Running subagent count |
| continuation_needed | INTEGER (bool) | Set when root should resume |
| ward_id | TEXT | Current ward |
| parent_session_id | TEXT | For child/delegated sessions |
| thread_id, connector_id, respond_to | TEXT | Routing fields |
| archived | INTEGER (bool) | User-archived flag |
| mode | TEXT | `fast`, NULL for deep mode |

### `agent_executions`
One row per agent invocation within a session.

| Column | Notes |
|--------|-------|
| id | `exec-{uuid}` |
| session_id | FK → sessions |
| agent_id | Which agent ran |
| parent_execution_id | For child/delegated executions |
| delegation_type | `root`, `sequential`, `parallel` |
| task | Task description passed to agent |
| status | `queued`, `running`, `completed`, `crashed`, `cancelled` |
| started_at, completed_at | Timing |
| tokens_in, tokens_out | LLM token usage |
| checkpoint, error | Execution state |
| log_path, child_session_id | Optional |

### `messages`
Every message in every session (user, assistant, tool, system).

| Column | Notes |
|--------|-------|
| id | `msg-{uuid}` |
| execution_id | FK → agent_executions |
| session_id | FK → sessions |
| role | `user`, `assistant`, `tool`, `system` |
| content | TEXT, full message content |
| created_at | ISO 8601 |
| token_count | Per-message tokens |
| tool_calls, tool_results | JSON for assistant/tool messages |
| tool_call_id | Links tool results to tool calls |

### `memory_facts`
The fact store.

| Column | Notes |
|--------|-------|
| id | TEXT PK |
| session_id | Originating session (optional) |
| agent_id | Owner agent |
| scope | `agent`, `shared`, `ward` |
| category | `correction`, `strategy`, `domain`, `instruction`, `pattern`, `user`, `skill`, `agent`, `ward` |
| key | Dot-notation hierarchy, e.g., `pattern.yfinance.rate_limit` |
| content | The actual fact text |
| confidence | REAL, 0.0–1.0 |
| mention_count | INTEGER |
| source_summary | Optional provenance string |
| embedding | BLOB (little-endian f32 array) |
| ward_id | `__global__` or specific ward |
| contradicted_by | FK → memory_facts.id (optional) |
| created_at, updated_at | Timestamps |
| expires_at | Optional TTL |
| **valid_from** *(v18)* | When fact became true |
| **valid_until** *(v18)* | When superseded (NULL = still valid) |
| **superseded_by** *(v18)* | FK → memory_facts.id of replacement |
| pinned | INTEGER (bool), protects from distillation overwrite |
| **epistemic_class** *(v21)* | `archival`, `current`, `convention`, `procedural` — defaults to `current` |
| **source_episode_id** *(v21)* | FK → kg_episodes.id |
| **source_ref** *(v21)* | Human-readable source pointer (e.g., `pdf:page_42`) |

Unique: `(agent_id, scope, ward_id, key)` — one active fact per key.

### `embedding_cache`
| Column | Notes |
|--------|-------|
| content_hash | TEXT, SHA-256 of text |
| model | Embedding model name |
| embedding | BLOB |
| created_at | Timestamp |

PK: `(content_hash, model)`.

### `session_episodes`
Outcome record per session.

| Column | Notes |
|--------|-------|
| id | TEXT PK |
| session_id | FK, one episode per session |
| agent_id | Root agent |
| ward_id | `__global__` or specific |
| task_summary | What was attempted |
| outcome | `success`, `partial`, `failed` |
| strategy_used | High-level approach |
| key_learnings | What went well/poorly |
| token_cost | Total tokens |
| embedding | BLOB for similarity search |
| created_at | Timestamp |

### `recall_log`
Tracks which facts were recalled per session (used by predictive recall boost).

| Column | Notes |
|--------|-------|
| session_id | FK |
| fact_key | memory_facts.key |
| recalled_at | Timestamp |

PK: `(session_id, fact_key)`.

### `distillation_runs`
Distillation attempt tracking.

| Column | Notes |
|--------|-------|
| id | TEXT PK |
| session_id | UNIQUE — one run per session |
| status | `pending`, `success`, `failed` |
| facts_extracted, entities_extracted, relationships_extracted, episode_created | Counts |
| error, retry_count | Failure tracking |
| duration_ms | Timing |
| created_at | Timestamp |

### `memory_facts_archive`
Pruned facts (for potential rollback).

Same schema as `memory_facts` + `archived_at`.

### `artifacts`
Files generated during sessions.

| Column | Notes |
|--------|-------|
| id | TEXT PK |
| session_id, ward_id, execution_id, agent_id | Provenance |
| file_path, file_name, file_type, file_size | Artifact info |
| label | Optional description |
| created_at | Timestamp |

### `ward_wiki_articles` *(v19, Phase 3)*

| Column | Notes |
|--------|-------|
| id | TEXT PK, `wiki-{ward}-{uuid}` or `wiki-{ward}-index` |
| ward_id | The ward this article belongs to |
| agent_id | Owner |
| title | Article title (UNIQUE with ward_id) |
| content | Markdown article content |
| tags | JSON array |
| source_fact_ids | JSON array of contributing fact IDs |
| embedding | BLOB for similarity search |
| version | Incremented on each recompilation |
| created_at, updated_at | Timestamps |

Special: `title = '__index__'` is the master index listing all articles.

UNIQUE: `(ward_id, title)`.

### `procedures` *(v20, Phase 4)*

| Column | Notes |
|--------|-------|
| id | TEXT PK |
| agent_id | Owner |
| ward_id | Scope (default `__global__`) |
| name | Procedure short name |
| description | What it accomplishes |
| trigger_pattern | When to use (freeform) |
| steps | JSON array of `{action, agent?, task_template?, note?}` |
| parameters | JSON array of parameter names |
| success_count, failure_count | Evolution counters |
| avg_duration_ms, avg_token_cost | Performance stats |
| last_used | Most recent invocation |
| embedding | BLOB for similarity search |
| created_at, updated_at | Timestamps |

### `kg_episodes` *(v21, Phase 6a)*

| Column | Notes |
|--------|-------|
| id | TEXT PK |
| source_type | `tool_result`, `ward_file`, `session`, `distillation`, `user_input` |
| source_ref | Exact identifier (tool_call_id / file_path / session_id / etc.) |
| content_hash | SHA-256 for dedup |
| session_id | Originating session (optional) |
| agent_id | Agent context |
| created_at | Timestamp |

UNIQUE: `(content_hash, source_type)` — prevents re-extracting unchanged content.

---

## `knowledge_graph.db` Tables

### `kg_entities`
Base columns (pre-Phase 6) plus Phase 6 additions:

| Column | Notes |
|--------|-------|
| id | TEXT PK |
| agent_id | Owner (or `__global__`) |
| entity_type | 13 variants (Person, Organization, Location, Event, TimePeriod, Document, Role, Artifact, Ward, Concept, Tool, Project, File) |
| name | Canonical name |
| properties | JSON, type-specific schema |
| first_seen_at, last_seen_at | Temporal |
| mention_count | Usage counter |
| **aliases** *(Phase 6b)* | JSON array of name variants |
| **epistemic_class** *(Phase 6c)* | `archival` \| `current` \| `convention` \| `procedural` |
| **source_episode_ids** *(Phase 6a)* | JSON array of kg_episodes.id |
| **valid_from, valid_until** *(Phase 6)* | Bitemporal entity lifetime |
| **confidence** *(Phase 6)* | REAL, default 0.8 |

Indexes: `(agent_id)`, `(name)`, `(agent_id, epistemic_class)`.

### `kg_relationships`

| Column | Notes |
|--------|-------|
| id | TEXT PK |
| agent_id | Owner |
| source_entity_id, target_entity_id | FK → kg_entities.id (ON DELETE CASCADE) |
| relationship_type | 27 variants grouped temporal/role/spatial/causal/hierarchical/generic |
| properties | JSON |
| first_seen_at, last_seen_at | Temporal |
| mention_count | Usage counter |
| **valid_at, invalidated_at** *(Phase 6c)* | Bitemporal relationship validity |
| **epistemic_class** *(Phase 6c)* | Same vocab as entities |
| **source_episode_ids** *(Phase 6a)* | JSON array |
| **confidence** *(Phase 6)* | REAL, default 0.8 |

Indexes: `(source_entity_id)`, `(target_entity_id)`, `(agent_id)`, `(valid_at)`.

UNIQUE: `(source_entity_id, target_entity_id, relationship_type)`.

### `kg_causal_edges` *(Phase 1)*

| Column | Notes |
|--------|-------|
| id | TEXT PK |
| agent_id | Owner |
| cause_entity_id, effect_entity_id | FK → kg_entities.id (ON DELETE CASCADE) |
| relationship | `causes`, `prevents`, `requires`, `enables` |
| confidence | REAL, default 0.7 |
| session_id | Provenance |
| created_at | Timestamp |

Indexes: `(cause_entity_id)`, `(effect_entity_id)`.

---

## File-Backed Memory Artifacts (Per Ward)

Not in any DB — generated during session distillation to `~/Documents/zbot/wards/{ward_id}/memory-bank/`:

- `ward.md` — curated corrections + strategies (capped at 1KB)
- `core_docs.md` — code inventory with function signatures
- `structure.md` — directory tree with file purposes
- `AGENTS.md` — agent roles and procedures (in ward root)

These are the "always in context" per-ward artifacts. The `ward_wiki_articles` table holds per-topic articles that are loaded on demand.

---

## Lifecycle Summary by Table

| Table | Write Events | Read Events |
|-------|-------------|-------------|
| sessions | Session start, status transitions | Dashboard, recall (session_id lookup) |
| memory_facts | Distillation, user input, ward sync, strategy emergence | Recall (every session start + mid-session) |
| session_episodes | Distillation (one per session) | Recall (similarity search for past experiences) |
| recall_log | Every recall call | Predictive recall boost |
| ward_wiki_articles | Post-distillation (`compile_ward_wiki`) | Recall (wiki-first) |
| procedures | Distillation (if multi-step success) | Intent analysis (pre-session) |
| kg_episodes | Every extraction (tool result, ward artifact, distillation) | Graph query (provenance drill-down) |
| kg_entities | Distillation, ward artifact indexer, tool result extractor | Graph query tool, micro-recall (entity mentions) |
| kg_relationships | Same as entities | Graph query (neighbors, multi-hop) |
| kg_causal_edges | Distillation (when causal language detected) | Graph query (`causal` view when implemented) |
| distillation_runs | Every distillation attempt | Retry logic, observability |

---

## Provenance Chain

A fact in `memory_facts` → traces via `source_episode_id` → to `kg_episodes` → which identifies the source (tool call, file, session, user input).

An entity in `kg_entities` → `source_episode_ids` JSON array → multiple episodes if the entity was observed in multiple sources.

A relationship in `kg_relationships` → `source_episode_ids` → the exact source that produced this edge.

This means any claim the agent makes can be traced back through the extraction pipeline to the original tool result, ward file, or session transcript. No orphan knowledge.

---

## Query Patterns (Common Examples)

**"What do I know about X?" (semantic breadth)**:
```sql
SELECT * FROM kg_entities
WHERE agent_id IN (?, '__global__') AND name LIKE '%X%'
ORDER BY mention_count DESC
LIMIT 10;
```

**"What was true at time T?" (bitemporal)**:
```sql
SELECT * FROM kg_relationships
WHERE (valid_at IS NULL OR valid_at <= '2026-04-12')
  AND (invalidated_at IS NULL OR invalidated_at > '2026-04-12')
  AND source_entity_id = ?;
```

**"Current corrections only (not archival records, not superseded)"**:
```sql
SELECT * FROM memory_facts
WHERE agent_id = ?
  AND category = 'correction'
  AND valid_until IS NULL
  AND epistemic_class IN ('convention', 'current')
ORDER BY confidence DESC;
```

**"Archival facts about a topic regardless of age"**:
```sql
SELECT * FROM memory_facts
WHERE agent_id = ?
  AND epistemic_class = 'archival'
  AND content LIKE '%Savarkar%'
ORDER BY confidence DESC;
-- no valid_until filter — archival facts don't "expire"
```
