//! Lazy HNSW vector-index management for the `entity` table.
//!
//! Strategy (per spec §6):
//! - Bootstrap does NOT define HNSW.
//! - On first embedding write: detect dim, persist `zmeta:embedding_config`,
//!   issue `DEFINE INDEX ... HNSW DIMENSION $dim ...`.
//! - On restart with embeddings present: read `zmeta:embedding_config`
//!   and re-issue `DEFINE INDEX ... IF NOT EXISTS DIMENSION $dim`. No rebuild.
//! - On dim change: REMOVE INDEX + clear stale + redefine + schedule re-embed.

use std::sync::Arc;

use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores::error::StoreError;

use crate::error::map_surreal_error;

const HNSW_INDEX_NAME: &str = "entity_embedding_hnsw";

/// Read the persisted embedding dim, if any.
pub async fn read_dim(db: &Arc<Surreal<Any>>) -> Result<Option<usize>, StoreError> {
    let mut resp = db
        .query("SELECT data FROM zmeta:embedding_config")
        .await
        .map_err(map_surreal_error)?;
    let rows: Vec<serde_json::Value> = resp.take(0).map_err(map_surreal_error)?;
    Ok(rows
        .into_iter()
        .next()
        .and_then(|v| {
            v.get("data")
                .and_then(|val| val.get("dim"))
                .and_then(|x| x.as_u64())
        })
        .map(|v| v as usize))
}

pub async fn write_dim(db: &Arc<Surreal<Any>>, dim: usize) -> Result<(), StoreError> {
    db.query("UPSERT zmeta:embedding_config SET data = { dim: $d }")
        .bind(("d", dim as i64))
        .await
        .map_err(map_surreal_error)?;
    Ok(())
}

/// Define the HNSW index for `entity.embedding`. Idempotent via `IF NOT EXISTS`.
pub async fn define_index(db: &Arc<Surreal<Any>>, dim: usize) -> Result<(), StoreError> {
    let q = format!(
        "DEFINE INDEX IF NOT EXISTS {HNSW_INDEX_NAME} ON entity FIELDS embedding \
         HNSW DIMENSION {dim} DIST COSINE"
    );
    db.query(q).await.map_err(map_surreal_error)?;
    Ok(())
}

/// Remove the HNSW index. Used during dim-change rebuild.
pub async fn remove_index(db: &Arc<Surreal<Any>>) -> Result<(), StoreError> {
    let q = format!("REMOVE INDEX {HNSW_INDEX_NAME} ON entity");
    db.query(q).await.map_err(map_surreal_error)?;
    Ok(())
}

/// Ensure the HNSW index exists for the given dim. Called on the first embedding
/// write and on every bootstrap when a dim is already known.
pub async fn ensure_index(db: &Arc<Surreal<Any>>, dim: usize) -> Result<(), StoreError> {
    let known = read_dim(db).await?;
    match known {
        Some(d) if d == dim => {
            // Idempotent restart path: define with IF NOT EXISTS.
            define_index(db, dim).await?;
        }
        Some(other) => {
            // Dim mismatch — caller must trigger reindex_embeddings, not us.
            return Err(StoreError::Backend(format!(
                "embedding dim mismatch: stored={other}, write={dim}; \
                 call reindex_embeddings first"
            )));
        }
        None => {
            write_dim(db, dim).await?;
            define_index(db, dim).await?;
        }
    }
    Ok(())
}
