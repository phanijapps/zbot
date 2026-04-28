//! Schema bootstrap. Runs `DEFINE ... IF NOT EXISTS` statements on every
//! startup. Idempotent.

pub mod bootstrap;
pub mod hnsw;

use std::sync::Arc;

use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores::error::StoreError;

use crate::error::map_surreal_error;

const MEMORY_KG_SCHEMA: &str = include_str!("memory_kg.surql");

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Apply the canonical schema. Idempotent — every statement uses
/// `IF NOT EXISTS`. Also runs version-tracked upgrade closures via
/// [`bootstrap::run_upgrades`].
pub async fn apply_schema(db: &Arc<Surreal<Any>>) -> Result<(), StoreError> {
    db.query(MEMORY_KG_SCHEMA)
        .await
        .map_err(map_surreal_error)?;
    bootstrap::run_upgrades(db).await?;
    Ok(())
}
