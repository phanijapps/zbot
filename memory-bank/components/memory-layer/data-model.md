# Memory Layer — v22 Data Model

Every table across both SQLite databases, every column, every index and
trigger. Copy-verbatim from
`gateway/gateway-database/src/schema.rs` (conversations.db) and
`gateway/gateway-database/src/knowledge_schema.rs` (knowledge.db) at the
time of writing — when in doubt, grep those files.

Default location: `~/Documents/zbot/data/`. `SCHEMA_VERSION = 22` in both
files.

---

## `conversations.db`

Operational state. Source: `gateway/gateway-database/src/schema.rs`.

### `sessions`

**Purpose:** Top-level container for a user's work session.
**DB:** `conversations.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | uuid |
| status | TEXT NOT NULL | default `'running'` |
| source | TEXT NOT NULL | default `'web'` |
| root_agent_id | TEXT NOT NULL | |
| title | TEXT | |
| created_at | TEXT NOT NULL | |
| started_at | TEXT | |
| completed_at | TEXT | |
| total_tokens_in | INTEGER | default 0 |
| total_tokens_out | INTEGER | default 0 |
| metadata | TEXT | JSON |
| pending_delegations | INTEGER | default 0 |
| continuation_needed | INTEGER | default 0 |
| ward_id | TEXT | |
| parent_session_id | TEXT | |
| thread_id | TEXT | routing |
| connector_id | TEXT | routing |
| respond_to | TEXT | routing |
| archived | INTEGER NOT NULL | default 0 |
| mode | TEXT | persistent execution mode |

**Indexes:** `idx_sessions_status`, `idx_sessions_created`,
`idx_sessions_root_agent`, `idx_sessions_source`, `idx_sessions_parent`.
**Writes:** `SessionRepository` in `repository.rs`.
**Reads:** gateway session APIs; recall filters.

### `agent_executions`

**Purpose:** An agent's participation in a session (root or delegated
subagent).
**DB:** `conversations.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| session_id | TEXT NOT NULL | FK sessions(id) CASCADE |
| agent_id | TEXT NOT NULL | |
| parent_execution_id | TEXT | FK self SET NULL |
| delegation_type | TEXT NOT NULL | default `'root'` |
| task | TEXT | |
| status | TEXT NOT NULL | default `'queued'` |
| started_at | TEXT | |
| completed_at | TEXT | |
| tokens_in | INTEGER | default 0 |
| tokens_out | INTEGER | default 0 |
| checkpoint | TEXT | |
| error | TEXT | |
| log_path | TEXT | |
| child_session_id | TEXT | FK sessions(id) SET NULL |

**Indexes:** `idx_executions_session`, `idx_executions_parent`,
`idx_executions_status`, `idx_executions_agent`, `idx_executions_started`.

### `messages`

**Purpose:** Individual messages in an agent's conversation.
**DB:** `conversations.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| execution_id | TEXT | FK agent_executions CASCADE |
| session_id | TEXT | FK sessions CASCADE |
| role | TEXT NOT NULL | |
| content | TEXT NOT NULL | |
| created_at | TEXT NOT NULL | |
| token_count | INTEGER | default 0 |
| tool_calls | TEXT | JSON |
| tool_results | TEXT | JSON |
| tool_call_id | TEXT | |

**Indexes:** `idx_messages_execution`, `idx_messages_created`,
`idx_messages_session`, `idx_messages_session_created`.

### `artifacts`

**Purpose:** File artifacts produced by agent executions.
**DB:** `conversations.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| session_id | TEXT NOT NULL | FK sessions CASCADE |
| ward_id | TEXT | |
| execution_id | TEXT | |
| agent_id | TEXT | |
| file_path | TEXT NOT NULL | |
| file_name | TEXT NOT NULL | |
| file_type | TEXT | |
| file_size | INTEGER | |
| label | TEXT | |
| created_at | TEXT NOT NULL | |

**Indexes:** `idx_artifacts_session`.

### `execution_logs`

**Purpose:** Detailed logs for debugging and tracing agent execution.
**DB:** `conversations.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| session_id | TEXT NOT NULL | |
| conversation_id | TEXT | |
| agent_id | TEXT NOT NULL | |
| parent_session_id | TEXT | |
| timestamp | TEXT NOT NULL | |
| level | TEXT NOT NULL | |
| category | TEXT NOT NULL | |
| message | TEXT NOT NULL | |
| metadata | TEXT | JSON |
| duration_ms | INTEGER | |

**Indexes:** `idx_logs_session`, `idx_logs_timestamp`, `idx_logs_agent`.

### `recall_log`

**Purpose:** Tracks which facts were recalled per session for predictive
recall.
**DB:** `conversations.db`

| Column | Type | Notes |
|---|---|---|
| session_id | TEXT NOT NULL | PK part 1 |
| fact_key | TEXT NOT NULL | PK part 2 |
| recalled_at | TEXT NOT NULL | |

**Indexes:** `idx_recall_log_session`.
**Writes:** `RecallLogRepository` (`recall_log_repository.rs`).

### `distillation_runs`

**Purpose:** Tracks distillation health per session.
**DB:** `conversations.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| session_id | TEXT NOT NULL UNIQUE | |
| status | TEXT NOT NULL | |
| facts_extracted | INTEGER | default 0 |
| entities_extracted | INTEGER | default 0 |
| relationships_extracted | INTEGER | default 0 |
| episode_created | INTEGER | default 0 |
| error | TEXT | |
| retry_count | INTEGER | default 0 |
| duration_ms | INTEGER | |
| created_at | TEXT NOT NULL | |

**Indexes:** `idx_distillation_runs_status`.
**Writes:** `DistillationRepository`.

### `bridge_outbox`

**Purpose:** Reliable delivery queue for outbound messages to bridge
workers.
**DB:** `conversations.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| adapter_id | TEXT NOT NULL | |
| capability | TEXT NOT NULL | |
| payload | TEXT NOT NULL | JSON |
| status | TEXT NOT NULL | default `'pending'` |
| session_id | TEXT | |
| thread_id | TEXT | |
| agent_id | TEXT | |
| created_at | TEXT NOT NULL | default `datetime('now')` |
| sent_at | TEXT | |
| error | TEXT | |
| retry_count | INTEGER NOT NULL | default 0 |
| retry_after | TEXT | |

**Indexes:** `idx_outbox_adapter_status`, `idx_outbox_created`.

---

## `knowledge.db`

Long-term memory. Source:
`gateway/gateway-database/src/knowledge_schema.rs`.

### `kg_entities`

**Purpose:** Typed entities in the knowledge graph.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| agent_id | TEXT NOT NULL | `__global__` allowed |
| entity_type | TEXT NOT NULL | |
| name | TEXT NOT NULL | surface form |
| normalized_name | TEXT NOT NULL | |
| normalized_hash | TEXT NOT NULL | |
| properties | TEXT | JSON |
| epistemic_class | TEXT NOT NULL | default `'current'` |
| confidence | REAL NOT NULL | default 0.8 |
| mention_count | INTEGER NOT NULL | default 1 |
| access_count | INTEGER NOT NULL | default 0 |
| first_seen_at | TEXT NOT NULL | |
| last_seen_at | TEXT NOT NULL | |
| last_accessed_at | TEXT | |
| valid_from | TEXT | |
| valid_until | TEXT | |
| invalidated_by | TEXT | |
| compressed_into | TEXT | winner id or `__pruned__` sentinel |
| source_episode_ids | TEXT | JSON array |

**Indexes:** `idx_entities_normalized_hash(agent_id, entity_type,
normalized_hash)`, `idx_entities_agent_type`, `idx_entities_name`,
`idx_entities_last_accessed`, `idx_entities_epistemic(agent_id,
epistemic_class)`.
**Writes:** `knowledge_graph::storage::store_entity`.
**Reads:** resolver, compactor, `GraphStorage::*`, recall adapter.

### `kg_relationships`

**Purpose:** Directional typed edges between entities.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| agent_id | TEXT NOT NULL | |
| source_entity_id | TEXT NOT NULL | FK kg_entities CASCADE |
| target_entity_id | TEXT NOT NULL | FK kg_entities CASCADE |
| relationship_type | TEXT NOT NULL | |
| properties | TEXT | JSON |
| epistemic_class | TEXT NOT NULL | default `'current'` |
| confidence | REAL NOT NULL | default 0.8 |
| mention_count | INTEGER NOT NULL | default 1 |
| access_count | INTEGER NOT NULL | default 0 |
| first_seen_at | TEXT NOT NULL | |
| last_seen_at | TEXT NOT NULL | |
| last_accessed_at | TEXT | |
| valid_at | TEXT | |
| invalidated_at | TEXT | |
| invalidated_by | TEXT | |
| source_episode_ids | TEXT | JSON array |

**Uniqueness:** `UNIQUE(source_entity_id, target_entity_id,
relationship_type)`.
**Indexes:** `idx_rels_source`, `idx_rels_target`, `idx_rels_agent`,
`idx_rels_valid`.

### `kg_aliases`

**Purpose:** Surface-form variants per entity. Drives resolver stage 1.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| entity_id | TEXT NOT NULL | FK kg_entities CASCADE |
| surface_form | TEXT NOT NULL | |
| normalized_form | TEXT NOT NULL | |
| source | TEXT NOT NULL | `'extraction'` \| `'merge'` \| ... |
| confidence | REAL NOT NULL | default 1.0 |
| first_seen_at | TEXT NOT NULL | |

**Uniqueness:** `UNIQUE(normalized_form, entity_id)`.
**Indexes:** `idx_aliases_normalized`, `idx_aliases_entity`.
**Writes:** `store_entity` seeds one `source='extraction'` alias per new
entity (`services/knowledge-graph/src/storage.rs:1798`); merges append
`source='merge'` aliases.

### `kg_episodes`

**Purpose:** One row per extraction run. Tracks pipeline state for the
ingestion queue.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| source_type | TEXT NOT NULL | e.g. `'document'`, `'session'` |
| source_ref | TEXT NOT NULL | e.g. `'book#chunk-3'` |
| content_hash | TEXT NOT NULL | |
| session_id | TEXT | |
| agent_id | TEXT NOT NULL | |
| status | TEXT NOT NULL | default `'pending'` (→ `'running'` → `'done'` \| `'failed'`) |
| retry_count | INTEGER NOT NULL | default 0 |
| error | TEXT | |
| created_at | TEXT NOT NULL | |
| started_at | TEXT | |
| completed_at | TEXT | |

**Uniqueness:** `UNIQUE(content_hash, source_type)` — idempotent ingest.
**Indexes:** `idx_episodes_status`, `idx_episodes_source_ref`,
`idx_episodes_session`.
**Writes:** `KgEpisodeRepository` (`kg_episode_repository.rs`).

### `kg_episode_payloads`

**Purpose:** Stores chunk text for a pending episode so workers can
re-read after restart.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| episode_id | TEXT PRIMARY KEY | FK kg_episodes CASCADE |
| text | TEXT NOT NULL | chunk text |
| created_at | TEXT NOT NULL | |

### `kg_goals` (Phase 3)

**Purpose:** Active and completed user/agent goals with slot tracking.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| agent_id | TEXT NOT NULL | |
| ward_id | TEXT | |
| title | TEXT NOT NULL | |
| description | TEXT | |
| state | TEXT NOT NULL | default `'active'` (→ `'done'` / `'abandoned'`) |
| parent_goal_id | TEXT | FK self |
| slots | TEXT | JSON array of slot names |
| filled_slots | TEXT | JSON object name→value |
| created_at | TEXT NOT NULL | |
| updated_at | TEXT NOT NULL | |
| completed_at | TEXT | |

**Indexes:** `idx_goals_agent_state`, `idx_goals_ward`.
**Writes/Reads:** `GoalRepository` (`goal_repository.rs`).

### `kg_compactions`

**Purpose:** Audit log for every merge and prune performed by the
sleep-time worker.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| run_id | TEXT NOT NULL | one id per sleep-time cycle |
| operation | TEXT NOT NULL | `'merge'` \| `'prune'` |
| entity_id | TEXT | loser (merge) / target (prune) |
| relationship_id | TEXT | reserved |
| merged_into | TEXT | winner id (merge only) |
| reason | TEXT | e.g. `'cosine=0.94'`, `'orphan age>30d ...'` |
| created_at | TEXT NOT NULL | |

**Indexes:** `idx_compactions_run`.
**Writes:** `CompactionRepository::record_merge` / `record_prune`
(`compaction_repository.rs`).

### `kg_causal_edges`

**Purpose:** Separate causal-edge store (cause → effect), distinct from
generic relationships.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| agent_id | TEXT NOT NULL | |
| cause_entity_id | TEXT NOT NULL | FK kg_entities CASCADE |
| effect_entity_id | TEXT NOT NULL | FK kg_entities CASCADE |
| relationship | TEXT NOT NULL | causal relation name |
| confidence | REAL | default 0.7 |
| session_id | TEXT | |
| created_at | TEXT NOT NULL | |

**Indexes:** `idx_causal_cause`, `idx_causal_effect`.

### `memory_facts`

**Purpose:** Atomic factual propositions; Layer 0 recall target.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| session_id | TEXT | |
| agent_id | TEXT NOT NULL | |
| scope | TEXT NOT NULL | `'agent'` \| `'session'` \| ... |
| category | TEXT NOT NULL | |
| key | TEXT NOT NULL | |
| content | TEXT NOT NULL | |
| confidence | REAL NOT NULL | default 0.8 |
| mention_count | INTEGER NOT NULL | default 1 |
| source_summary | TEXT | |
| source_episode_id | TEXT | |
| source_ref | TEXT | |
| ward_id | TEXT NOT NULL | default `'__global__'` |
| epistemic_class | TEXT NOT NULL | default `'current'` |
| contradicted_by | TEXT | |
| valid_from | TEXT | |
| valid_until | TEXT | |
| superseded_by | TEXT | |
| pinned | INTEGER NOT NULL | default 0 |
| created_at | TEXT NOT NULL | |
| updated_at | TEXT NOT NULL | |
| expires_at | TEXT | |

**Uniqueness:** `UNIQUE(agent_id, scope, ward_id, key)`.
**Indexes:** `idx_facts_agent_scope`, `idx_facts_category`,
`idx_facts_ward`, `idx_facts_epistemic`.
**No `embedding` BLOB column** — the vec0 partner `memory_facts_index`
holds the vectors. Asserted at
`gateway/gateway-database/src/knowledge_schema.rs:434`.
**Writes/Reads:** `MemoryRepository` (`memory_repository.rs`).

### `memory_facts_fts` (virtual, FTS5)

`CREATE VIRTUAL TABLE memory_facts_fts USING fts5(key, content,
category, content=memory_facts)`.

Contentless FTS5 index over `memory_facts`, kept in sync by three
triggers:

- `memory_facts_ai` — after insert, mirror row into FTS.
- `memory_facts_ad` — after delete, emit FTS `'delete'` command for
  the old row.
- `memory_facts_au` — after update, FTS `'delete'` + re-insert.

See `knowledge_schema.rs:350`.

### `memory_facts_archive`

**Purpose:** Cold storage for archived facts — retained for audit, no
recall.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| session_id | TEXT | |
| agent_id | TEXT NOT NULL | |
| scope | TEXT NOT NULL | |
| category | TEXT NOT NULL | |
| key | TEXT NOT NULL | |
| content | TEXT NOT NULL | |
| confidence | REAL NOT NULL | |
| ward_id | TEXT NOT NULL | |
| epistemic_class | TEXT NOT NULL | |
| archived_at | TEXT NOT NULL | |

**Indexes:** `idx_facts_archive_agent`.
**Writes:** `MemoryRepository::archive_fact`.

### `ward_wiki_articles`

**Purpose:** Compiled per-ward wiki articles, unique on
`(ward_id, title)`.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| ward_id | TEXT NOT NULL | |
| agent_id | TEXT NOT NULL | |
| title | TEXT NOT NULL | |
| content | TEXT NOT NULL | Markdown |
| tags | TEXT | JSON array |
| source_fact_ids | TEXT | JSON array |
| version | INTEGER NOT NULL | default 1 |
| created_at | TEXT NOT NULL | |
| updated_at | TEXT NOT NULL | |

**Uniqueness:** `UNIQUE(ward_id, title)`.
**Indexes:** `idx_wiki_ward`.
**Writes/Reads:** `WikiRepository` (`wiki_repository.rs`).

### `procedures`

**Purpose:** Reusable multi-step workflows with success/failure metrics.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| agent_id | TEXT NOT NULL | |
| ward_id | TEXT NOT NULL | default `'__global__'` |
| name | TEXT NOT NULL | |
| description | TEXT NOT NULL | |
| trigger_pattern | TEXT | |
| steps | TEXT NOT NULL | JSON |
| parameters | TEXT | JSON |
| success_count | INTEGER NOT NULL | default 1 |
| failure_count | INTEGER NOT NULL | default 0 |
| avg_duration_ms | INTEGER | |
| avg_token_cost | INTEGER | |
| last_used | TEXT | |
| created_at | TEXT NOT NULL | |
| updated_at | TEXT NOT NULL | |

**Indexes:** `idx_procedures_agent`, `idx_procedures_ward`.
**Writes/Reads:** `ProcedureRepository`.

### `session_episodes`

**Purpose:** One-sentence summary + outcome per completed session.
Searchable via `session_episodes_index`.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| id | TEXT PRIMARY KEY | |
| session_id | TEXT NOT NULL UNIQUE | |
| agent_id | TEXT NOT NULL | |
| ward_id | TEXT | |
| task_summary | TEXT | |
| outcome | TEXT | `'success'` \| `'failure'` \| ... |
| strategy_used | TEXT | |
| key_learnings | TEXT | JSON array |
| token_cost | INTEGER | |
| created_at | TEXT NOT NULL | |

**Indexes:** `idx_session_episodes_agent`, `idx_session_episodes_ward`,
`idx_session_episodes_outcome`.
**Writes:** `EpisodeRepository` (`episode_repository.rs`).

### `embedding_cache`

**Purpose:** Content-addressed cache of text embeddings so repeat
computations skip the encoder.
**DB:** `knowledge.db`

| Column | Type | Notes |
|---|---|---|
| content_hash | TEXT NOT NULL | PK part 1 |
| model | TEXT NOT NULL | PK part 2 |
| embedding | BLOB NOT NULL | f32 vector |
| created_at | TEXT NOT NULL | |

Primary key: `(content_hash, model)`.
**Writes/Reads:** `MemoryRepository::cache_embedding` /
`get_cached_embedding`.

---

## vec0 virtual tables (knowledge.db)

Created after `load_sqlite_vec` + base schema. All 384-dim `FLOAT[384]`,
partner row pk matches base table id.

```sql
CREATE VIRTUAL TABLE kg_name_index        USING vec0(entity_id    TEXT PRIMARY KEY, name_embedding FLOAT[384]);
CREATE VIRTUAL TABLE memory_facts_index   USING vec0(fact_id      TEXT PRIMARY KEY, embedding      FLOAT[384]);
CREATE VIRTUAL TABLE wiki_articles_index  USING vec0(article_id   TEXT PRIMARY KEY, embedding      FLOAT[384]);
CREATE VIRTUAL TABLE procedures_index     USING vec0(procedure_id TEXT PRIMARY KEY, embedding      FLOAT[384]);
CREATE VIRTUAL TABLE session_episodes_index USING vec0(episode_id TEXT PRIMARY KEY, embedding    FLOAT[384]);
```

Kept consistent with their base tables by five `AFTER DELETE` triggers
(`knowledge_schema.rs:315`):
`trg_entities_delete_vec`, `trg_facts_delete_vec`,
`trg_wiki_delete_vec`, `trg_procedures_delete_vec`,
`trg_episodes_delete_vec`.

---

## Structural conventions

### Bitemporal columns

Facts and graph rows carry time metadata so recall can reason about
temporal validity:

- `valid_from`, `valid_until` — when the fact/entity is asserted true.
- `valid_at` (on relationships) — point-in-time assertion.
- `invalidated_at`, `invalidated_by` — when and by whom the row was
  rejected.
- `superseded_by` (facts) — points to the new row that replaces this one.
- `compressed_into` (entities) — set on merge (to the winner id) or
  prune (to the `__pruned__` sentinel).

### Epistemic class

`epistemic_class` on `kg_entities`, `kg_relationships`, `memory_facts`,
`memory_facts_archive`:

- `current` — default; can contradict, decay, be pruned.
- `convention` — stable organizational norm.
- `procedural` — how-to knowledge.
- `archival` — never decays, never pruned. The DecayEngine
  explicitly excludes `epistemic_class = 'archival'`
  (`storage.rs:1581`), and the Compactor's `find_duplicate_candidates`
  filters it out at the SQL level.

### Soft-delete sentinel

`kg_entities.compressed_into = '__pruned__'` marks a soft-deleted
entity. The row is retained for referential integrity with episodes and
distillations that reference the id; its `kg_name_index` vec0 partner is
dropped in the same transaction. Every recall query filters
`compressed_into IS NULL` (or defers to the sentinel value when
explicitly looking for pruned rows).
