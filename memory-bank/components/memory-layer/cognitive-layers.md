# Cognitive Layers — Working Memory, Wiki, Procedures, Micro-Recall

This doc covers the four cognitive capability layers built on top of base memory. Each layer addresses a specific gap in the original passive-storage design.

Grounded in: MemGPT (MemGPT), Karpathy's LLM Wiki, A-MEM (NeurIPS 2025), Graphiti/Zep (2025).

---

## Layer 2 — Working Memory (Phase 2)

### Purpose

A mutable context block that evolves during execution. Unlike base recall which sets context once at session start, working memory updates after every tool result and is re-injected into the system prompt each LLM iteration.

### Problem It Solves

Before Phase 2: the agent reads the recall context on turn 1, then by turn 20 it has learned 15 new things (APIs that failed, errors encountered, entities discovered) — but that knowledge only lives in the conversation history. The next LLM call sees the raw history without any summarization or prioritization.

After Phase 2: the agent maintains a **live summary** of active entities, session discoveries, corrections, and delegation status. The summary is formatted as markdown and injected before each LLM turn.

### Data Structure

```rust
struct WorkingMemory {
    entities: IndexMap<String, WorkingEntity>,   // ordered, dedup by name
    discoveries: Vec<Discovery>,                  // session learnings
    corrections: Vec<String>,                     // active corrections
    delegations: Vec<DelegationSummary>,          // subagent results
    token_budget: usize,                          // default 1500 tokens
}
```

### How It Updates

After every tool result, `WorkingMemoryMiddleware` runs:

| Tool Signal | Working Memory Update |
|------------|----------------------|
| `shell` error | Add to `discoveries` as error with iteration number |
| `delegate_to_agent` completion | Update `delegations` with key findings extracted from result text |
| Any tool result with entity mentions (regex-detected) | `add_entity(name, type, context_snippet)` |
| Error matching known correction | `add_correction()` pulls from memory |

### Budget Management

When `total_tokens > token_budget`:
1. Evict least-recently-referenced entities first
2. Then discoveries older than 20 iterations
3. Evicted entities with meaningful summaries persist to `memory_facts` before eviction

### System Prompt Injection

Working memory renders as:

```markdown
## Working Memory (auto-updated)

### Active Entities
- **yfinance** (module): Python library for stock data. Rate limit: 1 req/sec
- **SPY**: S&P 500 ETF. Current price: $523.40

### Session Discoveries
- API returns paginated results (max 100/page) [iter 5, shell]
- pandas 2.x deprecated .append(), use pd.concat() [iter 7, shell]

### Active Corrections
- Do NOT use matplotlib inline; use plotly for interactive charts

### Delegation Status
- research-agent: completed — found 8 news sources, saved to ward
- writing-agent: running — generating HTML report
```

Injected as a system message updated before each LLM iteration.

### Key Files

- `gateway/gateway-execution/src/invoke/working_memory.rs` — struct + budget logic
- `gateway/gateway-execution/src/invoke/working_memory_middleware.rs` — tool result processor (UTF-8-safe via char-boundary helpers)
- `gateway/gateway-execution/src/runner.rs` — initialization and per-iteration injection

---

## Layer 3 — Ward Wiki (Phase 3)

### Purpose

Apply Karpathy's LLM Wiki pattern to per-ward knowledge: compile raw facts into structured markdown articles once, rather than re-deriving from scratch on every recall.

### Problem It Solves

Before Phase 3: recall scans 500+ flat facts every session, assembling an ad-hoc context. Token cost grows linearly. Similar patterns get rediscovered.

After Phase 3: each ward has a `wiki/` table of compiled articles. A session about "stock analysis" loads 3 pre-compiled articles (`yfinance-patterns`, `rate-limiting-strategies`, `risk-metrics-formulas`) instead of 30 disconnected facts.

### Compilation Flow

```
Session ends
  → Distillation extracts facts
  → compile_ward_wiki(new_facts, existing_articles)
      → LLM: "Given new facts + existing articles, UPDATE or CREATE articles.
              Prefer updating over new-title drift."
      → JSON response: {articles: [{title, content, tags}]}
      → Embedding-based dedup: if new title unique but content ≥0.82 cosine
        similar to existing, merge into existing (reuse title)
  → Upsert to ward_wiki_articles
  → Regenerate __index__ article
```

### Wiki-First Recall

During `recall_with_graph()`, before hybrid fact search:
1. Load ward's `__index__` article
2. Embed user query, search articles by cosine similarity (top 3)
3. Include matched article content (~1500 token budget) in recall result before individual facts

This means session start sees compiled knowledge first, raw facts second.

### Article Format

```json
{
  "id": "wiki-portfolio-analysis-<uuid>",
  "ward_id": "portfolio-analysis",
  "title": "yfinance Rate Limiting Patterns",
  "content": "# yfinance Rate Limiting\n\nyfinance requires 1 req/sec...",
  "tags": ["yfinance", "python", "rate-limiting"],
  "embedding": [...],
  "version": 3
}
```

The `__index__` article is a special record listing all titles with one-line summaries — fits in a single LLM context window.

### Dedup Strategy (Two-Stage)

1. **Title match**: if LLM reuses an exact existing title, UNIQUE(ward_id, title) constraint triggers in-place UPDATE.
2. **Embedding similarity**: if LLM invents a new title for overlapping content (cosine ≥0.82 against existing), system force-merges by reusing the existing title.

### Key Files

- `gateway/gateway-execution/src/ward_wiki.rs` — `compile_ward_wiki()` + prompt + dedup
- `stores/zero-stores-sqlite/src/wiki_repository.rs` — CRUD + vector search
- `gateway/gateway-execution/src/recall.rs` — wiki-first recall integration

---

## Layer 4 — Procedural Memory (Phase 4)

### Purpose

Capture successful multi-step action sequences as reusable procedures. The agent learns HOW to do things, not just WHAT it knows.

### Problem It Solves

Before Phase 4: every stock analysis session re-invents the orchestration (plan → research → code → visualize → HTML → archive). The agent knows "yfinance requires rate limiting" but doesn't remember that the 8-step procedure worked 7 times.

After Phase 4: successful procedures are extracted, stored with success/failure counts, and recalled during intent analysis. Proven procedures become templates the agent can follow or adapt.

### Data Model

```sql
CREATE TABLE procedures (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    ward_id TEXT DEFAULT '__global__',
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    trigger_pattern TEXT,        -- when to use this procedure
    steps TEXT NOT NULL,         -- JSON array of steps
    parameters TEXT,             -- JSON array of parameter names
    success_count INTEGER DEFAULT 1,
    failure_count INTEGER DEFAULT 0,
    avg_duration_ms INTEGER,
    avg_token_cost INTEGER,
    last_used TEXT,
    embedding BLOB,
    ...
);
```

### Step Format

```json
{
  "steps": [
    {"action": "ward", "note": "enter portfolio-analysis ward"},
    {"action": "delegate", "agent": "planner-agent", "task_template": "Plan portfolio dashboard for {tickers}"},
    {"action": "delegate", "agent": "code-agent", "task_template": "Create project structure under task/{project_name}"},
    {"action": "delegate", "agent": "research-agent", "task_template": "Fetch historical prices for {tickers}"},
    {"action": "delegate", "agent": "code-agent", "task_template": "Build core analysis functions"},
    {"action": "delegate", "agent": "code-agent", "task_template": "Generate charts with plotly"},
    {"action": "delegate", "agent": "code-agent", "task_template": "Assemble HTML dashboard"},
    {"action": "respond", "note": "provide dashboard link"}
  ],
  "parameters": ["tickers", "project_name"]
}
```

### Extraction (During Distillation)

Only fires when:
- Session status = `completed`
- Session had ≥2 delegations OR ≥3 distinct tool actions
- LLM's response includes a non-null `procedure` field

The distillation prompt includes a "REQUIRED: procedure extraction" section with a concrete example and positive trigger ("always extract when session had multi-step orchestration"). This was critical: earlier "optional" framing caused Gemini Flash to always return `null`.

### Recall (During Intent Analysis)

In `middleware/intent_analysis.rs`:
1. Embed user message
2. Query `procedures` by cosine similarity
3. If top match score > 0.7 AND `success_count >= 2`:
   - Append to intent prompt: "A proven procedure exists: {name} — {description}. Success rate: {N}%."
4. Intent LLM decides whether to follow or adapt

### Evolution

- Session success following a procedure → `increment_success(id, duration_ms, token_cost)`
- Session failure → `increment_failure(id)`
- Procedures with failure rate >40% after 5+ attempts are flagged/deprecated

### Key Files

- `gateway/gateway-execution/src/distillation.rs` — procedure extraction in LLM response
- `stores/zero-stores-sqlite/src/procedure_repository.rs` — CRUD + embedding search
- `gateway/gateway-execution/src/middleware/intent_analysis.rs` — recall during intent

---

## Layer 5 — Intelligent Micro-Recall (Phase 5)

### Purpose

Targeted, fast memory queries at key decision points during execution. Not full recall — surgical, <100ms lookups injected into working memory.

### Problem It Solves

Before Phase 5: memory consulted only at session start and continuation. Mid-session decisions (which subagent to delegate to, how to recover from an error, what does this newly-mentioned entity mean) happen blind.

After Phase 5: four trigger points fire automatic micro-queries. Results go into working memory, then into the next LLM iteration's context.

### Triggers

| Trigger | Detection | Query | Source | Budget |
|---------|-----------|-------|--------|--------|
| Before `delegate_to_agent` | Tool name = "delegate_to_agent" on success | "procedures + corrections for {agent_id}" | `procedures`, `memory_facts` WHERE agent_id=X | 300 tok |
| Tool error | Result has `error` field | Error message text | `memory_facts` WHERE category='correction' | 200 tok |
| Ward entry | Tool name = "ward" on success | "important for ward {ward_id}" | `ward_wiki_articles` (index article) | 500 tok |
| Entity first mention | Regex-detected entity not already in WM | Entity name | `kg_entities` + 1-hop neighbors | 200 tok |

### Flow

```
Tool result processed
  ↓
micro_recall::detect_triggers(tool_name, result, error, &wm)
  → Vec<MicroRecallTrigger>
  ↓
For each trigger:
  micro_recall::execute_micro_recall(&mut wm, trigger, &ctx, iteration)
  → Adds entity / discovery / correction to working memory
  ↓
Next LLM iteration sees updated WM in system prompt
```

### Implementation Notes

- Synchronous detection in the sync tool-result callback
- Async execution post-callback (avoid blocking the execution loop)
- Deduplication: skip if entity/discovery already present in working memory
- All errors silently ignored — micro-recall is best-effort and must never break execution

### Key Files

- `gateway/gateway-execution/src/invoke/micro_recall.rs` — trigger types, detection, handlers
- `gateway/gateway-execution/src/invoke/working_memory_middleware.rs` — dispatches triggers after tool result

---

## How the Layers Compose

A single iteration now sees context assembled from four sources:

```
╔══════════════════════════════════════════════════════════╗
║               SYSTEM MESSAGE (per iteration)             ║
╠══════════════════════════════════════════════════════════╣
║ [Instructions + intent analysis]  ← from setup           ║
║                                                          ║
║ ## Recalled Context              ← Layer 0: base recall  ║
║ - [correction] Research first...                         ║
║ - [pattern] yfinance rate limit...                       ║
║                                                          ║
║ ## Ward Knowledge Base           ← Layer 3: wiki-first   ║
║ ### yfinance Patterns (v3)                               ║
║ ...                                                      ║
║                                                          ║
║ ## Proven Procedure Available    ← Layer 4: procedures   ║
║ stock_analysis_report (87% success, 12 uses)             ║
║                                                          ║
║ ## Working Memory (auto-updated) ← Layer 2: WM           ║
║ ### Active Entities                                      ║
║ - SPY: S&P 500 ETF, price $523                           ║
║ ### Session Discoveries                                  ║
║ - API returns paginated results [iter 5]                 ║
║ ### Delegation Status                                    ║
║ - research-agent: completed — 8 sources                  ║
╚══════════════════════════════════════════════════════════╝
```

Between iterations, Layer 5 fires micro-recall triggers that augment Layer 2. Every piece of context has a purpose and a provenance trail.
