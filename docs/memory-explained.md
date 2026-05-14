# How zbot Remembers

## The big idea

A chat transcript is not memory. It captures what happened in order; it doesn't capture what was *learned*. zbot's memory is what makes the agent more useful over time — not raw history, but a curated reference that survives sessions.

Think of it as the difference between a daily log and a reference book. Every conversation is a page in the log; what gets distilled into the reference book is what the agent actually consults when a new session opens. The log is exhaustive but inert. The reference book is small, opinionated, and re-read constantly.

This document describes how the reference book is written, how it's pruned, how it's queried, and how the agent uses it.

## Two views of memory

Two structures sit side by side and answer different questions.

**Flat fragments** answer *"what should I remember about this topic?"* They live in the `memory_facts` table. Each row is a small, self-contained note with a category (corrections, schemas, user facts, etc.), a content string, a confidence value, an embedding, and a lifecycle status. Fragments are the content layer.

**The knowledge graph** answers *"what is connected to what?"* Entities (people, projects, concepts) and the relationships between them live in `kg_entities` and `kg_relationships`. The graph gives structure: a fact about `postgres-timeout` and another about `service-layer` don't just sit next to each other — there's a typed edge between the underlying entities.

The two are linked by provenance. When a fragment is written, the `source_episode_ids` column ties it back to the conversation it came from, and the same episode IDs are recorded on the entities that conversation produced. That means recall can start from a matched fragment, walk to the entities it referenced, and reach related entities (and their fragments) by graph traversal.

Fragments carry the content; the graph carries the shape.

## The fragment types

Every fragment has a category. The category controls how aggressively recall promotes it. The full taxonomy:

- `correction` — the user told the agent to do X instead of Y. e.g. `correction.5b2a`: *"Use sentence case in commit titles, not Title Case."*
- `schema` — a higher-level principle distilled from multiple corrections. Outranks the corrections it absorbed. e.g. `schema.corrections.a3f8b2c0`: *"Always use sentence case in commit titles and keep subject lines under 72 characters."*
- `strategy` — a cross-session approach the agent has accumulated. *"When postgres-timeout recurs in deploy code, prefer jittered exponential backoff."*
- `user` — durable facts about the user. Role, preferences, expertise areas.
- `instruction` — durable directives that aren't corrections (e.g. project conventions told to the agent).
- `domain` — general domain knowledge.
- `pattern` — procedural patterns abstracted from tool-call sequences across episodes.
- `ward` / `skill` / `agent` — scope-specific metadata; mostly lookup, low recall weight.
- `handoff` — session summaries written at session end. Special sentinel scope.

Every fragment is also in one of four lifecycle states:

- **active** — visible to recall.
- **contradicted** — `contradicted_by` is set. Still visible, but penalized.
- **superseded** — `superseded_by` is set. Hidden from recall, kept for audit.
- **archived** — soft-deleted (KG side: `epistemic_class = 'archival'`).

Together the category and the lifecycle decide whether a fragment surfaces and how heavily it weighs against alternatives.

## The knowledge graph

The graph is the second view of memory. Entities and relationships are extracted from conversations as they happen and from past episodes during background processing.

An **entity** is an addressable thing the agent has talked about. The row in `kg_entities` carries:

- `id`, `agent_id`, `name`, `normalized_name`, `normalized_hash` — identity. The normalized hash exists so re-encounters of the same name produce the same row.
- `entity_type` — `person`, `project`, `concept`, etc.
- `confidence` — starts at `0.8` on creation. Decayed during background processing.
- `mention_count` and `access_count` — how often the entity has been seen versus how often recall has actually pulled it.
- `first_seen_at`, `last_seen_at`, `last_accessed_at` — used by decay and orphan detection.
- `epistemic_class` — `'current'` for active entities, `'archival'` for soft-deleted.
- `source_episode_ids` — provenance back to the conversations that produced it.
- `evidence` — a JSON column reserved for contradiction-propagation records; structured to allow future writes from the conflict-resolution path.

A **relationship** in `kg_relationships` carries a `source_entity_id`, `target_entity_id`, `relationship_type`, and the same confidence / lifecycle columns as entities. A small example:

```
entity:   postgres-timeout (type=Concept, confidence=0.74, mention_count=6)
relation: affects                       (confidence=0.71)
entity:   service-layer    (type=Concept, confidence=0.82, mention_count=12)
```

Both ends have their own confidence and their own `last_seen_at`. The edge does too. None of them is renewed on every recall — only when the entity actually shows up in a new conversation.

**Confidence decays.** A background pass applies exponential decay based on `last_seen_at` with a configurable half-life (default `entity_half_life_days = 90`, `relationship_half_life_days = 90`). Confidence is floored at `min_confidence = 0.01` so nothing decays to literal zero, and rows touched within `skip_recent_hours = 24` are left alone. When an entity has no relationships and hasn't been seen recently, it's flagged as orphan and eventually archived. Archived rows are never deleted — they're just hidden from recall.

**Recall uses the graph as an expansion mechanism.** After the fragment search returns its top candidates, the recall pipeline takes the entities those fragments referenced, walks outward up to `max_hops = 2` along relationships, and adds the entities it finds. Each hop multiplies relevance by `hop_decay = 0.6` — directly connected nodes weigh 0.6×, two hops out weigh 0.36×. The expansion is capped at `max_graph_facts = 5` per recall. Today the traversal doesn't weight edges by their confidence; that's an obvious extension point.

The graph turns "I found one relevant fragment" into "I found one relevant fragment and the three concepts it implicates," even when the user's query didn't name any of them.

## Storage

Two SQLite databases own the physical layout, kept under the agent's data directory (`$XDG_DATA_HOME/agentzero/` on Linux, equivalent paths on macOS / Windows).

**`knowledge.db`** holds the live memory store:

- `memory_facts` — the fragment table. Content + metadata, no embeddings on the row itself.
- `kg_entities` — entities, with confidence and lifecycle columns.
- `kg_relationships` — typed edges, with the same confidence and lifecycle.
- `kg_episodes`, `session_episodes` — provenance from conversations to entities and fragments.
- `compaction_log` (audit trail for KG merges).
- `memory_facts_fts` — an FTS5 virtual table over fragment content, kept in sync with `memory_facts` by triggers.
- `memory_facts_index` — a `sqlite-vec` virtual table holding fact embeddings, joined to fragments by `fact_id`. Embeddings live here, not on the fragment row.

**`conversations.db`** is the daily log: chat history, separate file, separate concerns. The memory subsystem never touches it directly — it goes through the `ConversationStore` trait, which decouples memory from any specific chat backend.

## Recall: finding candidates

Recall is the first half of retrieval. Given a query (the user's message, or a synthetic query the bootstrap constructed), it returns a candidate set scored by raw relevance.

1. **Embed the query.** The caller's embedding client turns the text into a vector. If no embedding client is wired, recall degrades gracefully to keyword-only.
2. **Hybrid search.** `search_memory_facts_hybrid` runs two queries in parallel:
   - **FTS5 keyword match** against `memory_facts_fts.content`. Returns BM25-ranked rows.
   - **Vector cosine similarity** against `memory_facts_index`, when the query embedding is available.
3. **Blend via reciprocal rank fusion.** The two ranked lists are merged with weights `vector_weight = 0.7` and `bm25_weight = 0.3`. RRF is rank-based, so neither score scale dominates the other.
4. **Return candidates.** Each row comes back as a `ScoredFact` with a real `score` derived from the RRF blend.

This is the "search" stage. It deliberately returns more candidates than will survive — typically `limit * 2`. The next stage decides which ones actually deserve a slot in the recall block.

## Rescore: adjusting the scores

Rescore is where category preferences, contradiction penalties, supersession exclusion, and the noise floor are applied. It runs client-side after the SQL search returns, so the logic lives in the recall module — not buried in a SQL view.

The steps run in order:

1. **Category weight multiplier.** Each candidate's score is multiplied by `category_weights[category]`. Defaults:

   | Category | Weight |
   |----------|--------|
   | `schema` | 1.6 |
   | `correction` | 1.5 |
   | `strategy` | 1.4 |
   | `user` | 1.3 |
   | `instruction` | 1.2 |
   | `domain` | 1.0 |
   | `pattern` | 0.9 |
   | `ward` | 0.8 |
   | `skill` | 0.7 |
   | `agent` | 0.7 |
   | unknown | 1.0 |

   Schemas outrank corrections because they're distilled. Skill and agent indices sit at the bottom because they exist mostly for lookup, not for behavior shaping.

2. **Contradiction penalty.** If the fragment's `contradicted_by` column is set, the score is multiplied by `contradiction_penalty = 0.7`. The fragment is still visible — recall just trusts it less.

3. **Temporal decay (class-aware).** If `valid_until` is set, the fragment has been retired in time but not formally superseded. The multiplier depends on `epistemic_class`:
   - `current` → `0.1×` (the replacement is what matters now)
   - `archival` → `0.3×` (historical record; age isn't a defect, but supersession is meaningful)
   - `convention` / `procedural` → no penalty (rule-based, no temporal meaning)
   - unknown → conservative `0.3×`

4. **Ward affinity boost.** If the candidate's ward matches the active ward, the score is multiplied by `ward_affinity_boost = 1.3`. This is what keeps a maritime-tracking conversation pulling maritime-tracking memory instead of bleeding across the agent's whole history.

5. **Supersession filter.** Fragments whose `superseded_by` is set are dropped entirely. No penalty, just exclusion — the conflict resolver decided this fragment lost, and we don't want to revisit that decision on every query.

6. **Min-score threshold.** Any candidate scoring below `min_score = 0.3` is dropped. This is the noise floor: prevents low-relevance fragments from showing up for short, generic queries.

7. **Sort by final score, truncate to top-K.** `max_facts = 10` rows survive. A separate `max_recall_tokens = 3000` cap limits the total size of the formatted recall block.

The output of rescore is the list of fragments the agent actually sees. Everything else has been filtered, demoted, or replaced.

## Graph traversal

Once the surviving fragments are ranked, the pipeline expands the result set by walking the knowledge graph. The expansion is straightforward.

For each top fragment, follow `source_episode_ids` to the entities it referenced. From those entities, walk outward along `kg_relationships`, capped at `max_hops = 2`. Each hop applies `hop_decay = 0.6` to the relevance score the entity inherits from the seed fragment. Cap the total number of additional items at `max_graph_facts = 5`.

What this buys: when the user asks about commit hygiene and a `correction` fragment about "commit titles" matches, the graph traversal can surface related entities like "git workflow," "code review," or "release process" — and pull their fragments into the recall block too — even though none of those words appeared in the query.

The traversal currently treats every edge equally regardless of its `confidence`. Weighting hops by edge confidence is the obvious next step; the schema and decay machinery are already in place to support it.

## What gets injected at session start

When a session starts, the bootstrap reads memory in a specific order. The order matters — the LLM weights earlier context more strongly, so the most authoritative material goes first.

1. **Handoff block** — `## Last Session`. The handoff fragment from the previous session in the same ward, formatted as a compact summary. Ward-scoped on purpose: a maritime-tracking handoff shouldn't bleed into a finance session.
2. **Active goals** — `## Active Goals`. Pulled directly from the goal tracker, every goal with `state == "active"`. Always injected, no recall filter.
3. **Active corrections** — `## Active Corrections`. Every `correction` category fragment, fetched unconditionally. Recall can miss; corrections must not.
4. **Targeted recall from handoff** — `## Context from Last Session`. A separate recall pass that uses the handoff summary as the query string. Captures topical context from last time even when the user's first message is short or generic.
5. **User-query recall** — the standard recall pass against the actual user message.

After session start, recall continues to fire mid-session. Every `mid_session_recall.every_n_turns = 5` turns, the recall pass re-runs with the latest exchange as the query. New fragments that clear `min_novelty_score = 0.3` get injected — this catches topic drift without re-injecting everything the agent already saw.

When the session ends, a handoff writer summarizes the conversation via LLM (with tool-call names included so the next session knows what was attempted, not just what was said) and writes a fresh `handoff.latest` fragment. The loop closes.

## Background maintenance

A worker runs every hour (the overall cycle is configurable but defaults to hourly) to keep the reference book honest. Each job is independent; expensive ones throttle themselves so token spend stays bounded.

- **Compact** — find near-duplicate KG entities, ask an LLM judge whether they should merge, merge accepted pairs. Confidence and mention counts roll up into the survivor.
- **Synthesize** — find cross-session entity co-occurrences, ask an LLM to extract a reusable `strategy` fragment from them.
- **Extract patterns** — find recurring tool-call sequences across episodes, abstract them into procedural `pattern` fragments.
- **Abstract corrections** — when 3+ corrections share a theme, distill them into a single `schema` fragment (recall weight 1.6, outranking the raw corrections 1.5). Throttled to `corrections_abstractor_interval_hours = 24` by default.
- **Resolve conflicts** — scan `schema` fragment pairs by embedding cosine similarity. Send candidate contradictions to an LLM judge. Higher-confidence (ties broken by recency) wins; the loser gets `superseded_by` set and disappears from recall. Throttled to `conflict_resolver_interval_hours = 24` by default.
- **Decay** — apply exponential confidence decay to `kg_entities.confidence` and `kg_relationships.confidence` based on `last_seen_at`. Floored at `min_confidence`, skips rows newer than `skip_recent_hours`. Wrapped in a transaction.
- **Archive orphans** — flag entities with no relationships and old `last_seen_at` for soft-deletion. They're moved to `epistemic_class = 'archival'`, not dropped.
- **Prune** — soft-delete entities flagged by decay as below threshold.

The LLM-using jobs each have their own throttle so the hourly cycle doesn't burn tokens on jobs that don't have new material to chew on.

## Configurability

Two files own every knob.

**`settings.json` → `execution.memory`** controls the LLM-using background jobs' throttles:

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

**`config/recall_config.json`** controls the recall and rescore pipeline. Headline knobs:

| Knob | Default | What it controls |
|------|---------|------------------|
| `category_weights` | (table above) | Rescore step 1 multipliers |
| `min_score` | `0.3` | Rescore noise floor |
| `contradiction_penalty` | `0.7` | Rescore step 2 |
| `ward_affinity_boost` | `1.3` | Matching-ward boost |
| `vector_weight` / `bm25_weight` | `0.7` / `0.3` | RRF blend |
| `max_facts` | `10` | Recall top-K |
| `max_recall_tokens` | `3000` | Hard ceiling on recall block size |
| `graph_traversal.max_hops` | `2` | Expansion depth |
| `graph_traversal.hop_decay` | `0.6` | Per-hop relevance decay |
| `graph_traversal.max_graph_facts` | `5` | Cap on graph-expansion additions |
| `kg_decay.entity_half_life_days` | `90` | Entity confidence half-life |
| `kg_decay.relationship_half_life_days` | `90` | Relationship confidence half-life |
| `kg_decay.min_confidence` | `0.01` | Confidence floor |
| `kg_decay.skip_recent_hours` | `24` | Don't decay rows touched recently |
| `mid_session_recall.every_n_turns` | `5` | Mid-session recall cadence |
| `mid_session_recall.min_novelty_score` | `0.3` | Score floor for mid-session injection |

A missing file falls back to compiled defaults. A partial file deep-merges with defaults — user values win per key, missing keys keep their default. A corrupted file logs a warning and uses defaults.

The MMR knobs (`mmr.enabled`, `mmr.lambda`, `mmr.candidate_pool`) are also configurable via `settings.json → execution.memory.mmr` for a unified config surface — `settings.json` wins if both are set.

Cross-encoder reranking (BGE-reranker-base via fastembed-rs / ONNX) runs after MMR and before the final truncate. It is off by default because enabling triggers a one-time ~280 MB model download. Configure via `settings.json → execution.memory.rerank` or `recall_config.json` under the `rerank` key:

```json
{
  "rerank": {
    "enabled": true,
    "model_id": "BAAI/bge-reranker-base",
    "candidate_pool": 20,
    "top_k_after": 10,
    "score_threshold": 0.0
  }
}
```

Same precedence as MMR: `settings.json` wins. Model load failures or inference errors log a warning and fall back to the un-reranked candidates — recall never breaks because of reranking.

## A closed-loop walkthrough

One concrete trace, following a single correction through every stage of the system.

1. **The correction lands.** The user says: *"use sentence case in commit titles."* A fragment is written to `memory_facts`:
   ```
   key: correction.5b2a
   category: correction
   content: "use sentence case in commit titles"
   confidence: 0.85
   ```
   The embedding client turns the content into a vector; the vector is inserted into `memory_facts_index` keyed by `fact_id`. The FTS5 trigger indexes the content for keyword search.

2. **The graph catches it.** If the conversation mentioned addressable concepts ("commit titles," "git workflow"), entities are extracted into `kg_entities` and any explicit relationships into `kg_relationships`. The fragment's `source_episode_ids` and the entities' `source_episode_ids` link them.

3. **Next session opens.** The bootstrap reads in order: handoff → goals → corrections → handoff-recall → user-recall. The active-corrections block fetches all `correction` fragments for this agent and injects them unconditionally. The new correction shows up in `## Active Corrections` regardless of whether anyone queried for it.

4. **Mid-session match.** Hours later, the user asks: *"can you write the commit message for this?"* Recall fires. Hybrid search returns `correction.5b2a` near the top — FTS5 matches "commit," vector search matches the semantic content. RRF blends them. The category weight `1.5` is applied. Score above `min_score = 0.3`. Not contradicted, not superseded. Top-K includes it.

5. **The correction accumulates.** Over weeks, three more corrections about commit hygiene arrive: `correction.7c4f`, `correction.b219`, `correction.f08d`. The corrections-abstractor cycle (default every 24 hours) sees four corrections share a theme, asks an LLM to distill them, gets back a confident summary. A new fragment is written:
   ```
   key: schema.corrections.a3f8b2c0
   category: schema
   content: "Always use sentence case in commit titles and keep subject lines under 72 characters."
   confidence: 0.92
   ```
   The raw corrections aren't deleted. The schema (weight `1.6`) just outranks them (weight `1.5`).

6. **A conflict appears.** Months later, a different schema fragment is written from a different conversation — *"Use Title Case for commit titles to match the project's existing convention."* The conflict-resolver cycle finds the pair by embedding cosine `≥ 0.85`, sends them to an LLM judge, picks the higher-confidence winner. The loser gets `superseded_by = winner.id`. From the next recall onward, the loser is filtered out entirely — not penalized, not visible.

The reference book gets smaller, sharper, and more consistent with each cycle. Raw feedback becomes principles; principles get arbitrated against each other; stale principles disappear without anyone curating them by hand.

## Recap

- **Two views:** flat fragments hold the content, the knowledge graph holds the structure. Both have confidence and a lifecycle.
- **Recall:** hybrid FTS5 + vector cosine, blended via reciprocal rank fusion. Returns a candidate set.
- **Rescore:** category weights, contradiction penalty, temporal decay, ward boost, supersession filter, min-score gate. Decides which candidates survive.
- **Graph traversal:** expand from recalled fragments to related entities, with per-hop decay.
- **Background:** an hourly worker compacts the graph, abstracts schemas, resolves conflicts, decays confidence, and archives orphans.

The daily log records everything. The reference book is what the agent reads.
