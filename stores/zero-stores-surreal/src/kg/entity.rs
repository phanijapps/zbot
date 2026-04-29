//! Entity CRUD on the `entity` table.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use knowledge_graph::types::{Entity, EntityType};
use serde_json::Value;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
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
    // `properties` is the LLM-extracted metadata bag (e.g. country code,
    // ticker symbol). Stored as a SurrealDB object so per-key access is
    // possible later. SCHEMALESS table tolerates the dynamic shape.
    //
    // `first_seen_at` is set ONLY on first insert — `(first_seen_at OR
    // time::now())` evaluates to the existing value on update, falling
    // through to `time::now()` on the new row. Same idiom as the
    // pre-existing `mention_count` increment.
    db.query(
        r#"
        UPSERT $id SET
            agent_id = $agent_id,
            name = $name,
            entity_type = $entity_type,
            mention_count = (mention_count OR 0) + 1,
            first_seen_at = (first_seen_at OR time::now()),
            last_seen_at = time::now()
        "#,
    )
    .bind(("id", thing.clone()))
    .bind(("agent_id", agent_id.to_string()))
    .bind(("name", entity.name.clone()))
    .bind(("entity_type", entity.entity_type.as_str().to_string()))
    .await
    .map_err(map_surreal_error)?;

    // `properties` is written in a second query using MERGE — binding a
    // dynamically-shaped `serde_json::Value` into a multi-field SET
    // clause via the surrealdb 3.x SDK doesn't round-trip cleanly
    // (the parser silently no-ops the entire query). MERGE accepts an
    // object literal value and merges its keys into the existing row.
    // The Surreal `entity` table is SCHEMAFULL with a `metadata` field
    // (option<object>) — that's where the LLM-extracted property bag
    // lives on the SurrealDB side. Map domain-name `properties` to the
    // schema-name `metadata` here so callers don't need to know.
    // Written via MERGE because a multi-field SET with an embedded
    // dynamic value silently no-ops on the surrealdb 3.x SDK.
    if !entity.properties.is_empty() {
        let properties_value = serde_json::to_value(&entity.properties)
            .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
        let payload = Value::Object({
            let mut m = serde_json::Map::new();
            m.insert("metadata".to_string(), properties_value);
            m
        });
        let mut resp = db
            .query("UPDATE $id MERGE $payload")
            .bind(("id", thing))
            .bind(("payload", payload))
            .await
            .map_err(map_surreal_error)?;
        // Surface per-statement errors and inspect what came back.
        let _ = resp.take::<Vec<Value>>(0).map_err(|e| {
            tracing::warn!(error = %e, "metadata merge: statement-level error");
            map_surreal_error(e)
        })?;
    }

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
    db.query("UPDATE $id SET mention_count = (mention_count OR 0) + 1, last_seen_at = time::now()")
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
    // Surreal schema calls this `metadata`; we surface it as `properties`
    // on the Rust-side `Entity` to match the SQLite and domain naming.
    metadata: Option<Value>,
    mention_count: Option<i64>,
    first_seen_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
}

impl EntityRow {
    fn into_entity(self) -> Entity {
        let now = Utc::now();
        let properties: HashMap<String, Value> = match self.metadata {
            Some(Value::Object(map)) => map.into_iter().collect(),
            _ => HashMap::new(),
        };
        Entity {
            id: self.id.to_entity_id().0,
            agent_id: self.agent_id,
            entity_type: EntityType::from_str(&self.entity_type),
            name: self.name,
            properties,
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
    use crate::{connect, schema::apply_schema, SurrealConfig};

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
    async fn upsert_persists_properties_and_first_seen_at() {
        let db = fresh_db().await;
        let mut e = Entity::new("a1".into(), EntityType::Concept, "AAPL".into());
        e.properties.insert(
            "ticker".to_string(),
            serde_json::Value::String("AAPL".into()),
        );
        e.properties.insert(
            "exchange".to_string(),
            serde_json::Value::String("NASDAQ".into()),
        );

        let id = upsert(&db, "a1", e).await.expect("upsert");
        let fetched = get(&db, &id).await.expect("get").expect("present");

        // Properties round-trip: not Default::default() anymore.
        assert_eq!(
            fetched.properties.get("ticker").and_then(|v| v.as_str()),
            Some("AAPL"),
            "ticker property must survive write+read"
        );
        assert_eq!(
            fetched.properties.get("exchange").and_then(|v| v.as_str()),
            Some("NASDAQ")
        );

        // first_seen_at gets stamped on first insert (was missing pre-fix —
        // the row had no `first_seen_at` field and the read defaulted to
        // `Utc::now()` on every fetch, which made age-bucket UI lie).
        let first_seen_initial = fetched.first_seen_at;
        // Bump mention via re-upsert; first_seen_at must NOT change.
        let mut e2 = Entity::new("a1".into(), EntityType::Concept, "AAPL".into());
        e2.id = id.0.clone();
        upsert(&db, "a1", e2).await.expect("upsert 2");
        let again = get(&db, &id).await.expect("get").expect("present");
        assert_eq!(
            again.first_seen_at, first_seen_initial,
            "first_seen_at must be sticky across upserts"
        );
        assert!(
            again.last_seen_at >= first_seen_initial,
            "last_seen_at must move forward"
        );
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
