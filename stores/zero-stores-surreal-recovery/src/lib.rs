//! Placeholder corruption-recovery for `knowledge.surreal` RocksDB directories.
//!
//! Invoked by the `agentzero recover-knowledge` CLI subcommand when the
//! daemon refuses to start due to a corrupt RocksDB. NOT auto-invoked.

use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("recovery failed: {0}")]
    Failed(String),
}

#[derive(Debug)]
pub struct RecoveryReport {
    pub original_path: PathBuf,
    pub renamed_to: Option<PathBuf>,
    pub sidecar_export: Option<PathBuf>,
    pub entities_exported: usize,
    pub relationships_exported: usize,
}

/// Attempt to recover a corrupted SurrealDB RocksDB directory.
///
/// Strategy:
/// 1. Try to open with read-only mode.
/// 2. On success, export entities/relationships to a JSON sidecar.
/// 3. Rename the corrupt directory aside (`<path>.corrupted-<unix_ts>`).
/// 4. Return a report.
pub async fn recover_knowledge_db(path: &Path) -> Result<RecoveryReport, RecoveryError> {
    let _ = path;
    Err(RecoveryError::Failed(
        "recovery not yet implemented; see Task 14".into(),
    ))
}
