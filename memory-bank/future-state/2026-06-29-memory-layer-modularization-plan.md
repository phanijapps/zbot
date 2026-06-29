# Memory-Layer Plan — Design, Quality, Composability, DB-Portability

**Date:** 2026-06-29 (updated) · **Inputs:** data dictionary, gap analysis, data-quality assessment, serena call graph, trait/conformance verification.
**Goal:** make the memory layer **better** — fix the **design** (composability + DB-portability via SOLID traits, no hard datasource coupling) and fix **quality** (extraction noise, calibration, duplication), incrementally, with zero impact on current code until each cutover.

> **Correction (2026-06-29):** an earlier draft claimed vector/FTS search was "not behind a trait." Verified wrong. Vector + hybrid search **are** already on the domain store traits (`MemoryFactStore::recall_facts`, `search_procedures_by_similarity`, `search_wiki_hybrid`, `search_entities_by_name_embedding`); embedding generation is behind `EmbeddingClient`. DB-portability of the *consumer-facing* operations is ~90% complete. The real gaps are narrower — see §6.

> **Tooling:** codemem reindexed but its Rust symbol graph is partial; **serena LSP is the authoritative call-graph tool** here.

---

## 1. Current structure (summary — full detail in the data dictionary)

- **Store layer is already SOLID/DI at the interface level** — 4 crates: `zbot-stores-domain` (types) · `zbot-stores-traits` (**13 async traits**) · `zbot-stores-conformance` (**generic portability test harness**) · `zbot-stores-sqlite` (impl).
- **Fragmentation is in the *implementation*** — parallel repos + raw SQL bypass the traits: `memory_facts` (2 repos + raw SQL in `sleep/decay.rs`, `reindex.rs`), `kg_entities`/`kg_relationships` (6/5 write sites across `sleep/*` + `kg_backfill.rs`), `procedures` (legacy + trait repo), legacy `memory_repository`/`procedure_repository` dual paths.
- **`gateway-memory` conflates two layers** — Retrieval (`recall/`, sync, 4 call sites in `gateway-execution`) + Consolidation (`sleep/` 17 modules + distillation, async via `SleepTimeWorker` + HTTP `trigger_distillation`).
- **Quality issues** — extractor ingests code symbols/paths/URLs/raw JSON as facts/entities; fact confidence collapsed (mean 0.92, 311 at 1.0); duplication ~12%; type-casing bug (Concept vs concept); flat relationship hierarchy; 37% orphan entities; dead tables (`recall_log`, `tool_results`, `kg_goals`, `kg_causal_edges`, `kg_episode_payloads`).

---

## 2. What's already trait-fronted (DB-portability foundation — the good news)

| Concern | Behind a trait? | Where |
|---|---|---|
| Relational CRUD (13 subsystems) | ✅ | `zbot-stores-traits` (MemoryFactStore, KnowledgeGraphStore, ProcedureStore, BeliefStore, EpisodeStore, KgEpisodeStore, WikiStore, CompactionStore, ConversationStore, GoalStore, RecallLogStore, DistillationStore, OutboxStore, BeliefContradictionStore) |
| **Vector / hybrid search** | ✅ | `recall_facts` (hybrid), `search_procedures_by_similarity`, `search_episodes_by_similarity`, `search_wiki_hybrid`, `search_entities_by_name_embedding` — all on the domain traits |
| Embedding **generation** | ✅ | `EmbeddingClient` trait (`runtime/agent-runtime/src/llm/embedding.rs`) — model is swappable, decoupled from storage |
| **Conformance (portability proof)** | ✅ | `zbot-stores-conformance` — `fn entity_round_trip<S: KnowledgeGraphStore>(store: &S)`; each impl runs these; drift fails assertions |
| Low-level vector storage (`upsert`/`query_nearest`) | 🟡 exists but in **sqlite crate** (`vector_index.rs`) — impl-internal, not a portability contract | optional hoist to traits crate |

**Implication:** a new DB implements the 13 traits (CRUD + their search methods) however it likes (pgvector, tsvector, dedicated ANN), passes conformance, and swaps in. No consumer changes.

---

## 3. Target modular architecture

```
zbot-stores-domain              ← domain types (EXIST)
zbot-stores-traits              ← 13 subsystem traits (EXIST) + VectorIndex hoist (opt) + UnitOfWork (NEW)
zbot-stores-conformance         ← portability harness (EXIST; extend with vector + txn scenarios)
zbot-stores-sqlite              ← one impl (consolidate bypasses into it)

zbot-retrieval (NEW, from recall/)      ← Retrieval Layer behind a `Retrieval` trait
zbot-consolidation (NEW, from sleep/+distillation) ← behind a `Consolidation` trait

memory-facade (NEW, optional)   ← `Memory` trait = Episodic + Semantic + Procedural composition
knowledge-facade (NEW, optional)← `Knowledge` trait = Facts + Graph + Vector + Taxonomy composition
```
The facades are **additive composition over the fine-grained traits** — they give "Memory" and "Knowledge" as two first-class composable units (what you asked for) without disturbing the existing per-subsystem traits that actually enable DB-switching.

---

## 4. Four workstreams

- **Design (composability + DB-portability):** route all access through the 13 traits; split Retrieval/Consolidation behind traits; add `Memory`/`Knowledge` facades; add the `UnitOfWork` transaction abstraction; hoist `VectorIndex` (optional).
- **Quality (fix at the source):** extraction filters (reject code/path/URL/symbols), write-time dedup, type-casing normalization, confidence recalibration, taxonomy discipline.
- **Tightening (consolidation):** one repo per subsystem; retire parallel repos + raw SQL.
- **Loop-closure:** wire `ConflictResolver`, `kg_episode_payloads` writer, inter-cluster synthesis; drop/wire dead tables; bi-temporal/vector symmetry.

---

## 5. Strategy — incremental strangler (your pattern, per step)

For each step: **rewrite/consolidate alongside → validate (cargo test + conformance + a recall-replay check) → cutover in one PR → retire old.** No data migration, no second DB, zero impact on live paths until cutover. (In-place trait consolidation is preferred over a parallel rewrite *because the traits already exist* — there's nothing to rewrite, only to route.)

---

## 6. DB-portability — the REAL gaps (corrected)

1. **Raw-SQL bypasses escape the contract.** The `sleep/*` modules + `kg_backfill.rs` + legacy `memory_repository`/`procedure_repository` write SQL directly. A "DB switch" would leave these hitting SQLite. **Closing them (Track A) is the #1 portability prerequisite.**
2. **Cross-store transactions.** Distillation atomically writes facts + entities + relationships + episodes in one SQLite txn. A multi-DB future needs a **`UnitOfWork`/transaction trait** (begin/commit/rollback across stores) or an explicit decision to accept per-store eventual consistency. **This is the one genuinely new contract to design.**
3. **Low-level `VectorIndex` trait is in the wrong crate** (sqlite, not traits). Optional hoist — not a blocker since search is already trait-fronted at the domain level.
4. **Trait-doc SQL leakage.** `recall_facts` doc says "FTS5" — the *signature* is DB-neutral; only the doc names the impl. De-SQL the doc.

**The DB-switch path, once gaps close:** new impl crate → implement 13 traits (+ their search methods) → pass the (extended) conformance suite → swap wiring. Vector/FTS port with it because they're behind the same trait methods.

---

## 7. Phased plan

| Phase | Workstream | Delivers | Retires |
|---|---|---|---|
| **P0** | Tightening | One trait-backed repo per subsystem; route all raw-SQL bypasses through traits; **golden recall-replay check** | parallel repos, raw SQL in `sleep/*`+`kg_backfill`, dual legacy repos |
| **P1** | Design | `VectorIndex` hoisted to traits crate (opt); conformance extended with vector + hybrid-search scenarios; de-SQL trait docs | impl-internal vector trait coupling |
| **P2** | Design | `UnitOfWork` transaction trait + conformance txn scenarios; distillation rewritten to use it | implicit single-sqlite-txn assumption |
| **P3** | Design | Extract `zbot-retrieval` behind `Retrieval` trait; flip the 4 call sites | `gateway-memory/recall/` direct-struct coupling |
| **P4** | Design | Extract `zbot-consolidation` behind `Consolidation` trait; flip worker + trigger | `gateway-memory/sleep/` in-place |
| **P5** | Design (optional) | `Memory` + `Knowledge` facade traits composing the fine-grained stores | — |
| **P6** | Quality | Extraction filters + dedup + casing + recalibration + taxonomy at the (now single) write chokepoint | extractor noise (~30% facts, ~10-15% entities) |
| **P7** | Loop-closure | Wire `ConflictResolver` (construct in default SleepOps), `kg_episode_payloads`, inter-cluster synthesis; drop/wire dead tables; bi-temporal/vector symmetry | open contradictions, dead tables, flat hierarchy |

Each phase is independently shippable + reversible. **Quality (P6) deliberately comes after P0** — the write boundary must be a single chokepoint before guards land there.

---

## 8. Sequencing vs the 4 behavior specs

Structure first, behavior on the clean base: Spec 1 + Spec 3 land inside `zbot-retrieval` (P3); Spec 4 inside `zbot-consolidation` (P4/P7); Spec 2 in the procedural subsystem. (Specs parked per your instruction — sequenced, not started.)

---

## 9. Decisions (still open)

1. **Interface-level vs physical DB split** → recommend **interface-level** (one DB). The traits + conformance already deliver portability without a split.
2. **`UnitOfWork` transaction model** → cross-store atomic txn trait, or accept per-store eventual consistency? (Needed for P2.)
3. **Facades now or later** → P5 is optional; recommend doing it after P3/P4 so the facades compose already-trait-fronted layers.
4. **Recall-replay tolerance** → exact-match vs tolerance band for nondeterministic recall. (Needed to start P0.)

---

## 10. Next step

Start **P0** (consolidate `memory_facts` access — retire `MemoryRepository` fact path → `GatewayMemoryFactStore`, kill the 2 raw-SQL sites) as a focused work-loop with red-green tests proving "same rows written before/after." It's the smallest, unblocks P6 quality guards, and proves the consolidation pattern before the messier `kg_*` sites.

*Cross-references: `2026-06-29-memory-layer-data-dictionary.md` · `2026-06-29-memory-data-quality-assessment.md` · `2026-06-28-memory-application-gap-analysis.md` · [[project_memory_modularization]] · [[project_memory_application_gaps]] · [[project_memory_crate_extraction]]*
