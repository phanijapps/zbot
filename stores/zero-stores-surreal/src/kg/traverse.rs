//! Graph traversal — neighbors, BFS, subgraphs.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use knowledge_graph::types::{
    Direction as KgDirection, Entity, EntityType, NeighborInfo, Relationship, RelationshipType,
    Subgraph,
};
use serde_json::Value;
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

/// Hydrated neighbor info: each neighbor is a (entity, relationship,
/// direction) triple. Used by `/api/graph/.../neighbors` and the
/// trait-routed `GraphQueryTool` adapter so subagents and the UI side
/// panel see real connections instead of an empty list.
///
/// The query strategy is: pull all relationship rows touching
/// `entity_id` (outgoing where in == id, incoming where out == id),
/// extract the "other" endpoint id from each, batch-fetch those entity
/// rows, then join in-memory. SCHEMALESS-tolerant — works whether the
/// row was written via the typed UPSERT path or the legacy save_fact
/// path, by routing through the same shape-normalizing helpers used
/// by the listing endpoints.
pub async fn get_neighbors_full(
    db: &Arc<Surreal<Any>>,
    _agent_id: &str,
    entity_id: &str,
    direction: Direction,
    limit: usize,
) -> StoreResult<Vec<NeighborInfo>> {
    let id_thing = EntityId(entity_id.to_string()).to_thing();

    let mut rels: Vec<(Value, KgDirection)> = Vec::new();

    if matches!(direction, Direction::Outgoing | Direction::Both) {
        let q = format!(
            "SELECT * FROM relationship WHERE in = $id ORDER BY mention_count DESC LIMIT {limit}"
        );
        let mut resp = db
            .query(q)
            .bind(("id", id_thing.clone()))
            .await
            .map_err(map_surreal_error)?;
        let rows: Vec<Value> = resp.take(0).map_err(map_surreal_error)?;
        for row in rows {
            rels.push((row, KgDirection::Outgoing));
        }
    }

    if matches!(direction, Direction::Incoming | Direction::Both) {
        let q = format!(
            "SELECT * FROM relationship WHERE out = $id ORDER BY mention_count DESC LIMIT {limit}"
        );
        let mut resp = db
            .query(q)
            .bind(("id", id_thing))
            .await
            .map_err(map_surreal_error)?;
        let rows: Vec<Value> = resp.take(0).map_err(map_surreal_error)?;
        for row in rows {
            rels.push((row, KgDirection::Incoming));
        }
    }

    // Hydrate the "other" entity for each relationship. Batch by
    // collecting unique ids first so we don't N+1 the DB on dense
    // neighborhoods.
    let other_ids: Vec<String> = rels
        .iter()
        .filter_map(|(rel, dir)| match dir {
            KgDirection::Outgoing => extract_record_id(rel.get("out")),
            KgDirection::Incoming => extract_record_id(rel.get("in")),
            KgDirection::Both => None,
        })
        .collect();
    let entity_map = fetch_entities_by_ids(db, &other_ids).await?;

    let mut out = Vec::with_capacity(rels.len());
    for (rel_row, dir) in rels {
        let other_id = match dir {
            KgDirection::Outgoing => extract_record_id(rel_row.get("out")),
            KgDirection::Incoming => extract_record_id(rel_row.get("in")),
            KgDirection::Both => None,
        };
        let Some(other_id) = other_id else {
            continue;
        };
        let Some(entity) = entity_map.get(&other_id).cloned() else {
            continue;
        };
        let relationship = relationship_row_to_struct(&rel_row);
        out.push(NeighborInfo {
            entity,
            relationship,
            direction: dir,
        });
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

/// Subgraph BFS: collects entities and relationships within `max_hops`
/// of `center_entity_id`. Bounded by an internal sanity cap so a dense
/// graph doesn't blow up the response. Same row-shape tolerance as
/// `get_neighbors_full`.
pub async fn get_subgraph(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    center_entity_id: &str,
    max_hops: usize,
) -> StoreResult<Subgraph> {
    const HOP_FANOUT_LIMIT: usize = 50;
    const TOTAL_ENTITY_CAP: usize = 500;

    let max_hops = max_hops.clamp(1, 6);
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(center_entity_id.to_string());

    // Collect the center entity first so the response includes it
    // even if it's a dangling node with no edges.
    let mut entities: Vec<Entity> = Vec::new();
    let mut relationships: Vec<Relationship> = Vec::new();
    let mut rel_seen: HashSet<String> = HashSet::new();

    if let Some(e) = fetch_entity_by_id(db, center_entity_id).await? {
        entities.push(e);
    }

    let mut frontier: Vec<String> = vec![center_entity_id.to_string()];
    for _hop in 1..=max_hops {
        if entities.len() >= TOTAL_ENTITY_CAP {
            break;
        }
        let mut next_frontier: Vec<String> = Vec::new();
        for node_id in &frontier {
            let neighbors =
                get_neighbors_full(db, agent_id, node_id, Direction::Both, HOP_FANOUT_LIMIT)
                    .await?;
            for n in neighbors {
                if rel_seen.insert(n.relationship.id.clone()) {
                    relationships.push(n.relationship);
                }
                let oid = n.entity.id.clone();
                if visited.insert(oid.clone()) {
                    entities.push(n.entity);
                    next_frontier.push(oid);
                    if entities.len() >= TOTAL_ENTITY_CAP {
                        break;
                    }
                }
            }
            if entities.len() >= TOTAL_ENTITY_CAP {
                break;
            }
        }
        frontier = next_frontier;
        if frontier.is_empty() {
            break;
        }
    }

    Ok(Subgraph {
        entities,
        relationships,
        center: center_entity_id.to_string(),
        max_hops,
    })
}

// ---------------------------------------------------------------------------
// Row-shape helpers (shared between get_neighbors_full and get_subgraph)
// ---------------------------------------------------------------------------

/// Pull the bare entity/relationship id (no table prefix, no backticks)
/// out of a `Value` field that SurrealDB serializes as a Thing.
///
/// Observed wire formats from surrealdb 3.x's JSON serializer:
///   - String:    `"entity:\`entity_a1_56f48462-…\`"`  (the common case
///     when the id contains a hyphen — Surreal wraps it in backticks)
///   - String:    `"entity:abc123"`                    (no special chars)
///   - Object:    `{"tb":"entity","id":{"String":"…"}}` (older variants)
///
/// All forms collapse to the bare id string.
fn extract_record_id(value: Option<&Value>) -> Option<String> {
    let v = value?;
    if let Some(s) = v.as_str() {
        return Some(strip_thing_string(s));
    }
    if let Some(obj) = v.as_object() {
        if let Some(id) = obj.get("id") {
            if let Some(s) = id.as_str() {
                return Some(strip_backticks(s).to_string());
            }
            if let Some(inner) = id.as_object() {
                for k in ["String", "string", "Number", "Uuid"] {
                    if let Some(s) = inner.get(k).and_then(|v| v.as_str()) {
                        return Some(strip_backticks(s).to_string());
                    }
                }
            }
        }
    }
    None
}

/// Strip `<table>:` prefix if present, then strip surrounding backticks.
fn strip_thing_string(s: &str) -> String {
    let after_prefix = match s.find(':') {
        Some(idx) => &s[idx + 1..],
        None => s,
    };
    strip_backticks(after_prefix).to_string()
}

fn strip_backticks(s: &str) -> &str {
    let s = s.strip_prefix('`').unwrap_or(s);
    s.strip_suffix('`').unwrap_or(s)
}

/// Batch-fetch entities by id. Uses one SELECT per id (Surreal's
/// `SELECT * FROM entity WHERE id IN [...]` doesn't accept a vector
/// of `Thing`s the same way SQLite IN does — easier and predictable
/// to issue per-id queries; the caller bounds `ids` to `limit` so
/// the fanout is small).
async fn fetch_entities_by_ids(
    db: &Arc<Surreal<Any>>,
    ids: &[String],
) -> StoreResult<HashMap<String, Entity>> {
    let mut out: HashMap<String, Entity> = HashMap::new();
    for id in ids {
        if out.contains_key(id) {
            continue;
        }
        if let Some(e) = fetch_entity_by_id(db, id).await? {
            out.insert(id.clone(), e);
        }
    }
    Ok(out)
}

async fn fetch_entity_by_id(db: &Arc<Surreal<Any>>, id: &str) -> StoreResult<Option<Entity>> {
    let thing = EntityId(id.to_string()).to_thing();
    let mut resp = db
        .query("SELECT * FROM ONLY $id")
        .bind(("id", thing))
        .await
        .map_err(map_surreal_error)?;
    let row: Option<Value> = resp.take(0).map_err(map_surreal_error)?;
    Ok(row.map(|v| entity_row_to_struct(id, &v)))
}

/// Convert a SurrealDB row (as `serde_json::Value`) into an
/// `Entity`. Tolerates missing fields (legacy rows without
/// `metadata`, `first_seen_at`, etc.) by defaulting.
fn entity_row_to_struct(fallback_id: &str, row: &Value) -> Entity {
    let id = extract_record_id(row.get("id")).unwrap_or_else(|| fallback_id.to_string());
    let agent_id = row
        .get("agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("__global__")
        .to_string();
    let name = row
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let entity_type_str = row
        .get("entity_type")
        .and_then(|v| v.as_str())
        .unwrap_or("concept");
    let mention_count = row
        .get("mention_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);
    let properties: HashMap<String, Value> = match row.get("metadata") {
        Some(Value::Object(map)) => map.clone().into_iter().collect(),
        _ => HashMap::new(),
    };
    let now = Utc::now();
    let first_seen_at = parse_dt(row.get("first_seen_at")).unwrap_or(now);
    let last_seen_at = parse_dt(row.get("last_seen_at")).unwrap_or(now);
    Entity {
        id,
        agent_id,
        entity_type: EntityType::from_str(entity_type_str),
        name,
        properties,
        first_seen_at,
        last_seen_at,
        mention_count,
        name_embedding: None,
    }
}

/// Convert a SurrealDB relationship row into a `Relationship`. Same
/// shape-tolerance as `entity_row_to_struct`.
fn relationship_row_to_struct(row: &Value) -> Relationship {
    let id = extract_record_id(row.get("id")).unwrap_or_default();
    let agent_id = row
        .get("agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("__global__")
        .to_string();
    let source_entity_id = extract_record_id(row.get("in")).unwrap_or_default();
    let target_entity_id = extract_record_id(row.get("out")).unwrap_or_default();
    let rel_type_str = row
        .get("relationship_type")
        .and_then(|v| v.as_str())
        .unwrap_or("related_to");
    let mention_count = row
        .get("mention_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);
    let properties: HashMap<String, Value> = match row.get("metadata") {
        Some(Value::Object(map)) => map.clone().into_iter().collect(),
        _ => HashMap::new(),
    };
    let now = Utc::now();
    let first_seen_at = parse_dt(row.get("first_seen_at")).unwrap_or(now);
    let last_seen_at = parse_dt(row.get("last_seen_at")).unwrap_or(now);
    Relationship {
        id,
        agent_id,
        source_entity_id,
        target_entity_id,
        relationship_type: RelationshipType::from_str(rel_type_str),
        properties,
        first_seen_at,
        last_seen_at,
        mention_count,
    }
}

fn parse_dt(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let s = value?.as_str()?;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
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
        relationship::upsert_relationship(db, "a1", rel)
            .await
            .unwrap();
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

    #[tokio::test]
    async fn get_neighbors_full_returns_hydrated_triples() {
        let db = fresh_db().await;
        let alice = upsert_entity(&db, "Alice").await;
        let bob = upsert_entity(&db, "Bob").await;
        let carol = upsert_entity(&db, "Carol").await;
        link(&db, &alice, &bob).await; // alice -> bob
        link(&db, &carol, &alice).await; // carol -> alice (incoming for alice)

        let outgoing = get_neighbors_full(&db, "a1", alice.as_ref(), Direction::Outgoing, 10)
            .await
            .expect("outgoing");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].entity.name, "Bob");
        assert!(matches!(outgoing[0].direction, KgDirection::Outgoing));
        assert_eq!(outgoing[0].relationship.target_entity_id, bob.0);

        let incoming = get_neighbors_full(&db, "a1", alice.as_ref(), Direction::Incoming, 10)
            .await
            .expect("incoming");
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].entity.name, "Carol");
        assert!(matches!(incoming[0].direction, KgDirection::Incoming));

        let both = get_neighbors_full(&db, "a1", alice.as_ref(), Direction::Both, 10)
            .await
            .expect("both");
        assert_eq!(both.len(), 2);
        let names: HashSet<_> = both.iter().map(|n| n.entity.name.clone()).collect();
        assert!(names.contains("Bob"));
        assert!(names.contains("Carol"));
    }

    #[tokio::test]
    async fn get_subgraph_includes_center_and_one_hop() {
        let db = fresh_db().await;
        let a = upsert_entity(&db, "A").await;
        let b = upsert_entity(&db, "B").await;
        let c = upsert_entity(&db, "C").await;
        link(&db, &a, &b).await;
        link(&db, &b, &c).await;

        let sub = get_subgraph(&db, "a1", a.as_ref(), 1)
            .await
            .expect("subgraph 1-hop");
        let names: HashSet<_> = sub.entities.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains("A"), "center A must be in subgraph");
        assert!(names.contains("B"), "1-hop B must be in subgraph");
        assert!(
            !names.contains("C"),
            "C is 2-hop, must NOT be in 1-hop subgraph"
        );
        assert_eq!(sub.center, a.0);
        assert_eq!(sub.max_hops, 1);
        assert_eq!(sub.relationships.len(), 1);
    }
}
