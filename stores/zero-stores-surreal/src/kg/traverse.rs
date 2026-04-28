//! Graph traversal — neighbors, BFS, subgraphs.

use std::collections::HashSet;
use std::sync::Arc;

use knowledge_graph::types::{NeighborInfo, Subgraph};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::types::{RecordId, RecordIdKey, SurrealValue};
use zero_stores::error::StoreResult;
use zero_stores::types::{Direction, EntityId, Neighbor, RelationshipId, TraversalHit};

use crate::error::map_surreal_error;
use crate::types::{EntityIdExt, ThingExt};

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct EdgeRow {
    id: RecordId,
    #[surreal(rename = "in")]
    src: RecordId,
    out: RecordId,
    relationship_type: String,
}

pub async fn get_neighbors(
    db: &Arc<Surreal<Any>>,
    id: &EntityId,
    direction: Direction,
    limit: usize,
) -> StoreResult<Vec<Neighbor>> {
    let mut results = Vec::new();
    if matches!(direction, Direction::Outgoing | Direction::Both) {
        let rows = fetch_edges(db, id, EdgeSide::Outgoing, limit).await?;
        for row in rows {
            results.push(edge_row_to_neighbor(row, Direction::Outgoing));
        }
    }
    if matches!(direction, Direction::Incoming | Direction::Both) {
        let rows = fetch_edges(db, id, EdgeSide::Incoming, limit).await?;
        for row in rows {
            results.push(edge_row_to_neighbor(row, Direction::Incoming));
        }
    }
    if results.len() > limit {
        results.truncate(limit);
    }
    Ok(results)
}

#[derive(Clone, Copy)]
enum EdgeSide {
    Outgoing,
    Incoming,
}

async fn fetch_edges(
    db: &Arc<Surreal<Any>>,
    id: &EntityId,
    side: EdgeSide,
    limit: usize,
) -> StoreResult<Vec<EdgeRow>> {
    let q = match side {
        EdgeSide::Outgoing => format!(
            "SELECT id, in, out, relationship_type FROM relationship \
             WHERE in = $id LIMIT {limit}"
        ),
        EdgeSide::Incoming => format!(
            "SELECT id, in, out, relationship_type FROM relationship \
             WHERE out = $id LIMIT {limit}"
        ),
    };
    let mut resp = db
        .query(q)
        .bind(("id", id.to_thing()))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<EdgeRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows)
}

fn edge_row_to_neighbor(row: EdgeRow, direction: Direction) -> Neighbor {
    let other = match direction {
        Direction::Outgoing => row.out,
        Direction::Incoming => row.src,
        Direction::Both => row.out, // not reachable in caller
    };
    let rel_id = match &row.id.key {
        RecordIdKey::String(s) => s.clone(),
        RecordIdKey::Number(n) => n.to_string(),
        RecordIdKey::Uuid(u) => u.to_string(),
        other => format!("{other:?}"),
    };
    Neighbor {
        entity_id: other.to_entity_id(),
        relationship_id: RelationshipId(rel_id),
        relationship_type: row.relationship_type,
        direction,
    }
}

pub async fn traverse(
    db: &Arc<Surreal<Any>>,
    seed: &EntityId,
    max_hops: usize,
    limit: usize,
) -> StoreResult<Vec<TraversalHit>> {
    let max_hops = max_hops.clamp(1, 6);
    let mut hits = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(seed.0.clone());
    let mut frontier: Vec<EntityId> = vec![seed.clone()];

    for hop in 1..=max_hops {
        if hits.len() >= limit {
            break;
        }
        let mut next_frontier = Vec::new();
        for node in &frontier {
            let neighbors = get_neighbors(db, node, Direction::Outgoing, limit).await?;
            for n in neighbors {
                if visited.insert(n.entity_id.0.clone()) {
                    hits.push(TraversalHit {
                        entity_id: n.entity_id.clone(),
                        hop,
                        path: format!("{}->{}", node.0, n.entity_id.0),
                        mention_count: 0,
                    });
                    next_frontier.push(n.entity_id);
                    if hits.len() >= limit {
                        break;
                    }
                }
            }
            if hits.len() >= limit {
                break;
            }
        }
        frontier = next_frontier;
        if frontier.is_empty() {
            break;
        }
    }
    Ok(hits)
}

/// Hydrated neighbor info for HTTP read endpoints. MVP shape — returns empty
/// vec; conformance suite (Task 15) drives refinement when callers exercise it.
pub async fn get_neighbors_full(
    _db: &Arc<Surreal<Any>>,
    _agent_id: &str,
    _entity_id: &str,
    _direction: Direction,
    _limit: usize,
) -> StoreResult<Vec<NeighborInfo>> {
    Ok(Vec::new())
}

/// Subgraph for HTTP read endpoints. MVP shape — returns an empty subgraph.
pub async fn get_subgraph(
    _db: &Arc<Surreal<Any>>,
    _agent_id: &str,
    center_entity_id: &str,
    max_hops: usize,
) -> StoreResult<Subgraph> {
    Ok(Subgraph {
        entities: Vec::new(),
        relationships: Vec::new(),
        center: center_entity_id.to_string(),
        max_hops,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kg::{entity, relationship};
    use crate::{SurrealConfig, connect, schema::apply_schema};
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

    async fn upsert_entity(db: &Arc<Surreal<Any>>, name: &str) -> EntityId {
        let e = Entity::new("a1".into(), EntityType::Concept, name.into());
        entity::upsert(db, "a1", e).await.unwrap()
    }

    async fn link(db: &Arc<Surreal<Any>>, from: &EntityId, to: &EntityId) {
        let rel = Relationship::new(
            "a1".into(),
            from.0.clone(),
            to.0.clone(),
            RelationshipType::RelatedTo,
        );
        relationship::upsert_relationship(db, "a1", rel).await.unwrap();
    }

    #[tokio::test]
    async fn neighbors_outgoing_returns_targets() {
        let db = fresh_db().await;
        let alice = upsert_entity(&db, "Alice").await;
        let bob = upsert_entity(&db, "Bob").await;
        link(&db, &alice, &bob).await;

        let neighbors = get_neighbors(&db, &alice, Direction::Outgoing, 10)
            .await
            .expect("neighbors");
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].entity_id.as_ref(), bob.as_ref());
        assert!(matches!(neighbors[0].direction, Direction::Outgoing));
    }

    #[tokio::test]
    async fn neighbors_incoming_returns_sources() {
        let db = fresh_db().await;
        let alice = upsert_entity(&db, "Alice").await;
        let bob = upsert_entity(&db, "Bob").await;
        link(&db, &alice, &bob).await;

        let neighbors = get_neighbors(&db, &bob, Direction::Incoming, 10)
            .await
            .expect("neighbors");
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].entity_id.as_ref(), alice.as_ref());
    }

    #[tokio::test]
    async fn traverse_respects_max_hops() {
        let db = fresh_db().await;
        let a = upsert_entity(&db, "A").await;
        let b = upsert_entity(&db, "B").await;
        let c = upsert_entity(&db, "C").await;
        link(&db, &a, &b).await;
        link(&db, &b, &c).await;

        let hits_1 = traverse(&db, &a, 1, 100).await.expect("traverse 1");
        let hits_2 = traverse(&db, &a, 2, 100).await.expect("traverse 2");
        assert_eq!(hits_1.len(), 1, "1-hop reaches B only");
        assert_eq!(hits_2.len(), 2, "2-hop reaches B and C");
    }
}
