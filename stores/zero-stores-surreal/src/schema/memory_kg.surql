-- ===========================================================================
-- memory_kg namespace schema (SurrealDB 3.0).
-- Mirrors stores/zero-stores-sqlite knowledge schema 1:1 (column names, types,
-- defaults, constraints). Source of truth: gateway-database/src/knowledge_schema.rs
-- and gateway-database/migrations/v*.sql.
--
-- HNSW indexes are NOT defined here — they are created lazily by schema/hnsw.rs
-- on first embedding write (per design spec §6).
--
-- All statements use IF NOT EXISTS so the script is fully idempotent across
-- restarts.
-- ===========================================================================

-- --- entity ---------------------------------------------------------------
DEFINE TABLE IF NOT EXISTS entity SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS agent_id        ON entity TYPE string;
DEFINE FIELD IF NOT EXISTS name            ON entity TYPE string;
DEFINE FIELD IF NOT EXISTS entity_type     ON entity TYPE string;
DEFINE FIELD IF NOT EXISTS confidence      ON entity TYPE float DEFAULT 0.8;
DEFINE FIELD IF NOT EXISTS mention_count   ON entity TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS first_seen_at   ON entity TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS last_seen_at    ON entity TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS epistemic_class ON entity TYPE string DEFAULT 'standard';
DEFINE FIELD IF NOT EXISTS compressed_into ON entity TYPE option<string>;
DEFINE FIELD IF NOT EXISTS embedding       ON entity TYPE option<array<float>>;
DEFINE FIELD IF NOT EXISTS metadata        ON entity TYPE option<object>;

DEFINE INDEX IF NOT EXISTS entity_agent_name_type
    ON entity FIELDS agent_id, name, entity_type UNIQUE;
DEFINE INDEX IF NOT EXISTS entity_agent_type
    ON entity FIELDS agent_id, entity_type;
DEFINE INDEX IF NOT EXISTS entity_mention_count
    ON entity FIELDS mention_count;

-- --- entity_alias --------------------------------------------------------
DEFINE TABLE IF NOT EXISTS entity_alias SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS entity_id ON entity_alias TYPE record<entity>;
DEFINE FIELD IF NOT EXISTS surface   ON entity_alias TYPE string;
DEFINE INDEX IF NOT EXISTS alias_surface ON entity_alias FIELDS surface;

-- --- relationship --------------------------------------------------------
DEFINE TABLE IF NOT EXISTS relationship TYPE RELATION FROM entity TO entity SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS agent_id          ON relationship TYPE string;
DEFINE FIELD IF NOT EXISTS relationship_type ON relationship TYPE string;
DEFINE FIELD IF NOT EXISTS confidence        ON relationship TYPE float DEFAULT 0.8;
DEFINE FIELD IF NOT EXISTS mention_count     ON relationship TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS first_seen_at     ON relationship TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS last_seen_at      ON relationship TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS metadata          ON relationship TYPE option<object>;

DEFINE INDEX IF NOT EXISTS rel_agent_type
    ON relationship FIELDS agent_id, relationship_type;

-- --- memory_fact ---------------------------------------------------------
-- memory_fact is SCHEMALESS so the typed MemoryFact shape (with all the
-- SQLite-side columns: scope, key, pinned, epistemic_class, valid_from/until,
-- expires_at, superseded_by, contradicted_by, ward_id, etc.) round-trips
-- without an explicit DEFINE FIELD per column. Trade-off: no schema-level
-- enforcement, gained: full feature parity with SQLite without column-by-column
-- migration churn.
DEFINE TABLE IF NOT EXISTS memory_fact SCHEMALESS;
DEFINE FIELD IF NOT EXISTS agent_id     ON memory_fact TYPE string;
DEFINE FIELD IF NOT EXISTS content      ON memory_fact TYPE string;
DEFINE FIELD IF NOT EXISTS fact_type    ON memory_fact TYPE string;
DEFINE FIELD IF NOT EXISTS confidence   ON memory_fact TYPE float DEFAULT 0.8;
DEFINE FIELD IF NOT EXISTS created_at   ON memory_fact TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS last_used_at ON memory_fact TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS use_count    ON memory_fact TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS archived     ON memory_fact TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS embedding    ON memory_fact TYPE option<array<float>>;
DEFINE FIELD IF NOT EXISTS metadata     ON memory_fact TYPE option<object>;

DEFINE INDEX IF NOT EXISTS fact_agent_type ON memory_fact FIELDS agent_id, fact_type;
DEFINE INDEX IF NOT EXISTS fact_archived   ON memory_fact FIELDS archived;

-- --- wiki_doc (FTS-backed long-form notes) -------------------------------
-- SCHEMALESS so the typed `WikiArticle` shape (tags, source_fact_ids,
-- version, embedding) round-trips without per-column DEFINE FIELDs —
-- mirrors the memory_fact rationale.
DEFINE TABLE IF NOT EXISTS wiki_doc SCHEMALESS;
DEFINE FIELD IF NOT EXISTS agent_id   ON wiki_doc TYPE string;
DEFINE FIELD IF NOT EXISTS title      ON wiki_doc TYPE string;
DEFINE FIELD IF NOT EXISTS content    ON wiki_doc TYPE string;
DEFINE FIELD IF NOT EXISTS ward_id    ON wiki_doc TYPE option<string>;

-- --- analyzers + FTS indexes --------------------------------------------
DEFINE ANALYZER IF NOT EXISTS ascii TOKENIZERS class FILTERS lowercase, ascii;
DEFINE INDEX IF NOT EXISTS entity_name_fts
    ON entity FIELDS name FULLTEXT ANALYZER ascii BM25;
DEFINE INDEX IF NOT EXISTS wiki_doc_title_fts
    ON wiki_doc FIELDS title FULLTEXT ANALYZER ascii BM25;
DEFINE INDEX IF NOT EXISTS wiki_doc_content_fts
    ON wiki_doc FIELDS content FULLTEXT ANALYZER ascii BM25;
DEFINE INDEX IF NOT EXISTS memory_fact_fts
    ON memory_fact FIELDS content FULLTEXT ANALYZER ascii BM25;

-- --- episode (session_episodes parity) ----------------------------------
-- SCHEMALESS so the full SessionEpisode JSON shape (id, session_id, agent_id,
-- ward_id, task_summary, outcome, strategy_used, key_learnings, token_cost,
-- embedding, created_at) round-trips without a per-column DEFINE FIELD —
-- mirrors the memory_fact rationale.
DEFINE TABLE IF NOT EXISTS episode SCHEMALESS;
DEFINE FIELD IF NOT EXISTS session_id ON episode TYPE string;
DEFINE FIELD IF NOT EXISTS agent_id   ON episode TYPE string;
DEFINE FIELD IF NOT EXISTS ward_id    ON episode TYPE string;
DEFINE FIELD IF NOT EXISTS outcome    ON episode TYPE string;
DEFINE FIELD IF NOT EXISTS created_at ON episode TYPE string;
DEFINE INDEX IF NOT EXISTS episode_session_id ON episode FIELDS session_id UNIQUE;
DEFINE INDEX IF NOT EXISTS episode_ward       ON episode FIELDS ward_id;
DEFINE INDEX IF NOT EXISTS episode_agent_ward ON episode FIELDS agent_id, ward_id;

-- --- procedure (procedures parity) --------------------------------------
DEFINE TABLE IF NOT EXISTS procedure SCHEMALESS;
DEFINE FIELD IF NOT EXISTS agent_id ON procedure TYPE string;
DEFINE FIELD IF NOT EXISTS ward_id  ON procedure TYPE option<string>;
DEFINE FIELD IF NOT EXISTS name     ON procedure TYPE string;
DEFINE INDEX IF NOT EXISTS procedure_agent ON procedure FIELDS agent_id;
DEFINE INDEX IF NOT EXISTS procedure_ward  ON procedure FIELDS ward_id;

-- --- goal (kg_goals parity) ----------------------------------------------
DEFINE TABLE IF NOT EXISTS goal SCHEMALESS;
DEFINE FIELD IF NOT EXISTS agent_id ON goal TYPE string;
DEFINE FIELD IF NOT EXISTS state    ON goal TYPE string;
DEFINE INDEX IF NOT EXISTS goal_agent_state ON goal FIELDS agent_id, state;

-- --- recall_log ---------------------------------------------------------
-- (session_id, fact_key) pairs are unique to keep log_recall idempotent.
DEFINE TABLE IF NOT EXISTS recall_log SCHEMALESS;
DEFINE FIELD IF NOT EXISTS session_id  ON recall_log TYPE string;
DEFINE FIELD IF NOT EXISTS fact_key    ON recall_log TYPE string;
DEFINE FIELD IF NOT EXISTS recalled_at ON recall_log TYPE string;
DEFINE INDEX IF NOT EXISTS recall_log_session_key
    ON recall_log FIELDS session_id, fact_key UNIQUE;
DEFINE INDEX IF NOT EXISTS recall_log_session ON recall_log FIELDS session_id;

-- --- distillation_run ---------------------------------------------------
DEFINE TABLE IF NOT EXISTS distillation_run SCHEMALESS;
DEFINE FIELD IF NOT EXISTS session_id ON distillation_run TYPE string;
DEFINE FIELD IF NOT EXISTS status     ON distillation_run TYPE string;
DEFINE INDEX IF NOT EXISTS distillation_session
    ON distillation_run FIELDS session_id UNIQUE;

-- --- meta tracking ------------------------------------------------------
-- zmeta is intentionally SCHEMALESS — it carries arbitrary metadata payloads
-- (schema_version, embedding_config, etc.) that don't fit a single fixed shape.
DEFINE TABLE IF NOT EXISTS zmeta SCHEMALESS;
