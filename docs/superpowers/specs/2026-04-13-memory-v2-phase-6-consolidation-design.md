# Memory v2 — Phase 6: Consolidation

**Date:** 2026-04-13
**Branch target:** `feature/memory-v2-phase-6`
**Preceded by:** Phase 5 (hardening + docs) — shipped as `feature/memory-v2-phase-5`

---

## Executive summary

Phase 6 makes the memory layer **compound**. Today, each session dumps its findings into `memory_facts`, the graph, the wiki. Nothing higher-order emerges automatically. Phase 6 closes that loop: the system detects recurring patterns across sessions and synthesizes them into strategy facts, procedure templates, and cross-session context — all during the existing sleep-time cycle, using infrastructure Phases 1-5 already delivered.

At the same time, Phase 6 retires scaffolding. The three legacy recall methods that Phase 3 shadowed are removed. The `PairwiseVerifier` trait defined in Phase 4 finally gets wired. The JSON-parsing boilerplate duplicated across `LlmExtractor` passes is factored out. No new HTTP endpoints. No new background workers. No new schema (v22 stays).

This is a consolidation phase in the literal sense: *consolidate what the agent has learned, and consolidate the codebase that learned it.*

---

## Research foundation

| Source | Contribution |
|---|---|
| A-MEM (NeurIPS 2025) | Write-time linking — when a new fact arrives, consolidate with semantically-similar existing facts. We extend to session-boundary linking. |
| Graphiti / Zep (arXiv:2501.13956) | Episode-based provenance with community summaries via Leiden clustering. We keep the provenance, swap Leiden for a cheaper "recurring-subgraph" heuristic. |
| MemGPT / Letta sleep-time compute | Heavy consolidation runs in a background task so foreground agent latency is untouched. We reuse the `SleepTimeWorker` cycle Phase 4 shipped. |
| Ebbinghaus / Memory consolidation (hippocampus → cortex) | Inspiration: novel experiences get replayed and generalized. Our analogue: `session_episodes` are "replayed" during sleep-time; generalizations land as `category='strategy'` facts. |
| HTN planning | Procedural patterns recur. Extract them once, reuse many times. Maps to our `procedures` table. |

---

## Architecture — what changes and what doesn't

```
┌───────────────────────── SLEEP-TIME WORKER (Phase 4) ──────────────────────────┐
│                                                                                 │
│   existing ops:    Compactor      DecayEngine      Pruner                       │
│                        │              │              │                          │
│   new ops (6):    Synthesizer    PatternExtractor    ChainLinker                │
│                        │              │              │                          │
│                        ▼              ▼              ▼                          │
│                  memory_facts      procedures    session_episodes              │
│                  (category=         (trigger        (linked by ward             │
│                   strategy)         templates)      + recency)                  │
│                                                                                 │
└─────────────────────────────────────┬───────────────────────────────────────────┘
                                      │
                              audit: kg_compactions  (extended — see §Schema)
```

Every new op operates on tables v22 already has. No schema migration. One column added to `kg_compactions` to carry the operation type beyond merge/prune.

---

## The four components

### 6a. Cross-session strategy synthesis

**Purpose.** Detect entities + relationships that recur across ≥2 sessions. Ask an LLM to extract a generalizable strategy. Persist as a `memory_fact` with `category='strategy'`, provenance pointing at every contributing episode.

**Selection heuristic** (no Leiden, no community detection — just SQL):

```sql
-- Entities with edges from ≥ 2 distinct sessions, in the last 30 days.
SELECT e.id, e.name, e.entity_type, COUNT(DISTINCT ep.session_id) AS n_sessions
FROM kg_entities e
JOIN kg_relationships r ON r.source_entity_id = e.id OR r.target_entity_id = e.id
JOIN kg_episodes ep    ON instr(COALESCE(r.source_episode_ids, ''), ep.id) > 0
WHERE e.epistemic_class != 'archival'
  AND e.compressed_into IS NULL
  AND ep.created_at > datetime('now', '-30 days')
GROUP BY e.id
HAVING n_sessions >= 2
ORDER BY n_sessions DESC, e.mention_count DESC
LIMIT 20;
```

For each candidate, fetch the 1-hop neighborhood + the episode task summaries. Format as synthesis input. Send to LLM. Parse structured output:

```json
{
  "strategy": "short imperative sentence",
  "confidence": 0.0-1.0,
  "key_fact": "one-sentence summary",
  "decision": "synthesize" | "skip"
}
```

Write `memory_facts` row if `decision='synthesize' AND confidence ≥ 0.7`. Key format: `strategy.synthesis.{entity_name}.{hash8}`. `source_episode_ids` = every contributing episode.

**Dedup.** Before insert, cosine-search `memory_facts_index` for content similarity ≥ 0.88 against existing synthesis facts. If found, don't insert new row: bump `mention_count`, append episode ids, update `updated_at`.

**Budget per cycle.** Cap at 10 candidate-entity synthesis calls per sleep cycle. At 1s per LLM call on gpt-4o-mini, that's 10s of LLM time per hour. Negligible.

### 6b. Procedural pattern extraction

**Purpose.** When the same action sequence recurs across ≥2 successful sessions, synthesize it as a `procedures` row that future sessions can match against.

**Signal.** A session transcript's `messages` table contains tool_calls. Successful sessions (from `session_episodes.outcome='success'`) over the last 30 days are the sample.

**Heuristic.** For each pair of successful sessions with similar task_summaries (cosine ≥ 0.82 on their embeddings — we have `session_episodes.embedding`), extract their tool-call sequences. If the sequences match structurally (same 3+ tool names in same order, ignoring arguments), it's a pattern.

```
session A: shell → delegate(research-agent) → delegate(code-agent) → respond
session B: shell → delegate(research-agent) → delegate(code-agent) → respond
                   → MATCH: 4-step pattern
```

**LLM step.** Hand both task summaries + both action sequences to a small LLM with a generalization prompt. Parse:

```json
{
  "name": "snake_case_name",
  "description": "what it accomplishes",
  "trigger_pattern": "free text — when to use",
  "parameters": ["list", "of", "slot", "names"],
  "steps": [
    {"action": "shell", "note": "..."},
    {"action": "delegate", "agent": "research-agent", "task_template": "..."},
    ...
  ]
}
```

Insert via existing `ProcedureRepository::create` (exists from Phase 1b) — if a procedure with the same `name` exists and `success_count ≥ 2`, skip; else insert.

### 6c. Episode chain linking

**Purpose.** When a root session enters a ward with prior session history, inject a brief summary of the last 3 relevant episodes into recall context. The agent picks up where the previous session left off, without the user re-explaining.

**Trigger.** `MemoryRecall::recall_unified` already fires at session start (Phase 3 Task 8). Add a new input — `previous_episodes` — populated before the call if `session.ward_id != NULL`:

```sql
SELECT id, task_summary, outcome, key_learnings, created_at
FROM session_episodes
WHERE ward_id = ?1
  AND outcome IN ('success', 'partial')
  AND created_at > datetime('now', '-14 days')
ORDER BY created_at DESC
LIMIT 3;
```

Adapter: each row becomes a `ScoredItem` with `kind=ItemKind::Episode` (new enum variant), content = formatted summary, score = `1.0 / rank_since_now` (fresh sessions score higher). Enters RRF pool alongside facts/wiki/procedures/graph/goals.

**No new tables. No new endpoint. Pure adapter + one SQL query + one enum variant.**

### 6d. Housekeeping & deduplication

Three consolidations of existing code:

**Retire legacy recall paths.** Phase 3 added shadow calls at three sites. If production logs show `recall_unified` consistently returning meaningful items at all three, remove:
- `MemoryRecall::recall_with_graph`
- `MemoryRecall::recall_for_intent` (already has unified shadow alongside)
- `MemoryRecall::recall_for_delegation_with_graph`

Runner + intent middleware + delegation spawn all switch to `recall_unified` + a new formatter function `format_scored_items(&[ScoredItem]) -> String` that produces the same prompt-context shape.

**Wire the `PairwiseVerifier` trait** (defined Phase 4) into the Compactor. Simple LLM-backed impl:

```rust
pub struct LlmPairwiseVerifier {
    provider_service: Arc<ProviderService>,
}

#[async_trait]
impl PairwiseVerifier for LlmPairwiseVerifier {
    async fn should_merge(&self, a: &Entity, b: &Entity) -> bool {
        // Ask: "Are these two entities the same thing?" with strict JSON output.
        // Conservative default: false on LLM failure.
    }
}
```

Conservative threshold — compactor's 0.92 cosine gate still applies first; LLM is the final check. Wire it in `AppState::new` when the provider service is available.

**Factor out the duplicated JSON-parsing helpers.** Today:
- `ingest/extractor.rs::parse_entities_response`
- `ingest/extractor.rs::parse_relationships_response`
- `distillation.rs::parse_distillation_response`

All three do the same thing: strip code fences, `serde_json::from_str`, extract a field, handle errors. Factor into `gateway_execution::ingest::json_shape::parse_llm_json<T>()`:

```rust
pub fn parse_llm_json<T: DeserializeOwned>(content: &str) -> Result<T, String> {
    let stripped = strip_code_fence(content);
    serde_json::from_str(stripped).map_err(|e| {
        let preview: String = content.chars().take(200).collect();
        format!("parse LLM JSON: {e} (preview: {preview})")
    })
}
```

Every call site uses `parse_llm_json::<EntitiesResponse>(&response.content)?`. Three copies collapse to one.

Same for `strip_code_fence` — currently defined in extractor.rs privately. Promote to module-public in `ingest::json_shape`.

---

## Schema addition (minimal)

Exactly one column extension:

```sql
-- kg_compactions already records merges + prunes. Extend operation vocab
-- to also cover 'synthesize' and 'pattern_extract' — so every sleep-time
-- op has a uniform audit trail.

-- Schema: no change to columns; operation is already TEXT.
-- Documentation: valid operation values are now:
--   'merge' | 'prune' | 'invalidate' | 'synthesize' | 'pattern_extract'
```

That's it. No migration. `CompactionRepository` gains two new methods:

```rust
pub fn record_synthesis(&self, run_id: &str, fact_id: &str, reason: &str) -> Result<String, String>;
pub fn record_pattern(&self, run_id: &str, procedure_id: &str, reason: &str) -> Result<String, String>;
```

Both delegate to the existing INSERT path with different `operation` values.

---

## Components in code

**Created:**
- `gateway/gateway-execution/src/sleep/synthesizer.rs` — 6a logic
- `gateway/gateway-execution/src/sleep/pattern_extractor.rs` — 6b logic
- `gateway/gateway-execution/src/recall/previous_episodes.rs` — 6c adapter
- `gateway/gateway-execution/src/ingest/json_shape.rs` — shared LLM-JSON helpers
- `gateway/gateway-execution/src/sleep/verifier.rs` — `LlmPairwiseVerifier` impl

**Modified:**
- `gateway/gateway-execution/src/sleep/worker.rs` — add Synthesizer + PatternExtractor to the cycle
- `gateway/gateway-execution/src/sleep/compactor.rs` — construct with `Some(verifier)` in non-test paths
- `gateway/gateway-execution/src/recall/mod.rs` — `ItemKind::Episode`, fetch previous episodes at recall start
- `gateway/gateway-execution/src/runner.rs` — swap `recall_with_graph` for `recall_unified` + formatter
- `gateway/gateway-execution/src/middleware/intent_analysis.rs` — retire `recall_for_intent`
- `gateway/gateway-execution/src/delegation/spawn.rs` — retire `recall_for_delegation_with_graph`
- `gateway/gateway-execution/src/ingest/extractor.rs` — use `json_shape::parse_llm_json`
- `gateway/gateway-execution/src/distillation.rs` — use `json_shape::parse_llm_json`
- `gateway/gateway-database/src/compaction_repository.rs` — two new record methods
- `gateway/src/state.rs` — wire verifier into compactor when provider available

**Deleted:**
- `MemoryRecall::recall_with_graph` + its call chain
- `MemoryRecall::recall_for_intent` + its call chain
- `MemoryRecall::recall_for_delegation_with_graph` + its call chain
- Private duplicate `parse_*_response` + `strip_code_fence` in each file

Net change: **+~1200 lines of new capability, ~-700 lines of retired scaffolding**. Codebase ends lighter.

---

## Rollout — seven tasks

| # | Scope | Effort |
|---|---|---|
| 1 | `ingest/json_shape.rs` — shared LLM-JSON helpers. Factor out from `LlmExtractor` + `distillation`. Unit tests on the three shapes we parse. | small |
| 2 | `sleep/verifier.rs` — `LlmPairwiseVerifier` impl. Wire into Compactor from AppState. | small |
| 3 | `sleep/synthesizer.rs` — candidate query + LLM call + dedup + insert + audit. Integration test with seeded multi-session data. | medium |
| 4 | `sleep/pattern_extractor.rs` — similar-task pair detection + tool-sequence matching + LLM generalization + procedure insert. Integration test. | medium |
| 5 | `sleep/worker.rs` — orchestrate Synthesizer + PatternExtractor alongside Compactor + DecayEngine + Pruner. Preserve 60-min cadence. | small |
| 6 | `recall/previous_episodes.rs` + `ItemKind::Episode` + wiring in `recall_unified`. Integration test with seeded prior episodes. | medium |
| 7 | Retire three legacy recall paths + new `format_scored_items` formatter. Update all three call sites to use `recall_unified`. | medium |

Total: ~4-5 days of focused work. Every task ships independently. Worker can run with any subset of the new ops — if synthesizer fails cleanly, compactor and pruner continue.

---

## Testing strategy

**Unit (per-task):**
- `json_shape::parse_llm_json` — malformed, wrapped, empty, missing key
- Synthesizer candidate query returns correct multi-session entities on a seeded DB
- Pattern-extraction signature matching — identical tool sequences score same
- Episode-chain SQL returns last-N successful sessions only

**Integration (end-to-end):**
- 6a: seed 3 sessions that mention the same entity + relationship pattern. Mock the LLM to return a structured strategy. Run Synthesizer. Assert one `memory_facts` row inserted with `category='strategy'` and `source_episode_ids` containing all 3 episode ids.
- 6b: seed 2 successful sessions with identical tool sequences. Run PatternExtractor. Assert one `procedures` row inserted.
- 6c: seed 3 prior successful sessions in a ward. Run `recall_unified`. Assert ScoredItem pool contains 3 items of `ItemKind::Episode`.
- 6d: after migrating one call site, assert recall output matches the legacy path's output for a known query.

**Performance:**
- Synthesizer cycle on a DB with 10k entities, 30k relationships, 100 episodes — should complete in < 15s including LLM calls (capped at 10 per cycle).
- Full sleep-time cycle with all 5 ops active — should complete in < 30s.

**Dogfood:** run the daemon for a week against your real workflow, tail daemon logs for synthesis outputs, verify the strategy facts they produce are actually useful.

---

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Synthesized facts are low-quality / noisy | Confidence gate ≥ 0.7; LLM decision field; cosine dedup against existing; manual promotion via pinned=1 available; strategy facts never override user-pinned facts |
| Pattern extractor generates unusable procedures | Require same tool sequence exactly; success rate ≥ 67% retained in procedure; manual deletion via admin endpoint |
| LLM budget blowout | Cap 10 synthesis calls + 5 pattern-extraction calls per hourly cycle; total ~15 calls/hour = ~360/day on the smallest model |
| Legacy recall retirement breaks a caller we missed | Retire one call site at a time, each with a release cycle; grep-based verification |
| Episode chain injection bloats prompt | Summary is bounded to 150 tokens per episode × 3 episodes = 450 tokens; well within RRF budget |
| `PairwiseVerifier` false-positives merge genuinely different entities | Conservative default (false on LLM failure); cosine threshold still gates; audit log makes manual reversal trivial |
| Duplication elimination in 6d.factor-JSON-parser breaks an LLM edge case | `json_shape::parse_llm_json` keeps the preview-on-error behavior; existing tests assert it |

---

## What Phase 6 explicitly is NOT

- **Not a new HTTP surface.** No `/api/memory/synthesize` or similar. Synthesis is invisible; users see its outputs in recall quality.
- **Not a new table.** v22 schema stays. Only the operation enum in `kg_compactions` gains two string values — that's a documentation change.
- **Not community-detection.** Leiden/PageRank subgraph analysis is out of scope. Our heuristic is "entities with ≥3 edges across ≥2 sessions" — cheap, deterministic, sufficient for personal-agent scale.
- **Not a migration away from SQLite.** If data grows past the size where SQL aggregates become slow, Phase 7 addresses it. Not Phase 6.
- **Not the end of maintenance work.** Architecture SVG, Observatory UI memory tab, reindex-unification, cached stats — remain as separate follow-ups, not blocking.

---

## Acceptance

Phase 6 is done when all of:

1. After a week of real use, `SELECT count(*) FROM memory_facts WHERE category='strategy' AND source_summary LIKE '%synthesis%'` > 0 AND manual inspection of the rows shows they're useful (subjective but required).
2. `SELECT count(*) FROM procedures WHERE source = 'pattern_extraction'` > 0 AND the procedures, if manually applied, would plausibly succeed.
3. `recall_unified` is called from all three call sites; the three legacy methods are deleted; `grep 'recall_with_graph\|recall_for_intent\|recall_for_delegation'` in `src/` returns zero hits outside comments.
4. `parse_llm_json` is called from every JSON-parsing site; three private duplicates are deleted.
5. `Compactor::new` is constructed with `Some(verifier)` in `AppState`; the verifier gets called during at least one sleep-time cycle observed in logs.
6. All benchmarks from Phase 5 still pass their budgets. Full `cargo test --workspace` stays green.
7. Architecture SVG for Memory layer (remains deferred) is the one item explicitly allowed to lag.

---

## Closing note

Phase 6 is when the memory system stops being a warehouse and starts being a teacher. The agent stops rediscovering. Patterns get named. Scaffolding that outlived its purpose comes down.

Professionals ship, but they also prune. This is the prune.

---

## References

- `docs/superpowers/specs/2026-04-12-memory-layer-redesign-design.md` — v22 umbrella
- `docs/memory-v2-performance-baseline.md` — numbers to beat after Phase 6
- `memory-bank/components/memory-layer/overview.md` — layer architecture
- `memory-bank/components/memory-layer/knowledge-graph.md` — resolver + compactor
- Graphiti · Zep · A-MEM · MemGPT — citations in umbrella spec Appendix A
