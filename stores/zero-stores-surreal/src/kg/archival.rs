//! Sleep-time orphan archival logic.

use std::sync::Arc;

use surrealdb::engine::any::Any;
use surrealdb::types::{RecordId, SurrealValue};
use surrealdb::Surreal;
use zero_stores::error::StoreResult;
use zero_stores::types::{ArchivableEntity, EntityId};

use crate::error::map_surreal_error;
use crate::types::{EntityIdExt, ThingExt};

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct ArchivableRow {
    id: RecordId,
    agent_id: String,
    name: String,
    entity_type: String,
}

pub async fn list_archivable_orphans(
    db: &Arc<Surreal<Any>>,
    min_age_hours: u32,
    limit: usize,
) -> StoreResult<Vec<ArchivableEntity>> {
    let q = format!(
        "SELECT id, agent_id, name, entity_type FROM entity \
         WHERE mention_count = 1 \
           AND confidence < 0.5 \
           AND first_seen_at < (time::now() - duration::from_hours($h)) \
           AND epistemic_class != 'archival' \
           AND count(<-relationship) = 0 \
           AND count(->relationship) = 0 \
         LIMIT {limit}"
    );
    let mut resp = db
        .query(q)
        .bind(("h", min_age_hours as i64))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<ArchivableRow> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows
        .into_iter()
        .map(|r| ArchivableEntity {
            entity_id: r.id.to_entity_id(),
            agent_id: r.agent_id,
            entity_type: r.entity_type,
            name: r.name,
        })
        .collect())
}

pub async fn mark_entity_archival(
    db: &Arc<Surreal<Any>>,
    id: &EntityId,
    reason: &str,
) -> StoreResult<()> {
    db.query(
        r#"
        BEGIN;
        UPDATE $id SET epistemic_class = 'archival', compressed_into = $reason;
        DELETE entity_alias WHERE entity_id = $id;
        COMMIT;
        "#,
    )
    .bind(("id", id.to_thing()))
    .bind(("reason", reason.to_string()))
    .await
    .map_err(map_surreal_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kg::entity;
    use crate::{connect, schema::apply_schema, SurrealConfig};
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
    async fn mark_archival_sets_class_and_reason() {
        let db = fresh_db().await;
        let e = Entity::new("a1".into(), EntityType::Concept, "Orphan".into());
        let id = entity::upsert(&db, "a1", e).await.unwrap();
        mark_entity_archival(&db, &id, "stale").await.unwrap();

        #[derive(SurrealValue)]
        #[surreal(crate = "surrealdb::types")]
        struct R {
            epistemic_class: String,
            compressed_into: Option<String>,
        }
        let mut resp = db
            .query("SELECT epistemic_class, compressed_into FROM ONLY $id")
            .bind(("id", id.to_thing()))
            .await
            .unwrap();
        let row: Option<R> = resp.take(0).unwrap();
        let row = row.unwrap();
        assert_eq!(row.epistemic_class, "archival");
        assert_eq!(row.compressed_into.as_deref(), Some("stale"));
    }

    #[tokio::test]
    async fn list_archivable_returns_empty_for_recent_entities() {
        let db = fresh_db().await;
        let e = Entity::new("a1".into(), EntityType::Concept, "Recent".into());
        entity::upsert(&db, "a1", e).await.unwrap();

        let orphans = list_archivable_orphans(&db, 24, 100).await.unwrap();
        assert!(orphans.is_empty(), "fresh entity should not be archivable");
    }
}
