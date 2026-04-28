//! Alias management + entity resolution.

use std::sync::Arc;

use knowledge_graph::types::EntityType;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::types::{RecordId, SurrealValue};
use zero_stores::error::StoreResult;
use zero_stores::types::{EntityId, ResolveOutcome};

use crate::error::map_surreal_error;
use crate::types::{EntityIdExt, ThingExt};

pub async fn add_alias(
    db: &Arc<Surreal<Any>>,
    entity_id: &EntityId,
    surface: &str,
) -> StoreResult<()> {
    db.query("CREATE entity_alias SET entity_id = $eid, surface = $s")
        .bind(("eid", entity_id.to_thing()))
        .bind(("s", surface.to_string()))
        .await
        .map_err(map_surreal_error)?;
    Ok(())
}

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct EntityIdRow {
    id: RecordId,
}

#[derive(SurrealValue)]
#[surreal(crate = "surrealdb::types")]
struct AliasRow {
    entity_id: RecordId,
}

pub async fn resolve_entity(
    db: &Arc<Surreal<Any>>,
    agent_id: &str,
    entity_type: &EntityType,
    name: &str,
    _embedding: Option<&[f32]>,
) -> StoreResult<ResolveOutcome> {
    // Stage 1: exact match on (agent_id, name, entity_type)
    let mut resp = db
        .query(
            "SELECT id FROM entity \
             WHERE agent_id = $a AND name = $n AND entity_type = $t LIMIT 1",
        )
        .bind(("a", agent_id.to_string()))
        .bind(("n", name.to_string()))
        .bind(("t", entity_type.as_str().to_string()))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<EntityIdRow> = resp.take(0).map_err(map_surreal_error)?;
    if let Some(row) = rows.into_iter().next() {
        return Ok(ResolveOutcome::Match(row.id.to_entity_id()));
    }

    // Stage 2: alias surface match
    let mut resp = db
        .query("SELECT entity_id FROM entity_alias WHERE surface = $n LIMIT 1")
        .bind(("n", name.to_string()))
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<AliasRow> = resp.take(0).map_err(map_surreal_error)?;
    if let Some(row) = rows.into_iter().next() {
        return Ok(ResolveOutcome::Match(row.entity_id.to_entity_id()));
    }

    // Stage 3 (embedding-similarity match) is wired in Task 9 once HNSW exists.
    Ok(ResolveOutcome::NoMatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kg::entity;
    use crate::{SurrealConfig, connect, schema::apply_schema};
    use knowledge_graph::types::Entity;

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
    async fn resolve_exact_match_returns_existing() {
        let db = fresh_db().await;
        let e = Entity::new("a1".into(), EntityType::Person, "Carol".into());
        let id = entity::upsert(&db, "a1", e).await.expect("upsert");

        let out = resolve_entity(&db, "a1", &EntityType::Person, "Carol", None)
            .await
            .expect("resolve");
        match out {
            ResolveOutcome::Match(found) => assert_eq!(found.as_ref(), id.as_ref()),
            ResolveOutcome::NoMatch => panic!("should match existing"),
        }
    }

    #[tokio::test]
    async fn resolve_via_alias() {
        let db = fresh_db().await;
        let e = Entity::new("a1".into(), EntityType::Person, "Carol".into());
        let id = entity::upsert(&db, "a1", e).await.expect("upsert");
        add_alias(&db, &id, "Carolyn").await.expect("alias");

        let out = resolve_entity(&db, "a1", &EntityType::Person, "Carolyn", None)
            .await
            .expect("resolve");
        match out {
            ResolveOutcome::Match(found) => assert_eq!(found.as_ref(), id.as_ref()),
            ResolveOutcome::NoMatch => panic!("should match alias"),
        }
    }

    #[tokio::test]
    async fn resolve_no_match_returns_nomatch() {
        let db = fresh_db().await;
        let out = resolve_entity(&db, "a1", &EntityType::Person, "Unknown", None)
            .await
            .expect("resolve");
        assert!(matches!(out, ResolveOutcome::NoMatch));
    }
}
