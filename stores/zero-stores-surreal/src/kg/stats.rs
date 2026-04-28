//! Aggregate stats + paginated lists for HTTP read endpoints.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use knowledge_graph::types::{Entity, EntityType, GraphStats, Relationship, RelationshipType};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::types::{RecordId, SurrealValue};
use zero_stores::error::StoreResult;
use zero_stores::types::{KgStats, VecIndexHealth};

use crate::error::map_surreal_error;
use crate::schema::hnsw;
use crate::types::ThingExt;

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct CountRow {
    n: i64,
}

async fn run_count(db: &Arc<Surreal<Any>>, query: &str) -> StoreResult<i64> {
    let mut resp = db
        .query(query)
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<CountRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows.first().map(|r| r.n).unwrap_or(0))
}

async fn run_count_with_agent(
    db: &Arc<Surreal<Any>>,
    query: &str,
    agent_id: &str,
) -> StoreResult<i64> {
    let mut resp = db
        .query(query)
        .bind(("a", agent_id.to_string()))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<CountRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows.first().map(|r| r.n).unwrap_or(0))
}

pub async fn stats(db: &Arc<Surreal<Any>>) -> StoreResult<KgStats> {
    let entity_count = run_count(db, "SELECT count() AS n FROM entity GROUP ALL").await?;
    let rel_count = run_count(db, "SELECT count() AS n FROM relationship GROUP ALL").await?;
    let alias_count = run_count(db, "SELECT count() AS n FROM entity_alias GROUP ALL").await?;
    Ok(KgStats {
        entity_count: entity_count as u64,
        relationship_count: rel_count as u64,
        alias_count: alias_count as u64,
    })
}

pub async fn count_all_entities(db: &Arc<Surreal<Any>>) -> StoreResult<usize> {
    Ok(run_count(db, "SELECT count() AS n FROM entity GROUP ALL").await? as usize)
}

pub async fn count_all_relationships(db: &Arc<Surreal<Any>>) -> StoreResult<usize> {
    Ok(run_count(db, "SELECT count() AS n FROM relationship GROUP ALL").await? as usize)
}

pub async fn graph_stats(db: &Arc<Surreal<Any>>, agent_id: &str) -> StoreResult<GraphStats> {
    let entity_count = run_count_with_agent(
        db,
        "SELECT count() AS n FROM entity WHERE agent_id = $a GROUP ALL",
        agent_id,
    )
    .await? as usize;
    let rel_count = run_count_with_agent(
        db,
        "SELECT count() AS n FROM relationship WHERE agent_id = $a GROUP ALL",
        agent_id,
    )
    .await? as usize;
    Ok(GraphStats {
        entity_count,
        relationship_count: rel_count,
        entity_types: Default::default(),
        relationship_types: Default::default(),
        most_connected_entities: Vec::new(),
    })
}

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct EntityListRow {
    id: RecordId,
    agent_id: String,
    name: String,
    entity_type: String,
    mention_count: Option<i64>,
    first_seen_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
}

impl EntityListRow {
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

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct RelationshipListRow {
    id: RecordId,
    #[surreal(rename = "in")]
    src: RecordId,
    out: RecordId,
    agent_id: String,
    relationship_type: String,
    mention_count: Option<i64>,
    first_seen_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
}

impl RelationshipListRow {
    fn into_relationship(self) -> Relationship {
        let now = Utc::now();
        let id_raw = match &self.id.key {
            surrealdb::types::RecordIdKey::String(s) => s.clone(),
            other => format!("{other:?}"),
        };
        Relationship {
            id: id_raw,
            agent_id: self.agent_id,
            source_entity_id: self.src.to_entity_id().0,
            target_entity_id: self.out.to_entity_id().0,
            relationship_type: RelationshipType::from_str(&self.relationship_type),
            properties: Default::default(),
            first_seen_at: self.first_seen_at.unwrap_or(now),
            last_seen_at: self.last_seen_at.unwrap_or(now),
            mention_count: self.mention_count.unwrap_or(0),
        }
    }
}

pub async fn list_entities(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    entity_type: Option<&str>,
    limit: usize,
    offset: usize,
) -> StoreResult<Vec<Entity>> {
    let q = match entity_type {
        Some(_) => format!(
            "SELECT * FROM entity WHERE agent_id = $a AND entity_type = $t \
             ORDER BY mention_count DESC LIMIT {limit} START {offset}"
        ),
        None => format!(
            "SELECT * FROM entity WHERE agent_id = $a \
             ORDER BY mention_count DESC LIMIT {limit} START {offset}"
        ),
    };
    let mut q = db.query(q).bind(("a", agent_id.to_string()));
    if let Some(t) = entity_type {
        q = q.bind(("t", t.to_string()));
    }
    let mut resp = q.await.map_err(map_surreal_error)?;
    let rows: Vec<EntityListRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows.into_iter().map(|r| r.into_entity()).collect())
}

pub async fn list_relationships(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    relationship_type: Option<&str>,
    limit: usize,
    offset: usize,
) -> StoreResult<Vec<Relationship>> {
    let q = match relationship_type {
        Some(_) => format!(
            "SELECT * FROM relationship WHERE agent_id = $a AND relationship_type = $t \
             ORDER BY mention_count DESC LIMIT {limit} START {offset}"
        ),
        None => format!(
            "SELECT * FROM relationship WHERE agent_id = $a \
             ORDER BY mention_count DESC LIMIT {limit} START {offset}"
        ),
    };
    let mut q = db.query(q).bind(("a", agent_id.to_string()));
    if let Some(t) = relationship_type {
        q = q.bind(("t", t.to_string()));
    }
    let mut resp = q.await.map_err(map_surreal_error)?;
    let rows: Vec<RelationshipListRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows.into_iter().map(|r| r.into_relationship()).collect())
}

pub async fn list_all_entities(
    db: &Arc<Surreal<Any>>,
    _ward_id: Option<&str>,
    entity_type: Option<&str>,
    limit: usize,
) -> StoreResult<Vec<Entity>> {
    let q = match entity_type {
        Some(_) => format!(
            "SELECT * FROM entity WHERE entity_type = $t \
             ORDER BY mention_count DESC LIMIT {limit}"
        ),
        None => format!("SELECT * FROM entity ORDER BY mention_count DESC LIMIT {limit}"),
    };
    let mut q = db.query(q);
    if let Some(t) = entity_type {
        q = q.bind(("t", t.to_string()));
    }
    let mut resp = q.await.map_err(map_surreal_error)?;
    let rows: Vec<EntityListRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows.into_iter().map(|r| r.into_entity()).collect())
}

pub async fn list_all_relationships(
    db: &Arc<Surreal<Any>>,
    limit: usize,
) -> StoreResult<Vec<Relationship>> {
    let q = format!("SELECT * FROM relationship ORDER BY mention_count DESC LIMIT {limit}");
    let mut resp = db.query(q).await.map_err(map_surreal_error)?;
    let rows: Vec<RelationshipListRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows.into_iter().map(|r| r.into_relationship()).collect())
}

pub async fn vec_index_health(db: &Arc<Surreal<Any>>) -> StoreResult<VecIndexHealth> {
    let dim = hnsw::read_dim(db).await?;
    let indexed_rows = run_count(
        db,
        "SELECT count() AS n FROM entity WHERE embedding != NONE GROUP ALL",
    )
    .await? as usize;
    let (tables_present, tables_missing) = if dim.is_some() {
        (vec!["entity_embedding_hnsw".to_string()], Vec::new())
    } else {
        (Vec::new(), vec!["entity_embedding_hnsw".to_string()])
    };
    Ok(VecIndexHealth {
        tables_present,
        tables_missing,
        indexed_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kg::entity;
    use crate::{SurrealConfig, connect, schema::apply_schema};
    use knowledge_graph::types::{Entity, EntityType};

    async fn fresh_db() -> Arc<Surreal<Any>> {
        let cfg = SurrealConfig {
            url: "mem://".into(),
            namespace: "memory_kg".into(),
            database: "main".into(),
            credentials: None,
        };
        let db = connect(&cfg, None).await.unwrap();
        apply_schema(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn stats_returns_counts() {
        let db = fresh_db().await;
        entity::upsert(
            &db,
            "a1",
            Entity::new("a1".into(), EntityType::Person, "Alice".into()),
        )
        .await
        .unwrap();
        let s = stats(&db).await.unwrap();
        assert_eq!(s.entity_count, 1);
        assert_eq!(s.relationship_count, 0);
    }

    #[tokio::test]
    async fn list_entities_paginates() {
        let db = fresh_db().await;
        for i in 0..5 {
            let e = Entity::new("a1".into(), EntityType::Concept, format!("E{i}"));
            entity::upsert(&db, "a1", e).await.unwrap();
        }
        let page1 = list_entities(&db, "a1", None, 2, 0).await.unwrap();
        let page2 = list_entities(&db, "a1", None, 2, 2).await.unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page2.len(), 2);
    }

    #[tokio::test]
    async fn vec_index_health_reflects_state() {
        let db = fresh_db().await;
        let h = vec_index_health(&db).await.unwrap();
        assert!(h.tables_present.is_empty());
        assert_eq!(h.tables_missing.len(), 1);
        assert_eq!(h.indexed_rows, 0);
    }

    #[tokio::test]
    async fn count_all_returns_zero_on_empty() {
        let db = fresh_db().await;
        assert_eq!(count_all_entities(&db).await.unwrap(), 0);
        assert_eq!(count_all_relationships(&db).await.unwrap(), 0);
    }
}
