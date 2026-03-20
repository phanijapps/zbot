# Intent Analysis v2 — Autonomous Middleware Agent

## Problem

The current enrichment middleware is a dumb function that gets spoon-fed arrays of skills, agents, and wards from the runner. It:
- Sends ALL 21 skills to the LLM (token waste, unfocused)
- Doesn't index resources into memory (no semantic search)
- Doesn't understand wards (just lists directory names)
- Has no observability (silent failures)

## Design

Replace the current `analyze_intent(llm, msg, &skills, &agents, &wards)` with an autonomous middleware agent that:
1. Discovers and indexes resources itself using existing infrastructure
2. Semantically searches for relevant resources per user message
3. Sends only top-N matches to the LLM
4. Logs every step with structured `tracing` output

### Infrastructure Used (all existing, no new deps)

| Component | Crate | Purpose |
|---|---|---|
| `MemoryFactStore` | `zero-core` trait, `gateway-database` impl | Index + hybrid search (BM25 + cosine) |
| Local embeddings | `agent-runtime::llm::LocalEmbeddingClient` | fastembed ONNX, offline, `all-MiniLM-L6-v2` |
| `SkillService` | `gateway-services` | List all skills from disk |
| `AgentService` | `gateway-services` | List all agents from disk |
| `VaultPaths` | `gateway-services` | Resolve `wards/` directory path |
| `LlmClient` | `agent-runtime` | The analysis LLM call |

### Function Signature

```rust
pub async fn analyze_intent(
    llm_client: &dyn LlmClient,
    user_message: &str,
    fact_store: &dyn MemoryFactStore,
    skill_service: &SkillService,
    agent_service: &AgentService,
    vault_paths: &SharedVaultPaths,
) -> Result<IntentAnalysis, String>
```

No arrays passed in. The middleware discovers everything itself.

### Flow

```
analyze_intent(llm, message, fact_store, skill_service, agent_service, paths)
  |
  |-- ensure_indexed(fact_store, skill_service, agent_service, paths)
  |     |-- count skills in memory_facts WHERE category='skill'
  |     |-- count skills from SkillService::list()
  |     |-- if mismatch: re-index all skills with embeddings
  |     |-- same for agents
  |     |-- same for wards (scan wards dir, read AGENTS.md)
  |
  |-- search_relevant_resources(fact_store, message)
  |     |-- recall_facts("root", message, limit) WHERE category='skill' -> top skills
  |     |-- recall_facts("root", message, limit) WHERE category='agent' -> top agents
  |     |-- recall_facts("root", message, limit) WHERE category='ward'  -> top wards
  |
  |-- build LLM prompt with only top-N results
  |-- call LLM
  |-- parse JSON response
  |-- return IntentAnalysis
```

### Indexing

**Skills**: `save_fact("root", "skill", "skill:{name}", "{name} | {description} | keywords: {kw1, kw2}", 1.0, None)`

**Agents**: `save_fact("root", "agent", "agent:{name}", "{name} | {description}", 1.0, None)`

**Wards**: For each directory in `{vault}/wards/`:
- Read `AGENTS.md` if it exists, extract first paragraph as purpose
- `save_fact("root", "ward", "ward:{name}", "{name} | {purpose from AGENTS.md}", 1.0, None)`
- Wards without AGENTS.md: `save_fact("root", "ward", "ward:{name}", "{name}", 1.0, None)`

**Staleness check**: Compare `SkillService::list().len()` vs fact count for category. If mismatch, delete stale facts and re-index. Simple count comparison — smarter hashing deferred to later.

### Semantic Search

Uses existing `recall_facts` which does hybrid BM25 + cosine (0.7 vector / 0.3 keyword). The `recall_facts` currently doesn't filter by category. Two options:

**Option A**: Call `recall_facts` once and filter results by category in Rust.
**Option B**: Add a category filter to the recall query.

Option A is simpler and the result set is small. Go with A.

### LLM Prompt

Same prompt as current but the resource lists are now curated (top matches only, not all):

```
### User Request
{message}

### Relevant Skills (matched by semantic search)
- yf-data: Use direct yfinance calls as the data foundation...
- yf-options: Extract and analyze options microstructure...

### Relevant Agents (matched by semantic search)
- research-agent: Conducts web research and synthesizes findings

### Relevant Wards (matched by semantic search)
- financial-analysis: Stock and options analysis workspace with reusable code
- scratch: Default ward for quick tasks
```

### Output Schema (unchanged)

```rust
pub struct IntentAnalysis {
    pub primary_intent: String,
    pub hidden_intents: Vec<String>,
    pub recommended_skills: Vec<String>,
    pub recommended_agents: Vec<String>,
    pub ward_recommendation: WardRecommendation,
    pub execution_strategy: ExecutionStrategy,
    pub rewritten_prompt: String,
}

pub struct WardRecommendation {
    pub action: String,       // "use_existing" | "create_new"
    pub ward_name: String,    // domain-level: "financial-analysis", "math-tutor"
    pub subdirectory: Option<String>, // task-specific: "stocks/lmnd", "trinomials"
    pub reason: String,
}
```

### Logging

Every step emits structured `tracing::info!`:

```
intent_analysis: Starting intent analysis for root session
intent_analysis: Indexing — skills={count} agents={count} wards={count}
intent_analysis: Semantic search — query="{message}" skills_matched={n} agents_matched={n} wards_matched={n}
intent_analysis: LLM call — sending {n} skills, {n} agents, {n} wards
intent_analysis: Result — primary_intent="{intent}" hidden_intents={n} ward="{ward}" approach="{approach}"
intent_analysis: Enrichment injected — {chars} chars added to system prompt
```

Failures: `tracing::warn!` with error detail. Raw LLM response at `tracing::debug!`.

### Runner Integration

In `create_executor()`:

```rust
// Before (spoon-fed):
analyze_intent(llm, msg, &available_skills, &available_agents, &existing_wards)

// After (autonomous):
analyze_intent(llm, msg, fact_store.as_ref(), &skill_service, &agent_service, &paths)
```

The `fact_store` clone we already have (`fact_store_for_indexing`) is passed directly. `skill_service` and `agent_service` are available via `self`. `paths` is `self.paths`.

### File Changes

| File | Change |
|---|---|
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Rewrite: autonomous discovery, indexing, semantic search, logging |
| `gateway/gateway-execution/src/runner.rs` | Simplify: pass services instead of arrays, remove manual indexing block |
| `gateway/gateway-execution/tests/intent_analysis_tests.rs` | Update: mock MemoryFactStore for tests |
| `gateway/gateway-execution/Cargo.toml` | Add `gateway-services` if not already a dep (it is) |
| `gateway/gateway-execution/AGENTS.md` | Update with middleware architecture |
| `runtime/agent-tools/AGENTS.md` | Update to note indexer module is unused (or wire it) |

### What Gets Removed from runner.rs

- The `collect_skills_summary` / `collect_agents_summary` calls are no longer needed for enrichment (still needed for executor initial state)
- The `fact_store_for_indexing` manual indexing loop (30 lines)
- The `existing_wards` filesystem scan (10 lines)
- The `&available_skills, &available_agents` parameters to `analyze_intent`

### Testing

| Test | What it verifies |
|---|---|
| Index skills on first run | Empty fact store triggers indexing, fact count matches skill count |
| Skip indexing when current | Fact count matches, no re-index |
| Re-index on staleness | New skill added, count mismatch triggers re-index |
| Semantic search returns relevant | "analyze LMND" matches yf-* skills, not doc/imagegen |
| Ward AGENTS.md reading | Reads purpose from AGENTS.md, indexes it |
| Ward without AGENTS.md | Indexed with name only |
| Full flow with mock LLM | Index -> search -> LLM -> parse -> inject |
| Logging output | Key tracing events emitted |
