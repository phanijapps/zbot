# Knowledge Graph — v22 Architecture

## Core concept

Four primary object classes make up the graph. All live in `knowledge.db`:

- **Entities** (`kg_entities`) — typed nodes: `Person`, `Organization`,
  `Location`, `Event`, `Concept`, `File`, ... Each carries an
  `epistemic_class` and bitemporal columns (`valid_from`, `valid_until`,
  `compressed_into`).
- **Relationships** (`kg_relationships`) — directional typed edges,
  `UNIQUE(source, target, type)`.
- **Aliases** (`kg_aliases`) — surface-form variants of an entity's name.
  Drives the resolver's O(1) stage 1.
- **Episodes** (`kg_episodes` + `kg_episode_payloads`) — one row per
  extraction run, recording `source_type`, `source_ref`, `content_hash`,
  and status (`pending` → `running` → `done` | `failed`).

Causal edges (`kg_causal_edges`) are kept separate from generic
relationships. Merge/prune audit goes to `kg_compactions`. Full column
lists in [`data-model.md`](./data-model.md).

## Entity resolver (Phase 1c)

### Cascade

Two stages execute on every `store_entity`. Source:
`services/knowledge-graph/src/resolver.rs:36`.

```
candidate Entity
        │
        ▼
 ┌──────────────────────────────────────────────────────────┐
 │ Stage 1: alias lookup                                    │
 │   normalize_name(candidate.name) = normalized_form       │
 │   SELECT a.entity_id FROM kg_aliases a                   │
 │     JOIN kg_entities e ON e.id = a.entity_id             │
 │    WHERE a.normalized_form = ?1                          │
 │      AND e.entity_type     = ?2                          │
 │      AND (e.agent_id = ?3 OR e.agent_id = '__global__')  │
 │   --> idx_aliases_normalized → O(1)                      │
 └───────────────┬──────────────────────────────────────────┘
                 │ miss
                 ▼
 ┌──────────────────────────────────────────────────────────┐
 │ Stage 2: embedding ANN                                   │
 │   SELECT entity_id, distance FROM kg_name_index          │
 │    WHERE name_embedding MATCH ?1                         │
 │    ORDER BY distance LIMIT 10                            │
 │   filter by entity_type + agent                          │
 │   accept first distance ≤ 0.26                           │
 │   (L2_sq ≤ 0.26 ⇔ cosine ≥ 0.87 on L2-normalised vecs)   │
 └───────────────┬──────────────────────────────────────────┘
                 │ miss
                 ▼
          ResolveOutcome::Create
```

`normalize_name` (resolver.rs:65): lowercase, trim, strip honorifics
(`Dr.`, `Mr.`, `Mrs.`, `Ms.`, `Prof.`, `Sir`, `Shri`, `Smt`), strip
`.`/`,`.

`MatchReason::ExactNormalized` or `MatchReason::EmbeddingSimilarity` is
returned with the merge outcome for observability.

A third stage — **LLM pairwise verify** — is defined as the
`PairwiseVerifier` trait
(`gateway/gateway-execution/src/sleep/compactor.rs:34`) but is not wired
into the live resolver. It lives with the Compactor.

### Self-alias seeding

Every new entity written via `store_entity`
(`services/knowledge-graph/src/storage.rs:1713`) seeds one
`kg_aliases` row:

```sql
INSERT OR IGNORE INTO kg_aliases (
    id, entity_id, surface_form, normalized_form,
    source, confidence, first_seen_at
) VALUES (?, ?, ?, ?, 'extraction', 1.0, ?)
```

This means the next mention of the same surface form short-circuits the
cascade at stage 1 — no embedding work needed. Merges append
`source='merge'` aliases (`storage.rs:1684`).

### Merge semantics

`GraphStorage::merge_entity_into(loser_id, winner_id)`
(`services/knowledge-graph/src/storage.rs:1416`) runs one transaction:

1. Drop would-be-duplicate relationships (`DELETE` any loser edge that
   would collide with an existing winner edge under the
   `UNIQUE(source, target, type)` constraint).
2. Re-point remaining relationships: `UPDATE kg_relationships SET
   source_entity_id = winner WHERE source_entity_id = loser` (same for
   target).
3. Transfer aliases: `UPDATE OR IGNORE kg_aliases SET entity_id = winner
   WHERE entity_id = loser`, then `DELETE FROM kg_aliases WHERE
   entity_id = loser` for any IGNORE losers.
4. Mark `kg_entities.compressed_into = winner` for loser.
5. `DELETE FROM kg_name_index WHERE entity_id = loser` so ANN stops
   surfacing the loser.

Returns a `MergeResult` with `relationships_repointed`,
`aliases_transferred`, and `duplicate_relationships_dropped`.

The audit row is written by the caller via
`CompactionRepository::record_merge` — the merge function itself is
audit-agnostic.

## Compactor (Phase 4)

Source: `gateway/gateway-execution/src/sleep/compactor.rs`.

- Thresholds: `cosine ≥ 0.92` (default), `per_type_limit = 50`.
- Scans five entity types: `Person`, `Organization`, `Location`,
  `Event`, `Concept`.
- Duplicate candidates produced by
  `GraphStorage::find_duplicate_candidates(agent_id, type, cosine,
  limit)` (storage.rs:1277). The query joins `kg_name_index`
  (self-join on distance) against `kg_entities`, filtering out
  `compressed_into IS NOT NULL` and `epistemic_class = 'archival'`.
- Per pair:
  1. Optionally ask the `PairwiseVerifier` (off by default in Phase 4).
  2. `pick_loser_winner`: the entity with the smaller
     `mention_count` is the loser; ties go to `b`.
  3. `GraphStorage::merge_entity_into(loser, winner)`.
  4. `CompactionRepository::record_merge(run_id, loser, winner,
     "cosine=0.94")`.

Emits `CompactionStats { candidates_considered, merges_performed,
merges_skipped_by_verifier }`.

## Pruner (Phase 4)

Source: `gateway/gateway-execution/src/sleep/pruner.rs` +
`decay.rs`.

- `DecayEngine::list_prune_candidates(agent_id)` runs
  `GraphStorage::list_orphan_old_candidates`
  (`services/knowledge-graph/src/storage.rs:1560`):

```sql
SELECT e.id, e.name, e.entity_type, e.mention_count, e.last_seen_at
  FROM kg_entities e
 WHERE (e.agent_id = ?1 OR e.agent_id = '__global__')
   AND e.epistemic_class != 'archival'
   AND (e.compressed_into IS NULL OR e.compressed_into = '')
   AND e.last_seen_at < ?2                              -- cutoff
   AND NOT EXISTS (SELECT 1 FROM kg_relationships r
                    WHERE r.source_entity_id = e.id)
   AND NOT EXISTS (SELECT 1 FROM kg_relationships r
                    WHERE r.target_entity_id = e.id)
 ORDER BY e.mention_count ASC, e.last_seen_at ASC
 LIMIT ?3
```

- `Pruner::prune` calls `GraphStorage::mark_pruned` on each candidate
  (`storage.rs:1517`). In one transaction:

  ```sql
  UPDATE kg_entities SET compressed_into = '__pruned__' WHERE id = ?
  DELETE FROM kg_name_index WHERE entity_id = ?
  ```

- Never hard-deletes. Every read path in recall + graph queries filters
  `compressed_into IS NULL`. The row is retained so any episode or
  distillation that references the id still dereferences cleanly.

Reason string encoded on each prune audit row:
`"orphan age>{days}d mention_count={n}"`
(`sleep/decay.rs:64`).

## Populating kg_name_index

Entities carry an optional 384-dim `Entity.name_embedding:
Option<Vec<f32>>`. When `Some`, `store_entity` writes to the vec0
partner in the same transaction
(`services/knowledge-graph/src/storage.rs:1815`):

```rust
if let Some(emb) = entity.name_embedding.as_ref() {
    if !emb.is_empty() {
        conn.execute("DELETE FROM kg_name_index WHERE entity_id = ?1", ...)?;
        conn.execute("INSERT INTO kg_name_index (entity_id, name_embedding) VALUES (?1, ?2)", ...)?;
    }
}
```

vec0 does not support `UPSERT`, so it's emulated with delete+insert.
Safe under SQLite's single-writer semantics.

**If the caller passes `None`, stage 2 of the resolver cannot find the
entity.** The ingestion Extractor is responsible for computing name
embeddings before handing the `ExtractedKnowledge` to
`GraphStorage::store_knowledge`.

## Graph queries from agents

- `graph_query` agent tool — `action ∈ {search, neighbors, context}`.
  Wires through `GraphService` to `GraphStorage::search_entities*`,
  `get_neighbors`, and traversal helpers in
  `services/knowledge-graph/src/traversal.rs`.
- `GraphStorage::search_entities_by_name_embedding(emb, k, agent_id)`
  (`storage.rs:416`) is the helper consumed by the recall adapter for
  unified recall (ANN over `kg_name_index`). Result: top-k entities by
  cosine, filtered by agent scope and `compressed_into IS NULL`.

## Known gaps (for future work)

- **LLM pairwise verifier not wired.** Trait `PairwiseVerifier` is
  defined and `Compactor::new` accepts it, but all callers pass
  `None`. Threshold-only merges at cosine ≥ 0.92 are conservative
  enough for background operation.
- **Cross-session synthesis not implemented.** The spec describes
  promoting "strong subgraph" patterns into strategy facts after
  distillation; no such promotion pass exists yet.
- **Worker panic leaves zombie `running` episodes.** If a queue worker
  panics between `claim_next_pending` (which flips `status='running'`)
  and `mark_done`/`mark_failed`, the episode stays `running` forever.
  Tokio task isolation means the process keeps going, but that
  episode won't be retried without manual intervention. A stale-claim
  sweeper is not yet implemented.
- **Resolver stage 2 requires an embedding.** Entities extracted
  without a `name_embedding` are invisible to ANN and rely on stage 1
  alias matching only.
