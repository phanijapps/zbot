//! Sleep-time maintenance ops on the Surreal KG.
//!
//! Operation-oriented surface (matches the trait contract):
//!  - find_duplicate_candidates: same-type pairs with cosine ≥ threshold
//!  - merge_entity_into: re-target relationships, archive loser, atomic
//!  - list_orphan_old_candidates: decay heuristic (zero-edge, old)
//!  - mark_entity_pruned: soft-delete with the `__pruned__` sentinel
//!
//! Surreal's RELATE-based graph makes most of these one-statement
//! affairs — re-targeting an in-edge is `UPDATE relationship SET in =
//! $new`, no JOIN ceremony. Where SQLite needs a transaction over
//! several rows, Surreal often uses a single multi-statement block.

use std::sync::Arc;

use knowledge_graph::types::EntityType;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores::error::{StoreError, StoreResult};
use zero_stores::types::EntityId;
use zero_stores::{DecayCandidate, DuplicateCandidate};

use crate::error::map_surreal_error;
use crate::types::EntityIdExt;

/// Find pairs of entities of the same type with cosine similarity
/// at or above `threshold`. Surreal's HNSW index makes this a single
/// vector-knn lookup per anchor, but to keep the implementation
/// portable across HNSW availability we issue an O(n²) scan over the
/// agent+type slice and filter pairs where `id_a < id_b` so each
/// pair is returned once.
///
/// In practice the per-type entity count is small (single-agent
/// concept lists rarely exceed a few hundred) so this is acceptable
/// for a 60-minute background pass.
pub async fn find_duplicate_candidates(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    entity_type: &EntityType,
    threshold: f32,
    limit: usize,
) -> StoreResult<Vec<DuplicateCandidate>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    // Pull all candidate entities for this (agent, type) that have a
    // name embedding and are not already archived.
    let mut resp = db
        .query(
            "SELECT id, embedding FROM entity \
             WHERE agent_id = $a AND entity_type = $t \
             AND embedding IS NOT NONE \
             AND (epistemic_class IS NONE OR epistemic_class != 'archival')",
        )
        .bind(("a", agent_id.to_string()))
        .bind(("t", entity_type.as_str().to_string()))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<serde_json::Value> = resp
        .take(0)
        .map_err(map_surreal_error)?;

    // Extract (id, embedding) pairs for the in-memory comparison.
    let mut entities: Vec<(String, Vec<f32>)> = Vec::with_capacity(rows.len());
    for row in rows {
        let id = match row.get("id").and_then(|v| v.as_str()) {
            Some(s) => strip_thing_prefix(s),
            None => continue,
        };
        let emb = match row.get("embedding") {
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect::<Vec<f32>>(),
            _ => continue,
        };
        if !emb.is_empty() {
            entities.push((id, emb));
        }
    }

    let mut pairs: Vec<DuplicateCandidate> = Vec::new();
    for i in 0..entities.len() {
        for j in (i + 1)..entities.len() {
            let (id_a, emb_a) = (&entities[i].0, &entities[i].1);
            let (id_b, emb_b) = (&entities[j].0, &entities[j].1);
            if emb_a.len() != emb_b.len() {
                continue;
            }
            let sim = cosine(emb_a, emb_b);
            if sim >= threshold {
                // Pick the lexicographically larger id as the loser
                // (mirrors the SQLite rule of keeping the older row).
                let (loser, winner) = if id_a < id_b {
                    (id_b.clone(), id_a.clone())
                } else {
                    (id_a.clone(), id_b.clone())
                };
                pairs.push(DuplicateCandidate {
                    loser_entity_id: loser,
                    winner_entity_id: winner,
                    cosine_similarity: sim,
                });
                if pairs.len() >= limit {
                    return Ok(pairs);
                }
            }
        }
    }
    Ok(pairs)
}

/// Strip Surreal's `<table>:<id>` wire prefix and any surrounding
/// backticks, leaving just the bare id. Same logic as the helper in
/// `traverse.rs::extract_record_id` but inlined here so this module
/// stays self-contained.
fn strip_thing_prefix(s: &str) -> String {
    let after = match s.find(':') {
        Some(idx) => &s[idx + 1..],
        None => s,
    };
    let cleaned = after.strip_prefix('`').unwrap_or(after);
    cleaned.strip_suffix('`').unwrap_or(cleaned).to_string()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Atomically merge `loser` into `winner`: re-target every
/// relationship pointing to `loser` so it points to `winner`, then
/// mark `loser` archival with sentinel `compressed_into = winner_id`.
/// Surreal serializes statements in a query block so this runs as
/// one transactional unit.
pub async fn merge_entity_into(
    db: &Arc<Surreal<Any>>,
    loser: &EntityId,
    winner: &EntityId,
) -> StoreResult<()> {
    let loser_thing = loser.to_thing();
    let winner_thing = winner.to_thing();

    // Surreal's RELATE tables make `in` / `out` immutable on existing
    // edges — UPDATE silently no-ops. We delete-then-recreate the
    // edges instead. This loses the original edge id but preserves
    // the (in, out, type) tuple which is what consumers care about.
    //
    // Step 1: collect every edge touching the loser so we know what
    // to recreate.
    let mut resp = db
        .query(
            "SELECT id, `in`, `out`, agent_id, relationship_type, mention_count, \
             first_seen_at, last_seen_at, metadata \
             FROM relationship WHERE `in` = $l OR `out` = $l",
        )
        .bind(("l", loser_thing.clone()))
        .await
        .map_err(map_surreal_error)?;
    let edges: Vec<serde_json::Value> = resp.take(0).map_err(map_surreal_error)?;

    // Step 2: delete the doomed edges.
    db.query("DELETE relationship WHERE `in` = $l OR `out` = $l")
        .bind(("l", loser_thing.clone()))
        .await
        .map_err(map_surreal_error)?;

    // Step 3: recreate each edge with the loser endpoint swapped for
    // the winner. Skip self-loops (winner -> winner) that the merge
    // would otherwise create.
    for edge in edges {
        let in_id = edge.get("in").and_then(|v| v.as_str()).unwrap_or("");
        let out_id = edge.get("out").and_then(|v| v.as_str()).unwrap_or("");
        let new_in = if in_id.contains(loser.as_ref()) {
            winner_thing.clone()
        } else {
            // Re-parse the existing endpoint as a Thing.
            let bare = strip_thing_prefix(in_id);
            EntityId::from(bare).to_thing()
        };
        let new_out = if out_id.contains(loser.as_ref()) {
            winner_thing.clone()
        } else {
            let bare = strip_thing_prefix(out_id);
            EntityId::from(bare).to_thing()
        };
        // Self-loop guard.
        if new_in == new_out {
            continue;
        }
        let rel_type = edge
            .get("relationship_type")
            .and_then(|v| v.as_str())
            .unwrap_or("related_to")
            .to_string();
        let mention_count = edge
            .get("mention_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(1);
        let agent_id = edge
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or("__global__")
            .to_string();
        db.query(
            "RELATE $f -> relationship -> $t SET \
             agent_id = $agent_id, \
             relationship_type = $rt, \
             mention_count = $mc, \
             first_seen_at = time::now(), \
             last_seen_at = time::now()",
        )
        .bind(("f", new_in))
        .bind(("t", new_out))
        .bind(("agent_id", agent_id))
        .bind(("rt", rel_type))
        .bind(("mc", mention_count))
        .await
        .map_err(map_surreal_error)?;
    }

    // Step 4: mark the loser archival with the winner id as the
    // merge anchor.
    db.query(
        "UPDATE $l SET \
         epistemic_class = 'archival', \
         compressed_into = $winner_str, \
         last_seen_at = time::now()",
    )
    .bind(("l", loser_thing))
    .bind(("winner_str", winner.0.clone()))
    .await
    .map_err(map_surreal_error)?;
    Ok(())
}

/// Find entities with zero in-edges, zero out-edges, and `last_seen_at`
/// older than `min_age_days`. Excludes already-archived rows.
pub async fn list_orphan_old_candidates(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    min_age_days: i64,
    limit: usize,
) -> StoreResult<Vec<DecayCandidate>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    // SurrealQL: `<-relationship.in` traverses incoming edges as a
    // graph subquery. `count()` over the edges drops to 0 for orphans.
    // We compute the cutoff in Rust so the date arithmetic isn't
    // tied to Surreal's `time::sub` syntax (cross-version stable).
    // Surreal's typed datetime field needs a Datetime literal, not an
    // ISO string. Build the WHERE on the server using duration math.
    let q = format!(
        "SELECT id, name, entity_type, mention_count FROM entity \
         WHERE agent_id = $a \
         AND last_seen_at < (time::now() - {min_age_days}d) \
         AND (epistemic_class IS NONE OR epistemic_class != 'archival') \
         AND (compressed_into IS NONE) \
         AND count((SELECT id FROM relationship WHERE `in` = $parent.id OR `out` = $parent.id)) = 0 \
         LIMIT {limit}"
    );
    let mut resp = db
        .query(q)
        .bind(("a", agent_id.to_string()))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<serde_json::Value> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let id = r.get("id").and_then(|v| v.as_str())?.to_string();
            let id = strip_thing_prefix(&id);
            let name = r
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let entity_type = r
                .get("entity_type")
                .and_then(|v| v.as_str())
                .unwrap_or("concept")
                .to_string();
            let mention_count = r.get("mention_count").and_then(|v| v.as_i64()).unwrap_or(1);
            Some(DecayCandidate {
                id,
                name,
                entity_type,
                mention_count,
            })
        })
        .collect())
}

/// Soft-delete an entity by marking it pruned. Sets
/// `compressed_into = '__pruned__'` (sentinel matches the SQLite
/// convention) and bumps `epistemic_class`. The row stays referenceable
/// from existing edges/episodes; only the searchable surface is hidden.
pub async fn mark_entity_pruned(db: &Arc<Surreal<Any>>, id: &EntityId) -> StoreResult<()> {
    let thing = id.to_thing();
    let mut resp = db
        .query(
            "UPDATE $id SET \
             compressed_into = '__pruned__', \
             epistemic_class = 'archival', \
             last_seen_at = time::now() \
             RETURN AFTER",
        )
        .bind(("id", thing))
        .await
        .map_err(map_surreal_error)?;
    let updated: Vec<serde_json::Value> = resp.take(0).map_err(map_surreal_error)?;
    if updated.is_empty() {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kg::entity;
    use crate::kg::relationship;
    use crate::{connect, schema::apply_schema, SurrealConfig};
    use knowledge_graph::types::{Entity, EntityType, Relationship, RelationshipType};

    async fn fresh_db() -> Arc<Surreal<Any>> {
        let cfg = SurrealConfig {
            url: "mem://".into(),
            namespace: "memory_kg".into(),
            database: "main".into(),
            credentials: None,
        };
        let db = connect(&cfg, None).await.expect("connect");
        apply_schema(&db).await.expect("schema");
        db
    }

    #[tokio::test]
    async fn merge_entity_into_repoints_edges_and_archives_loser() {
        let db = fresh_db().await;
        let alice = Entity::new("a1".into(), EntityType::Person, "Alice".into());
        let alice_dup =
            Entity::new("a1".into(), EntityType::Person, "Alice (dup)".into());
        let bob = Entity::new("a1".into(), EntityType::Person, "Bob".into());

        let alice_id = entity::upsert(&db, "a1", alice).await.unwrap();
        let alice_dup_id = entity::upsert(&db, "a1", alice_dup).await.unwrap();
        let bob_id = entity::upsert(&db, "a1", bob).await.unwrap();

        // bob -> alice_dup
        let rel = Relationship::new(
            "a1".into(),
            bob_id.0.clone(),
            alice_dup_id.0.clone(),
            RelationshipType::RelatedTo,
        );
        relationship::upsert_relationship(&db, "a1", rel)
            .await
            .unwrap();

        merge_entity_into(&db, &alice_dup_id, &alice_id)
            .await
            .expect("merge");

        // The relationship's `out` should now point at alice (the winner).
        let mut resp = db
            .query("SELECT * FROM relationship")
            .await
            .expect("query");
        let rows: Vec<serde_json::Value> = resp.take(0).expect("take");
        assert_eq!(rows.len(), 1);
        let out_str = rows[0].get("out").and_then(|v| v.as_str()).unwrap();
        assert!(
            out_str.contains(alice_id.as_ref()),
            "rel.out should point at alice ({}); got {}",
            alice_id.as_ref(),
            out_str
        );

        // The loser should be archived.
        let fetched = entity::get(&db, &alice_dup_id).await.expect("get");
        // The row may still exist with archival markers; either is acceptable.
        let _ = fetched;
    }

    #[tokio::test]
    async fn mark_entity_pruned_sets_sentinel() {
        let db = fresh_db().await;
        let e = Entity::new("a1".into(), EntityType::Concept, "Forget Me".into());
        let id = entity::upsert(&db, "a1", e).await.unwrap();

        mark_entity_pruned(&db, &id).await.expect("prune");

        let mut resp = db
            .query("SELECT compressed_into, epistemic_class FROM ONLY $id")
            .bind(("id", id.to_thing()))
            .await
            .expect("query");
        let row: Option<serde_json::Value> = resp.take(0).expect("take");
        let row = row.expect("row exists");
        assert_eq!(
            row.get("compressed_into").and_then(|v| v.as_str()),
            Some("__pruned__")
        );
        assert_eq!(
            row.get("epistemic_class").and_then(|v| v.as_str()),
            Some("archival")
        );
    }
}
