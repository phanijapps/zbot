# Memory Crate Extraction — Change Tracking

**Purpose:** Catalog of every memory-related code change made across Phases 1–3 of the Reflective Memory roadmap, so a future refactor to extract memory into its own crate has a complete starting inventory. No code changes yet — this is a tracking document only.

**Author trail:** Built up across the work in sessions starting `dd8b51a8-e020-4ada-bb4f-e59ccacbb115` and continuing in `2be03b02-1859-4ed2-8680-3a36172e9215` / `d0db5589-286d-4cfd-99ed-c9b9a697e67f`. Last updated 2026-05-13.

Last updated: 2026-05-13. **Extraction status: complete (Phases A–F + MEM-005 all shipped).**

---

## 1. Scope: What "Memory" Means Today

The memory subsystem currently lives across three crates:

| Crate | Role |
|-------|------|
| `stores/zero-stores-traits` | `MemoryFactStore`, `CompactionStore`, `EpisodeStore`, `KnowledgeGraphStore` traits + `MemoryFact` re-exports |
| `stores/zero-stores-sqlite` | SQLite implementations of all above traits |
| `stores/zero-stores-domain` | `MemoryFact`, `StrategyFactInsert`, `StrategyFactMatch` types |
| `gateway/gateway-execution/src/sleep/` | All sleep-time memory components (Compactor, Synthesizer, PatternExtractor, Pruner, OrphanArchiver, **CorrectionsAbstractor**, **ConflictResolver**, **HandoffWriter**) |
| `gateway/gateway-memory` | **COMPLETE — Phases A through F.** The full memory subsystem: config types, all sleep-component engines + abstraction traits, recall pipeline, `MemoryLlmFactory` + 6 production `Llm*` impls, `HandoffWriter` struct, `SleepOps` + `SleepTimeWorker`, and the `MemoryServices::new(MemoryServicesConfig)` factory. Gateway constructs services in 41 lines (was 104). |
| `gateway/gateway-execution/src/recall/` | Recall scoring + retrieval pipeline |
| `gateway/gateway-execution/src/runner/invoke_bootstrap.rs` | Session-start memory injection (handoff, corrections, goals, targeted recall) |
| `gateway/gateway-services/src/recall_config.rs` | `RecallConfig` |
| `gateway/gateway-services/src/settings.rs` | `MemorySettings` (new) inside `ExecutionSettings` |
| `gateway/src/state/mod.rs` | Construction + wiring of all memory components |

A future `zero-memory` crate would naturally absorb most of `gateway/gateway-execution/src/sleep/` and `recall/`, plus `RecallConfig` and `MemorySettings`. The store traits already live in `zero-stores-*` and don't need to move.

---

## 2. New Components Added in Phases 1–3

These are the *new* memory components built across the three phases. All live in `gateway/gateway-execution/src/sleep/`.

### Phase 1 — Session Handoff + Targeted Recall + Always-Inject

| Component | File | Responsibility |
|-----------|------|----------------|
| `HandoffWriter` | `sleep/handoff_writer.rs` | LLM-summarizes a session at end, writes a `handoff.latest` + `handoff.{session_id}` fact to the memory store under the `__handoff__` sentinel agent |
| `read_handoff_block` | `sleep/handoff_writer.rs` | Session-start reader. Ward-scoped (returns `None` if stored handoff's `ward_id` doesn't match current session's ward). Returns formatted `## Last Session` block |
| `LlmHandoffWriter` | `sleep/handoff_writer.rs` | Production LLM impl wired via `ProviderService` |
| `format_corrections_block` | `runner/invoke_bootstrap.rs` | Always-injects active correction facts at session start (independent of recall) |
| `format_goals_block` | `runner/invoke_bootstrap.rs` | Injects `state=="active"` goals from `goal_adapter.list_active()` |
| Targeted recall pass | `runner/invoke_bootstrap.rs` | Second `recall_unified` call using `handoff.latest.summary` as query, formatted as `## Context from Last Session` |

### Phase 2 — Pattern Abstraction

| Component | File | Responsibility |
|-----------|------|----------------|
| `CorrectionsAbstractor` | `sleep/corrections_abstractor.rs` | Promotes 3+ correction facts to a single `schema` category fact via LLM abstraction. In-memory `Mutex<Option<Instant>>` throttle |
| `AbstractionLlm` trait | `sleep/corrections_abstractor.rs` | LLM judge interface (mockable) |
| `LlmCorrectionsAbstractor` | `sleep/corrections_abstractor.rs` | Production LLM impl |

### Phase 3 — Conflict Resolution

| Component | File | Responsibility |
|-----------|------|----------------|
| `ConflictResolver` | `sleep/conflict_resolver.rs` | Scans schema-fact pairs by embedding cosine ≥ 0.85, LLM-judges contradictions, calls `supersede_fact(loser, winner)`. In-memory throttle |
| `ConflictJudgeLlm` trait | `sleep/conflict_resolver.rs` | LLM judge interface |
| `LlmConflictJudge` | `sleep/conflict_resolver.rs` | Production LLM impl |
| `cosine` helper | `sleep/conflict_resolver.rs` | f32 cosine similarity (defensive against empty / mismatched lengths) |
| `pick_winner` helper | `sleep/conflict_resolver.rs` | Higher-confidence wins, tie-break by `updated_at` |

### Phase 4 — Belief Network Foundation

| Component | File | Responsibility |
|-----------|------|----------------|
| `decay_entity_confidence` / `decay_relationship_confidence` | `stores/zero-stores/src/knowledge_graph.rs` (trait), `stores/zero-stores-sqlite/src/knowledge_graph.rs` (impl) | Batch-apply temporal decay to KG `confidence` columns based on `last_seen_at` age, floor at `min_confidence`, transaction-wrapped |
| `DecayEngine::decay_kg_confidence` | `gateway/gateway-execution/src/sleep/decay.rs` | Orchestrates both store calls, enabled-guard, returns `KgDecayStats` |
| `evidence` TEXT column | `stores/zero-stores-sqlite/src/knowledge_schema.rs` | Schema-only — preparatory for future contradiction-propagation work. No code populates it yet. |
| `KgDecayConfig` | `gateway/gateway-services/src/recall_config.rs` | Configurable half-lives + floor + skip-recent guard |

---

## 3. Recall Pipeline Changes

`gateway/gateway-execution/src/recall/mod.rs` was modified across all three phases.

| Change | Commit | What it does |
|--------|--------|--------------|
| Real similarity scores extracted from `search_memory_facts_hybrid` instead of synthesizing 0.5 | `158d9d77` | Phase 1 root-cause fix — recall was returning every fact with score 0.5 because the hybrid-search JSON's `score` field was being ignored |
| `min_score` threshold filter (default 0.3) | `c51b9066` + `158d9d77` | Suppresses noise; phase 1 |
| Schema category weight 1.6 added to `RecallConfig` | `cd3421f3` | Phase 2; schemas rank above corrections in recall |
| Superseded-fact retain filter in `recall_facts` legacy path | `2d1c7a80` | Phase 3; placed before sort so we don't waste work scoring then dropping |
| Superseded-fact filter inside `filter_map` in `recall_unified` | `2d1c7a80` | Phase 3; short-circuits before `fact_to_item` |

---

## 4. Settings Additions

`gateway/gateway-services/src/settings.rs` gained a new `MemorySettings` struct nested inside `ExecutionSettings`:

```rust
pub struct MemorySettings {
    pub corrections_abstractor_interval_hours: u32,  // default 24, Phase 2
    pub conflict_resolver_interval_hours: u32,       // default 24, Phase 3
}
```

Also added: `pub use settings::MemorySettings` re-export from `gateway/gateway-services/src/lib.rs`.

The HTTP layer `UpdateExecutionSettingsRequest` in `gateway/src/http/settings.rs` also gained `pub memory: Option<MemorySettings>` for the settings update endpoint.

**For extraction:** `MemorySettings` belongs in the future `zero-memory` crate. The wrapper `ExecutionSettings.memory` field can stay where it is — `gateway-services` would just depend on `zero-memory` for the inner type.

---

## 5. Store-Trait Additions

These trait method additions happened during Phase 3 and represent a real architectural fact: embeddings live in a separate `memory_facts_index` sqlite-vec table since v22 schema, so `get_facts_by_category` returns `embedding: None` always.

| Method | File | Phase | Why |
|--------|------|-------|-----|
| `MemoryFactStore::get_fact_embedding(fact_id) -> Result<Option<Vec<f32>>>` | `stores/zero-stores-traits/src/memory_facts.rs` | Phase 3 | Hydrate embeddings on-demand for `ConflictResolver` pair scoring |
| SQLite impl: one-line delegation to `memory_repo.get_fact_embedding` | `stores/zero-stores-sqlite/src/memory_fact_store.rs` | Phase 3 | (`memory_repository.rs:1121` already had the method) |

**For extraction:** these trait additions stay in `zero-stores-traits` — they're part of the store contract, not the memory crate proper.

---

## 6. Wiring Points (Construction Sites)

The future `zero-memory` crate would export factory functions or builder types. Today, the gateway constructs every memory component manually in `gateway/src/state/mod.rs` between lines ~767–845. The exact construction block has grown to:

1. `Compactor::new(kg_store, compaction_store, embedding_client)`
2. `DecayEngine::new(kg_store, DecayConfig::default())`
3. `Pruner::new(kg_store, compaction_store)`
4. `Synthesizer::new(kg_store, episode_store, memory_store, compaction_store, llm_synth, embedder)`
5. `PatternExtractor::new(episode_store, conversation_store, procedure_store, compaction_store, llm_pattern)`
6. `OrphanArchiver::new(kg_store, compaction_store)`
7. `CorrectionsAbstractor::new(memory_store, compaction_store, llm_abstractor, interval)` *(Phase 2)*
8. `ConflictResolver::new(memory_store, compaction_store, llm_judge, interval)` *(Phase 3)*
9. All assembled into `SleepOps { ... }` literal
10. Passed to `SleepTimeWorker::start_with_ops(...)` with hardcoded `Duration::from_secs(60 * 60)` overall interval

**For extraction:** This block is the natural seam. A `MemoryServices::new(stores, llm_clients, settings)` factory would replace the ~80 lines of imperative construction.

Also: the `SleepOps` struct and `SleepTimeWorker` itself live in `gateway/gateway-execution/src/sleep/worker.rs`. Both should move to the memory crate.

---

## 7. Dependencies the Memory Crate Would Need

Today's memory code reaches into:

| Dep | Used by | Required? |
|-----|---------|-----------|
| `agent_runtime::llm::{ChatMessage, LlmClient, LlmConfig}` | Every `Llm*` production impl | Yes — needs to stay or move to a `zero-memory-llm` adapter |
| `gateway_services::ProviderService` | Every `Llm*` production impl (resolves the default provider) | Awkward — `ProviderService` should arguably not be a memory dep. Could be replaced by an `LlmClientFactory` trait passed in |
| `gateway_services::VaultPaths` | Test harnesses only | Tests-only; not a runtime dep |
| `agent_tools::GoalSummary` | `format_goals_block` in `invoke_bootstrap.rs` | Yes — goals are conceptually orthogonal to memory but injected alongside |
| `zero_stores`, `zero_stores_traits`, `zero_stores_domain`, `zero_stores_sqlite` | All store ops | Yes — stays in stores crates |
| `chrono`, `serde`, `async_trait`, `tracing`, `uuid` | Pervasive | Standard, no friction |

**Blocker:** the `ProviderService` dependency is the main awkwardness. A clean extraction would introduce a `MemoryLlmFactory` trait in `zero-memory` that the gateway implements by wrapping `ProviderService`. This keeps the memory crate provider-agnostic.

---

## 8. Complete Commit Inventory (Phase 1–3)

Listed in reverse-chronological order. All on branch `feat/parallel-delegation-aggregation`.

### Phase E + F — Worker move, MemoryServices factory, gateway wiring collapse (2026-05-13)
- `20d59f9b` refactor(gateway): collapse memory construction into MemoryServices::new — Phase F
- `2b894611` feat(gateway-memory): add MemoryServices factory — Phase E.2
- `3c4e9973` refactor(gateway-memory): move SleepOps + SleepTimeWorker to gateway-memory — Phase E.1

**Effect:** `gateway/src/state/mod.rs` construction block reduced from 104 lines to 41 lines (63-line reduction). All memory subsystem construction now lives in `gateway-memory::MemoryServices::new(MemoryServicesConfig)`. Gateway retains only policy (interval hours, agent_id) — wiring is owned by the factory.

**Extraction complete.** A future memory-feature developer touches only `gateway-memory`.

### Phase D — LLM factory abstraction + 6 LLM impls migrated (2026-05-13)
- `f8adf7b1` refactor(handoff): move HandoffWriter struct via ConversationStore trait — MEM-005
- `87b46052` refactor(gateway-memory): move LlmPairwiseVerifier + use MemoryLlmFactory
- `d014de6a` refactor(gateway-memory): move LlmHandoffWriter + use MemoryLlmFactory
- `137d17c7` refactor(gateway-memory): move LlmConflictJudge + use MemoryLlmFactory
- `0f0806df` refactor(gateway-memory): move LlmCorrectionsAbstractor + use MemoryLlmFactory
- `c442cd3a` refactor(gateway-memory): move LlmPatternExtractor + use MemoryLlmFactory
- `9d0b6eb7` refactor(gateway-memory): move LlmSynthesizer + use MemoryLlmFactory
- `e9099d99` feat(gateway-memory): add MemoryLlmFactory trait + ProviderServiceLlmFactory impl
- `cb926b41` refactor(gateway-memory): move parse_llm_json into gateway-memory::util

**Per-impl LlmClientConfig** preserved exactly: Synthesizer (0.0, 512), PatternExtractor (0.0, 1024), CorrectionsAbstractor (0.0, 512), ConflictJudge (0.0, 256), HandoffWriter (0.2, 256), PairwiseVerifier (0.0, 128).

**MEM-005** bonus: HandoffWriter struct also moved via a one-method extension to `ConversationStore` trait (`get_session_messages`) + hoisting `Message` POD type into `zero-stores-domain`.

### Phase C — Recall module moved to gateway-memory (2026-05-13)
- `f1c3be31` refactor(gateway-memory): move recall module from gateway-execution — Phase C

### Phase B — Sleep components moved to gateway-memory (2026-05-13)
- `bba92b87` refactor(gateway-memory): move HandoffWriter from gateway-execution
- `4ae55c8c` refactor(gateway-memory): move PatternExtractor from gateway-execution
- `4793aa9d` refactor(gateway-memory): move Synthesizer from gateway-execution
- `1e2e3062` refactor(gateway-memory): move Compactor from gateway-execution
- `dec59f88` refactor(gateway-memory): move ConflictResolver from gateway-execution
- `1a89ac97` refactor(gateway-memory): move CorrectionsAbstractor from gateway-execution
- `312a8ea5` refactor(gateway-memory): move OrphanArchiver from gateway-execution
- `89ad46ac` refactor(gateway-memory): move Pruner from gateway-execution
- `59a660a1` refactor(gateway-memory): move DecayEngine from gateway-execution
- `aa98a131` chore(gateway-memory): scaffold sleep submodule + add deps for Phase B

**Note:** `HandoffWriter` struct itself stayed in gateway-execution (concrete-type dep would cycle through zero-stores-sqlite). Trait + read_handoff_block + supporting types + 5 of 10 tests moved. Full struct move tracked as MEM-005.

### Phase A — Memory crate extraction begins (2026-05-13)
- `a1e96a74` feat(gateway-memory): extract config types — Phase A of memory crate extraction

### Phase 4 Foundation — KG Confidence Decay (2026-05-13)
- `e3a606a4` feat(sleep): wire KG confidence decay into sleep worker
- `8e7e22ee` feat(sleep): add decay_kg_confidence to DecayEngine
- `d6c11834` feat(kg): add decay_entity_confidence + decay_relationship_confidence store methods
- `c5927dc5` feat(recall): add KgDecayConfig to RecallConfig
- `207e3ed7` feat(kg): add evidence column to kg_entities and kg_relationships

### Phase 3 — Conflict Resolution (2026-05-13)
- `93206722` feat(gateway): wire ConflictResolver into sleep worker
- `ae969b87` feat(sleep): wire ConflictResolver into SleepOps and cycle loop
- `50b1a35d` chore(sleep): export ConflictResolver from sleep mod
- `9ff7002d` feat(sleep): add ConflictResolver — supersede contradicting schema facts *(also added `MemoryFactStore::get_fact_embedding` trait method)*
- `9c9a447c` feat(settings): add conflict_resolver_interval_hours to MemorySettings
- `2d1c7a80` fix(recall): exclude superseded facts from results

### Phase 2 — Pattern Abstraction (2026-05-12 → 13)
- `34d6058e` feat(memory): configurable corrections_abstractor_interval_hours in settings.json *(introduced `MemorySettings` struct)*
- `9b51842e` feat(sleep): wire CorrectionsAbstractor into SleepOps and cycle loop
- `11c1c939` feat(sleep): add CorrectionsAbstractor — distill correction facts into schema facts
- `cd3421f3` feat(recall): add schema category weight (1.6) to RecallConfig

### Phase 1 — Reflective Recall + Handoff (2026-05-10 → 12)
- `9adb8b62` feat(handoff): targeted recall from last session topics
- `158d9d77` fix(recall): use real similarity scores + suppress results below min_score
- `c51b9066` feat(recall): add min_score threshold to RecallConfig (default 0.3)
- `354e5ea9` feat(handoff): inject active goals at session start
- `9d0891c8` feat(handoff): always-inject active corrections at session start
- `c0a9850b` feat(handoff): include tool-call names in LLM summary prompt
- `9c67f2a1` fix(handoff): ward-scoping + real correction_count
- `f807185b` refactor(handoff): store handoff in fact DB instead of flat file
- `b1bf766c` feat(handoff): inject ## Last Session block + wire LlmHandoffWriter
- `e4495dc0` feat(handoff): wire HandoffWriter through execution pipeline
- `3c29221e` refactor(handoff): eliminate double-parse of completed_at in read_handoff_block
- `4868ef75` feat(handoff): export HandoffWriter types from sleep/mod.rs
- `57f057e9` feat(handoff): implement read_handoff_block + 3 tests
- `e4ce967b` fix(handoff): prefix unused stub params with underscore
- `779eaaea` style(handoff): apply cargo fmt to handoff_writer.rs
- `160b2232` feat(handoff): add HandoffWriter with 4 passing unit tests

---

## 9. Plan Documents (Specs)

Each phase has a written plan that describes the *intent* of the change set. These are the authoritative specs to consult during extraction:

- `docs/superpowers/plans/2026-05-12-reflective-memory-phase1-completion.md`
- `docs/superpowers/plans/2026-05-12-recall-min-score-threshold.md`
- `docs/superpowers/plans/2026-05-12-pattern-abstraction.md` *(Phase 2)*
- `docs/superpowers/plans/2026-05-13-conflict-resolution.md` *(Phase 3)*

---

## 10. Suggested Extraction Order (when you're ready)

1. ✅ **DONE (commit `a1e96a74`)** — **First**: Move `RecallConfig` + `MemorySettings` types into the new `zero-memory` crate. Keep references in `gateway-services` as re-exports. Low risk, no logic changes.
2. ✅ **DONE (Phase B, 9 commits)** — **Second**: Move the sleep-time components (`Compactor`, `Synthesizer`, `PatternExtractor`, `Pruner`, `OrphanArchiver`, `CorrectionsAbstractor`, `ConflictResolver`, `HandoffWriter`) — they're self-contained behind their trait-object inputs.
3. ✅ **DONE (Phase D + MEM-005, 9 commits)** — **Third**: Introduce `MemoryLlmFactory` trait, replace direct `ProviderService` dependency. This is the hardest decoupling because every `Llm*` production impl has the same `build_client` shape — there's an opportunity to DRY this into one shared helper.
4. ✅ **DONE (Phase C, 1 commit)** — **Fourth**: Move `recall/mod.rs` + `RecallConfig` into `zero-memory`. Caller in `invoke_bootstrap.rs` stays in gateway because it composes recall with goals + handoff + corrections (all of which are memory) but also with the orchestrator runtime (not memory).
5. ✅ **DONE (Phase E)** — Move `SleepOps` + `SleepTimeWorker` into `gateway-memory`. Replace the imperative construction block in `state/mod.rs` with a `MemoryServices::new(MemoryServicesConfig { ... })` factory call.

   ✅ **DONE (Phase F)** — `gateway/src/state/mod.rs` construction block collapsed from 104 lines to 41 lines via `MemoryServices::new(...)`. All 14 config fields accept trait objects (no concrete-type leaks into gateway).

After extraction, the `gateway` crate should only need:
- `zero-memory::MemoryServices` (factory)
- `zero-memory::SleepTimeWorker` (started in state/mod.rs)
- `zero-memory::RecallEngine` (called from invoke_bootstrap.rs)

Plus a few injected traits for cross-cutting concerns: `MemoryLlmFactory`, the existing store traits.

**Phase 4 foundation already in `zero-memory` candidates:** the new `KgDecayConfig`, `DecayEngine::decay_kg_confidence`, and `KgDecayStats` all live in the same files as their Phase 1–3 siblings, so the extraction migration touches the same crates.
