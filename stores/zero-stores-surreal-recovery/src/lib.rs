//! Corruption-recovery for `knowledge.surreal` RocksDB directories.
//!
//! Invoked by the `zero recover-knowledge` CLI subcommand when the daemon
//! refuses to start due to a corrupt RocksDB. NOT auto-invoked.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("path does not exist: {0}")]
    NotFound(PathBuf),
    #[error("rename failed: {0}")]
    Rename(#[from] std::io::Error),
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

/// Attempt recovery. Strategy v0:
/// 1. Verify the path exists.
/// 2. Try to open with SurrealDB. If successful, export entities/relationships
///    to a JSON sidecar.
/// 3. Rename the directory aside (`<path>.corrupted-<unix_ts>`).
/// 4. Return a report.
pub async fn recover_knowledge_db(path: &Path) -> Result<RecoveryReport, RecoveryError> {
    if !path.exists() {
        return Err(RecoveryError::NotFound(path.to_path_buf()));
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| RecoveryError::Failed(format!("clock: {e}")))?
        .as_secs();

    let sidecar = path
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("knowledge.recovery.{timestamp}.json"));
    let url = format!("rocksdb://{}", path.display());
    let mut entities_exported = 0;
    let mut relationships_exported = 0;
    let sidecar_export = match try_export(&url, &sidecar).await {
        Ok((e, r)) => {
            entities_exported = e;
            relationships_exported = r;
            Some(sidecar)
        }
        Err(_) => None,
    };

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "knowledge".into());
    let renamed = path
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{file_name}.corrupted-{timestamp}"));
    std::fs::rename(path, &renamed)?;

    Ok(RecoveryReport {
        original_path: path.to_path_buf(),
        renamed_to: Some(renamed),
        sidecar_export,
        entities_exported,
        relationships_exported,
    })
}

async fn try_export(url: &str, sidecar: &Path) -> Result<(usize, usize), RecoveryError> {
    use surrealdb::engine::any::connect;

    let db = connect(url)
        .await
        .map_err(|e| RecoveryError::Failed(format!("open: {e}")))?;
    db.use_ns("memory_kg")
        .use_db("main")
        .await
        .map_err(|e| RecoveryError::Failed(format!("ns: {e}")))?;

    let mut resp = db
        .query("SELECT * FROM entity; SELECT * FROM relationship")
        .await
        .map_err(|e| RecoveryError::Failed(format!("query: {e}")))?;
    let entities: Vec<serde_json::Value> = resp
        .take(0)
        .map_err(|e| RecoveryError::Failed(format!("take entities: {e}")))?;
    let relationships: Vec<serde_json::Value> = resp
        .take(1)
        .map_err(|e| RecoveryError::Failed(format!("take rels: {e}")))?;

    let payload = serde_json::json!({
        "entities": entities,
        "relationships": relationships,
    });
    std::fs::write(
        sidecar,
        serde_json::to_vec_pretty(&payload)
            .map_err(|e| RecoveryError::Failed(format!("serialize: {e}")))?,
    )?;

    Ok((entities.len(), relationships.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn errors_when_path_missing() {
        let res = recover_knowledge_db(Path::new("/nonexistent/path/xyz")).await;
        assert!(matches!(res, Err(RecoveryError::NotFound(_))));
    }

    #[tokio::test]
    async fn renames_directory_aside() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("knowledge.surreal");
        std::fs::create_dir(&path).expect("mkdir");
        std::fs::write(path.join("MARKER"), b"x").expect("write marker");

        let report = recover_knowledge_db(&path).await.expect("recover");
        assert!(report.renamed_to.is_some());
        let renamed = report.renamed_to.unwrap();
        assert!(renamed.exists());
        assert!(renamed.join("MARKER").exists());
        assert!(!path.exists(), "original path should be gone");
    }
}
