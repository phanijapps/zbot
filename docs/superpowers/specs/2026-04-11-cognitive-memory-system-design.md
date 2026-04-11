# Cognitive Memory System — Design Specification

**Date**: 2026-04-11
**Status**: Draft
**Branch**: `feature/cognitive-memory`

## 1. Problem Statement

AgentZero's memory system has three organs that don't communicate effectively:

1. **Memory facts** — flat key-value store with embeddings. No inter-fact structure.
2. **Knowledge graph** — entities and relationships. Written post-hoc via distillation, barely read during execution.
3. **Wards** — workspace containers with files. No semantic bridge to memory.

The agent treats memory as a filing cabinet it checks at session start and forgets about. Context is assembled once and never actively managed. Subagents arrive with minimal knowledge. The knowledge graph is underutilized — no agent tool, no mid-execution queries, no temporal reasoning.

### Research Foundation

This design is grounded in:

- **Karpathy's Context Engineering** (2025): "The delicate art and science of filling the context window with just the right information for the next step." Not prompt engineering — context engineering.
- **Karpathy's LLM Wiki** (2026): Compile knowledge once (raw to wiki), don't re-derive on every query. Three layers: raw (source material) to wiki (compiled articles) to index (fits in one context window).
- **MAGMA** (2026, arXiv:2601.03236): Multi-graph memory with orthogonal semantic, temporal, causal, and entity views. Query-adaptive traversal. 18-45% improvement over baselines.
- **Zep/Graphiti** (2025, arXiv:2501.13956): Temporal knowledge graphs with bitemporal tracking (event time vs ingestion time). 90% latency reduction vs MemGPT. Fact supersession and evolution.
- **A-MEM** (NeurIPS 2025, arXiv:2502.12110): Self-organizing Zettelkasten-style memory. Dynamic indexing and linking. Memories connect to form knowledge networks with higher-order attributes.
- **MemGPT/Letta** (2023, arXiv:2310.08560): Two-tier virtual context management. Agent manages its own context window like an OS manages virtual memory.

## 2. Goals

Transform AgentZero's memory from **passive storage** (write after session, read at start) to **active cognition** (continuous read/write, compiled knowledge, self-organizing structure).

### Success Criteria

- Agents can query the knowledge graph directly during execution
- Subagents arrive with graph-enriched context, not just flat facts
- Context evolves during execution via working memory (not static after session start)
- Ward knowledge compounds across sessions via compiled wiki articles
- Proven action sequences are captured and reused as procedures
- Memory is consulted at every key decision point, not just session boundaries

## 3. Architecture Overview

Five independently deployable phases:

```
Phase 1: Graph Query Tool + Graph-Enriched Delegation     (foundation)
Phase 2: Working Memory (live context management)          (highest impact)
Phase 3: Ward Knowledge Compilation (Karpathy pattern)     (compounding value)
Phase 4: Procedural Memory (learning HOW)                  (strategy evolution)
Phase 5: Intelligent Micro-Recall (context engineering)    (polish)
```

### Pipeline Evolution

**Current pipeline (session start only)**:

```
Session Start -> Recall (5 facts + shallow graph) -> Intent Analysis -> Execute -> Distill
```

**Target pipeline (continuous cognition)**:

```
Session Start -> Recall (wiki + facts + graph + procedures)
                 -> Intent Analysis (with procedure matching)
                 -> Execute
                    -> Working Memory updates each iteration
                    -> Micro-recall at tool calls, errors, delegation, entity mentions
                    -> Graph queries by agent on demand
                 -> Distill
                    -> Compile ward wiki
                    -> Extract procedures
                    -> Update temporal facts
```

## 4. Data Model

### 4.1 Modified Table: `memory_facts`

Three new columns (additive, no migration risk):

```sql
ALTER TABLE memory_facts ADD COLUMN valid_from TEXT;
ALTER TABLE memory_facts ADD COLUMN valid_until TEXT;
ALTER TABLE memory_facts ADD COLUMN superseded_by TEXT;
```

| Column | Type | Default | Purpose |
|--------|------|---------|---------|
| `valid_from` | TEXT (ISO 8601) | NULL (legacy compat) | When this fact became true |
| `valid_until` | TEXT (ISO 8601) | NULL (still valid) | When superseded by a newer fact |
| `superseded_by` | TEXT | NULL | FK to the newer fact's ID |

**Index**: `CREATE INDEX idx_facts_temporal ON memory_facts(valid_from, valid_until);`

### 4.2 New Table: `kg_causal_edges`

```sql
CREATE TABLE kg_causal_edges (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    cause_entity_id TEXT NOT NULL,
    effect_entity_id TEXT NOT NULL,
    relationship TEXT NOT NULL,
    confidence REAL DEFAULT 0.7,
    session_id TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (cause_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (effect_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
);
CREATE INDEX idx_causal_cause ON kg_causal_edges(cause_entity_id);
CREATE INDEX idx_causal_effect ON kg_causal_edges(effect_entity_id);
```

| Column | Purpose |
|--------|---------|
| `relationship` | One of: `causes`, `prevents`, `requires`, `enables` |
| `confidence` | Extraction confidence (0.0-1.0) |
| `session_id` | Provenance: which session produced this edge |

### 4.3 New Table: `procedures`

```sql
CREATE TABLE procedures (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    ward_id TEXT DEFAULT '__global__',
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    trigger_pattern TEXT,
    steps TEXT NOT NULL,
    parameters TEXT,
    success_count INTEGER DEFAULT 1,
    failure_count INTEGER DEFAULT 0,
    avg_duration_ms INTEGER,
    avg_token_cost INTEGER,
    last_used TEXT,
    embedding BLOB,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_procedures_agent ON procedures(agent_id);
CREATE INDEX idx_procedures_ward ON procedures(ward_id);
```

| Column | Type | Purpose |
|--------|------|---------|
| `steps` | TEXT (JSON) | Array of `{ action, agent?, task_template?, args?, note? }` |
| `parameters` | TEXT (JSON) | Array of parameter names that vary across invocations |
| `trigger_pattern` | TEXT | Human-readable description of when this procedure applies |
| `success_count` | INTEGER | Times this procedure led to successful completion |
| `failure_count` | INTEGER | Times this procedure led to failure |

**Step JSON format**:

```json
[
  {"action": "ward", "args": {"name": "stock-analysis"}, "note": "always work in ward"},
  {"action": "delegate", "agent": "research-agent", "task_template": "fetch {ticker} data"},
  {"action": "delegate", "agent": "writing-agent", "task_template": "create HTML dashboard"},
  {"action": "respond", "format": "markdown", "include_artifacts": true}
]
```

### 4.4 New Table: `ward_wiki_articles`

```sql
CREATE TABLE ward_wiki_articles (
    id TEXT PRIMARY KEY,
    ward_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT,
    source_fact_ids TEXT,
    embedding BLOB,
    version INTEGER DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(ward_id, title)
);
CREATE INDEX idx_wiki_ward ON ward_wiki_articles(ward_id);
```

| Column | Type | Purpose |
|--------|------|---------|
| `title` | TEXT | Article topic (e.g., "yfinance-patterns", "rate-limiting-strategies") |
| `content` | TEXT | Compiled markdown article |
| `tags` | TEXT (JSON) | Array of topic tags for filtering |
| `source_fact_ids` | TEXT (JSON) | Array of fact IDs that contributed to this article |
| `version` | INTEGER | Incremented on each recompilation |

The special article with `title = '__index__'` contains the master index (all titles + one-line summaries).

### 4.5 Schema Version

Increment `schema_version` from current value. All migrations are additive.

## 5. Phase 1: Graph as First-Class Citizen

### 5.1 Graph Query Tool

**New file**: `runtime/agent-tools/src/tools/graph_query.rs`

**Tool name**: `graph_query`

**Registration**: Both root and delegated agents (same as `memory` tool). Added in `gateway/gateway-execution/src/invoke/executor.rs` tool registration block.

**Schema**:

```json
{
  "name": "graph_query",
  "description": "Query the knowledge graph to explore entities and their relationships",
  "parameters": {
    "action": {
      "type": "string",
      "enum": ["search", "neighbors", "context"],
      "description": "search: find entities by name/type. neighbors: get connected entities. context: get everything relevant to a topic."
    },
    "query": {
      "type": "string",
      "description": "Search query or entity name"
    },
    "entity_type": {
      "type": "string",
      "description": "Filter by entity type (optional)"
    },
    "direction": {
      "type": "string",
      "enum": ["outgoing", "incoming", "both"],
      "default": "both",
      "description": "Direction for neighbor traversal"
    },
    "depth": {
      "type": "integer",
      "default": 1,
      "maximum": 2,
      "description": "Traversal depth for neighbors"
    },
    "limit": {
      "type": "integer",
      "default": 10,
      "maximum": 25
    }
  },
  "required": ["action", "query"]
}
```

**Action implementations**:

| Action | Implementation | Data Source |
|--------|---------------|-------------|
| `search` | `graph_storage.search_entities(query, entity_type, limit)` | `kg_entities` (LIKE search) |
| `neighbors` | `graph_storage.get_neighbors(entity_id, direction)` then limit | `kg_entities` + `kg_relationships` |
| `context` | Embed query, find entities by name similarity, return subgraph | `kg_entities` + `kg_relationships` + `kg_causal_edges` |

**Output format** (returned to agent):

```
## Entities Found
- **yfinance** (module): provides quote_data, analyzes stocks. Mentions: 12
- **rate_limiting** (pattern): prevents API bans. Mentions: 8

## Relationships
- yfinance --requires--> rate_limiting
- yfinance --provides--> quote_data

## Causal Links
- rate_limiting --prevents--> api_ban
```

**Dependencies**: Requires `GraphStorage` (already available via `knowledge-graph` service). The tool receives graph_storage via `ToolContext` state, same pattern as the `memory` tool receiving `MemoryFactStore`.

### 5.2 Graph-Enriched Delegation

**Modified file**: `gateway/gateway-execution/src/delegation/spawn.rs` (~line 320)

**Current flow**:
```rust
recall.recall_for_delegation(&child_agent_id, &task, ward_id, 8)
```

**New flow**:
```rust
recall.recall_for_delegation_with_graph(&child_agent_id, &task, ward_id, 8, &graph_storage)
```

**New function**: `recall_for_delegation_with_graph()` in `gateway/gateway-execution/src/recall.rs`

1. Call existing `recall_for_delegation()` — returns 8 categorized facts
2. Extract entity names from delegation task (simple tokenization: capitalized words, quoted strings)
3. For each entity name: `graph_storage.get_entity_by_name(name)`
4. For each found entity: `graph_storage.get_neighbors(entity_id, Both)` — 1-hop
5. Format as `## Related Knowledge Graph Context` section
6. Token budget: graph context capped at 500 tokens (truncate lowest-mention entities first)
7. Append to delegation context after the existing categorized facts

### 5.3 Temporal Fact Management

**Modified file**: `gateway/gateway-execution/src/distillation.rs`

During fact upsert (existing `upsert_facts` flow):

1. Set `valid_from = session.started_at` on new facts
2. Before upsert: check if a fact with the same `(agent_id, scope, ward_id, key)` already exists
3. If exists AND content differs:
   - Set old fact's `valid_until = NOW()`, `superseded_by = new_fact.id`
   - Insert new fact with `valid_from = NOW()`
4. If exists AND content is same: bump `mention_count` only (no supersession)

**Modified file**: `gateway/gateway-execution/src/recall.rs`

In scoring logic:

```rust
// Superseded facts get a 0.3x penalty (still retrievable for history)
if fact.valid_until.is_some() {
    score *= 0.3;
}
```

## 6. Phase 2: Working Memory

### 6.1 Data Structure

**New file**: `gateway/gateway-execution/src/invoke/working_memory.rs`

```rust
use indexmap::IndexMap;

pub struct WorkingMemory {
    entities: IndexMap<String, WorkingEntity>,
    discoveries: Vec<Discovery>,
    corrections: Vec<String>,
    delegation_context: Vec<DelegationSummary>,
    token_budget: usize,
    total_tokens: usize,
}

pub struct WorkingEntity {
    name: String,
    entity_type: Option<String>,
    summary: String,
    last_referenced_iteration: u32,
}

pub struct Discovery {
    content: String,
    iteration: u32,
    source: String,
}

pub struct DelegationSummary {
    agent_id: String,
    task_summary: String,
    key_findings: Vec<String>,
    status: String,
}
```

**Public API**:

| Method | Purpose |
|--------|---------|
| `new(token_budget: usize) -> Self` | Create with budget (default: 1500 tokens) |
| `seed_from_recall(recall_result: &RecallResult)` | Initialize from session-start recall |
| `add_entity(name, entity_type, summary)` | Add/update entity |
| `add_discovery(content, iteration, source)` | Record a session learning |
| `add_correction(correction)` | Record an active correction |
| `update_delegation(agent_id, status, findings)` | Update delegation state |
| `evict_if_over_budget()` | Remove LRU entities until within budget |
| `format_for_prompt() -> String` | Render as markdown for system prompt injection |
| `token_count() -> usize` | Estimated token count (chars / 4) |

### 6.2 Working Memory Middleware

**New file**: `gateway/gateway-execution/src/invoke/working_memory_middleware.rs`

Implements a callback that runs after each tool result is processed in `spawn_execution_task()`.

**Integration point**: `gateway/gateway-execution/src/runner.rs`, inside `spawn_execution_task()` (~line 917), in the stream event processing loop. After `ToolResult` events are processed:

```rust
// After tool result processing
if let Some(ref mut wm) = working_memory {
    wm.process_tool_result(&tool_name, &tool_result, current_iteration);
}
```

**Processing logic per tool type**:

| Tool Result | Extraction | Working Memory Update |
|-------------|------------|----------------------|
| `shell` error | Error message text | `add_discovery("Shell error: {msg}", iteration, "shell")` |
| `delegate_to_agent` started | Agent ID, task | `update_delegation(agent_id, "running", [])` |
| Delegation completed | Result text | `update_delegation(agent_id, "completed", extract_key_lines(result))` |
| Any tool mentioning entities | Regex: capitalized multi-word, quoted strings | `add_entity(name, None, context_snippet)` |
| Tool error matching known correction | Memory fact search (category=correction) | `add_correction(matched_correction)` |

**Entity extraction regex** (lightweight, no LLM):
```rust
// Match: PascalCase words, "quoted strings", ALLCAPS acronyms
static ENTITY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:"([^"]+)"|([A-Z][a-z]+(?:\s+[A-Z][a-z]+)+)|([A-Z]{2,}))"#).unwrap()
});
```

### 6.3 System Prompt Injection

**Modified file**: `gateway/gateway-execution/src/runner.rs`, inside the execution loop

Before each LLM call, working memory is formatted and injected as the last system message in history:

```rust
// Update working memory system message before each LLM call
if let Some(ref wm) = working_memory {
    let wm_content = wm.format_for_prompt();
    // Replace or append working memory system message
    update_working_memory_message(&mut history, &wm_content);
}
```

**Format output**:

```markdown
## Working Memory (auto-updated)

### Active Entities
- **yfinance** (module): Python library for stock data. Rate limit: 1 req/sec.
- **SPY**: S&P 500 ETF. Current price: $523.40.

### Session Discoveries
- API returns paginated results (max 100 per page) [iteration 5]
- pandas 2.x deprecated .append(), use pd.concat() [iteration 7]

### Active Corrections
- Do NOT use matplotlib inline; use plotly for interactive charts

### Delegation Status
- research-agent: completed — found 8 news sources, saved to ward
- writing-agent: running — generating HTML report
```

### 6.4 Budget Management

When `total_tokens > token_budget`:

1. Sort entities by `last_referenced_iteration` ascending (LRU)
2. Remove entities one at a time until under budget
3. Evicted entities with meaningful summaries are saved to `memory_facts` via `memory_repo.upsert_fact()` (if not already persisted)
4. Discoveries older than 20 iterations are also candidates for eviction

## 7. Phase 3: Ward Knowledge Compilation

### 7.1 Compilation Step

**New file**: `gateway/gateway-execution/src/ward_wiki.rs`

**Entry point**: `compile_ward_wiki(ward_id, agent_id, new_facts, ...)` — called after distillation completes, inside the same fire-and-forget task.

**Modified file**: `gateway/gateway-execution/src/runner.rs` (~line 1101, distillation spawn)

```rust
// After distillation
tokio::spawn(async move {
    if let Err(e) = distiller.distill(&sid, &aid).await {
        tracing::warn!("Session distillation failed: {}", e);
    }
    // NEW: compile ward wiki if ward_id is set
    if let Some(ref wid) = ward_id {
        if let Err(e) = ward_wiki::compile_ward_wiki(wid, &aid, &wiki_repo, &llm_client).await {
            tracing::warn!("Ward wiki compilation failed: {}", e);
        }
    }
});
```

**Compilation flow**:

1. Load existing articles for this ward: `wiki_repo.list_articles(ward_id)`
2. Load new facts from this session's distillation: `memory_repo.get_facts_by_session(session_id)`
3. If no new facts: skip compilation
4. Build compilation prompt:

```
You are a knowledge compiler. Given new facts from a session and existing wiki articles,
produce updated or new articles. Each article covers ONE topic.

## Existing Articles
{for each article: "### {title}\n{content}"}

## New Facts From This Session
{for each fact: "- [{category}] {key}: {content}"}

## Instructions
- Update existing articles if new facts are relevant to them
- Create new articles for topics not covered by existing ones
- Each article: title, 100-500 word content, list of tags
- Note contradictions between new and existing knowledge
- Keep articles factual and concise

Respond with JSON:
{
  "articles": [
    {"title": "...", "content": "...", "tags": ["..."], "is_new": true/false}
  ]
}
```

5. Parse response, upsert articles to `ward_wiki_articles`
6. Regenerate `__index__` article: list of all titles with one-line summaries

**LLM config**: Same provider/model as distillation. Temperature: 0.3. Max tokens: 4096.

### 7.2 Wiki-First Recall

**Modified file**: `gateway/gateway-execution/src/recall.rs` — inside `recall_with_graph()`

Before the existing hybrid fact search:

1. Check if ward_id is set
2. Load `__index__` article: `wiki_repo.get_article(ward_id, "__index__")`
3. Embed user query, search `ward_wiki_articles` by embedding similarity: `wiki_repo.search_articles(ward_id, query_embedding, limit=3)`
4. Include matched article content in the recall result
5. Token budget allocation: wiki articles get up to 1500 tokens. Remaining budget (1500 tokens from `max_recall_tokens=3000`) goes to individual facts.

### 7.3 Wiki Repository

**New file**: `gateway/gateway-database/src/wiki_repository.rs`

```rust
pub struct WikiRepository<D: DatabaseProvider> {
    db: Arc<D>,
}

impl<D: DatabaseProvider> WikiRepository<D> {
    pub fn list_articles(&self, ward_id: &str) -> Result<Vec<WikiArticle>, String>;
    pub fn get_article(&self, ward_id: &str, title: &str) -> Result<Option<WikiArticle>, String>;
    pub fn upsert_article(&self, article: &WikiArticle) -> Result<(), String>;
    pub fn search_articles(&self, ward_id: &str, embedding: &[f32], limit: usize) -> Result<Vec<WikiArticle>, String>;
    pub fn delete_article(&self, ward_id: &str, title: &str) -> Result<bool, String>;
}
```

## 8. Phase 4: Procedural Memory

### 8.1 Procedure Extraction

**Modified file**: `gateway/gateway-execution/src/distillation.rs`

After existing fact/entity extraction, add procedure extraction step:

1. **Gate**: Only for successful root sessions (`session.status == "completed"`, no error executions)
2. **Gate**: Only if session had >= 2 tool calls (trivial sessions don't have procedures worth saving)

**Extended distillation prompt** (appended to existing prompt):

```
## Procedure Extraction

If this session followed a multi-step approach that could be reused, extract it:

{
  "procedure": {
    "name": "short_name",
    "description": "what this procedure does",
    "steps": [
      {"action": "ward|delegate|shell|respond", "agent": "agent-id", "task_template": "...", "note": "..."}
    ],
    "parameters": ["param1", "param2"],
    "trigger_pattern": "when to use this procedure"
  }
}

If the session was too simple or too unique to be a reusable procedure, set "procedure": null.
```

3. If procedure extracted:
   - Embed `procedure.description`
   - Search existing procedures by embedding similarity (threshold: 0.85)
   - If similar exists: merge — increment `success_count`, update `avg_duration_ms`, update steps if the new variation is better (more successful)
   - If no match: insert new procedure

### 8.2 Procedure Repository

**New file**: `gateway/gateway-database/src/procedure_repository.rs`

```rust
pub struct ProcedureRepository<D: DatabaseProvider> {
    db: Arc<D>,
}

impl<D: DatabaseProvider> ProcedureRepository<D> {
    pub fn upsert_procedure(&self, procedure: &Procedure) -> Result<(), String>;
    pub fn search_procedures(&self, embedding: &[f32], agent_id: &str, limit: usize) -> Result<Vec<ScoredProcedure>, String>;
    pub fn get_procedure(&self, id: &str) -> Result<Option<Procedure>, String>;
    pub fn increment_success(&self, id: &str, duration_ms: i64, token_cost: i64) -> Result<(), String>;
    pub fn increment_failure(&self, id: &str) -> Result<(), String>;
    pub fn list_procedures(&self, agent_id: &str, ward_id: Option<&str>) -> Result<Vec<Procedure>, String>;
}
```

### 8.3 Intent Analysis Integration

**Modified file**: `gateway/gateway-execution/src/middleware/intent_analysis.rs`

In `analyze_intent()`, before calling the LLM:

1. Embed user message
2. Search procedures: `procedure_repo.search_procedures(embedding, agent_id, 3)`
3. If top match score > 0.7 AND `success_count >= 2`:
   - Append to intent analysis prompt: `## Proven Procedure Available\n{procedure.name}: {procedure.description}\nSteps: {procedure.steps}\nSuccess rate: {success_count}/{success_count + failure_count}`
4. The intent analysis LLM can choose to recommend following the procedure or not

### 8.4 Procedure Evolution

**Post-session update** (in distillation):

- On successful session that followed a procedure: `procedure_repo.increment_success(id, duration, tokens)`
- On failed session that followed a procedure: `procedure_repo.increment_failure(id)`
- **Deprecation**: Procedures with `failure_count / (success_count + failure_count) > 0.4` AND `success_count + failure_count >= 5` are excluded from recall results (filter in `search_procedures`)

## 9. Phase 5: Intelligent Micro-Recall

### 9.1 Event-Driven Recall Points

**Modified file**: `gateway/gateway-execution/src/invoke/working_memory_middleware.rs`

Extend the working memory middleware (Phase 2) with targeted recall queries:

| Trigger | Detection | Query | Source | Budget |
|---------|-----------|-------|--------|--------|
| Before `delegate_to_agent` | Tool name = "delegate_to_agent" | `"best approach for {agent_id} on {task}"` | `procedures` + `memory_facts` | 300 tokens |
| Tool error | Tool result contains error flag | Error message text | `memory_facts` WHERE category='correction' | 200 tokens |
| Ward entry | Tool name = "ward", action = "enter" | `"important for ward {ward_id}"` | `ward_wiki_articles` | 500 tokens |
| Entity first mention | Entity detected in tool result, not in working memory | Entity name | `kg_entities` + 1-hop neighbors | 200 tokens |

### 9.2 Implementation

Each micro-recall is a method on `WorkingMemory`:

```rust
impl WorkingMemory {
    /// Fast, targeted recall for a specific decision point
    pub async fn micro_recall(
        &mut self,
        trigger: MicroRecallTrigger,
        memory_repo: &MemoryRepository,
        graph_storage: Option<&GraphStorage>,
        wiki_repo: Option<&WikiRepository>,
        embedding_client: &dyn EmbeddingClient,
    ) -> Result<(), String> {
        match trigger {
            MicroRecallTrigger::PreDelegation { agent_id, task } => { ... }
            MicroRecallTrigger::ToolError { error_message } => { ... }
            MicroRecallTrigger::WardEntry { ward_id } => { ... }
            MicroRecallTrigger::EntityMention { entity_name } => { ... }
        }
    }
}

pub enum MicroRecallTrigger {
    PreDelegation { agent_id: String, task: String },
    ToolError { error_message: String },
    WardEntry { ward_id: String },
    EntityMention { entity_name: String },
}
```

### 9.3 Deduplication

Before adding micro-recalled facts to working memory:

1. Check if fact key already exists in `working_memory.entities` or `working_memory.discoveries`
2. If duplicate: skip (don't re-add)
3. If new: add to working memory, which will appear in next iteration's context

### 9.4 Interaction with Existing mid_session_recall

The existing `MidSessionRecallConfig` (`every_n_turns: 5`, `min_novelty_score: 0.3`) remains unchanged. It performs a broader recall sweep. Micro-recalls are complementary: smaller, faster, targeted at specific decision points. They do not replace mid-session recall.

## 10. Non-Functional Requirements

### 10.1 Rust Backend Quality

| Requirement | Standard | Enforcement |
|-------------|----------|-------------|
| Formatting | `cargo fmt --all --check` | Pre-commit hook, CI (`security.yaml`) |
| Linting | `cargo clippy --all-targets -- -D warnings` | Pre-commit hook, CI |
| Cognitive complexity | < 15 per function | SonarQube scan, code review |
| No `unwrap()` in production | Use `?`, `unwrap_or`, `unwrap_or_else`, `unwrap_or_default` | Clippy lint, code review |
| Error handling | `Result<T, String>` consistent with codebase | No silent failures, meaningful `map_err` |
| Nesting | Maximum 4 levels of nested functions | SonarQube S2004 |

### 10.2 Rust Unit Tests

Every new public function must have unit tests. Test file co-located or in `tests/` module.

**Phase 1 tests**:
- `graph_query.rs`: Test `search`, `neighbors`, `context` actions with in-memory SQLite. Test error cases (entity not found, empty graph). Test output formatting.
- `recall.rs`: Test `recall_for_delegation_with_graph()` — mock graph_storage, verify graph context appended and token-budgeted.
- `distillation.rs`: Test temporal fact supersession — verify `valid_until` and `superseded_by` set correctly on conflict.

**Phase 2 tests**:
- `working_memory.rs`: Test `add_entity`, `add_discovery`, `add_correction`, `evict_if_over_budget` (budget enforcement, LRU ordering). Test `format_for_prompt` output format. Test `seed_from_recall` initialization. Test `token_count` estimation.
- `working_memory_middleware.rs`: Test entity extraction regex. Test tool-specific processing (shell error, delegation completion). Test deduplication.

**Phase 3 tests**:
- `ward_wiki.rs`: Test compilation prompt construction. Test article upsert (new vs update, version increment). Test index regeneration.
- `wiki_repository.rs`: Test CRUD operations, `search_articles` by embedding, `UNIQUE(ward_id, title)` constraint.

**Phase 4 tests**:
- `distillation.rs` (procedure extraction): Test procedure parsing from LLM response. Test similarity merge (threshold 0.85). Test deprecation logic (`failure_rate > 0.4`).
- `procedure_repository.rs`: Test CRUD, `search_procedures` by embedding, `increment_success/failure`.
- `intent_analysis.rs`: Test procedure injection into prompt (score > 0.7, success_count >= 2 gate).

**Phase 5 tests**:
- `working_memory.rs`: Test `micro_recall` for each trigger type. Test deduplication (skip already-known facts). Test token budget per trigger.

### 10.3 TypeScript/UI Quality

| Requirement | Standard | Enforcement |
|-------------|----------|-------------|
| Linting | `npm run lint` — 0 new errors | CI |
| Build | `npm run build` — clean | CI |
| Accessibility | `role`, `tabIndex`, `onKeyDown` on interactive elements | ESLint a11y rules |
| Cognitive complexity | < 15 per function | SonarQube |
| Number parsing | `Number.parseInt()` over `parseInt()` | ESLint |

### 10.4 TypeScript/UI Unit Tests

Framework: Vitest + React Testing Library (existing test setup).

**Phase 2 UI tests** (if working memory gets a UI display):
- Render test: working memory panel shows entities, discoveries, corrections
- Empty state: panel shows placeholder when no working memory data
- Update test: new entity appears when event received

**All phases — event handler tests**:
- Test any new WS event types added to `mission-hooks.ts` switch statement
- Test `fast-chat-hooks.ts` if new events apply to fast mode

### 10.5 Performance Requirements

| Metric | Target | Measurement |
|--------|--------|-------------|
| Micro-recall latency | < 100ms per trigger | Local embeddings (all-MiniLM-L6-v2) + indexed SQLite queries |
| Working memory update | < 50ms per tool result | Regex extraction + IndexMap operations |
| Ward wiki compilation | < 30s total | Async, non-blocking (fire-and-forget after distillation) |
| Graph query tool | < 200ms for 2-hop traversal | Indexed FK lookups, recursive CTE |
| Working memory overhead | < 64KB per session | IndexMap + Vec with token budget cap |
| Procedure search | < 150ms | Embedding similarity on indexed table |

### 10.6 Database Requirements

| Requirement | Standard |
|-------------|----------|
| Migrations | Additive only: new columns (with defaults), new tables. No destructive changes to existing tables. |
| Indexes | Every FK column and frequent query column indexed |
| Schema version | Increment `schema_version` value in migration |
| Backward compatibility | Existing sessions must work without new columns populated (NULL defaults) |

### 10.7 CI Pipeline Compatibility

All changes must pass:
- `security.yaml`: `cargo fmt --all --check`, `cargo clippy`, `cargo audit`, `npm audit`, `gitleaks`
- `sonarqube.yml`: Coverage scan on push to main
- Local: `cargo test --workspace`, `cd apps/ui && npm run build && npm run lint`

## 11. Files Modified/Created Per Phase

### Phase 1

| Action | File |
|--------|------|
| CREATE | `runtime/agent-tools/src/tools/graph_query.rs` |
| MODIFY | `runtime/agent-tools/src/tools/mod.rs` (register tool) |
| MODIFY | `gateway/gateway-execution/src/invoke/executor.rs` (add tool to registry) |
| MODIFY | `gateway/gateway-execution/src/delegation/spawn.rs` (graph-enriched delegation) |
| MODIFY | `gateway/gateway-execution/src/recall.rs` (add `recall_for_delegation_with_graph`, temporal scoring) |
| MODIFY | `gateway/gateway-execution/src/distillation.rs` (temporal fact columns, causal edges) |
| MODIFY | `gateway/gateway-database/src/schema.rs` (migration: new columns, new table) |
| MODIFY | `services/knowledge-graph/src/storage.rs` (causal edge CRUD) |

### Phase 2

| Action | File |
|--------|------|
| CREATE | `gateway/gateway-execution/src/invoke/working_memory.rs` |
| CREATE | `gateway/gateway-execution/src/invoke/working_memory_middleware.rs` |
| MODIFY | `gateway/gateway-execution/src/invoke/mod.rs` (export new modules) |
| MODIFY | `gateway/gateway-execution/src/runner.rs` (initialize WM, inject into loop) |

### Phase 3

| Action | File |
|--------|------|
| CREATE | `gateway/gateway-execution/src/ward_wiki.rs` |
| CREATE | `gateway/gateway-database/src/wiki_repository.rs` |
| MODIFY | `gateway/gateway-database/src/mod.rs` (export wiki repo) |
| MODIFY | `gateway/gateway-database/src/schema.rs` (migration: new table) |
| MODIFY | `gateway/gateway-execution/src/recall.rs` (wiki-first recall) |
| MODIFY | `gateway/gateway-execution/src/runner.rs` (call compile after distillation) |

### Phase 4

| Action | File |
|--------|------|
| CREATE | `gateway/gateway-database/src/procedure_repository.rs` |
| MODIFY | `gateway/gateway-database/src/mod.rs` (export procedure repo) |
| MODIFY | `gateway/gateway-database/src/schema.rs` (migration: new table) |
| MODIFY | `gateway/gateway-execution/src/distillation.rs` (procedure extraction) |
| MODIFY | `gateway/gateway-execution/src/middleware/intent_analysis.rs` (procedure recall) |

### Phase 5

| Action | File |
|--------|------|
| MODIFY | `gateway/gateway-execution/src/invoke/working_memory.rs` (add micro_recall) |
| MODIFY | `gateway/gateway-execution/src/invoke/working_memory_middleware.rs` (trigger detection) |

## 12. Out of Scope

- **UI for knowledge graph visualization** — existing Observatory page is sufficient
- **UI for ward wiki browsing** — can be added later, not required for backend value
- **Cross-ward wiki synthesis** — future enhancement after single-ward compilation is proven
- **Real-time graph updates during execution** (without LLM) — Phase 1 focuses on read access; write improvements come via enhanced distillation
- **Procedure editing UI** — procedures are managed automatically; manual editing is future work
- **Multi-agent graph** (shared graph across different agent IDs) — current per-agent scoping is maintained
