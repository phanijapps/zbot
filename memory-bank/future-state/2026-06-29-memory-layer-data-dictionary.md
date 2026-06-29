# Memory / Knowledge Storage Layer — Data Dictionary

**Generated:** 2026-06-29
**Source DBs (sqlite3):**
- `~/Documents/zbot/data/knowledge.db`  — schema_version `31` (applied 2026-06-22)
- `~/Documents/zbot/data/conversations.db` — schema_version `22`

**Purpose:** authoritative table-by-table reference feeding the architecture-modularization plan. Each row maps a physical table to a target subsystem and names its read/write owners by `crate::module`. Fragmented ownership is the modularization signal.

**Conventions used below**
- **Bi-temporal cols:** `valid_from` / `valid_until` / `superseded_by` (facts, beliefs) and the asymmetric KG variants `valid_at` / `invalidated_at` / `invalidated_by` (relationships) / `invalidated_by` + `compressed_into` (entities).
- **Scoring cols:** `confidence` / `mention_count` / `access_count` (+ `pinned`, `stale`, `success_count`/`failure_count` for procedures).
- **Provenance cols:** `source` / `source_id` / `session_id` / `ward_id` / `agent_id` / `source_episode_id(s)` / `source_fact_ids` / `evidence`.
- **Lifecycle cols:** `status` / `started_at` / `completed_at` / `resolved` / `resolved_at`.

---

## 1. Full Schema Map

### knowledge.db (schema v31)

| User-facing table | Role | Vector / FTS backing |
|---|---|---|
| `schema_version` | migration tracker | — |
| `kg_entities` | KG nodes (named things) | vec0 `kg_name_index` (entity_id, name_embedding FLOAT[1024]) |
| `kg_relationships` | KG edges | — (no vec; queryable via entity FKs) |
| `kg_aliases` | surface-form → entity | — |
| `kg_episodes` | distillation work-queue rows (pending/done/failed) | — |
| `kg_episode_payloads` | episode text body (1:1 with kg_episodes) | — |
| `kg_goals` | agent goal/slot state | — |
| `kg_compactions` | audit log of entity/rel merges | — |
| `kg_causal_edges` | cause→effect links | — |
| `memory_facts` | flat fact store | FTS5 `memory_facts_fts` + vec0 `memory_facts_index` (fact_id, embedding[1024]) |
| `memory_facts_archive` | tombstone archive for superseded facts | — |
| `ward_wiki_articles` | consolidated ward docs | FTS5 `ward_wiki_articles_fts` (trigger-synced) + vec0 `wiki_articles_index` |
| `procedures` | learned procedural patterns | vec0 `procedures_index` (procedure_id, embedding[1024]) |
| `session_episodes` | per-session outcome summaries | vec0 `session_episodes_index` (episode_id, embedding[1024]) |
| `kg_beliefs` | synthesised belief atoms | inline `embedding BLOB` column (no separate vec0 table) |
| `kg_belief_contradictions` | belief-vs-belief conflict records | — |
| `embedding_cache` | content_hash → embedding blob cache | — |
| `skill_index_state` | skill file index bookkeeping | — |

**vec0 shadow tables (group under parent — not user-facing):**
- `kg_name_index`: `kg_name_index_info`, `kg_name_index_chunks`, `kg_name_index_rowids`, `kg_name_index_vector_chunks00`
- `memory_facts_index`: `_info`, `_chunks`, `_rowids`, `_vector_chunks00`
- `wiki_articles_index`: `_info`, `_chunks`, `_rowids`, `_vector_chunks00`
- `procedures_index`: `_info`, `_chunks`, `_rowids`, `_vector_chunks00`
- `session_episodes_index`: `_info`, `_chunks`, `_rowids`, `_vector_chunks00`

**Triggers (info only):** `trg_*_delete_vec` (5 — keep vec0 in sync on parent DELETE), `memory_facts_ai/ad/au` + `ward_wiki_articles_fts_ai/ad/au` (FTS5 sync).

### conversations.db (schema v22)

| Table | Role |
|---|---|
| `schema_version` | migration tracker |
| `sessions` | session lifecycle root |
| `agent_executions` | per-execution row (one per agent invocation in a session) |
| `messages` | chat messages (role/content/tool_calls). **`tool_results` is uniformly NULL in this vault** — outcomes live in `execution_logs`. |
| `execution_logs` | structured log lines (level/category/metadata/duration) |
| `bridge_outbox` | outbound adapter queue (Telegram etc.) |
| `distillation_runs` | per-session distillation outcome tracking |
| `recall_log` | (session_id, fact_key) recall-dedup log — **0 rows in this vault** |
| `artifacts` | per-session file artifacts |

---

## 2. Per-Table Data Dictionary

### knowledge.db

#### `kg_entities`
- **Target subsystem:** Knowledge Graph
- **Purpose:** Named nodes in the agent's knowledge graph (people, tools, concepts).
- **Key cols:** `id` TEXT PK · `agent_id` · `entity_type` · `name` · `normalized_name` · `normalized_hash` · `properties` JSON · `epistemic_class` (default `'current'`) · `confidence` REAL · `mention_count` · `access_count` · `first_seen_at` · `last_seen_at` · `last_accessed_at` · **bi-temporal:** `valid_from` / `valid_until` / `invalidated_by` / `compressed_into` (hierarchical compress) · `source_episode_ids` · `evidence` · **hierarchy (v31):** `layer` INT · `parent_cluster_id`.
- **Vector/FTS:** vec0 `kg_name_index` (name_embedding[1024]).
- **Rows:** 508 · **Most-recent write:** `last_seen_at` = 2026-06-29T11:00:05Z.
- **Write owners (fragmented):**
  - `stores::zbot-stores-sqlite::knowledge_graph` (`stores/zbot-stores-sqlite/src/knowledge_graph.rs:1454`, `:1541`) — canonical upsert/insert.
  - `gateway::gateway-memory::sleep::synthesizer` (`gateway/gateway-memory/src/sleep/synthesizer.rs:508, :549`) — raw INSERT during sleep synthesis.
  - `gateway::gateway-memory::sleep::orphan_archiver` (`orphan_archiver.rs:192`) — re-parenting INSERTs.
  - `gateway::gateway-memory::sleep::hierarchy_builder` (`hierarchy_builder.rs:713, :810`) — aggregate layer-1 INSERTs.
  - `gateway::gateway-memory::sleep::decay` (`decay.rs:497, :720`) — decay writes.
  - `gateway::gateway-execution::sleep::kg_backfill` (`gateway-execution/src/sleep/kg_backfill.rs:376` insert, `:165` UPDATE) — legacy backfill.
- **Read owners:** `stores/zbot-stores-sqlite::knowledge_graph` (`:939`, `:1110`) · `kg::storage` (`stores/zbot-stores-sqlite/src/kg/storage.rs`) · `gateway::gateway-memory::sleep::pruner` (`pruner.rs:153`) · `gateway::gateway-execution::sleep::kg_backfill` (`:138`) · recall adapters in `gateway-memory::src/recall/adapters.rs`.
- **Notes:** 6 distinct write sites across 2 crates. **Highly fragmented.**

#### `kg_relationships`
- **Target subsystem:** Knowledge Graph
- **Purpose:** Typed directed edges between KG entities.
- **Key cols:** `id` PK · `agent_id` · `source_entity_id` · `target_entity_id` · `relationship_type` · `properties` · `epistemic_class` · `confidence` · `mention_count` · `access_count` · `first_seen_at` · `last_seen_at` · **bi-temporal (asymmetric — three pairs):** `valid_at` / `invalidated_at` AND `valid_from` / `valid_until` / `invalidated_by` · `source_episode_ids` · `evidence` · **hierarchy:** `layer` · `is_inter_cluster` · UNIQUE(source,target,type).
- **Vector/FTS:** none.
- **Rows:** 824 · **Most-recent write:** `last_seen_at` = 2026-06-29T11:00:05Z.
- **Write owners (fragmented):**
  - `stores::zbot-stores-sqlite::knowledge_graph` (`knowledge_graph.rs:1630`) — canonical.
  - `stores::zbot-stores-sqlite::knowledge_schema` (`knowledge_schema.rs:1187, :1541, :1681`) — schema-upgrade-time INSERTs (bi-temporal backfill).
  - `gateway::gateway-memory::sleep::synthesizer` (`synthesizer.rs:558`) — raw INSERT.
  - `gateway::gateway-memory::sleep::orphan_archiver` (`orphan_archiver.rs:217`).
  - `gateway::gateway-execution::sleep::kg_backfill` (`kg_backfill.rs:401` insert, `:229` UPDATE).
- **Read owners:** `knowledge_graph.rs:984, :1046, :1117, :1208` (UPDATE on read for access_count) · `knowledge_schema.rs:1146` (point-in-time bi-temporal query).
- **Notes:** The asymmetric bi-temporal column set (`valid_at`/`invalidated_at` + `valid_from`/`valid_until`) is a known schema smell — Phase-3 of the bi-temporal roadmap flagged symmetry cleanup here. 5 write sites.

#### `kg_aliases`
- **Target subsystem:** Knowledge Graph
- **Purpose:** Alternate surface forms resolving to one entity.
- **Key cols:** `id` PK · `entity_id` FK · `surface_form` · `normalized_form` · `source` (provenance) · `confidence` · `first_seen_at` · UNIQUE(normalized_form, entity_id).
- **Vector/FTS:** none.
- **Rows:** 376 · **Most-recent write:** `first_seen_at` = 2026-06-29T04:02:23Z.
- **Write owners:** `stores::zbot-stores-sqlite::kg::storage` (`kg/storage.rs:2733, :2954, :3104` INSERT OR IGNORE) · cleanup at `:2530, :2536, :731`.
- **Read owners:** `kg::storage` (COUNT lookups `:1528, :3706`) · `services::knowledge-graph::resolver` (`services/knowledge-graph/src/resolver.rs:96`).
- **Notes:** Single-owner — clean.

#### `kg_episodes`
- **Target subsystem:** Consolidation / sleep pipeline (work queue)
- **Purpose:** Deduplication + status tracker for KG extraction episodes (one per content_hash+source_type).
- **Key cols:** `id` PK · `source_type` · `source_ref` · `content_hash` · `session_id` (provenance) · `agent_id` · **lifecycle:** `status` (pending/running/done/failed) · `retry_count` · `error` · `started_at` · `completed_at` · UNIQUE(content_hash, source_type).
- **Vector/FTS:** none (text body is in `kg_episode_payloads`).
- **Rows:** 70 · **Most-recent write:** `created_at` = 2026-06-29T02:10:58Z.
- **Write owners:** `stores::zbot-stores-sqlite::kg_episode_repository` (`kg_episode_repository.rs:37, :122` INSERT; `:171, :187, :199, :227` UPDATE lifecycle) — canonical. Also `gateway::gateway-memory::sleep::synthesizer` (`synthesizer.rs:519`) raw INSERT.
- **Read owners:** `kg_episode_repository.rs:71, :86, :101, :139, :158, :215, :268, :280, :296` (queue/stats) · `knowledge_graph.rs:1014, :1066` · `memory_repository.rs:137, :161-163` (dashboard stats).
- **Notes:** Mostly single-owner via `KgEpisodeRepository`; one bypass from synthesizer.

#### `kg_episode_payloads`
- **Target subsystem:** Consolidation / sleep pipeline
- **Purpose:** Body text for each kg_episode (1:1).
- **Key cols:** `episode_id` PK FK · `text` · `created_at`.
- **Rows:** 0 (no payloads written in this vault).
- **Write owner:** `stores::zbot-stores-sqlite::kg_episode_repository` (`:240`).
- **Read owner:** `kg_episode_repository.rs:253`.
- **Notes:** **Empty — dead in this vault.** Code paths exist but unused. Candidate to drop or fold into `kg_episodes`.

#### `kg_goals`
- **Target subsystem:** Working memory (transient agent goal state) — arguably Session/orchestration
- **Purpose:** Hierarchical goal tree with slot-filling state.
- **Key cols:** `id` PK · `agent_id` · `ward_id` (provenance) · `title` · `description` · **lifecycle:** `state` (active/…) · `parent_goal_id` · `slots` · `filled_slots` · `created_at` · `updated_at` · `completed_at`.
- **Vector/FTS:** none.
- **Rows:** 0 · **Most-recent write:** none.
- **Write owner:** `stores::zbot-stores-sqlite::goal_repository` (`goal_repository.rs:25` INSERT, `:74, :93` UPDATE).
- **Read owners:** `goal_repository.rs:56, :108` · `gateway::gateway-memory::recall::mod` (`recall/mod.rs:586` — surfaces as recall source) · `memory_repository.rs:140` (count).
- **Notes:** **Empty — feature appears unused in this vault.** Code intact, repository wired.

#### `kg_compactions`
- **Target subsystem:** Consolidation / sleep pipeline (audit)
- **Purpose:** Append-only audit of every entity/relationship merge.
- **Key cols:** `id` PK · `run_id` · `operation` · `entity_id` · `relationship_id` · `merged_into` · `reason` · `created_at`.
- **Vector/FTS:** none.
- **Rows:** 31 · **Most-recent write:** `created_at` = 2026-06-29T07:48:10Z.
- **Write owners (split):** `stores::zbot-stores-sqlite::compaction_repository` (`compaction_repository.rs:56, :79, :101, :122` — 4 INSERT variants) · `gateway::gateway-execution::sleep::kg_backfill` (`kg_backfill.rs:111`).
- **Read owners:** `compaction_repository.rs:137, :159, :161` · `gateway-execution::sleep::kg_backfill` (`:97, :577`).

#### `kg_causal_edges`
- **Target subsystem:** Knowledge Graph (causal sub-graph)
- **Purpose:** Cause→effect edges between entities.
- **Key cols:** `id` PK · `agent_id` · `cause_entity_id` · `effect_entity_id` · `relationship` · `confidence` · `session_id` (provenance) · `created_at`.
- **Vector/FTS:** none.
- **Rows:** 0.
- **Write/read owner:** `stores::zbot-stores-sqlite::kg::causal` (`kg/causal.rs:49` insert, `:75, :105` reads).
- **Notes:** **Empty — feature unused in this vault.** Single owner, clean code, no data.

#### `memory_facts`
- **Target subsystem:** Flat Facts (primary) — also Semantic once distilled
- **Purpose:** Atomic key→content facts, scoped per agent/ward/scope. The backbone of recall.
- **Key cols:** `id` PK · `session_id` (provenance) · `agent_id` · `scope` · `category` · `key` · `content` · `confidence` · `mention_count` · `source_summary` · `source_episode_id` · `source_ref` · `ward_id` (default `'__global__'`) · `epistemic_class` · `contradicted_by` · **bi-temporal:** `valid_from` / `valid_until` / `superseded_by` · `pinned` · `created_at` · `updated_at` · `expires_at` · UNIQUE(agent_id, scope, ward_id, key).
- **Vector/FTS:** FTS5 `memory_facts_fts` (trigger-synced) + vec0 `memory_facts_index` (embedding[1024]).
- **Rows:** 1211 · **Most-recent write:** `updated_at` = 2026-06-29T11:03:43Z.
- **Write owners (heavily fragmented):**
  - `stores::zbot-stores-sqlite::memory_repository` (`memory_repository.rs:187` INSERT, `:279, :314` DELETE, `:497` supersession UPDATE, `:657` decay UPDATE) — legacy raw-SQL repo.
  - `stores::zbot-stores-sqlite::memory_fact_store` (`GatewayMemoryFactStore`, `memory_fact_store.rs:63`, trait impl at `:146`) — newer trait-backed store.
  - `gateway::gateway-memory::sleep::decay` (`decay.rs:696`) — raw INSERT.
  - `stores::zbot-stores-sqlite::reindex` (`reindex.rs:492`) — backfill INSERT during reindex.
- **Read owners:** `memory_repository.rs:244, :259, :399, :425, :446, :470, :519, :535, :550, :565, :595` (many SELECT shapes) · `memory_fact_store.rs` · recall adapter `gateway-memory::recall::adapters.rs:23` · HTTP `gateway/src/http/memory_search.rs` · micro-recall `gateway-execution::invoke::micro_recall.rs`.
- **Notes:** Two parallel repositories (`MemoryRepository` + `GatewayMemoryFactStore`) plus direct SQL from the sleep pipeline. **Single biggest modularization target.**

#### `memory_facts_archive`
- **Target subsystem:** Flat Facts (tombstone archive)
- **Purpose:** Snapshot of superserseded facts at archive time.
- **Key cols:** subset of `memory_facts` + `archived_at`.
- **Rows:** 0.
- **Write owner:** `stores::zbot-stores-sqlite::memory_repository` (`memory_repository.rs:692`).
- **Notes:** **Empty — archive path unused in this vault.**

#### `ward_wiki_articles`
- **Target subsystem:** Taxonomy / Hierarchy (consolidated docs)
- **Purpose:** Consolidated per-ward wiki articles distilled from facts.
- **Key cols:** `id` PK · `ward_id` · `agent_id` · `title` · `content` · `tags` · `source_fact_ids` (provenance) · `version` · `created_at` · `updated_at` · UNIQUE(ward_id, title).
- **Vector/FTS:** FTS5 `ward_wiki_articles_fts` (trigger-synced) + vec0 `wiki_articles_index`.
- **Rows:** 46 · **Most-recent write:** `updated_at` = 2026-06-29T11:00:30Z.
- **Write/read owner:** `stores::zbot-stores-sqlite::wiki_repository` (`wiki_repository.rs:80` INSERT, `:173` DELETE, reads at `:40, :58, :143, :184, :218`). Newer trait store at `wiki_store.rs`.
- **Notes:** Single-owner. Clean.

#### `procedures`
- **Target subsystem:** Procedural
- **Purpose:** Learned multi-step procedures keyed by trigger pattern.
- **Key cols:** `id` PK · `agent_id` · `ward_id` · `name` · `description` · `trigger_pattern` · `steps` · `parameters` · **scoring:** `success_count` · `failure_count` · `avg_duration_ms` · `avg_token_cost` · `last_used` · `created_at` · `updated_at`.
- **Vector/FTS:** vec0 `procedures_index`.
- **Rows:** 215 · **Most-recent write:** `updated_at` = 2026-06-29T11:00:05Z.
- **Write owners:** `stores::zbot-stores-sqlite::procedure_repository` (`procedure_repository.rs:49` INSERT, `:229, :313` UPDATE) — legacy. Newer trait store `GatewayProcedureStore` at `procedure_store.rs:18`. Read store at `procedure_store.rs:105, :138`.
- **Read owners:** `procedure_repository.rs:89, :114, :128, :148, :192, :270` · recall adapter `gateway-memory::recall::adapters.rs:151` · HTTP `gateway/src/http/memory_search.rs`.
- **Notes:** Two-store split (legacy repo + trait store). Moderate fragmentation.

#### `session_episodes`
- **Target subsystem:** Episodic
- **Purpose:** One row per session: task_summary, outcome, strategy, learnings, token cost.
- **Key cols:** `id` PK · `session_id` UNIQUE · `agent_id` · `ward_id` (provenance) · `task_summary` · `outcome` · `strategy_used` · `key_learnings` · `token_cost` · `created_at`.
- **Vector/FTS:** vec0 `session_episodes_index`.
- **Rows:** 109 · **Most-recent write:** `created_at` = 2026-06-29T04:02:22Z.
- **Write owners (fragmented):** `stores::zbot-stores-sqlite::episode_repository` (`episode_repository.rs:78` INSERT) — canonical. Also `gateway::gateway-memory::sleep::synthesizer` (`synthesizer.rs:534`) and `gateway::gateway-memory::sleep::pattern_extractor` (`pattern_extractor.rs:638`) raw INSERTs.
- **Read owners:** `episode_repository.rs` (many) · `episode_store.rs:123, :158, :168` · recall adapter `gateway-memory::recall::previous_episodes.rs:63`.

#### `kg_beliefs`
- **Target subsystem:** Semantic (synthesised) — "Belief Network" phase of reflective memory roadmap
- **Purpose:** Synthesised belief atoms derived from facts; bi-temporal + supersession.
- **Key cols:** `id` PK · `partition_id` (generic; ward-agnostic by design) · `subject` · `content` · `confidence` · **bi-temporal:** `valid_from` / `valid_until` / `superseded_by` · `source_fact_ids` (provenance) · `synthesizer_version` · `reasoning` · `created_at` · `updated_at` · `stale` · **inline vector:** `embedding` BLOB · UNIQUE(partition_id, subject, valid_from).
- **Vector/FTS:** inline BLOB (no vec0 table).
- **Rows:** 348 · **Most-recent write:** `updated_at` = 2026-06-29T07:48:43Z.
- **Write owner:** `stores::zbot-stores-sqlite::belief_store` (`belief_store.rs:114` INSERT, `:166, :182, :198, :271` UPDATEs).
- **Read owners:** `belief_store.rs:53, :78, :213, :230, :253, :301` · recall adapter `gateway-memory::recall::adapters.rs:75` · HTTP `gateway/src/http/beliefs.rs`, `gateway/src/http/belief_network.rs`.
- **Notes:** Single store owner. Cleanest of the higher-order subsystems.

#### `kg_belief_contradictions`
- **Target subsystem:** Consolidation / sleep pipeline (conflict detection)
- **Purpose:** Pairs of beliefs judged to contradict; resolution tracking.
- **Key cols:** `id` PK · `belief_a_id` · `belief_b_id` · `contradiction_type` · `severity` · `judge_reasoning` · `detected_at` · **lifecycle:** `resolved_at` · `resolution` · UNIQUE(a,b).
- **Vector/FTS:** none.
- **Rows:** 18 · **Most-recent write:** `detected_at` = 2026-06-29T05:49:54Z.
- **Write/read owner:** `stores::zbot-stores-sqlite::belief_contradiction_store` (`belief_contradiction_store.rs:104, :133, :157, :178, :194`).
- **Producer:** `gateway::gateway-memory::sleep::belief_contradiction_detector` (the judge).

#### `embedding_cache`
- **Target subsystem:** Embedding cache
- **Purpose:** content_hash+model → embedding blob; avoids re-embedding identical text.
- **Key cols:** `content_hash` · `model` · `embedding` BLOB · `created_at` · PK(content_hash, model).
- **Rows:** 1407 · **Most-recent write:** `created_at` = 2026-06-29T11:00:05Z.
- **Write/read owner:** `stores::zbot-stores-sqlite::memory_repository` (`memory_repository.rs:1144` read, `:1170` insert-or-replace).
- **Notes:** Wired through `MemoryRepository` rather than its own store — surprising owner for a cross-cutting cache.

#### `skill_index_state`
- **Target subsystem:** Index / FTS shadow (skill discovery bookkeeping)
- **Purpose:** Tracks which SKILL.md files have been indexed and their mtime/size.
- **Key cols:** `name` PK · `source_root` (vault|agent) · `file_path` · `mtime_unix` · `size_bytes` · `last_indexed_unix` · `format_version`.
- **Rows:** 13 · **Most-recent write:** `last_indexed_unix` = 1782097084.
- **Write/read owner:** `stores::zbot-stores-sqlite::memory_repository` (`memory_repository.rs:332, :360, :384`).

### conversations.db (memory-related tables only)

#### `recall_log`
- **Target subsystem:** Index / recall-dedup shadow
- **Purpose:** Tracks which fact_keys have already been recalled in a session (dedup).
- **Key cols:** `session_id` · `fact_key` · `recalled_at` · PK(session_id, fact_key).
- **Rows:** 0.
- **Write owner:** `stores::zbot-stores-sqlite::recall_log_repository` (`recall_log_repository.rs:31` INSERT OR IGNORE).
- **Read owner:** `recall_log_repository.rs:42, :65`.
- **Delete owner:** `services::execution-state::repository` (`services/execution-state/src/repository.rs:618`).
- **Notes:** **Empty.** Repository exists (`RecallLogRepository`, exported `lib.rs:72`), but **no runtime code path writes to it** — only the repository itself and archiver deletes. Effectively dead at runtime. Strong candidate to either wire up or drop.

#### `distillation_runs`
- **Target subsystem:** Consolidation / sleep pipeline (per-session status)
- **Purpose:** One row per session recording distillation outcome (facts/entities/relationships extracted).
- **Key cols:** `id` PK · `session_id` UNIQUE · **lifecycle:** `status` (success/failed/skipped/…) · `facts_extracted` · `entities_extracted` · `relationships_extracted` · `episode_created` · `error` · `retry_count` · `duration_ms` · `created_at`.
- **Rows:** 120 · status spread: success=107, failed=9, skipped=4.
- **Write owners (split):** `gateway::gateway-execution::archiver` (`archiver.rs:453` INSERT) and `gateway::gateway-execution::distillation` (calls repo at success, `distillation.rs:746`). Repo: `stores::zbot-stores-sqlite::distillation_repository` (`:64` insert, `:123, :143` updates).
- **Read owners:** `distillation_repository.rs:99, :163, :223` · archiver gating `archiver.rs:242` (joins to sessions for archive eligibility) · `services::execution-state::repository.rs:610` (cascade delete).
- **Notes:** Split between archiver (writes a placeholder row) and distillation pipeline (writes the real outcome). Both go through `DistillationRepository`.

#### `agent_executions` (memory-relevant slice)
- **Target subsystem:** Session / orchestration
- **Rows:** 136 · status: completed=112, crashed=24.
- **Write owners:** `gateway::gateway-execution::archiver` (`archiver.rs:592`) · `stores::zbot-stores-sqlite::repository` (`repository.rs:446`) · `services::api-logs::repository` (`:954`) · `services::execution-state::repository` (`:754, :909, :916, :921, :933, :950`).
- **Notes:** Execution state is owned by `services::execution-state`; gateway + api-logs also write. Fragmented across 3 services.

#### `messages`
- **Target subsystem:** Session / orchestration (raw transcript)
- **Rows:** 5260 · **tool_results is NULL for all 5260 rows** — outcomes are in `execution_logs`.
- **Write owners:** `gateway::gateway-execution::archiver` (`archiver.rs:606`) · `gateway::gateway-memory::sleep::pattern_extractor` (`pattern_extractor.rs:670`) · `stores::zbot-stores-sqlite::repository` (`repository.rs:74`).
- **Read owners:** `stores::zbot-stores-sqlite::repository::get_messages` (`:94`) · `gateway::gateway-execution::archiver` (`:71`) · distillation transcript builder `gateway::gateway-execution::distillation::build_transcript` (`:1950`) · HTTP `gateway/src/http/conversations.rs:84`, `chat.rs:185`.
- **Notes:** `tool_results` column is dead in practice. Candidate to drop in a rewrite.

#### `execution_logs`
- **Target subsystem:** Session / orchestration (structured logs — where tool outcomes actually live)
- **Rows:** 3286.
- **Write owners:** `gateway::gateway-execution::archiver` (`archiver.rs:619`) · `services::api-logs::repository` (`:47, :80`).
- **Read owners:** `services::api-logs::repository` (`:163, :287, :347, :383, :395`) · `services::execution-state::repository` (`:689` for session-aggregate reads).
- **Notes:** The de-facto tool-outcome store. Owned by `services::api-logs`.

#### `bridge_outbox` (tangential — not memory, listed for completeness)
- **Rows:** 0 in this vault.
- **Owner:** `gateway::gateway-bridge::outbox` (full lifecycle). Cascade-deleted by `services::execution-state`.

#### `sessions` (root, listed for completeness)
- **Rows:** 136 · status: completed=121, crashed=11, running=4.
- **Write owners:** `gateway::gateway-execution::archiver` (archive flag) · `gateway::gateway-memory::sleep::pattern_extractor` (test inserts) · `services::execution-state`.
- **Read owners:** many (HTTP, websocket, server recovery).

#### `artifacts` (tangential)
- **Rows:** 137.
- **Owner:** `services::execution-state::repository` (`:1315` insert, `:1343, :1375` reads).

---

## 3. Subsystem Rollup

| Target subsystem | Tables | Owner today | Fragmented? |
|---|---|---|---|
| **Working memory** | `kg_goals` | `goal_repository` | No (but empty) |
| **Episodic** | `session_episodes` | `episode_repository` + `episode_store` (split) + 2 sleep raw-INSERT sites | **Yes — 4 sites** |
| **Semantic (facts)** | `memory_facts`, `memory_facts_archive` | `memory_repository` (legacy) + `GatewayMemoryFactStore` (trait) + `decay` + `reindex` | **Yes — 4 sites, two parallel repos** |
| **Semantic (synthesised beliefs)** | `kg_beliefs`, `kg_belief_contradictions` | `belief_store` + `belief_contradiction_store` + `belief_contradiction_detector` (producer) | No — single store each |
| **Procedural** | `procedures` | `procedure_repository` (legacy) + `GatewayProcedureStore` (trait) | **Yes — two parallel repos** |
| **Flat Facts** | (covered under Semantic above) | — | — |
| **Taxonomy / Hierarchy** | `ward_wiki_articles`, KG `layer`/`parent_cluster_id` cols, `is_inter_cluster` | `wiki_repository` + `wiki_store` + `hierarchy_builder` | Partial — wiki clean, hierarchy writes raw |
| **Vector** | `kg_name_index`, `memory_facts_index`, `wiki_articles_index`, `procedures_index`, `session_episodes_index`, inline `kg_beliefs.embedding`, `embedding_cache` | vec0 shadows via `SqliteVecIndex` loader; cache via `memory_repository` | **Yes — cache mis-located** |
| **Knowledge Graph** | `kg_entities`, `kg_relationships`, `kg_aliases`, `kg_causal_edges` | `knowledge_graph` + `kg::storage` (split) + 5 sleep raw-INSERT sites + `kg_backfill` | **Yes — 7+ write sites** |
| **Consolidation / sleep pipeline** | `kg_episodes`, `kg_episode_payloads`, `kg_compactions`, `distillation_runs`, `kg_belief_contradictions` (producer side) | dedicated repos per table (`kg_episode_repository`, `compaction_repository`, `distillation_repository`) + `gateway-memory/src/sleep/*` raw writes | **Yes — repos exist but bypassed by pipeline** |
| **Embedding cache** | `embedding_cache` | `memory_repository` (no dedicated store) | Mis-placed (single owner, wrong module) |
| **Index / FTS shadow** | `memory_facts_fts`, `ward_wiki_articles_fts`, `skill_index_state`, `recall_log` | triggers (FTS) + `memory_repository` (skill_index_state) + `recall_log_repository` (recall_log) | Mixed — recall_log dead |
| **Session / orchestration** (conversations.db) | `sessions`, `agent_executions`, `messages`, `execution_logs`, `bridge_outbox`, `artifacts` | `services::execution-state` + `services::api-logs` + `gateway::gateway-execution::archiver` + `stores::repository::ConversationRepository` | **Yes — 4 owners** |

### Read/write separation status
- **Cleanly separated:** `kg_aliases`, `kg_beliefs`, `kg_belief_contradictions`, `ward_wiki_articles`, `embedding_cache`, `kg_episodes` (mostly), `goal_repository`, `kg::causal`.
- **Entangled (read paths also mutate):** `kg_relationships` (access_count UPDATE on read at `knowledge_graph.rs:1208`), `memory_facts` (access-count updates implicit in some recall paths), `procedures` (last_used updated on use).

---

## 4. Cross-DB Memory Tables in conversations.db — Detail

| Table | Purpose | Cols | Owners | Status |
|---|---|---|---|---|
| `recall_log` | per-session fact recall dedup | session_id, fact_key, recalled_at | write: `recall_log_repository.rs:31`; read: `:42, :65`; delete: `execution-state::repository.rs:618` | **DEAD — 0 rows; no runtime writer outside the repo itself** |
| `distillation_runs` | per-session distillation outcome | id, session_id UNIQUE, status, facts/entities/relationships_extracted, episode_created, error, retry_count, duration_ms, created_at | write: `archiver.rs:453` (placeholder) + `distillation_repository.rs:64, :123, :143` (real); read: `distillation_repository.rs:99, :163, :223`, `archiver.rs:242` | Active, 120 rows, split writer |
| `agent_executions` | per-execution lifecycle | id, session_id, agent_id, parent_execution_id, delegation_type, task, status, started_at, completed_at, tokens_in/out, checkpoint, error, log_path, child_session_id | write: `execution-state`, `archiver`, `api-logs`, `repository.rs:446`; read: many | Active, 136 rows |
| `messages` | raw chat transcript | id, execution_id, session_id, role, content, created_at, token_count, tool_calls, **tool_results (uniformly NULL)**, tool_call_id | write: `archiver.rs:606`, `repository.rs:74`, `pattern_extractor.rs:670`; read: `repository.rs:94`, archiver, distillation transcript, HTTP | Active, 5260 rows; **`tool_results` column unused** |
| `execution_logs` | structured logs (real tool-outcome home) | id, session_id, conversation_id, agent_id, parent_session_id, timestamp, level, category, message, metadata, duration_ms | write: `api-logs::repository:47, :80`, `archiver.rs:619`; read: `api-logs`, `execution-state::repository:689` | Active, 3286 rows |

---

## 5. Dead / Empty / Misused Tables (rewrite candidates)

| Table | DB | Rows | Verdict |
|---|---|---|---|
| `kg_goals` | knowledge | 0 | Code intact, wired, unused — feature dormant. |
| `kg_causal_edges` | knowledge | 0 | Single-owner code, no data — dormant. |
| `kg_episode_payloads` | knowledge | 0 | Payload body never written; episode text must live elsewhere or is dropped. |
| `memory_facts_archive` | knowledge | 0 | Archive path never triggered in this vault. |
| `recall_log` | conversations | 0 | **Dead at runtime** — repo + archiver-delete exist but no producer. |
| `bridge_outbox` | conversations | 0 | Telegram/bridge not configured in this vault (not dead code-side). |
| `messages.tool_results` | conversations | 5260 (all NULL) | **Column is dead** — outcomes live in `execution_logs`. |

---

## 6. Bi-Temporal / Vector / Scoring Coverage

| Subsystem | Bi-temporal | Vector | Scoring | Provenance | Lifecycle |
|---|---|---|---|---|---|
| Flat Facts (`memory_facts`) | full (valid_from/until/superseded_by) | vec0 `memory_facts_index` + FTS | confidence, mention_count, pinned | session_id, source_episode_id, source_ref, ward_id | expires_at, epistemic_class |
| Facts archive | (snapshot only) | — | — | — | archived_at |
| KG entities | partial (valid_from/until + invalidated_by + **compressed_into**) | vec0 `kg_name_index` | confidence, mention_count, access_count | source_episode_ids, evidence, agent_id | epistemic_class |
| KG relationships | **asymmetric — 5 cols (valid_at/invalidated_at AND valid_from/until/invalidated_by)** | none | confidence, mention_count, access_count | source_episode_ids, evidence | epistemic_class |
| KG aliases | none | none | confidence | source | first_seen_at |
| KG causal edges | none | none | confidence | session_id | created_at |
| Beliefs (`kg_beliefs`) | full (valid_from/until/superseded_by) | inline BLOB (no vec0) | confidence, stale | source_fact_ids, partition_id | synthesizer_version |
| Belief contradictions | none | none | severity | — | resolved_at, resolution |
| Procedures | none | vec0 `procedures_index` | success/failure count, avg_duration, avg_token_cost | agent_id, ward_id | last_used |
| Session episodes | none | vec0 `session_episodes_index` | token_cost | session_id, agent_id, ward_id | outcome |
| Ward wiki articles | none | vec0 `wiki_articles_index` + FTS | version | source_fact_ids, ward_id | created/updated_at |
| Goals | none | none | — | ward_id | state, completed_at |
| Embedding cache | none | (is the cache) | — | — | created_at |

### Coverage gaps to flag for the modularization plan
- **No bi-temporal:** procedures, session_episodes, ward_wiki_articles, goals, aliases, causal_edges, beliefs' contradictions.
- **No vector index:** kg_relationships, kg_aliases, kg_belief_contradictions (relationships are navigated via entity FKs only).
- **Asymmetric bi-temporal on `kg_relationships`** (two overlapping column sets) — known schema debt from the bi-temporal rollout; flagged for symmetry cleanup.
- **Inline BLOB embedding on `kg_beliefs`** rather than a vec0 table — inconsistent with every other embedding-backed table.
- **`embedding_cache` owned by `memory_repository`** rather than a dedicated cache module — crosses subsystem boundaries.
