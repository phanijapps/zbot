//! `reindex_embeddings` — idempotent when dim unchanged, full rebuild on dim change.

use std::sync::Arc;

use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use zero_stores::error::StoreResult;
use zero_stores::types::ReindexReport;

use crate::error::map_surreal_error;
use crate::schema::hnsw;

pub async fn reindex_embeddings(
    db: &Arc<Surreal<Any>>,
    new_dim: usize,
) -> StoreResult<ReindexReport> {
    let current_dim = hnsw::read_dim(db).await?;
    if current_dim == Some(new_dim) {
        return Ok(ReindexReport {
            tables_rebuilt: Vec::new(),
            rows_indexed: 0,
        });
    }

    // Drop old HNSW index (no-op if absent).
    let _ = hnsw::remove_index(db).await;

    // Clear stale embeddings whose dim no longer matches the target.
    db.query(
        "UPDATE entity SET embedding = NONE \
         WHERE embedding != NONE AND array::len(embedding) != $d",
    )
    .bind(("d", new_dim as i64))
    .await
    .map_err(map_surreal_error)?;

    // Persist new dim + define new HNSW with that dim.
    hnsw::write_dim(db, new_dim).await?;
    hnsw::define_index(db, new_dim).await?;

    Ok(ReindexReport {
        tables_rebuilt: vec!["entity_embedding_hnsw".to_string()],
        rows_indexed: 0,
    })
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
        let db = connect(&cfg, None).await.unwrap();
        apply_schema(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn reindex_idempotent_when_dim_matches() {
        let db = fresh_db().await;
        hnsw::ensure_index(&db, 1024).await.unwrap();

        let report = reindex_embeddings(&db, 1024).await.unwrap();
        assert!(report.tables_rebuilt.is_empty(), "should be no-op");
    }

    #[tokio::test]
    async fn reindex_rebuilds_on_dim_change() {
        let db = fresh_db().await;
        hnsw::ensure_index(&db, 1024).await.unwrap();

        let report = reindex_embeddings(&db, 1536).await.unwrap();
        assert!(!report.tables_rebuilt.is_empty());
        assert_eq!(hnsw::read_dim(&db).await.unwrap(), Some(1536));
    }

    #[tokio::test]
    async fn reindex_from_empty_state_creates_index() {
        let db = fresh_db().await;
        let report = reindex_embeddings(&db, 1024).await.unwrap();
        assert!(!report.tables_rebuilt.is_empty());
        assert_eq!(hnsw::read_dim(&db).await.unwrap(), Some(1024));
    }
}
