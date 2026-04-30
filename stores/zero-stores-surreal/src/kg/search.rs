//! Name-FTS and embedding-KNN search.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use knowledge_graph::types::{Entity, EntityType};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::types::{RecordId, SurrealValue};
use zero_stores::error::StoreResult;
use zero_stores::types::EntityId;

use crate::error::map_surreal_error;
use crate::types::{ThingExt, embedding_to_value};

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct EntitySearchRow {
    id: RecordId,
    agent_id: String,
    name: String,
    entity_type: String,
    mention_count: Option<i64>,
    first_seen_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
}

impl EntitySearchRow {
    fn into_entity(self) -> Entity {
        let now = Utc::now();
        Entity {
            id: self.id.to_entity_id().0,
            agent_id: self.agent_id,
            entity_type: EntityType::from_str(&self.entity_type),
            name: self.name,
            properties: Default::default(),
            first_seen_at: self.first_seen_at.unwrap_or(now),
            last_seen_at: self.last_seen_at.unwrap_or(now),
            mention_count: self.mention_count.unwrap_or(0),
            name_embedding: None,
        }
    }
}

pub async fn search_entities_by_name(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    query: &str,
    limit: usize,
) -> StoreResult<Vec<Entity>> {
    let q = format!("SELECT * FROM entity WHERE agent_id = $a AND name @@ $q LIMIT {limit}");
    let mut resp = db
        .query(q)
        .bind(("a", agent_id.to_string()))
        .bind(("q", query.to_string()))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<EntitySearchRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows.into_iter().map(|r| r.into_entity()).collect())
}

/// Exact-name lookup. Returns the first entity matching `name` for
/// `agent_id`, or `None` if absent. Mirrors the SQLite
/// `GraphStorage::get_entity_by_name` semantics.
pub async fn get_entity_by_name(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    name: &str,
) -> StoreResult<Option<Entity>> {
    let mut resp = db
        .query("SELECT * FROM entity WHERE agent_id = $a AND name = $n LIMIT 1")
        .bind(("a", agent_id.to_string()))
        .bind(("n", name.to_string()))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<EntitySearchRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows.into_iter().next().map(|r| r.into_entity()))
}

/// Search entities through a [`zero_stores::GraphView`] lens.
/// `Semantic` orders by `mention_count DESC` (matches SQLite).
/// `Temporal` orders by `last_seen_at DESC`. `Entity` and `Hybrid`
/// degrade to `Semantic` with a tracing warn — surfacing connection
/// counts and RRF ranking on Surreal is a follow-up.
pub async fn search_entities_view(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    query: &str,
    view: zero_stores::GraphView,
    limit: usize,
) -> StoreResult<Vec<Entity>> {
    use zero_stores::GraphView;
    let order = match view {
        GraphView::Semantic => "mention_count DESC",
        GraphView::Temporal => "last_seen_at DESC",
        GraphView::Entity | GraphView::Hybrid => {
            tracing::warn!(
                view = ?view,
                "search_entities_view: Surreal degrades non-Semantic/Temporal views to Semantic"
            );
            "mention_count DESC"
        }
    };
    let q = format!(
        "SELECT * FROM entity \
         WHERE agent_id = $a AND name @@ $q \
         ORDER BY {order} LIMIT {limit}"
    );
    let mut resp = db
        .query(q)
        .bind(("a", agent_id.to_string()))
        .bind(("q", query.to_string()))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<EntitySearchRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows.into_iter().map(|r| r.into_entity()).collect())
}

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct KnnRow {
    id: RecordId,
    dist: f32,
}

pub async fn search_by_embedding(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    query_vec: &[f32],
    k: usize,
) -> StoreResult<Vec<(EntityId, f32)>> {
    let q = format!(
        "SELECT id, vector::distance::knn() AS dist FROM entity \
         WHERE embedding <|{k},40|> $vec AND agent_id = $a ORDER BY dist"
    );
    let mut resp = db
        .query(q)
        .bind(("vec", embedding_to_value(query_vec)))
        .bind(("a", agent_id.to_string()))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<KnnRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows
        .into_iter()
        .map(|r| (r.id.to_entity_id(), r.dist))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kg::entity;
    use crate::{SurrealConfig, connect, schema::apply_schema};

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
    async fn fts_finds_match() {
        let db = fresh_db().await;
        let alice = Entity::new("a1".into(), EntityType::Person, "Alice Walker".into());
        let bob = Entity::new("a1".into(), EntityType::Person, "Bob Smith".into());
        entity::upsert(&db, "a1", alice).await.unwrap();
        entity::upsert(&db, "a1", bob).await.unwrap();

        let hits = search_entities_by_name(&db, "a1", "alice", 10)
            .await
            .unwrap();
        assert!(hits.iter().any(|e| e.name.contains("Alice")));
    }

    #[tokio::test]
    async fn fts_respects_agent_isolation() {
        let db = fresh_db().await;
        let alice_a1 = Entity::new("a1".into(), EntityType::Person, "Alice".into());
        let alice_a2 = Entity::new("a2".into(), EntityType::Person, "Alice".into());
        entity::upsert(&db, "a1", alice_a1).await.unwrap();
        entity::upsert(&db, "a2", alice_a2).await.unwrap();

        let hits = search_entities_by_name(&db, "a1", "alice", 10)
            .await
            .unwrap();
        assert!(hits.iter().all(|e| e.agent_id == "a1"));
    }
}
