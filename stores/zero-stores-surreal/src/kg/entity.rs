//! Entity CRUD on the `entity` table.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use knowledge_graph::types::{Entity, EntityType};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use zero_stores::error::StoreResult;
use zero_stores::types::EntityId;

use surrealdb::types::SurrealValue;

use crate::error::map_surreal_error;
use crate::types::{EntityIdExt, ThingExt};

pub async fn upsert(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    entity: Entity,
) -> StoreResult<EntityId> {
    let id = EntityId(entity.id.clone());
    let thing = id.to_thing();
    db.query(
        r#"
        UPSERT $id SET
            agent_id = $agent_id,
            name = $name,
            entity_type = $entity_type,
            mention_count = (mention_count OR 0) + 1,
            last_seen_at = time::now()
        "#,
    )
    .bind(("id", thing))
    .bind(("agent_id", agent_id.to_string()))
    .bind(("name", entity.name.clone()))
    .bind(("entity_type", entity.entity_type.as_str().to_string()))
    .await
    .map_err(map_surreal_error)?;
    Ok(id)
}

pub async fn get(db: &Arc<Surreal<Any>>, id: &EntityId) -> StoreResult<Option<Entity>> {
    let mut resp = db
        .query("SELECT * FROM ONLY $id")
        .bind(("id", id.to_thing()))
        .await
        .map_err(map_surreal_error)?;
    let row: Option<EntityRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(row.map(|r| r.into_entity()))
}

pub async fn delete(db: &Arc<Surreal<Any>>, id: &EntityId) -> StoreResult<()> {
    db.query(
        r#"
        BEGIN;
        DELETE relationship WHERE in = $id OR out = $id;
        DELETE entity_alias WHERE entity_id = $id;
        DELETE $id;
        COMMIT;
        "#,
    )
    .bind(("id", id.to_thing()))
    .await
    .map_err(map_surreal_error)?;
    Ok(())
}

pub async fn bump_mention(db: &Arc<Surreal<Any>>, id: &EntityId) -> StoreResult<()> {
    db.query(
        "UPDATE $id SET mention_count = (mention_count OR 0) + 1, last_seen_at = time::now()",
    )
    .bind(("id", id.to_thing()))
    .await
    .map_err(map_surreal_error)?;
    Ok(())
}

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct EntityRow {
    id: surrealdb::types::RecordId,
    agent_id: String,
    name: String,
    entity_type: String,
    mention_count: Option<i64>,
    first_seen_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
}

impl EntityRow {
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
            mention_count: self.mention_count.unwrap_or(1),
            name_embedding: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn upsert_then_get_roundtrip() {
        let db = fresh_db().await;
        let e = Entity::new("a1".into(), EntityType::Person, "Alice".into());
        let original_id = e.id.clone();

        let id = upsert(&db, "a1", e).await.expect("upsert");
        assert_eq!(id.as_ref(), original_id);

        let fetched = get(&db, &id).await.expect("get");
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.name, "Alice");
        assert_eq!(fetched.agent_id, "a1");
    }

    #[tokio::test]
    async fn upsert_increments_mention_count() {
        let db = fresh_db().await;
        let e = Entity::new("a1".into(), EntityType::Person, "Bob".into());
        let id = upsert(&db, "a1", e.clone()).await.expect("upsert 1");
        upsert(&db, "a1", e.clone()).await.expect("upsert 2");
        upsert(&db, "a1", e).await.expect("upsert 3");
        let fetched = get(&db, &id).await.expect("get").expect("present");
        assert_eq!(fetched.mention_count, 3);
    }

    #[tokio::test]
    async fn delete_removes_entity() {
        let db = fresh_db().await;
        let e = Entity::new("a1".into(), EntityType::Concept, "X".into());
        let id = upsert(&db, "a1", e).await.expect("upsert");
        delete(&db, &id).await.expect("delete");
        assert!(get(&db, &id).await.expect("get").is_none());
    }

    #[tokio::test]
    async fn bump_mention_increments() {
        let db = fresh_db().await;
        let e = Entity::new("a1".into(), EntityType::Concept, "Y".into());
        let id = upsert(&db, "a1", e).await.expect("upsert");
        bump_mention(&db, &id).await.expect("bump");
        bump_mention(&db, &id).await.expect("bump");
        let fetched = get(&db, &id).await.expect("get").expect("present");
        assert_eq!(fetched.mention_count, 3);
    }
}
