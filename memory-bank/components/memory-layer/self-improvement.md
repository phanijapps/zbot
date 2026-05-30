# z-Bot Self-Improving Memory

## What makes an agent self-improving

A chat transcript is not memory. Self-improvement means the agent gets better at its job over time without human intervention. That requires three loops running continuously:

1. **Capture** — extract knowledge from every conversation
2. **Distill** — compress raw experience into reusable principles
3. **Retrieve** — surface the right knowledge at the right time

z-Bot runs all three. This document covers what to enable, what to leave off, and why the result outperforms flat memory systems.

---

## What to enable now

Two config changes. Everything else is already on or can wait.

### 1. Belief Network

The single highest-impact feature. Facts are atomic. Beliefs are synthesized stances about the same subject, built from one or more facts, with contradiction detection and event-driven confidence propagation.

**Why it's nearly free:**

- 95%+ of subjects have a single backing fact — zero LLM calls, just a wrapper
- Multi-fact subjects invoke the synthesizer LLM — ~15 calls per sleep cycle for a typical user
- Contradiction budget capped at 20 LLM calls per cycle
- Propagation is event-driven, not polling — fires only when a source fact loses confidence
- Every failure mode degrades gracefully: belief propagator errors log a warning and continue, never blocking fact supersession

```json
"beliefNetwork": {
  "enabled": true
}
```

**What you get:**

- Facts grouped by subject into beliefs with derived confidence
- LLM-judged contradiction detection between belief pairs (logical vs tension)
- Event-driven propagation: when a fact is superseded, single-source beliefs retract, multi-source beliefs get re-synthesized
- Beliefs searchable in recall alongside facts, surfaced under their own `## Active Beliefs` heading
- Human-auditable reasoning stored on every contradiction row

### 2. Query Gate (Self-RAG)

An LLM call before hybrid search classifies the user's input. Failure-safe by design: any error, timeout, or malformed response falls back to `Direct(raw_input)`. The system always retrieves something.

```json
"queryGate": {
  "enabled": true
}
```

**What you get:**

- `Skip` — small talk and self-contained questions skip recall entirely (no noise injection)
- `Direct(query)` — clean single-topic questions use the query as-is
- `Split([q1, q2, q3])` — multi-topic questions get decomposed, each subquery runs independently, results dedup-merge by fact ID

Cost: 200-800ms per recall. Acceptable for the quality gain.

---

## What's already on by default

These ship enabled and require no config changes:

| Feature | What it does |
|---------|--------------|
| **Hybrid recall** | FTS5 BM25 + sqlite-vec cosine similarity, fused via reciprocal rank fusion |
| **Knowledge graph** | Entities + typed relationships extracted from conversations, with graph traversal expansion at recall time (max 2 hops, 0.6 decay per hop) |
| **Session distillation** | Post-session LLM extraction of facts, entities, relationships, episodes, and procedures from conversation transcripts |
| **Sleep cycle (hourly)** | 14 named workers: Compactor, Synthesizer, PatternExtractor, CorrectionsAbstractor, ConflictResolver, DecayEngine, OrphanArchiver, Pruner, Verifier, and more |
| **Confidence decay** | Exponential decay on entities and relationships (90-day half-life), floor at 0.01 |
| **Bi-temporal model** | Every fact and relationship tracks when it became true and when it stopped, separate from when it was written |
| **Category-weighted rescore** | Fragments rescored by category (schema 1.6, correction 1.5, user 1.3, down to skill 0.7), contradiction penalty, ward affinity boost |
| **Mid-session recall** | Recall re-fires every 5 turns to catch topic drift |
| **Ward scoping** | Memory partitioned by project ward with 1.3x affinity boost for same-ward facts |

---

## What to leave off until validated

| Feature | Status | Why wait |
|---------|--------|----------|
| **MMR diversity rerank** | Off | Available behind `execution.memory.mmr.enabled`; needs validation before becoming default. |
| **Hierarchical Memory** | Off | Newest and most complex feature. K-means clustering + LLM aggregate synthesis + LCA topical map walks. Let the Belief Network bake first. |

These opt-in features are structurally sound but lack the empirical validation
to ship as defaults. Add and maintain a dedicated recall validation corpus
before flipping them on by default.

---

## How this compares to flat memory

Most agent memory systems (including Hermes) store flat facts with semantic search. z-Bot with Belief Network + Query Gate enabled operates at a different depth:

| Capability | Flat memory | z-Bot |
|-----------|-------------|-------|
| Storage | Key-value facts with embeddings | 4-layer: fragments → knowledge graph → beliefs → hierarchical aggregates |
| Retrieval | Vector similarity search | 12-step pipeline: embed → query gate → hybrid search → graph traversal → belief surfacing → rescore → MMR → threshold → bi-temporal filter |
| Temporal model | Timestamp per fact | Bi-temporal interval (valid_from / valid_until) with point-in-time recall |
| Conflict handling | None | Contradiction detection, LLM-judged classification, event-driven propagation |
| Maintenance | Manual or none | 14-worker sleep cycle with decay, compaction, synthesis, abstraction, pruning |
| Query quality | Single vector pass | Query decomposition, subquery merging, diversity rerank |

---

## The closed loop

1. User has a conversation. Session ends.
2. **Distillation** extracts facts, entities, and relationships from the transcript.
3. **Sleep cycle** synthesizes beliefs from related facts, detects contradictions, resolves conflicts, abstracts corrections into schemas, decays stale knowledge.
4. Next session opens. **Bootstrap** loads handoff → goals → corrections → beliefs → recalled context.
5. User asks a question. **Query gate** classifies it. **Hybrid search** retrieves facts, graph traversal expands to related entities, beliefs surface alongside.
6. Agent answers with accumulated knowledge. Loop repeats.

The reference book gets smaller, sharper, and more consistent with each cycle. Raw feedback becomes principles. Principles get arbitrated against each other. Stale principles disappear from current recall without losing their place in history.

---

## Config reference

Add to `settings.json` under `execution.memory`:

```json
{
  "execution": {
    "memory": {
      "queryGate": {
        "enabled": true,
        "maxSubqueries": 4,
        "maxSubqueryLen": 200,
        "timeoutMs": 3000
      },
      "beliefNetwork": {
        "enabled": true,
        "intervalHours": 24,
        "neighborhoodPrefixDepth": 1,
        "contradictionBudgetPerCycle": 20,
        "factConfidenceDropThreshold": 0.3
      }
    }
  }
}
```

All other blocks (mmr, kgDecay, mid_session_recall) use compiled defaults when absent.
