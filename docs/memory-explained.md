# How zbot Remembers

## The big idea

A chat transcript is not memory. It captures what happened in order; it doesn't capture what was *learned*. zbot's memory system works differently. Every correction the user gives, every principle the agent infers, every session it completes gets distilled into small **fragments** in a growing notebook. Over time the system gets smarter at writing the right fragments, connecting related ones, retiring contradicting ones, and surfacing the most relevant ones at the start of a new session.

Think of it as the difference between a daily log and a reference book. The log captures everything in sequence; the reference book captures what actually matters, distilled. Both exist, but when the agent starts a new session, it opens the reference book — not the log.

## Architecture at a glance

Until recently, the memory subsystem was spread across more than five files in three different crates. It worked, but adding a new component meant touching the gateway, the execution crate, and the services crate together. As of the `feat/memory-crate-phase-a` branch (Phases A–F shipped 2026-05-13), the entire subsystem lives in a single crate: `gateway/gateway-memory/`. The gateway's wiring code for memory collapsed from 104 lines to 41. Adding a new memory component now touches one crate.

The crate stays decoupled from concrete LLM providers and store backends through trait boundaries. `MemoryLlmFactory` builds LLM clients on demand (production implementation wraps `ProviderService`, but the memory crate itself doesn't know that exists). `ConversationStore`, `MemoryFactStore`, `KnowledgeGraphStore`, `EpisodeStore`, `ProcedureStore`, and `CompactionStore` are all trait objects — the gateway picks the concrete adapter (SQLite today) at startup.

| Concern | Where it lives |
|---------|----------------|
| Config types (`RecallConfig`, `MemorySettings`, `KgDecayConfig`) | `gateway-memory::{...}` |
| Sleep components (Compactor, Synthesizer, PatternExtractor, Pruner, OrphanArchiver, HandoffWriter, CorrectionsAbstractor, ConflictResolver, DecayEngine) | `gateway-memory::sleep::*` |
| Recall pipeline (`MemoryRecall`, scoring, RRF, traversal adapters) | `gateway-memory::recall::*` |
| LLM abstraction (`MemoryLlmFactory`, `LlmClientConfig`) | `gateway-memory::llm_factory` |
| Factory (`MemoryServices::new`) and worker (`SleepTimeWorker`) | `gateway-memory::{services, sleep::worker}` |
| Storage traits and DDL | `stores/zero-stores-traits`, `stores/zero-stores-sqlite` |
| Session-start injection (handoff/goals/corrections/recall) | `gateway-execution::runner::invoke_bootstrap` |
| Wiring & policy (interval hours, agent_id) | `gateway::state::mod` |

`invoke_bootstrap.rs` stays in the gateway-execution crate because it composes memory output with orchestrator-runtime concerns. That's the seam: memory produces blocks, the runtime decides where they go.

## The fragment types

Memory is stored as fragments in the `memory_facts` table. Each fragment has a category (`correction`, `schema`, `strategy`, `user`, `instruction`, `domain`, `pattern`, `ward`, `skill`, `agent`), a confidence score, an embedding, a creation/update timestamp, and a key. Some have `superseded_by` set, in which case they're hidden from recall but kept for audit.

Three concrete examples:

```
key: correction.5b2a
category: correction
content: "Use sentence case in commit titles, not Title Case."
confidence: 0.85

key: schema.corrections.a3f8b2c0
category: schema
content: "Always use sentence case in commit titles and keep subject lines under 72 characters."
confidence: 0.92

key: handoff.latest
category: (sentinel)
content: "Worked on memory-crate Phase F. Collapsed wiring from 104 → 41 lines..."
```

The categories aren't arbitrary — recall weights them differently. Schemas (distilled principles) rank highest. Corrections (raw feedback) rank second. Strategies, user facts, instructions follow. Skill and agent indices are the bottom of the stack because they exist mostly for lookup, not behavior.

## The storage layer

Beneath gateway-memory sit two SQLite files, both managed by the `stores/zero-stores-sqlite` crate.

- **`knowledge.db`** — the live memory store. Tables: `memory_facts` (the notebook), `kg_entities` and `kg_relationships` (the knowledge graph), plus auxiliary `kg_episodes`, `session_episodes`, and a `compaction_log` audit trail.
- **`memory_facts_index`** — an sqlite-vec virtual table holding embeddings, joined to facts by `fact_id`. Embeddings don't live on the `memory_facts` row itself; the row carries content + metadata, the index carries the vector. This split is why `get_facts_by_category` returns `embedding: None` and why Phase 3 needed a separate `get_fact_embedding(fact_id)` trait method on `MemoryFactStore`.
- **FTS5 virtual table** over `memory_facts` for keyword search, kept in sync via triggers.
- **`conversations.db`** — chat history. Separate file, separate concerns: it's the daily log, not the reference book.

A note on the schema: Phase 4 foundation added an `evidence TEXT` column to both `kg_entities` and `kg_relationships`. No code populates it yet — it's provisioning for future contradiction-propagation work (MEM-001). The migration is idempotent: `ensure_evidence_column` checks `PRAGMA table_info` and does nothing if the column is already there.

## How a memory comes back: the recall pipeline

When a session needs memory, the recall pipeline runs. It's a hybrid: keyword search (FTS5) plus semantic search (cosine over the sqlite-vec table) blended via reciprocal-rank fusion, then category-weighted, then filtered. The pipeline lives in `gateway-memory::recall::MemoryRecall`.

Step by step:

1. **Embed the query.** The user message goes through the configured embedding client. If no embedding client is wired, the pipeline degrades gracefully to FTS-only.
2. **Hybrid search.** `search_memory_facts_hybrid` returns candidates, each with a real similarity `score`. Before Phase 1, every fact came back with a synthesized 0.5 — the actual score field in the underlying SQL was being ignored. That bug is fixed.
3. **Apply category weights.** Schema 1.6, correction 1.5, strategy 1.4, user 1.3, instruction 1.2, domain 1.0, pattern 0.9, ward 0.8, skill 0.7, agent 0.7. Unknown categories default to 1.0. See `category_weights` in `RecallConfig`.
4. **Apply ward affinity boost.** Facts whose key starts with the current ward's prefix (or whose category is `ward`) get a 1.3× boost.
5. **Apply temporal decay.** Older facts score lower based on per-category half-lives (correction 90 days, strategy 60, domain 30, user 180, pattern 45, instruction 120). Skill and agent indices skip decay because they're rebuilt each session.
6. **Apply post-hoc penalties.** `contradicted_by` set → score × 0.7. `valid_until` set (i.e. fact has been superseded) → score × 0.1 for `current`-class, × 0.3 for `archival`-class, no penalty for `convention`/`procedural`.
7. **Drop superseded facts** (`superseded_by` set) before sorting — no point scoring items we'll discard.
8. **Filter by min score.** Default `min_score: 0.3`. Results below the floor are dropped. This is the noise gate.
9. **Graph traversal expansion.** If enabled (default true), the pipeline walks 2 hops out from directly recalled entities via `kg_relationships`, with `hop_decay: 0.6` per hop, capped at `max_graph_facts: 5`.
10. **Take top-K** capped by `max_facts: 10` and `max_recall_tokens: 3000`.

`recall_unified` is the modern entry point: it pulls from facts, wiki articles, procedures, graph ANN, previous-episode chains, and active goals, then fuses everything via RRF with `intent_boost` against active goals. Each source is silently skipped if its store isn't wired — the caller gets whatever's available.

For the full surface, see `gateway-memory::recall::mod.rs` and the `RecallConfig` definition.

## How memory grows: the sleep-time pipeline

Every hour (the overall cycle interval, configurable but defaulted), a background worker runs the sleep cycle. The worker is `SleepTimeWorker` and the bundle of components it drives is `SleepOps`. Each component does one thing.

1. **Compactor** — finds near-duplicate KG entities, asks an LLM judge (`PairwiseVerifier`) if they should merge, merges the winners. Confidence + mention counts roll up.
2. **Synthesizer** — looks at cross-session entity co-occurrences, asks an LLM to extract a `strategy` fact (e.g. *"when postgres-timeout recurs in deploy code, prefer jittered exponential backoff"*).
3. **PatternExtractor** — looks at tool-call sequences across episodes, abstracts procedural patterns into the `procedures` store.
4. **OrphanArchiver** — archives entities with no relationships that haven't been touched recently. They aren't deleted, just marked archival.
5. **CorrectionsAbstractor** (Phase 2) — fetches all `correction` fragments for an agent. If 3+ accumulate, asks an LLM whether they share a common theme. If yes with sufficient confidence, writes a single `schema.corrections.*` fragment that ranks above the raw corrections. Has its own throttle: skips if it ran within `corrections_abstractor_interval_hours`.
6. **ConflictResolver** (Phase 3) — scans schema-fact pairs by embedding cosine ≥ 0.85, sends contradicting pairs to an LLM judge, marks the loser with `superseded_by`. Higher confidence wins; ties broken by recency. The loser is hidden from recall but kept for audit. Throttled by `conflict_resolver_interval_hours`.
7. **DecayEngine** (Phase 4 foundation) — applies temporal decay to KG entity + relationship `confidence` columns based on `last_seen_at`, floored at `min_confidence`, skipping rows newer than `skip_recent_hours`. Transaction-wrapped.
8. **Pruner** — soft-deletes entities flagged by DecayEngine as below threshold.

Each LLM-based component has its own configurable throttle interval so token spend is bounded — the overall cycle is hourly, but the expensive components (CorrectionsAbstractor, ConflictResolver) only fire once a day by default. Each one is constructed with its own `LlmClientConfig` (temperature + max-tokens) tuned for its task: Synthesizer runs at temperature 0.0 / 512 tokens, HandoffWriter at 0.2 / 256, the judge components at 0.0 / very tight budgets.

## A session's life cycle

A new session opens. Before the agent generates its first token, `invoke_bootstrap.rs` injects up to five memory blocks into the executor's context:

1. **Read handoff.** Looks up `handoff.latest` for this agent, formats it as a `## Last Session` block. Ward-scoped: if the last session was in a different ward, the handoff is skipped (you don't want maritime-tracking handoff bleeding into a finance session).
2. **Inject active goals.** Pulls `state == "active"` goals from `goal_adapter.list_active()`, formats as `## Active Goals`.
3. **Inject corrections.** Always-on. Fetches all active `correction` category facts (not just whatever recall happened to surface), formats as `## Active Corrections`. This is what makes corrections stick — recall can miss, but the corrections block is unconditional.
4. **Targeted recall from handoff.** A second `recall_unified` call using the handoff summary as the query string. Captures topical context from last time even when the user's first message in this session is short or generic. Formatted as `## Context from Last Session`.
5. **User-query recall.** The standard `recall_unified` pass against the actual user message.

Mid-session, the same recall pass re-runs every 5 turns (configurable: `mid_session_recall.every_n_turns`) to capture topic drift. New facts that score above `min_novelty_score: 0.3` get injected.

At session end, **HandoffWriter** kicks in:

1. Pulls the session's chat history via `ConversationStore::get_session_messages`.
2. Sends it to an LLM (the production impl, `LlmHandoffWriter`, at temperature 0.2 / 256 tokens) for compact summarization, including tool-call names so the next session knows what was actually attempted.
3. Writes the summary back to `memory_facts` under two keys: `handoff.latest` (overwrites the previous) and `handoff.{session_id}` (audit trail). Both stored under the sentinel `__handoff__` agent so reads are agent-agnostic.

The handoff is what the *next* session reads as step 1. The loop closes.

## Configurability

Two config files own all the knobs. Missing files fall back to compiled defaults; partial files deep-merge with defaults (user values win per key).

**`settings.json` → `execution.memory`** (struct: `MemorySettings`):

```json
{
  "execution": {
    "memory": {
      "correctionsAbstractorIntervalHours": 24,
      "conflictResolverIntervalHours": 24
    }
  }
}
```

Set either to `0` to run on every hourly cycle.

**`config/recall_config.json`** (struct: `RecallConfig`):

| Knob | Default | What it controls |
|------|---------|------------------|
| `category_weights.schema` | 1.6 | Recall priority for distilled principles |
| `category_weights.correction` | 1.5 | Recall priority for raw corrections |
| `min_score` | 0.3 | Drop facts scoring below this |
| `max_facts` | 10 | Cap on facts per recall pass |
| `max_episodes` | 3 | Cap on previous-episode chain items |
| `max_recall_tokens` | 3000 | Total token budget for recall block |
| `vector_weight` / `bm25_weight` | 0.7 / 0.3 | Hybrid search blend |
| `contradiction_penalty` | 0.7 | Multiplier applied to `contradicted_by` facts |
| `ward_affinity_boost` | 1.3 | Multiplier when key prefix matches ward |
| `temporal_decay.half_life_days.{category}` | 30–180 | Per-category half-lives |
| `graph_traversal.max_hops` / `hop_decay` | 2 / 0.6 | KG expansion depth and decay |
| `kg_decay.entity_half_life_days` | 90 | Half-life for KG entity confidence |
| `kg_decay.relationship_half_life_days` | 90 | Half-life for KG relationship confidence |
| `kg_decay.min_confidence` | 0.01 | Floor for decayed confidence |
| `kg_decay.skip_recent_hours` | 24 | Don't decay rows touched recently |
| `mid_session_recall.every_n_turns` | 5 | How often mid-session recall fires |

The full surface lives in `RecallConfig` — this table covers the headline knobs.

## How it all fits together

A concrete walkthrough. The user gives a correction: *"Use sentence case in commit titles."*

1. **Storage.** The correction is written to `memory_facts` as a `correction` category fragment, key something like `correction.5b2a`. The embedder generates a vector; the vector lands in `memory_facts_index` keyed by `fact_id`. FTS5 indexes the content.
2. **Next session.** Handoff is read first, then goals, then the `## Active Corrections` block is injected unconditionally. The new correction shows up there.
3. **Later in the same session.** The user asks about commit hygiene. Recall fires: the correction's content matches semantically, the hybrid score is high, the `correction` category weight of 1.5 boosts it, it passes `min_score: 0.3`, it lands in the `## Recalled Context` block.
4. **Hours later, after 3+ similar corrections accumulate.** The sleep cycle's `CorrectionsAbstractor` checks its throttle (default 24 hours), pulls all `correction` facts for this agent, and asks the LLM whether they share a theme. The LLM responds with a distilled principle plus a confidence score. A new `schema.corrections.*` fragment is written with category weight 1.6 — ranking *above* the raw corrections. The raw corrections aren't deleted; the schema just outranks them.
5. **Days later, ConflictResolver runs.** It finds another schema fragment that disagrees (different agent or different era). LLM judges the pair, picks a winner by confidence then recency. The loser gets `superseded_by` set and disappears from recall.

That's the closed loop. Raw feedback becomes distilled rules. Distilled rules get conflict-resolved against each other. Stale or wrong rules are retired. The agent gets smarter at remembering what's worth remembering.

## History — the four phases

### Phase 1 — Session handoff and better recall

**The problem:** Every new session started cold. The agent had no memory of what was said last week, no record of corrections, no sense of what the user was actively working on.

**What it does now:** When a session ends, the system runs an LLM over the conversation and writes a compact handoff fragment. The next session reads it as a `## Last Session` block. Alongside it, any active corrections and goals are always injected directly, independent of recall. A second targeted recall pass searches for anything relevant to what the handoff mentions.

Recall itself was also fixed: real similarity scores replaced the synthesized 0.5, and a `min_score: 0.3` threshold drops noise.

**How it helps:** Corrections stick across sessions. The agent knows what you were working on. "Didn't I tell you this last week?" becomes rarer.

### Phase 2 — Pattern abstraction

**The problem:** Five corrections about commit titles become five separate fragments. The notebook gets noisier, not smarter, as the user gives more feedback.

**What it does now:** Each sleep cycle, `CorrectionsAbstractor` checks if there are 3+ correction fragments for an agent. If so, an LLM is asked whether they share a theme. If yes with sufficient confidence, a single `schema` fragment captures the distilled principle. Schema fragments rank above corrections (weight 1.6 vs 1.5).

**How it helps:** The notebook gets smarter the more feedback you give. Ten corrections become one authoritative principle.

### Phase 3 — Conflict resolution

**The problem:** Over months, the notebook accumulates schemas that contradict. "Always rebase" and "Never rebase — always merge." Both surface at session start. Inconsistent behavior follows.

**What it does now:** Each sleep cycle, `ConflictResolver` scans schema pairs by embedding cosine ≥ 0.85. For each plausibly contradicting pair, an LLM judges whether they actually disagree. If yes, the higher-confidence fragment wins (recency breaks ties); the loser gets `superseded_by` set and is filtered out of recall but kept for audit.

**How it helps:** The notebook self-cleans. Stale principles get retired without manual curation.

### Phase 4 — Belief network foundation

**The problem:** Confidence lives on individual fragments but not on knowledge-graph nodes. A person node, a project node, a relationship — none of them carries an uncertainty that decays with contradictions or strengthens with consistent evidence.

**What's shipped:** The DecayEngine now applies temporal decay to `kg_entities.confidence` and `kg_relationships.confidence` based on `last_seen_at`, with per-table half-lives, a confidence floor, and a `skip_recent_hours` guard. The `evidence TEXT` column is provisioned on both KG tables.

**What's still ahead (MEM-001):** Contradiction propagation from facts into the graph, and using KG confidence as a multiplier on graph-traversal hop weight. Both are unscheduled.

## The crate extraction (Phases A–F)

The memory subsystem used to live across 5+ files in `gateway-execution`, `gateway-services`, and `gateway` itself. Adding a new component touched three crates. Phases A–F consolidated everything into `gateway/gateway-memory/`.

- **Phase A — Config types.** `RecallConfig`, `MemorySettings`, `KgDecayConfig` moved into the new crate. Re-exports kept in `gateway-services` for backward compat.
- **Phase B — Sleep components.** Nine components moved one commit at a time: Compactor, Synthesizer, PatternExtractor, Pruner, OrphanArchiver, HandoffWriter (trait + helpers), CorrectionsAbstractor, ConflictResolver, DecayEngine. Each move was independently shippable.
- **Phase C — Recall pipeline.** `recall/mod.rs` plus adapters moved. The composition root in `invoke_bootstrap.rs` stayed put — it composes recall with goals + handoff + corrections, which is gateway concern.
- **Phase D — LLM abstraction.** `MemoryLlmFactory` trait was introduced so the production LLM impls no longer depend directly on `ProviderService`. Six impls (`LlmHandoffWriter`, `LlmSynthesizer`, `LlmPatternExtractor`, `LlmCorrectionsAbstractor`, `LlmConflictJudge`, `LlmPairwiseVerifier`) all migrated. Per-impl temperature and max-tokens preserved exactly.
- **MEM-005 — HandoffWriter struct.** The struct itself initially had to stay in gateway-execution because it took a concrete `Arc<ConversationRepository>`. A one-method extension to `ConversationStore` (`get_session_messages`) plus hoisting the `Message` POD into `zero-stores-domain` resolved the cycle, and the full struct moved.
- **Phase E — Worker and factory.** `SleepOps` and `SleepTimeWorker` moved into `gateway-memory`. A `MemoryServices::new(MemoryServicesConfig)` factory was added that builds every component, assembles `SleepOps`, and starts the worker in one call.
- **Phase F — Gateway collapse.** `gateway/src/state/mod.rs` construction block went from 104 lines to 41. The gateway now only owns policy (interval hours, agent_id) and trait-routed inputs; wiring is owned by the factory.

The headline metric: the gateway's memory wiring shrunk by 63 lines. Adding a new memory component now touches only `gateway-memory`.

## What's still on the roadmap

None of the remaining items is scheduled. Each has a concrete trigger that decides when it's worth picking up — see `memory-bank/future-state/2026-05-13-memory-backlog.md` for full triggers and scopes.

- **MEM-001 — Phase 4b: propagation + recall weighting.** When `memory_facts.contradicted_by` is set, propagate decay to the KG entities and relationships referenced by the fact's `source_episode_ids`. Populate the `evidence` column with a JSON record of the propagation. Use KG `confidence` as a multiplier on graph-traversal hop weight and filter below a threshold. Trigger: the agent acting on contradicted schemas, or low-confidence noise polluting recall.
- **MEM-003 — Cache LLM client in ConflictResolver.** Every `judge()` call currently rebuilds a fresh `LlmClient`. For N pairs in a cycle that's N redundant constructions. Trigger: observed sleep-cycle latency.
- **MEM-004 — Bulk UPDATE for KG decay.** `decay_kg_table` runs a per-row loop because SQLite's `exp()` requires the optional math extension. A single bulk UPDATE would be one round-trip. Trigger: `kg_entities` row count >10k, or sleep-cycle duration >10s.

## One-page recap

- **The metaphor:** memory is the reference book, not the daily log.
- **What's working:** session handoff, always-on corrections, schema abstraction from repeated corrections, conflict resolution between schemas, temporal decay on KG entities and relationships, hybrid recall with category weights and a 0.3 min-score floor.
- **The architecture:** one crate (`gateway-memory/`), trait-routed stores and LLM factory, composition via `MemoryServices::new`.
- **The configuration:** two files — `settings.json` for sleep-cycle intervals, `recall_config.json` for everything recall-related. Missing files fall back to compiled defaults; partial files deep-merge.
- **The next milestone (trigger-based, not scheduled):** MEM-001 propagation, when the agent starts acting on contradicted schemas.
