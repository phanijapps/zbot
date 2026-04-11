//! # Artifact Processing
//!
//! Resolves artifact declarations from agent responses, validates file existence,
//! and persists metadata to the artifacts table.

use execution_state::{Artifact, StateService};
use gateway_database::DatabaseManager;
use std::path::Path;
use std::sync::Arc;

/// Process artifact declarations from a respond action.
///
/// For each declaration:
/// 1. Resolve absolute path (relative paths resolved against ward directory)
/// 2. Verify the file exists and read metadata (size)
/// 3. Detect file type from extension
/// 4. Persist to the `artifacts` table
///
/// Returns the list of successfully persisted artifacts.
pub fn process_artifact_declarations(
    declarations: &[zero_core::event::ArtifactDeclaration],
    session_id: &str,
    execution_id: &str,
    agent_id: &str,
    ward_id: Option<&str>,
    vault_dir: &Path,
    state_service: &Arc<StateService<DatabaseManager>>,
) -> Vec<Artifact> {
    let mut persisted = Vec::new();

    for decl in declarations {
        // Resolve absolute path
        let abs_path = if std::path::Path::new(&decl.path).is_absolute() {
            std::path::PathBuf::from(&decl.path)
        } else if let Some(ward) = ward_id {
            vault_dir.join("wards").join(ward).join(&decl.path)
        } else {
            std::path::PathBuf::from(&decl.path)
        };

        // Check file exists
        let meta = match std::fs::metadata(&abs_path) {
            Ok(m) => m,
            Err(_) => {
                tracing::warn!(path = %abs_path.display(), "Artifact declared but file not found");
                continue;
            }
        };

        let file_name = abs_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| decl.path.clone());

        let file_type = abs_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase());

        let mut artifact = Artifact::new(
            session_id,
            abs_path.to_string_lossy().to_string(),
            &file_name,
        );
        artifact.ward_id = ward_id.map(|s| s.to_string());
        artifact.execution_id = Some(execution_id.to_string());
        artifact.agent_id = Some(agent_id.to_string());
        artifact.file_type = file_type;
        artifact.file_size = Some(meta.len() as i64);
        artifact.label = decl.label.clone();

        if let Err(e) = state_service.create_artifact(&artifact) {
            tracing::warn!(artifact_id = %artifact.id, "Failed to persist artifact: {}", e);
            continue;
        }

        tracing::info!(
            artifact_id = %artifact.id,
            path = %abs_path.display(),
            "Artifact persisted"
        );
        persisted.push(artifact);
    }

    persisted
}
