# Memory Subsystem Backlog

**Purpose:** Track deferred work for the memory subsystem after Phases 1–4 foundation shipped. None of these items is scheduled — each has a concrete trigger that decides when it's worth picking up.

**Last updated:** 2026-05-13. Branch: `feat/parallel-delegation-aggregation`.

---

## MEM-001 — Phase 4b: Contradiction propagation + recall traversal weighting

**Status:** Pending
**Severity:** Medium (only when symptom appears)
**Trigger:** Either (a) you observe the agent acting on schemas that have been contradicted in newer memory facts, OR (b) recall starts returning entities/relationships whose confidence has decayed to near-zero. Symptom: agent uses stale knowledge despite explicit corrections; or low-confidence noise pollutes recall results.

### Scope

Two independent extensions on top of the Phase 4 foundation.

**Part A — Contradiction propagation from facts to KG nodes**
- When `memory_facts.contradicted_by` is set on a fact, locate the KG entities and relationships referenced by that fact's `source_episode_ids`
- Decay their `confidence` proportionally (e.g. `× 0.9` for an indirect contradiction)
- Optionally populate the `evidence TEXT` column (already provisioned in Phase 4 foundation) with a JSON record of the contradicting fact IDs

**Part B — Recall traversal weighting by confidence**
- In `gateway/gateway-execution/src/recall/mod.rs` graph-traversal block, multiply hop weight by `kg_relationships.confidence` (currently only `hop_decay^depth` is used)
- In the same path, filter out entities/relationships below a threshold (e.g. `confidence < 0.1`)
- Add a `min_kg_confidence: f64` field to `KgDecayConfig` or `GraphTraversalConfig` for the threshold

### Files affected

- `gateway/gateway-execution/src/sleep/decay.rs` — new method `propagate_fact_contradictions(agent_id) -> PropagationStats`
- `stores/zero-stores/src/knowledge_graph.rs` + sqlite impl — new method to locate entities/relationships by episode IDs, plus a method to apply a multiplicative confidence reduction
- `gateway/gateway-execution/src/recall/mod.rs` — graph traversal block + filter
- `gateway/gateway-services/src/recall_config.rs` — new fields
- `gateway/gateway-execution/src/sleep/worker.rs` — wire propagation into cycle, add stats fields
- `docs/memory-slides.html` + tracking doc — sync per [[feedback-memory-docs-keep-in-sync]]

### Effort

~6 tasks, similar shape to Phase 4 (TDD per task, subagent-driven execution, two-stage review). 1 day focused work.

### Dependencies

None. Phase 4 foundation is sufficient. The `evidence` column is already in place to record propagation provenance.

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

## MEM-006 — MMR diversity reranking in top-K

**Status:** Pending
**Severity:** Medium
**Trigger:** Observed top-K floods with near-duplicates of a single theme. Concrete symptom from session `sess-c913e2cd-34bc-41dc-873e-838503095323`: "200+ corrections about Flux/LoRA flooding context when working on math slides" — when a query lightly touches one heavily-corrected topic, the entire top-K becomes variations of that topic, drowning the actually-relevant signal.

### What it solves

zbot's current rescore chain in `gateway-memory/src/recall/mod.rs` produces a ranked list that is purely relevance-ordered: hybrid score × category weight × ward affinity × temporal decay × contradiction penalty × supersession penalty. There is no awareness that two of the top-K candidates may carry essentially the same information. When memory grows long-tail in any single direction (e.g. 200+ Flux/LoRA correction facts), a query that even tangentially matches that theme returns a top-K that is 8–10 near-duplicates of the same fact restated differently — the math-slides query gets buried.

Maximal Marginal Relevance (Carbonell & Goldstein, SIGIR 1998 — "The Use of MMR, Diversity-Based Reranking for Reordering Documents and Producing Summaries") is the canonical fix. It is a cheap O(K²) greedy re-selection over the candidate pool, post-rescore and pre-truncate. At each step it picks the next item to maximize `λ · score − (1 − λ) · max sim(item, already_picked)`. With `λ ≈ 0.6` the selector prefers relevance but penalizes a candidate every time it semantically overlaps with something already in the result set.

This is the cheapest of the three rerank improvements (~50 lines, no new deps, no new model files), and it directly attacks the symptom the user named.

### Architecture

MMR slots in as **step 8.5** of the existing `MemoryRecall::recall` pipeline in `gateway-memory/src/recall/mod.rs` — after the `min_score` retain (step 8/9) and before the final `truncate(limit)` (end of step 9).

The algorithm needs two inputs per candidate: its current scalar score (already on `ScoredFact.score`) and its embedding vector (NOT on `MemoryFact` rows by default — `get_facts_by_category` and `search_memory_facts_hybrid` both return `embedding: None`). The hydration pattern is already established in `gateway-memory/src/sleep/conflict_resolver.rs:101-104`: call `memory_store.get_fact_embedding(&fact.id)` for each candidate whose embedding is `None`. We will reuse that pattern.

Configuration uses a new `MmrConfig` substruct on `RecallConfig`, following the same shape as `MidSessionRecallConfig` / `GraphTraversalConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MmrConfig {
    /// Master toggle. When false, the recall pipeline skips MMR entirely.
    pub enabled: bool,
    /// Relevance-vs-diversity tradeoff. 1.0 = pure score-rank (no diversity),
    /// 0.0 = pure novelty (ignore score). Default 0.6.
    pub lambda: f64,
    /// How many top-scored candidates to consider before MMR re-ordering.
    /// Larger pool = more diversity opportunity at higher latency. Default 30.
    pub candidate_pool: usize,
}

impl Default for MmrConfig {
    fn default() -> Self {
        Self { enabled: true, lambda: 0.6, candidate_pool: 30 }
    }
}
```

`RecallConfig` gains a `pub mmr: MmrConfig` field; `Default::default()` constructs it with the above values. Because deep-merge in `RecallConfig::load_from_path` already handles missing JSON keys, no extra migration is needed.

The MMR loop itself is a single private function in `recall/mod.rs`:

```rust
async fn mmr_rerank(
    candidates: &mut Vec<ScoredFact>,
    memory_store: &Arc<dyn zero_stores::MemoryFactStore>,
    cfg: &MmrConfig,
    limit: usize,
) -> Result<(), String> {
    if !cfg.enabled || candidates.len() <= limit { return Ok(()); }
    // 1. truncate to candidate_pool
    // 2. hydrate embeddings via get_fact_embedding for each row missing one
    // 3. greedy pick: at each step, choose argmax over remaining of
    //    λ · score(c) − (1−λ) · max_picked cosine(emb(c), emb(picked))
    // 4. replace `candidates` with the picked order, limit-truncated
    Ok(())
}
```

Cosine similarity helper lives in the same file (small, no new dep). Candidates with no embedding even after hydration get a similarity term of `0.0` against everyone (treated as maximally novel — better than dropping them, which would silently lose recall coverage on facts indexed before sqlite-vec was enabled).

No new wiring in `MemoryServicesConfig` or `gateway/src/state/mod.rs`: MMR is internal to `MemoryRecall` and uses the already-wired `memory_store`.

### Files to change

| File | What changes |
|------|-------------|
| `gateway/gateway-memory/src/lib.rs` | Add `MmrConfig` struct + `Default` impl; add `pub mmr: MmrConfig` field on `RecallConfig`; update `Default for RecallConfig` to initialize it. |
| `gateway/gateway-memory/src/recall/mod.rs` | Add `mmr_rerank` helper + `cosine_similarity` helper; call from `MemoryRecall::recall` between min_score retain and truncate; add unit tests. |

That's it — 2 files. No `Cargo.toml`, no state wiring, no new traits.

### Implementation plan

- [ ] **Step 1 — Add `MmrConfig` struct.** Edit `gateway/gateway-memory/src/lib.rs`. Add the struct + `Default` impl shown above near `GraphTraversalConfig`. Add `pub mmr: MmrConfig` to `RecallConfig`. Initialize in `Default::default()`. Verify: `cargo check -p gateway-memory` succeeds.

- [ ] **Step 2 — Add cosine similarity helper.** Edit `gateway/gateway-memory/src/recall/mod.rs`. Add `fn cosine_similarity(a: &[f32], b: &[f32]) -> f64` that returns `0.0` when either slice is empty or lengths differ. Co-locate unit test `cosine_orthogonal_is_zero`, `cosine_identical_is_one`, `cosine_mismatched_lengths_returns_zero`. Verify: `cargo test -p gateway-memory cosine_` passes.

- [ ] **Step 3 — Add `mmr_rerank` helper.** Same file. Function signature given above. Hydrate embeddings via `memory_store.get_fact_embedding(&id).await` for rows missing one. Greedy loop: O(K²) — for desktop top-K = 10 with pool = 30, that's 300 cosine ops, trivial. Verify: `cargo check -p gateway-memory` succeeds.

- [ ] **Step 4 — Wire into recall pipeline.** Same file, inside `MemoryRecall::recall`. After `results.retain(|sf| sf.score >= self.config.min_score)` and before `results.truncate(limit)`, call `mmr_rerank(&mut results, store, &self.config.mmr, limit).await?` when `self.memory_store` is wired. Verify: `cargo test -p gateway-memory` passes.

- [ ] **Step 5 — Unit tests for MMR.** Same file, in the `tests` module. See **Test plan** below for the four scenarios. Verify: `cargo test -p gateway-memory mmr_` passes.

- [ ] **Step 6 — Integration test.** Add a test that wires a stub `MemoryFactStore` returning a candidate set with three near-duplicate embeddings + one distinct embedding, and asserts that with `lambda = 0.5`, the distinct candidate is in the top-3 even though all four have similar scores. Verify: `cargo test -p gateway-memory mmr_integration` passes.

Total: 6 commits, half-day. Each step is committable independently.

### Configuration

To enable / tune in `<vault>/config/recall_config.json`:

```json
{
  "mmr": {
    "enabled": true,
    "lambda": 0.6,
    "candidate_pool": 30
  }
}
```

Knobs:
- `enabled` (default `true`) — master switch. Set to `false` to bypass MMR entirely and ship pure score-rank.
- `lambda` (default `0.6`) — relevance-vs-diversity tradeoff. Range `[0.0, 1.0]`. `1.0` makes MMR a no-op (equivalent to pure sort). `0.0` ignores score entirely and just picks the most-novel item each step.
- `candidate_pool` (default `30`) — how many top-scored items to consider before MMR re-orders. Smaller = faster but less diversity opportunity. Larger = more diversity at O(pool²) cost.

### Test plan

1. `mmr_disabled_is_identity` — set `enabled: false`, run MMR over 10 candidates with random embeddings, assert output equals input.
2. `mmr_lambda_one_preserves_score_order` — `lambda: 1.0`, 5 candidates with descending scores and randomly-varied embeddings, assert output order matches descending-score order.
3. `mmr_lambda_zero_picks_most_novel` — `lambda: 0.0`, 4 candidates where #1 has highest score, #4 is most orthogonal to #1, assert #4 appears in position 2.
4. `mmr_demotes_near_duplicate_of_top` — 4 candidates, #1 score = 1.0, #2 score = 0.95 with `cosine(emb1, emb2) = 0.99`, #3 score = 0.8 with `cosine(emb1, emb3) = 0.1`, `lambda: 0.6`, assert #3 appears in position 2 (the diversity term pushes #2 below #3).
5. `mmr_handles_missing_embedding` — candidate with `embedding: None` and `get_fact_embedding` returning `Ok(None)`, assert it's not dropped and gets similarity term `0.0` against everything.
6. `mmr_integration_diverse_set` — wire a fake store with 4 facts (3 near-duplicate via similar content/embedding, 1 distinct), run full `MemoryRecall::recall`, assert the distinct fact appears in top-3.

### Effort

3–4 commits, half-day. Smallest of the three.

### Dependencies

None. Pure post-processing over already-recalled candidates. Independent of MEM-007 and MEM-008.

### Out of scope

- Cross-encoder reranking (that's MEM-007).
- Query-conditional MMR lambda (that's MEM-008's job — per-intent profile can override `mmr.lambda`).
- DPP / submodular diversification — research-tier, deferred per the rerank-research doc.
- Embedding cache layer — the hydration cost is N round-trips per query but with `candidate_pool = 30` and SQLite-vec local-fs reads, this is <5 ms. Don't optimize until measured.

---

## MEM-007 — ONNX cross-encoder reranker

**Status:** Pending
**Severity:** High
**Trigger:** Either (a) MMR (MEM-006) shipped but recall quality on mixed factoid + conversational queries is still subjectively poor, OR (b) you observe the static category-weight chain (schema 1.6, correction 1.5, etc.) misranking obvious cases — e.g. a factoid query "what's my OpenAI key path?" returning corrections above the actual key. Stronger trigger: the user complaint about "200+ corrections flooding math slides context" persists even after MMR.

### What it solves

zbot today does no learned scoring at the rerank layer. Hybrid retrieval (FTS5 + sqlite-vec + RRF) gives an initial ranked list; the rescore chain then multiplies in static category weights, ward affinity, temporal decay, etc. None of these account for *semantic match between query and candidate text*. A correction fact about "Flux LoRA training" has high `category_weight = 1.5`, so any query that retrieves it via hybrid (even faintly) gets it pushed up — regardless of whether the math-slides query actually needs Flux-LoRA information.

A cross-encoder fixes this exactly: it takes `(query, candidate)` jointly through a transformer and produces a learned relevance score. Anthropic's Contextual Retrieval write-up (Sep 2024) reports +18 percentage points of failure-rate reduction from *adding a reranker* on top of hybrid BM25+embeddings (5.7% → 2.9% failure → 1.9% with reranker = 67% total reduction). The BGE-reranker-v2 family (BAAI, 2024) is the best-published open cross-encoder; `BGE-reranker-base` at ~280MB and ~5–15 ms/candidate on CPU is the right size for a desktop assistant.

This is the highest-leverage of the three rerank improvements, and also the highest-cost (new dep, model download, lazy-init dance, fallback path). Treat it as a half-week of focused work.

### Architecture

A new trait `CrossEncoderReranker` lives in a fresh `gateway/gateway-memory/src/rerank.rs`. The recall pipeline calls `rerank()` after MMR (if MEM-006 done) or after the rescore chain (if not). A no-op default impl returns candidates unchanged so the rest of the system never needs to special-case absence.

```rust
use async_trait::async_trait;
use zero_stores_domain::ScoredFact;

#[async_trait]
pub trait CrossEncoderReranker: Send + Sync {
    /// Re-score a batch of candidates against a query. Returning the input
    /// unchanged is always a valid implementation (no-op fallback).
    async fn rerank(&self, query: &str, candidates: Vec<ScoredFact>) -> Vec<ScoredFact>;
}

/// No-op fallback. Used when reranking is disabled or when the production
/// reranker fails to load — keeps the recall path infallible.
pub struct IdentityReranker;

#[async_trait]
impl CrossEncoderReranker for IdentityReranker {
    async fn rerank(&self, _query: &str, candidates: Vec<ScoredFact>) -> Vec<ScoredFact> {
        candidates
    }
}
```

The production impl `FastembedReranker` wraps `fastembed::TextRerank` (https://github.com/Anush008/fastembed-rs). The crate exposes ONNX inference for `BGEReranker` variants out-of-the-box; we pick `BGEReranker::BGERerankerBase` for the desktop tier. Construction is lazy — we don't load the 280 MB model until the first `rerank()` call. The wrapped state is `Mutex<OnceCell<Arc<fastembed::TextRerank>>>` so concurrent first calls don't race-load.

```rust
pub struct FastembedReranker {
    model_id: String,
    cache_dir: PathBuf,
    inner: Mutex<OnceCell<Arc<fastembed::TextRerank>>>,
}

#[async_trait]
impl CrossEncoderReranker for FastembedReranker {
    async fn rerank(&self, query: &str, candidates: Vec<ScoredFact>) -> Vec<ScoredFact> {
        let model = match self.lazy_load().await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("reranker load failed, falling back to identity: {e}");
                return candidates;
            }
        };
        // model.rerank(query, docs, top_n) → Vec<(idx, score)>; map back onto ScoredFact
        // On any inference error: log + return candidates unchanged.
        ...
    }
}
```

Failure modes: model download fails (network), ONNX file corrupt, inference throws → all log + return input unchanged. Recall never fails because the reranker failed. This is critical — the user complaint we're solving is "noise floods context", not "memory is broken".

Configuration uses a new `RerankConfig` substruct on `RecallConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankConfig {
    pub enabled: bool,
    /// fastembed model id. Default "BAAI/bge-reranker-base".
    pub model_id: String,
    /// Override for the ONNX cache dir. None = use fastembed default
    /// (which itself defaults to `~/.cache/fastembed`). The gateway can
    /// pass a vault-relative path here.
    pub cache_dir: Option<String>,
    /// How many top-scored candidates to actually run through the reranker.
    /// O(N) latency; ~10 ms × N on CPU for bge-reranker-base. Default 20.
    pub candidate_pool: usize,
    /// How many to keep after reranking. Default 10.
    pub top_k_after: usize,
    /// Minimum cross-encoder score to keep a candidate. Default 0.0
    /// (no filtering by absolute score — relative rank is enough).
    pub score_threshold: f64,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            enabled: false, // OFF by default — opt-in until validated
            model_id: "BAAI/bge-reranker-base".to_string(),
            cache_dir: None,
            candidate_pool: 20,
            top_k_after: 10,
            score_threshold: 0.0,
        }
    }
}
```

`MemoryServicesConfig` gains `pub reranker: Option<Arc<dyn CrossEncoderReranker>>`. `MemoryRecall` gains a `set_reranker(...)` setter and a `reranker: Option<Arc<dyn CrossEncoderReranker>>` field, mirroring the existing store-setter pattern. The gateway's `state/mod.rs` constructs the reranker once at startup, after `recall_config` is loaded, then injects it.

External crate dep: `fastembed = "5"` (or whichever version is current at implementation time) on `gateway-memory/Cargo.toml`. Note that fastembed pulls in `ort` (ONNX Runtime), `tokenizers`, and `hf-hub` — verify the workspace is OK with the transitive size before merging.

### Files to change

| File | What changes |
|------|-------------|
| `gateway/gateway-memory/Cargo.toml` | Add `fastembed = "5"` dep + any feature flags it needs. |
| `gateway/gateway-memory/src/rerank.rs` | New file: `CrossEncoderReranker` trait, `IdentityReranker`, `FastembedReranker`. |
| `gateway/gateway-memory/src/lib.rs` | Add `RerankConfig` struct + `Default`; add `pub rerank: RerankConfig` on `RecallConfig`; add `pub mod rerank;` + re-export trait. |
| `gateway/gateway-memory/src/recall/mod.rs` | Add `reranker: Option<Arc<dyn CrossEncoderReranker>>` field on `MemoryRecall`; add `set_reranker` setter; call `reranker.rerank(query, candidates)` after MMR (or after rescore if MMR off). |
| `gateway/gateway-memory/src/services.rs` | Add `pub reranker: Option<Arc<dyn CrossEncoderReranker>>` to `MemoryServicesConfig`; thread it into the constructed `MemoryRecall` (this requires `MemoryServices` to also build/own a `MemoryRecall` — currently it only builds `SleepTimeWorker`, so adding a `recall: Arc<MemoryRecall>` exposed handle is part of this work). |
| `gateway/src/state/mod.rs` | Construct `Option<Arc<dyn CrossEncoderReranker>>` after `recall_config` is loaded; gate on `recall_config.rerank.enabled`; build `FastembedReranker` with `cache_dir` resolving to `paths.vault_dir().join("models")` when None; pass to `MemoryServicesConfig`. Also wire it to `memory_recall_inner.set_reranker(...)` to cover the recall-side path. |
| `gateway/Cargo.toml` | No change (already depends on `gateway-memory` via path). |

### Implementation plan

- [ ] **Step 1 — Add fastembed dep.** Edit `gateway/gateway-memory/Cargo.toml`. Add `fastembed = "5"`. Verify: `cargo check -p gateway-memory` succeeds; check `cargo tree -p gateway-memory` for the transitive set; note ONNX Runtime version pulled in.

- [ ] **Step 2 — Define trait + identity impl.** Create `gateway/gateway-memory/src/rerank.rs` with `CrossEncoderReranker` trait + `IdentityReranker`. Add `pub mod rerank;` + `pub use rerank::{CrossEncoderReranker, IdentityReranker};` to `lib.rs`. Verify: `cargo check -p gateway-memory` succeeds.

- [ ] **Step 3 — Add `RerankConfig`.** Edit `gateway/gateway-memory/src/lib.rs`. Add the struct + `Default` (enabled=false). Add `pub rerank: RerankConfig` on `RecallConfig`. Initialize in `Default`. Verify: `cargo test -p gateway-memory` passes (default still loads).

- [ ] **Step 4 — `FastembedReranker` lazy-load + rerank.** Same `rerank.rs`. Implement `FastembedReranker::new(model_id, cache_dir)` (cheap, no load). Implement `lazy_load(&self) -> Result<Arc<fastembed::TextRerank>, String>` using `OnceCell`. Implement `rerank` that calls `model.rerank(query, &doc_texts, candidates.len(), None)`. Map back: fastembed returns `Vec<RerankResult { index, score, .. }>` in re-ranked order; reorder the input `Vec<ScoredFact>` accordingly and overwrite `score` with the new score. Verify: `cargo check -p gateway-memory`.

- [ ] **Step 5 — Wire into `MemoryRecall`.** Edit `gateway/gateway-memory/src/recall/mod.rs`. Add the field + setter. In `recall()` after MMR and before final `truncate`, if `self.reranker.is_some()` and `self.config.rerank.enabled`: take the first `candidate_pool` items, call `rerank(query, pool).await`, retain results with `score >= score_threshold`, replace `results` with the reranked list, truncate to `top_k_after`. Verify: `cargo check -p gateway-memory`.

- [ ] **Step 6 — Wire through `MemoryServices` + state.** Edit `services.rs` to expose `recall: Arc<MemoryRecall>` on the `MemoryServices` struct (if not already), and accept `reranker: Option<Arc<dyn CrossEncoderReranker>>` on `MemoryServicesConfig`. Edit `gateway/src/state/mod.rs` to construct the reranker once (gated on enabled flag) and pass it in. Verify: `cargo check --workspace` succeeds; gateway boots with `rerank.enabled = false` (default) without trying to download anything.

- [ ] **Step 7 — Unit tests.** Mock reranker that scores by string-matching keywords from the query against candidate content. Assert order changes. See test plan. Verify: `cargo test -p gateway-memory rerank_` passes.

- [ ] **Step 8 — Optional feature-gated smoke test.** Behind `--features rerank-smoke`, actually load `BGE-reranker-base` and verify it ranks an obvious better-match (query "math slides", candidates "math slide layout" / "Flux LoRA training") in the expected order. Default-off in CI to avoid the 280 MB download. Verify (locally): `cargo test -p gateway-memory --features rerank-smoke rerank_smoke` passes.

Total: 5–6 commits, 1 day. Biggest of the three.

### Configuration

To enable in `<vault>/config/recall_config.json`:

```json
{
  "rerank": {
    "enabled": true,
    "model_id": "BAAI/bge-reranker-base",
    "cache_dir": null,
    "candidate_pool": 20,
    "top_k_after": 10,
    "score_threshold": 0.0
  }
}
```

Knobs:
- `enabled` (default `false`) — opt-in. First-time enabling triggers a ~280 MB download on next recall.
- `model_id` (default `"BAAI/bge-reranker-base"`) — any model ID that fastembed-rs supports. Larger models (e.g. `BAAI/bge-reranker-v2-m3` at ~570 MB) give higher quality at higher latency.
- `cache_dir` (default `null`) — where ONNX files cache. `null` uses fastembed's default (`~/.cache/fastembed`). Set to a vault-relative path to keep the model under the vault root.
- `candidate_pool` (default `20`) — how many candidates from the upstream pipeline to actually run through the reranker. Each rerank call is O(N) at ~10 ms/candidate on CPU.
- `top_k_after` (default `10`) — final truncation after reranking.
- `score_threshold` (default `0.0`) — drop candidates whose cross-encoder score is below this. BGE-reranker-base scores are unbounded (logits), so `0.0` is a reasonable "above-random" floor.

### Test plan

1. `identity_reranker_returns_input_unchanged` — construct `IdentityReranker`, pass 5 candidates, assert exact same `Vec<ScoredFact>` returned in order.
2. `mock_reranker_reorders_by_keyword_match` — implement a test-only `KeywordMockReranker { keyword: String }` that scores `1.0` if candidate content contains the keyword, else `0.0`. Pass a candidate list with mixed contents, assert keyword-matching candidates float to the top.
3. `recall_pipeline_calls_reranker_when_enabled` — wire `KeywordMockReranker` into `MemoryRecall` via setter, set `RerankConfig::enabled = true`, run full `recall()` over a stub store, assert mock was called once and output order reflects mock's scoring.
4. `recall_pipeline_skips_reranker_when_disabled` — same setup but `enabled = false`, assert mock's call counter stays at 0.
5. `reranker_failure_is_swallowed` — mock that returns `panic!` or in async-trait terms returns an error, assert recall returns the un-reranked candidates and logs a warning (use `tracing_test`).
6. `score_threshold_filters_low_scores` — mock scores [1.0, 0.5, 0.1, 0.05], threshold = 0.3, assert top-K contains only the 1.0 and 0.5 items.
7. `top_k_after_truncates_correctly` — mock returns 20 reranked items, `top_k_after = 5`, assert final list length is 5.
8. (Feature-gated, `rerank-smoke`) `bge_reranker_base_ranks_relevant_higher` — actually load `BGE-reranker-base`, query "math slides", candidates ["math slide layout", "Flux LoRA training tips", "TypeScript generics"], assert "math slide layout" ranks first.

### Effort

5–6 commits, 1 day. The biggest item: model integration, lazy load, fallback path, optional smoke test.

### Dependencies

Best implemented after MEM-006 so MMR runs before the cross-encoder (smaller candidate set going in = lower rerank latency). Functionally independent — can ship in either order, but the recall-pipeline call ordering is documented as: rescore → MMR → cross-encoder → truncate.

Optional dep on MEM-008: if you want per-intent reranker control (e.g. disable reranker for factoid queries where BM25 is enough), MEM-008's per-intent profile system should override `RerankConfig` fields. Both can ship before that — MEM-008 just adds the per-query override layer.

### Out of scope

- Hosted rerankers (Cohere Rerank 3.5, Voyage rerank) — these need an HTTP path + API key handling, deferred.
- Reciprocal-rank-fusion of multiple rerankers — see the rerank-research doc Section 6; defer until single-reranker quality is validated.
- LLM listwise reranking (RankGPT) — research-tier, deferred.
- Confidence-thresholded recall escalation (Cohere two-stage pattern) — interesting follow-up; track as a separate MEM-NNN once we have score histograms from real reranker output.
- Logging rerank scores for offline analysis — useful but a feature, not the MVP.

---

## MEM-008 — Semantic intent router with per-intent recall profiles

**Status:** Pending
**Severity:** Medium
**Trigger:** Either (a) MEM-006 + MEM-007 shipped but recall quality is *still* uneven across query types — factoid queries return too many corrections, conversational queries miss recent context, etc. OR (b) the user complaint persists in a different shape: the static category weights are right for some queries but wrong for others, and there is no way to make them query-conditional. Concrete observable: edit `recall_config.json` to tune for one query type, and a different query type silently regresses.

### What it solves

zbot's `RecallConfig.category_weights` (schema 1.6, correction 1.5, strategy 1.4, …) are *static* — they apply identically to every query. But the right weights depend on what the query is asking. A factoid lookup ("what's my OpenAI key path?") wants BM25-heavy + `domain` weight up + `correction` weight down. A correction-recall query ("did I tell you not to use X?") wants the inverse — `correction` weight cranked, larger top-K. A code-help query wants `pattern` weight up + graph traversal deeper. Today these all share one weight vector, which is necessarily a compromise.

The fix is a query intent classifier (kNN over canonical exemplar utterances — the Aurelio Semantic Router pattern, https://github.com/aurelio-labs/semantic-router) that picks an intent label, plus a per-intent profile that overlays specific `RecallConfig` fields just for that query. The classifier is sub-100 ms (one embedding call + cosine over a small bank). The profile system is a simple key-by-key overlay on the base config.

This is the multiplier on top of MEM-006 + MEM-007. The reranker improves *any* query; the router makes the upstream candidate pool query-appropriate before reranking sees it.

### Architecture

A new trait `IntentClassifier` lives in a fresh `gateway/gateway-memory/src/intent_router.rs`:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait IntentClassifier: Send + Sync {
    /// Classify the query. Return `None` to mean "no confident intent —
    /// use the default RecallConfig". The string is opaque to the recall
    /// pipeline; it's just a lookup key into the per-intent profile bank.
    async fn classify(&self, query: &str) -> Option<String>;
}
```

Production impl `KnnIntentClassifier`:

```rust
pub struct KnnIntentClassifier {
    embedding_client: Arc<dyn EmbeddingClient>,
    /// Pre-embedded exemplars: each entry is (intent_label, exemplar_embedding).
    bank: Vec<(String, Vec<f32>)>,
    /// Top-K vote depth. Default 5.
    k: usize,
    /// Minimum cosine similarity to the nearest exemplar for a confident
    /// classification. Below this, return None. Default 0.55.
    confidence_threshold: f64,
}
```

The bank is constructed at startup by reading two JSON config files from the vault:

```
<vault>/config/memory/intent_exemplars.json
<vault>/config/memory/intent_profiles.json
```

`intent_exemplars.json` format:

```json
{
  "intents": {
    "factoid-lookup": [
      "what is my openai api key path",
      "when did I last commit to feature/foo",
      "where is the daemon config stored",
      "what's the value of X"
    ],
    "how-to": ["how do I run the daemon", "how to disable telemetry"],
    "domain-question": ["explain conflict resolution", "what does supersession mean"],
    "user-fact": ["what do I prefer for X", "what did I say about Y"],
    "code-help": ["why does this Rust trait not compile", "fix this error message"],
    "correction-recall": ["did I tell you not to use Z", "didn't we agree on Q"],
    "procedural": ["walk me through how to A then B then C"]
  }
}
```

At classifier-construction time, the gateway embeds every exemplar (one batch call to `EmbeddingClient::embed`) and stores `(label, embedding)` pairs in `bank`. At query time: embed the query, cosine over the bank, take top-K (default 5) nearest, vote by label. If the top-1 cosine is below `confidence_threshold`, return `None`.

`intent_profiles.json` format — each intent gets a partial `RecallConfig` overlay:

```json
{
  "factoid-lookup": {
    "category_weights": { "domain": 1.5, "correction": 0.8 },
    "graph_traversal": { "enabled": false },
    "max_facts": 5
  },
  "correction-recall": {
    "category_weights": { "correction": 2.5, "domain": 0.5 },
    "max_facts": 20
  },
  "code-help": {
    "category_weights": { "pattern": 1.4 },
    "graph_traversal": { "max_hops": 3 }
  }
}
```

The profile is stored as `HashMap<String, serde_json::Value>` (partial JSON object) and applied via the same `deep_merge` function already used by `RecallConfig::load_from_path` (`gateway/gateway-memory/src/lib.rs:324`). At recall time: serialize the base `RecallConfig` to JSON Value, deep-merge the intent's profile on top, deserialize back to an effective per-query `RecallConfig`. Pass that to the rest of the pipeline.

```rust
pub struct IntentProfiles {
    overrides: HashMap<String, Value>,
}

impl IntentProfiles {
    pub fn apply(&self, base: &RecallConfig, intent: &str) -> RecallConfig {
        let Some(overlay) = self.overrides.get(intent) else { return base.clone(); };
        let base_v = serde_json::to_value(base).unwrap();
        let merged = deep_merge(base_v, overlay.clone());
        serde_json::from_value(merged).unwrap_or_else(|_| base.clone())
    }
}
```

`MemoryRecall` gains a `classifier: Option<Arc<dyn IntentClassifier>>` field, a `profiles: Option<Arc<IntentProfiles>>` field, and setters. `recall()` starts by calling `classifier.classify(query)` → `intent`, then `profiles.apply(&self.config, &intent)` → effective config, then runs the rest of the pipeline using the effective config (instead of `self.config` directly).

`MemoryServicesConfig` gains:

```rust
pub intent_classifier: Option<Arc<dyn IntentClassifier>>,
pub intent_profiles: Option<Arc<IntentProfiles>>,
```

The gateway's `state/mod.rs` constructs these once at startup. Bank construction is async (one `embedding_client.embed(...)` batch call); profile loading is sync (file read + JSON parse).

The intent classifier slots in at the *start* of the recall pipeline — before rescore, before MMR, before reranker. This way every downstream stage sees the per-intent config.

### Files to change

| File | What changes |
|------|-------------|
| `gateway/gateway-memory/src/intent_router.rs` | New file: `IntentClassifier` trait, `IdentityClassifier` (no-op returning None), `KnnIntentClassifier`, `IntentProfiles`. |
| `gateway/gateway-memory/src/lib.rs` | Add `pub mod intent_router;` + re-exports. Export the existing `deep_merge` as `pub(crate)` (or move it into a `util` module) so `IntentProfiles::apply` can use it. |
| `gateway/gateway-memory/src/recall/mod.rs` | Add `classifier` + `profiles` fields + setters on `MemoryRecall`; thread them through `recall()` so the rest of the pipeline runs against the per-query effective config. |
| `gateway/gateway-memory/src/services.rs` | Add `intent_classifier` + `intent_profiles` fields to `MemoryServicesConfig`; thread them onto the constructed `MemoryRecall`. |
| `gateway/src/state/mod.rs` | After `embedding_client` is constructed and `recall_config` is loaded: read `<vault>/config/memory/intent_exemplars.json` + `intent_profiles.json` (both optional — absence = no router, no overrides); embed all exemplars; construct `KnnIntentClassifier`; construct `IntentProfiles`; pass to `MemoryServicesConfig`. |
| `assets/memory/intent_exemplars.json` | New asset: default exemplar bank (7 intents × 5–10 exemplars each, per the taxonomy below). Copied into the vault on first launch via the existing asset-seeding mechanism. |
| `assets/memory/intent_profiles.json` | New asset: default per-intent profile overlays. |

(Path for default assets: confirm where vault-seeded JSON lives today — likely under `apps/daemon/assets/` or `gateway/assets/`. Use the same pattern as existing seeded JSON.)

### Implementation plan

- [ ] **Step 1 — Trait + identity impl.** Create `gateway/gateway-memory/src/intent_router.rs` with `IntentClassifier` trait + `IdentityClassifier`. Add `pub mod intent_router;` + re-exports in `lib.rs`. Verify: `cargo check -p gateway-memory`.

- [ ] **Step 2 — `KnnIntentClassifier` construction.** Implement `KnnIntentClassifier::new(embedding_client, exemplars: HashMap<String, Vec<String>>, k, threshold) -> Result<Self, String>`. Construction batch-embeds all exemplars in one call. Verify: unit test that constructor produces non-empty bank.

- [ ] **Step 3 — `KnnIntentClassifier::classify`.** Implement: embed query, cosine against every bank entry, take top-K, vote by label (majority wins, ties broken by sum-of-similarity), gate on `confidence_threshold`. Verify: unit tests for happy-path classify + low-confidence-returns-None.

- [ ] **Step 4 — `IntentProfiles`.** Implement `IntentProfiles::from_json(value: Value) -> Self` and `apply(base, intent) -> RecallConfig` using `deep_merge`. Make `deep_merge` `pub(crate)` (or move to `util`). Verify: unit test that profile correctly overlays a single field, leaves others untouched.

- [ ] **Step 5 — Wire into `MemoryRecall`.** Add fields + setters. Edit `recall()` to call `classifier.classify(query)` first, then `profiles.apply(...)` to compute effective config, then use the effective config for category weights, ward affinity, temporal decay, contradiction, supersession, min_score, MMR, reranker. Verify: `cargo check -p gateway-memory`.

- [ ] **Step 6 — Wire through services + state.** Add fields to `MemoryServicesConfig` + state-side construction. Read exemplar + profile JSON from vault dir; absence = `None` for both fields (recall pipeline degrades to base config). Verify: gateway boots without the JSON files present.

- [ ] **Step 7 — Default JSON assets.** Author `intent_exemplars.json` + `intent_profiles.json` per the taxonomy section below. Wire into the asset-seeding path so first-launch vaults get them. Verify: fresh vault gets the files in `<vault>/config/memory/` after boot.

- [ ] **Step 8 — Unit tests.** See test plan. Verify: `cargo test -p gateway-memory intent_` passes.

- [ ] **Step 9 — Integration test.** Wire real `KnnIntentClassifier` with a small bank, run a handful of queries through full `MemoryRecall::recall`, assert each routes to its expected intent and that the effective config differs from the base config in the expected fields. Verify: `cargo test -p gateway-memory intent_integration` passes.

Total: 4–5 commits, 1 day.

### Intent taxonomy (default bank shape)

Starter set, 7 intents:

| Intent | Example exemplars | Profile direction |
|---|---|---|
| `factoid-lookup` | "what is X", "when did Y", "where is Z" | `domain` up, `correction` down, graph off, small top-K |
| `how-to` | "how do I", "how to do X" | `instruction` up, `pattern` up |
| `domain-question` | abstract concept queries — "explain X", "what does Y mean" | `domain` up significantly, graph depth bumped |
| `user-fact` | "what do I prefer", "what did I say about", "my N is" | `user` up, smaller top-K |
| `code-help` | programming questions, error messages | `pattern` up, graph depth bumped, larger top-K |
| `correction-recall` | "did I tell you", "didn't we agree" | `correction` way up, larger top-K |
| `procedural` | multi-step task instructions, "walk me through" | `strategy` up, `pattern` up |

Default fallthrough (no intent confident enough) = use base config unchanged.

### Configuration

`<vault>/config/memory/intent_exemplars.json` — see format example above.
`<vault>/config/memory/intent_profiles.json` — see format example above.

Both files are optional. Missing exemplar bank = no classifier (every query uses base config). Missing profile bank = classifier still runs but every intent maps to base config (functionally a no-op).

Tunable knobs (passed through to `KnnIntentClassifier` from the gateway):
- `k` (default `5`) — kNN vote depth.
- `confidence_threshold` (default `0.55`) — minimum cosine to top exemplar for a confident classification.

These two could live in a small section of `recall_config.json`:

```json
{
  "intent_router": {
    "k": 5,
    "confidence_threshold": 0.55
  }
}
```

(Add `IntentRouterConfig` to `RecallConfig` if you want JSON-tunable knobs; not strictly required for MVP.)

### Test plan

1. `identity_classifier_returns_none` — `IdentityClassifier::classify("anything")` returns `None`.
2. `knn_constructor_embeds_all_exemplars` — wire a mock embedding client that counts calls, construct classifier with 3 intents × 4 exemplars, assert exactly 12 embeddings were requested in one batch.
3. `knn_classify_picks_nearest_intent` — fixed embedding-client mock returning known vectors, exemplars laid out so query embedding is closest to `factoid-lookup` bank entries, assert `classify(query) == Some("factoid-lookup")`.
4. `knn_classify_below_threshold_returns_none` — same setup but query embedding is orthogonal to all exemplars, assert `None`.
5. `knn_vote_breaks_ties_by_sum_similarity` — when top-K contains 2 intent A + 2 intent B + 1 intent C, but A's total similarity is higher, assert A wins.
6. `profiles_apply_overlays_category_weights` — base config has `correction: 1.5`, profile sets `correction: 2.5`, effective config has `correction: 2.5` and other weights unchanged.
7. `profiles_apply_unknown_intent_returns_base` — profile bank has only `factoid-lookup`, apply with `intent = "made-up"`, assert returned config equals base.
8. `profiles_apply_partial_overlay_preserves_other_fields` — base config has `max_facts: 10`, profile only sets `category_weights.correction: 2.5`, effective config has `max_facts: 10` (unchanged) AND `correction: 2.5`.
9. `recall_pipeline_uses_effective_config_when_classifier_present` — wire mock classifier returning `"correction-recall"`, wire profile that sets `correction: 2.5`, run full `MemoryRecall::recall` with a candidate fact of category `correction`, assert its final score reflects the `2.5` weight not the base `1.5`.
10. `recall_pipeline_uses_base_config_when_classifier_returns_none` — wire mock classifier returning `None`, assert effective config is identical to base.
11. (Integration) `intent_integration_routes_distinct_queries` — load real default exemplar bank, run 3 distinct queries (factoid, correction-recall, code-help), assert each routes to the expected intent label.

### Effort

4–5 commits, 1 day.

### Dependencies

Independent of MEM-006 and MEM-007. Works on top of either, or neither. The recommended composition order at runtime is: intent classifier → effective config → existing rescore → MMR (MEM-006) → cross-encoder (MEM-007) → truncate. Each stage benefits from the per-intent override that this entry installs.

### Out of scope

- DistilBERT/fastText classifier — defer, kNN is enough at zbot's query volume per the rerank-research doc.
- LLM classifier — slower (200–2000 ms), expensive. Worth revisiting if kNN accuracy proves insufficient.
- Online learning of new intents from user feedback — feature, not MVP.
- Per-intent reranker model selection (e.g. use Cohere for one intent, BGE for another) — deferred until a second reranker exists.
- Per-intent BM25/vector weight overrides — easy to add later by including those keys in the profile; not part of MVP.
- A UI for editing exemplars / profiles — JSON-only for v1.

---

## How to use this backlog

- **Triggering an item** — when its trigger condition occurs, lift it into a fresh plan under `docs/superpowers/plans/` using the standard plan structure.
- **Adding items** — append below, use the next `MEM-NNN` number, keep the same fields: Status / Severity / Trigger / Scope / Files affected / Effort / Dependencies.
- **Closing items** — mark Status as `done — commit <SHA>` and stop tracking. Don't delete; the history is useful.
- **Don't pre-schedule** — none of these are calendar-driven. Wait for the trigger.
