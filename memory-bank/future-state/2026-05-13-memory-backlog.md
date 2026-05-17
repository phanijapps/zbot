# Memory Subsystem Backlog

**Purpose:** Track deferred work for the memory subsystem after Phases 1–4 foundation shipped. None of these items is scheduled — each has a concrete trigger that decides when it's worth picking up.

**Last updated:** 2026-05-13. Branch: `feat/parallel-delegation-aggregation`.

---

## MEM-001 — Phase 4b: Contradiction propagation + recall traversal weighting

**Status:**
- **Part A — Contradiction propagation from facts to KG nodes:** ✅ Done — shipped on `feat/mem-001-contradiction-propagation` (PR pending). See tracking doc Phase 5 row for the trait methods + engine wiring.
- **Part B — Recall traversal weighting by confidence:** Re-filed as new scope (see below). Original premise no longer holds.

**Severity:** Medium (only when symptom appears)
**Trigger:** Either (a) you observe the agent acting on schemas that have been contradicted in newer memory facts, OR (b) recall starts returning entities/relationships whose confidence has decayed to near-zero. Symptom: agent uses stale knowledge despite explicit corrections; or low-confidence noise pollutes recall results.

### What shipped (Part A)

When the sleep cycle runs, the DecayEngine now:
1. Lists distinct `source_episode_id` values from `memory_facts` rows where `contradicted_by IS NOT NULL` and `updated_at > now - lookback_hours`.
2. Finds kg_entities + kg_relationships whose `source_episode_ids` blob contains any of those episode ids (comma-flanked token match, agent-scoped, archival excluded).
3. Applies `confidence = MAX(min_floor, confidence * decay_factor)` to those rows.

Defaults: `enabled=true`, `decay_factor=0.9`, `min_floor=0.05`, `lookback_hours=24`. Wired in `gateway-memory/src/sleep/worker.rs` immediately after `decay_kg_confidence`. Cycle stats expose `contradiction_episodes_processed` / `contradiction_entities_decayed` / `contradiction_relationships_decayed`.

### Part B — Recall weighting (re-filed, NOT scheduled)

**Original premise was wrong.** The 2026-05-13 backlog assumed `gateway/gateway-execution/src/recall/mod.rs` had a graph-traversal block that multiplied `hop_decay^depth` × `kg_relationships.confidence`. That block never existed. `GraphTraversalConfig` is defined in `gateway-memory/src/lib.rs` but consumed by zero call sites. Recall today surfaces KG entities only via step 4's `search_entities_by_name_embedding` (cosine + rank, no hops) and step 5c's LCA walk over `parent_cluster_id` (hierarchy ancestry, not relationship traversal).

**Two ways forward, both deferred:**

- **B-1 (small):** Weight the existing step-4 graph-ANN hits by `kg_entities.confidence`. Skip hits below a `min_confidence` threshold. Requires adding `confidence` to `EntityNameEmbeddingHit` carrier + the trait method. Doesn't add traversal — just makes the existing ScoredItem score honest about per-entity confidence.
- **B-2 (large):** Actually build the hop-walking graph traversal the original backlog assumed exists. New trait method `walk_relationships(start: &[EntityId], max_hops, hop_decay, min_confidence) -> Vec<TraversalHit>`. Wire into recall as a new step. Multi-day scope; effectively a new recall feature.

**Effort:** B-1 ~half day. B-2 ~3-4 days.

**Dependencies:** None for B-1. None for B-2.

---

## MEM-002 — Memory subsystem extraction into `gateway-memory/` crate

**Status:** ✅ Done — Phases A through F shipped 2026-05-13 (commits a1e96a74 through 20d59f9b). See tracking doc for inventory.
**Severity:** Low (cosmetic / maintainability)
**Trigger:** Any of: (a) a second consumer wants the memory subsystem (e.g. a second daemon, a published library); (b) `gateway/gateway-execution/` becomes hard to navigate; (c) you want to publish memory as a standalone open-source crate. Until one of those, the inventory tracking doc is sufficient.

### Scope

Extract the memory subsystem from `gateway/gateway-execution/src/sleep/`, `gateway/gateway-execution/src/recall/`, and parts of `gateway/gateway-services/src/` into a new `gateway/gateway-memory/` crate. See `memory-bank/future-state/2026-05-13-memory-crate-extraction-tracking.md` for the complete change inventory.

### Phased migration plan

**Phase A — Empty crate + config types** (~1 day, 1 commit)
- Create `gateway/gateway-memory/` with `Cargo.toml` depending on `zero-stores-traits`, `zero-stores-domain`, `chrono`, `serde`, `tokio`, `async-trait`, `tracing`
- Move `RecallConfig`, `MemorySettings`, `KgDecayConfig` from `gateway-services` to `gateway-memory`
- Add `pub use gateway_memory::{...}` re-exports in `gateway-services/src/lib.rs` for backward compat (zero caller changes)

**Phase B — Sleep components** (~1 day, 8-10 commits)
- One commit per component, moved one at a time: Compactor → Synthesizer → PatternExtractor → Pruner → OrphanArchiver → HandoffWriter → CorrectionsAbstractor → ConflictResolver → DecayEngine
- After each move: `pub use` from `gateway-execution::sleep` for backward compat
- Update internal imports inside `gateway-memory` to use crate-local paths

**Phase C — Recall module** (~half day, 2 commits)
- Move `gateway/gateway-execution/src/recall/mod.rs` and adapters
- Update `runner/invoke_bootstrap.rs` to call `gateway_memory::RecallEngine` directly (or keep the existing free function with a moved body)

**Phase D — LLM factory trait** (~half day, 2 commits)
- Define `MemoryLlmFactory` trait in `gateway-memory`: `async fn build_client(purpose: LlmPurpose) -> Result<Arc<dyn LlmClient>>`
- Implement once in `gateway` by wrapping `ProviderService`
- Refactor 6 production LLM impls (`LlmHandoffWriter`, `LlmSynthesizer`, `LlmPatternExtractor`, `LlmCorrectionsAbstractor`, `LlmConflictJudge`, `LlmPairwiseVerifier`) to accept `Arc<dyn MemoryLlmFactory>` instead of `Arc<ProviderService>`
- The 6 impls' `build_client` helpers all have the same shape today — opportunity to DRY into one shared helper

**Phase E — Worker + factory** (~half day, 2 commits)
- Move `SleepOps`, `SleepTimeWorker` into `gateway-memory`
- Add a `MemoryServices` factory: `MemoryServices::new(stores, llm_factory, settings, paths) -> MemoryServices` returning a struct that bundles the constructed worker + recall engine
- This replaces the ~80-line imperative construction block currently in `gateway/src/state/mod.rs`

**Phase F — Wire from gateway** (~1 commit)
- Update `gateway/src/state/mod.rs` to construct a `MemoryServices` and store it on `AppState`
- Remove the now-redundant per-component construction code
- Verify `cargo check --workspace` + full test suite

### Files affected

Per the tracking doc Section 1, all of:
- `gateway/gateway-execution/src/sleep/` (entire module)
- `gateway/gateway-execution/src/recall/` (entire module)
- `gateway/gateway-services/src/recall_config.rs` (move out)
- `gateway/gateway-services/src/settings.rs` (`MemorySettings` moves out, `ExecutionSettings.memory` field stays)
- `gateway/src/state/mod.rs` (construction site collapses)
- `gateway/src/http/settings.rs` (`UpdateExecutionSettingsRequest.memory` keeps same shape, just imports from new crate)
- `runner/invoke_bootstrap.rs` (session-start injection — STAYS in gateway, composes memory + orchestrator; just changes its imports)

### Effort

Total: ~16–18 commits across 2–3 days focused work. Each phase is independently shippable behind re-exports.

### Risks

- **LLM factory abstraction** is the trickiest piece. The 6 production impls all have the same `build_client` shape that resolves the default provider from `ProviderService` and constructs an `OpenAiClient`. Replacing this cleanly requires a small refactor to share the build logic.
- **Test harness coupling**: existing tests construct stores directly via `KnowledgeDatabase` + `MemoryRepository` etc. The new crate will pull in `zero-stores-sqlite` as a dev-dep for tests, which is fine but adds a transitive dep.
- **Re-export discipline**: backward-compat re-exports from `gateway-services` and `gateway-execution` need to stay until callers migrate. Plan a follow-up commit per phase to remove the re-exports once nothing uses them.

### Dependencies

None. Phase 4 foundation done.

---

## MEM-003 — ConflictResolver: cache LLM client across pair judgments

**Status:** Pending. (Note: Phase D's `MemoryLlmFactory` abstraction makes implementing this easier — the factory can cache the client.)
**Severity:** Low (perf nit)
**Trigger:** Observed judge-call latency exceeds expectation, OR a single sleep cycle examines >20 pairs and you notice the cycle takes noticeably longer than expected.

### Scope

`LlmConflictJudge::judge` in `gateway/gateway-execution/src/sleep/conflict_resolver.rs` calls `build_client()` on every invocation — it lists providers, picks the default, constructs an `OpenAiClient`. For N pairs in a cycle this is N redundant client constructions. Same pattern exists in `LlmCorrectionsAbstractor`, `LlmSynthesizer`, etc.

### Fix

Cache `Arc<dyn LlmClient>` on the `LlmConflictJudge` struct. Either:
- Build once in `new()` and store; never rebuild (simplest)
- Lazily build on first `judge()` via `OnceCell` (handles delayed provider config)

Same pattern for the other LLM impls in `sleep/`.

### Effort

~1 commit. ~30 minutes. Sub-task of MEM-002 Phase D if the extraction happens (the new shared LLM factory naturally caches).

---

## MEM-004 — DecayEngine: bulk UPDATE via SQLite math extension

**Status:** Pending
**Severity:** Low (perf nit)
**Trigger:** Observed sleep-cycle duration exceeds 10 seconds, OR `kg_entities` row count exceeds 10,000.

### Scope

`decay_kg_table` in `stores/zero-stores-sqlite/src/knowledge_graph.rs` runs an O(N) per-row UPDATE loop because SQLite's `exp()` is in an optional math extension. A single bulk UPDATE would be one SQL round-trip:

```sql
UPDATE kg_entities
SET confidence = MAX(?, confidence * exp(? * (julianday('now') - julianday(last_seen_at))))
WHERE agent_id = ? AND epistemic_class != 'archival' AND last_seen_at < ?
```

### Fix

1. Confirm SQLite math functions are bundled in the project's `rusqlite` features (likely yes since `bundled` feature is enabled, but verify)
2. Replace the loop with the bulk UPDATE
3. Keep the per-row fallback as a feature-gated alternative for portability (or just remove if no portability concern)

### Effort

~1 commit. ~1 hour including verification.

---

## MEM-005 — Move `HandoffWriter` struct into `gateway-memory`

**Status:** ✅ Done — commit `f8adf7b1` (2026-05-13)
**Severity:** Low (architectural inconsistency)
**Trigger:** Phase D/E of the extraction (when introducing the LLM/store factory abstractions), OR when something else needs a clean `ConversationStore` trait abstraction.

### Context

During Phase B of the memory crate extraction (commit `bba92b87`), the `HandoffWriter` *engine struct* could not be moved into `gateway-memory` because it takes a concrete `Arc<zero_stores_sqlite::ConversationRepository>` parameter (not a trait). Moving it would create a crate-dependency cycle:

```
gateway-memory → zero-stores-sqlite → gateway-services → gateway-memory
```

What *did* move into `gateway-memory/src/sleep/handoff_writer.rs`: `HandoffLlm` trait, `HandoffEntry`, `HandoffInput`, `HANDOFF_*` constants, `should_inject`, `read_handoff_block`, and tests 4-8.

What *stayed* in `gateway-execution/src/sleep/handoff_writer.rs`: the `HandoffWriter` struct + impl, `LlmHandoffWriter` (expected — production LLM impl), `format_conversation_for_summary` helper, tests 1/2/3/9/10.

### Fix

Two options, in increasing order of cleanliness:

**Option A — Extend `ConversationStore` trait (preferred)**
- Add `get_session_conversation(session_id) -> Result<Conversation>` and `session_messages_to_chat_format(...)` to the `ConversationStore` trait in `stores/zero-stores-traits/src/conversation.rs`
- Implement on `zero-stores-sqlite::ConversationRepository`
- Change `HandoffWriter::new` to accept `Arc<dyn ConversationStore>` instead of `Arc<ConversationRepository>`
- Move the struct + impl + remaining tests + helper into `gateway-memory/src/sleep/handoff_writer.rs`

**Option B — Keep the split** (current state) — defer indefinitely

### Effort

Option A: ~1 commit, 1-2 hours. Mostly trait method additions + signature change + move. Tests already cover the behavior.

### Dependencies

None blocking. Best paired with Phase D (LLM factory) since that's when the abstraction-introduction work is happening anyway.

---

## How to use this backlog

- **Triggering an item** — when its trigger condition occurs, lift it into a fresh plan under `docs/superpowers/plans/` using the standard plan structure.
- **Adding items** — append below, use the next `MEM-NNN` number, keep the same fields: Status / Severity / Trigger / Scope / Files affected / Effort / Dependencies.
- **Closing items** — mark Status as `done — commit <SHA>` and stop tracking. Don't delete; the history is useful.
- **Don't pre-schedule** — none of these are calendar-driven. Wait for the trigger.
