//! Schema-version tracking and upgrade closures.

use std::sync::Arc;

use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores::error::StoreError;

use crate::error::map_surreal_error;

/// Read the current schema version stored in `zmeta:version`. Returns 0
/// if no version record exists (first launch) or the `zmeta` table is missing
/// (pre-bootstrap state — SurrealDB 3.0 errors on non-existent tables).
pub async fn read_version(db: &Arc<Surreal<Any>>) -> Result<u32, StoreError> {
    let mut resp = match db.query("SELECT data FROM zmeta:version").await {
        Ok(r) => r,
        Err(e) => {
            if format!("{e}").contains("does not exist") {
                return Ok(0);
            }
            return Err(map_surreal_error(e));
        }
    };
    let rows: Vec<serde_json::Value> = match resp.take(0) {
        Ok(r) => r,
        Err(e) => {
            if format!("{e}").contains("does not exist") {
                return Ok(0);
            }
            return Err(map_surreal_error(e));
        }
    };
    Ok(rows
        .into_iter()
        .next()
        .and_then(|v| {
            v.get("data")
                .and_then(|val| val.get("schema_version"))
                .and_then(|x| x.as_u64())
        })
        .map(|v| v as u32)
        .unwrap_or(0))
}

/// Write the current schema version into `zmeta:version`.
pub async fn write_version(db: &Arc<Surreal<Any>>, version: u32) -> Result<(), StoreError> {
    db.query("UPSERT zmeta:version SET data = { schema_version: $v }")
        .bind(("v", version as i64))
        .await
        .map_err(map_surreal_error)?;
    Ok(())
}

/// Run upgrade closures sequentially from `current+1 .. CURRENT_SCHEMA_VERSION`.
///
/// Today there are no upgrades — `CURRENT_SCHEMA_VERSION = 1` and any DB at
/// version 0 just gets bumped. Future breaking changes plug in here.
pub async fn run_upgrades(db: &Arc<Surreal<Any>>) -> Result<(), StoreError> {
    let current = read_version(db).await?;
    if current < super::CURRENT_SCHEMA_VERSION {
        write_version(db, super::CURRENT_SCHEMA_VERSION).await?;
    }
    Ok(())
}
