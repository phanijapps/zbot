//! GET / PUT endpoints for editing markdown files under `<vault>/config/`.
//! Used by the Settings → Customization UI tab.

use crate::state::AppState;
use axum::{extract::State, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Validate a relative path supplied by the UI.
///
/// Allowed shapes (server only ever reads/writes files matching these):
/// - `<file>.md`               → root-level config markdown
/// - `shards/<file>.md`        → markdown shard
///
/// Rejected:
/// - empty
/// - absolute path (`/...` or `\...`)
/// - parent traversal (`..`)
/// - non-`.md` files
/// - any nested path beyond `shards/<file>.md`
// Allowed: handlers that consume this validator land in a follow-up task; tests already exercise it.
#[allow(dead_code)]
pub(crate) fn validate_customization_path(p: &str) -> Result<PathBuf, &'static str> {
    if p.is_empty() || p.starts_with('/') || p.starts_with('\\') {
        return Err("invalid path");
    }
    if p.contains("..") {
        return Err("invalid path");
    }
    if !p.ends_with(".md") {
        return Err("only markdown files allowed");
    }
    let parts: Vec<&str> = p.split('/').collect();
    match parts.as_slice() {
        [_file] => Ok(PathBuf::from(p)),
        ["shards", _file] => Ok(PathBuf::from(p)),
        _ => Err("invalid path"),
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub kind: FileKind,
    pub size: u64,
    pub modified_at: String,
    pub auto_generated: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Root,
    Shard,
}

// Wired by the route registration in a follow-up task.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

const AUTO_GENERATED_NAMES: &[&str] = &["OS.md"];

/// Walk `config_dir/` and `config_dir/shards/` and return entries for every `*.md`.
pub(crate) fn enumerate_customization_files(config_dir: &Path) -> std::io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    push_md_files(config_dir, FileKind::Root, "", &mut entries)?;
    let shards = config_dir.join("shards");
    if shards.is_dir() {
        push_md_files(&shards, FileKind::Shard, "shards/", &mut entries)?;
    }
    entries.sort_by(|a, b| {
        a.kind_order()
            .cmp(&b.kind_order())
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(entries)
}

fn push_md_files(
    dir: &Path,
    kind: FileKind,
    prefix: &str,
    out: &mut Vec<FileEntry>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if n.ends_with(".md") => n.to_string(),
            _ => continue,
        };
        let metadata = entry.metadata()?;
        let modified_at: DateTime<Utc> = metadata.modified()?.into();
        let auto_generated = AUTO_GENERATED_NAMES.contains(&name.as_str());
        out.push(FileEntry {
            path: format!("{}{}", prefix, name),
            kind,
            size: metadata.len(),
            modified_at: modified_at.to_rfc3339(),
            auto_generated,
        });
    }
    Ok(())
}

impl FileEntry {
    fn kind_order(&self) -> u8 {
        match self.kind {
            FileKind::Root => 0,
            FileKind::Shard => 1,
        }
    }
}

/// `GET /api/customization/files` — list editable markdowns.
// Wired by the route registration in a follow-up task.
#[allow(dead_code)]
pub async fn list_files(State(state): State<AppState>) -> (StatusCode, Json<ListResponse>) {
    let config_dir = state.paths.config_dir();
    match enumerate_customization_files(&config_dir) {
        Ok(files) => (
            StatusCode::OK,
            Json(ListResponse {
                success: true,
                files: Some(files),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ListResponse {
                success: false,
                files: None,
                error: Some(e.to_string()),
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert_eq!(validate_customization_path(""), Err("invalid path"));
    }

    #[test]
    fn rejects_absolute_unix() {
        assert_eq!(
            validate_customization_path("/etc/passwd"),
            Err("invalid path")
        );
    }

    #[test]
    fn rejects_absolute_windows() {
        assert_eq!(
            validate_customization_path("\\Windows\\System32"),
            Err("invalid path")
        );
    }

    #[test]
    fn rejects_parent_traversal() {
        assert_eq!(
            validate_customization_path("../../etc/passwd"),
            Err("invalid path")
        );
        assert_eq!(
            validate_customization_path("shards/../../etc/passwd"),
            Err("invalid path")
        );
    }

    #[test]
    fn rejects_non_md() {
        assert_eq!(
            validate_customization_path("settings.json"),
            Err("only markdown files allowed")
        );
        assert_eq!(
            validate_customization_path("shards/foo.txt"),
            Err("only markdown files allowed")
        );
    }

    #[test]
    fn rejects_nested_subdirs() {
        assert_eq!(
            validate_customization_path("wards/foo/bar.md"),
            Err("invalid path")
        );
        assert_eq!(
            validate_customization_path("shards/foo/bar.md"),
            Err("invalid path")
        );
    }

    #[test]
    fn accepts_root_md() {
        assert_eq!(
            validate_customization_path("SOUL.md"),
            Ok(PathBuf::from("SOUL.md"))
        );
        assert_eq!(
            validate_customization_path("INSTRUCTIONS.md"),
            Ok(PathBuf::from("INSTRUCTIONS.md"))
        );
    }

    #[test]
    fn accepts_shard_md() {
        assert_eq!(
            validate_customization_path("shards/memory_learning.md"),
            Ok(PathBuf::from("shards/memory_learning.md"))
        );
    }

    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn list_files_finds_root_and_shards() {
        let tmp = tempdir().unwrap();
        let config = tmp.path();
        fs::write(config.join("SOUL.md"), "soul").unwrap();
        fs::write(config.join("INSTRUCTIONS.md"), "instr").unwrap();
        fs::write(config.join("OS.md"), "os").unwrap();
        fs::write(config.join("settings.json"), "{}").unwrap();
        fs::create_dir_all(config.join("shards")).unwrap();
        fs::write(
            config.join("shards").join("first_turn_protocol.md"),
            "shard",
        )
        .unwrap();
        fs::write(config.join("shards").join("ignored.txt"), "no").unwrap();

        let entries = enumerate_customization_files(config).expect("enumerate ok");
        let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"SOUL.md"));
        assert!(paths.contains(&"INSTRUCTIONS.md"));
        assert!(paths.contains(&"OS.md"));
        assert!(paths.contains(&"shards/first_turn_protocol.md"));
        assert!(!paths.contains(&"settings.json"));
        assert!(!paths.contains(&"shards/ignored.txt"));

        let os_entry = entries.iter().find(|e| e.path == "OS.md").unwrap();
        assert!(os_entry.auto_generated);

        let soul_entry = entries.iter().find(|e| e.path == "SOUL.md").unwrap();
        assert!(!soul_entry.auto_generated);
    }
}
