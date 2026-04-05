# Memory as Brain — Intelligent Memory Layer

## Goal

Transform z-Bot's memory from passive storage into an active cognitive layer that saves tokens, improves accuracy, and learns from every session. Memory becomes the brain — not an afterthought.

## Design Principles

1. **Every recalled fact is a fact the agent doesn't rediscover** — memory saves tokens
2. **Every learned pattern is a workflow the planner doesn't re-plan** — memory saves time
3. **Every remembered failure is tokens NOT wasted retrying** — memory prevents waste
4. **The graph is the associative cortex** — not a log, but a queryable network of knowledge
5. **Accuracy over volume** — 10 verified facts beat 100 hallucinated ones

## Current State (What Works)

- 65 memory facts with hybrid search (FTS5 + vector, tunable weights)
- Knowledge graph with 18 entities, 24 relationships, multi-hop CTE traversal
- Session distillation (LLM extracts facts/entities/relationships after sessions)
- Episodic memory (session outcomes + learnings stored with embeddings)
- Temporal decay, contradiction detection, ward-scoped isolation
- Configurable recall with deep-merge JSON overrides

## Current Gaps

| Gap | Impact |
|-----|--------|
| Graph has duplicate relationships | `financial-analysis --[analyzedby]--> PTON` ×3 |
| Entity name drift | `data_utils.py` vs `core/data_utils.py` → 2 entities for 1 file |
| Intent analysis has no memory access | Every session starts as if user is new |
| Subagents start cold | Read AGENTS.md + ward files = 3-6 tool calls × ~1K tokens each |
| Graph never queried at runtime | 24 relationships sitting idle |
| Mid-session recall not wired | Long sessions lose context |
| Predictive recall data collected but unused | `recall_log` has data, no prediction loop |
| Distillation facts unverified | LLM can hallucinate facts from transcript |

## Architecture: Five Memory Loops

### Loop 1: Intent Analysis + Memory (P0 — highest token savings)

**Current flow**: User message → intent LLM call (no history) → plan from scratch

**New flow**: User message → memory query → enrich intent prompt → plan with context

When `analyze_intent` runs:
1. Embed user message (already done for recall)
2. Hybrid search memory facts → top 5 relevant facts
3. Graph query → entities related to keywords in the message (1-hop neighbors)
4. Episode search → top 3 similar past sessions (task_summary + outcome + strategy)
5. Inject as `<memory_context>` block in the intent analysis prompt

**Intent prompt addition**:
```
<memory_context>
## Recalled Facts
- [correction] Always use duckduckgo-search skill for web research, not raw shell curl
- [strategy] Stock analysis uses: planner→code→data-analyst×3→research pipeline
- [domain] PTON data exists in financial-analysis/stocks/pton/ ward

## Related Entities (from knowledge graph)
- PTON (organization) → analyzed by data-analyst, code-agent
- financial-analysis (project) → contains pton_fundamentals.json, data_utils.py

## Similar Past Sessions
- "Analyze PTON stock" → success, strategy: planner-first with yf-* skills, 6 delegations
</memory_context>
```

**Token savings**: Intent gets context in ~500 tokens. Without it, agents spend 5-10 tool calls rediscovering the same information (~5K-50K tokens).

**Files to modify**:
- `gateway/gateway-execution/src/middleware/intent_analysis.rs` — add memory query before LLM call
- `gateway/gateway-execution/src/recall.rs` — expose a lightweight `recall_for_intent()` that returns formatted context

### Loop 2: Subagent Priming (P0 — eliminates cold starts)

**Current flow**: Subagent spawned → reads AGENTS.md (1 call) → reads ward.md (1 call) → reads core_docs.md (1 call) → reads specs (1 call) → loads skills (N calls)

**New flow**: Subagent spawned with pre-injected context in system prompt

When `spawn_delegated_agent` runs:
1. Query memory for facts relevant to the delegation task (semantic search on task description)
2. Include ward knowledge (already in ward.md)
3. Include corrections for this agent (category: "correction", agent_id match)
4. Include relevant skill names from memory (category: "skill" or fact mentions skill names)
5. Inject as `<primed_context>` in the subagent's system prompt

**System prompt injection**:
```
<primed_context>
## Ward Context
financial-analysis ward. Files: core/data_utils.py, specs/plan.md

## Corrections (MUST follow)
- Use yf-data skill for OHLCV downloads, not raw yfinance API calls
- Save outputs to ward subdirectory, not /tmp

## Recommended Skills
yf-fundamentals (fundamental-field-map.md has the field reference)

## Previous Work in Ward
data-analyst created pton_fundamentals.json (fundamentals data exists)
</primed_context>
```

**Token savings**: Eliminates 3-6 orientation tool calls per subagent. 6 subagents × ~5K tokens each = **~30K tokens saved per session**.

**Files to modify**:
- `gateway/gateway-execution/src/delegation/spawn.rs` — inject recalled context
- `gateway/gateway-execution/src/recall.rs` — add `recall_for_delegation(task, ward_id, agent_id)` method

### Loop 3: Graph-Powered Recall (P1 — already built, needs activation)

**Current**: `recall_with_graph()` exists in recall.rs but isn't called from the main recall path.

**Fix**: Replace the basic `recall()` call with `recall_with_graph()` in the session start flow. This adds:
- Entity extraction from recalled facts → graph neighborhood lookup
- Episode similarity search → past session context
- Graph traversal (2 hops, decay 0.6) to discover related facts

**Files to modify**:
- `gateway/gateway-execution/src/runner.rs` — switch recall call to `recall_with_graph()`

### Loop 4: Predictive Recall (P1 — data exists, logic needs wiring)

**Current**: `recall_log` table tracks which facts were recalled per session. Episode embeddings exist. But the prediction loop isn't connected.

**New flow**: When recalling for a new session:
1. Find top 3 similar past sessions (episode embedding similarity)
2. Get the fact keys that were recalled in those sessions (`recall_log`)
3. Boost scores of those facts by `predictive_boost` (config: 1.3x)

This means: "Last time you did a stock analysis, you needed yf-data and yf-fundamentals corrections. You'll probably need them again."

**Files to modify**:
- `gateway/gateway-execution/src/recall.rs` — add predictive boost step after base scoring

### Loop 5: Mid-Session Injection (P2)

**Current**: Configured in `recall_config` (every 5 turns, min novelty 0.3) but not wired.

**New flow**: In the executor loop, after every N assistant turns:
1. Embed the last few messages
2. Hybrid search for new relevant facts not already in context
3. If novelty score > threshold, inject as a system message: `[Memory Update] New relevant context: ...`

**Files to modify**:
- `runtime/agent-runtime/src/executor.rs` — add mid-session recall check in the iteration loop

## Accuracy Layer

### Fact Verification (during distillation)

**Problem**: LLM extracts facts from transcript but can hallucinate or misattribute.

**Solution**: Two-pass verification:
1. **Extract**: LLM produces candidate facts (current behavior)
2. **Verify**: For each fact, check if it's grounded in tool outputs:
   - Facts about file creation → verify shell/write_file tool outputs contain the file path
   - Facts about data values → verify they appear in tool results
   - Facts about agent behavior → verify delegation events match

**Implementation**: Add a `verify_fact()` function in distillation that cross-references the fact content against tool call results in the transcript. Assign `confidence` based on grounding:
- Grounded in tool output: confidence = LLM's confidence (0.7-0.95)
- Not grounded but plausible: confidence = LLM's confidence × 0.6
- Contradicts tool output: discard

### Entity Normalization (during distillation)

**Problem**: Same entity gets different names → graph fragments.

**Solution**: Before inserting entities:
1. Strip path prefixes (`core/data_utils.py` → `data_utils.py` for matching, keep full path as property)
2. Lowercase comparison for matching
3. Merge aliases: if new entity name has cosine similarity > 0.85 with existing entity, merge (bump mention count, add name as alias in properties)
4. Canonicalize agent names (already stable IDs)

**Implementation**: Add `normalize_entity_name()` and `find_similar_entity()` to the graph storage layer.

### Relationship Dedup (immediate fix)

**Problem**: `store_relationship` does `ON CONFLICT(id)` on UUID — never conflicts. Duplicates inserted on every distillation.

**Fix**: Add unique index on `(source_entity_id, target_entity_id, relationship_type)` and change upsert to conflict on that triple:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_kg_rel_unique
ON kg_relationships(source_entity_id, target_entity_id, relationship_type);
```

```rust
fn store_relationship(...) {
    conn.execute(
        "INSERT INTO kg_relationships (id, agent_id, source_entity_id, target_entity_id,
         relationship_type, properties, first_seen_at, last_seen_at, mention_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(source_entity_id, target_entity_id, relationship_type) DO UPDATE SET
            last_seen_at = excluded.last_seen_at,
            mention_count = mention_count + 1,
            properties = excluded.properties",
        ...
    )
}
```

**Also**: Add a one-time migration to dedup existing relationships:
```sql
DELETE FROM kg_relationships WHERE rowid NOT IN (
    SELECT MIN(rowid) FROM kg_relationships
    GROUP BY source_entity_id, target_entity_id, relationship_type
);
```

## Token Budget Analysis

Based on the PTON analysis session (sess-8962ab0a):

| Component | Current Tokens | With Memory Brain | Savings |
|-----------|---------------|-------------------|---------|
| Root orientation | 20K | 5K (memory-primed intent) | 15K |
| Planner skill loading | 190K | 50K (skills pre-identified) | 140K |
| Data-analyst ×3 cold starts | 150K | 60K (primed with ward context) | 90K |
| Research-agent discovery | 428K | 100K (corrections: use skills not shell) | 328K |
| **Total** | **788K** | **215K** | **~573K (73% reduction)** |

These are estimates, but the direction is clear: memory-primed agents skip the discovery phase entirely.

## Implementation Order

| Phase | What | Token Impact | Complexity |
|-------|------|-------------|------------|
| **Phase 0** | Graph dedup fix (relationship unique index + migration) | Correctness | Low |
| **Phase 1a** | Intent analysis + memory query (Loop 1) | ~50K/session | Medium |
| **Phase 1b** | Subagent priming (Loop 2) | ~30K/session | Low |
| **Phase 2a** | Activate graph-powered recall (Loop 3) | ~20K/session | Low (code exists) |
| **Phase 2b** | Wire predictive recall (Loop 4) | ~10K/session | Low (data exists) |
| **Phase 3** | Mid-session injection (Loop 5) | Variable | Medium |
| **Phase 4** | Fact verification + entity normalization | Accuracy | Medium |

## Files Summary

| File | Changes |
|------|---------|
| `services/knowledge-graph/src/storage.rs` | Relationship dedup unique index, entity normalization |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Memory query before intent LLM call |
| `gateway/gateway-execution/src/recall.rs` | `recall_for_intent()`, `recall_for_delegation()`, predictive boost, switch to `recall_with_graph()` |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Inject primed context into subagent system prompt |
| `gateway/gateway-execution/src/runner.rs` | Switch to graph-powered recall |
| `gateway/gateway-execution/src/distillation.rs` | Fact verification, entity normalization |
| `runtime/agent-runtime/src/executor.rs` | Mid-session recall check (Phase 3) |
| `gateway/gateway-database/src/schema.rs` | Migration: unique index + dedup existing rows |
