//! GET / PUT endpoints for editing markdown files under `<vault>/config/`.
//! Used by the Settings → Customization UI tab.

use std::path::PathBuf;

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
}
