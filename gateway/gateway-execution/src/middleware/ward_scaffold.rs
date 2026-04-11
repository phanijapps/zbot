//! # Ward Scaffold Middleware
//!
//! Creates a ward directory structure based on `ward_setup` frontmatter from
//! one or more skills. Scaffolding is best-effort — failures are logged as
//! warnings and never propagate as errors, so they cannot crash execution.

use gateway_services::skills::{WardAgentsMdConfig, WardSetup};
use std::path::Path;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scaffold a ward directory from one or more skill `ward_setup` configs.
///
/// # Behaviour
/// 1. If `setups` is empty, returns immediately (no-op).
/// 2. Merges directories from all setups (deduplicated, order preserved).
/// 3. Creates each directory under `ward_dir`, skipping ones that already exist.
///    Nested paths like `specs/archive/` are handled via `create_dir_all`.
/// 4. Writes `AGENTS.md` **only** if it does not already exist.  Uses the first
///    setup that has an `agents_md` config.
pub fn scaffold_ward(ward_dir: &Path, ward_name: &str, setups: &[WardSetup]) {
    if setups.is_empty() {
        return;
    }

    let directories = merge_directories(setups);

    create_directories(ward_dir, &directories);

    write_agents_md_if_absent(ward_dir, ward_name, setups, &directories);
}

// ---------------------------------------------------------------------------
// Directory helpers
// ---------------------------------------------------------------------------

/// Merge directories from all setups, preserving insertion order and
/// deduplicating by the normalised directory name (trailing slash stripped).
fn merge_directories(setups: &[WardSetup]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for setup in setups {
        for dir in &setup.directories {
            let normalised = dir.trim_end_matches('/').to_string();
            if normalised.is_empty() {
                continue;
            }
            if seen.insert(normalised.clone()) {
                result.push(normalised);
            }
        }
    }

    result
}

/// Create each directory under `ward_dir`.  Nested paths are created
/// recursively via `create_dir_all`.  Existing directories are silently
/// skipped.
fn create_directories(ward_dir: &Path, directories: &[String]) {
    for dir_name in directories {
        let target = ward_dir.join(dir_name);

        if target.exists() {
            continue;
        }

        if let Err(e) = std::fs::create_dir_all(&target) {
            tracing::warn!(
                dir = %target.display(),
                error = %e,
                "Failed to create ward directory — skipping"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// AGENTS.md helpers
// ---------------------------------------------------------------------------

/// Write `AGENTS.md` under `ward_dir` if it doesn't already exist.
///
/// Uses the first `WardSetup` that carries `agents_md` config.  If none do,
/// no file is written.
fn write_agents_md_if_absent(
    ward_dir: &Path,
    ward_name: &str,
    setups: &[WardSetup],
    directories: &[String],
) {
    let agents_md_path = ward_dir.join("AGENTS.md");

    if agents_md_path.exists() {
        return;
    }

    let config = setups.iter().find_map(|s| s.agents_md.as_ref());

    let Some(config) = config else {
        return;
    };

    let content = generate_agents_md(ward_name, config, directories);

    if let Err(e) = std::fs::write(&agents_md_path, &content) {
        tracing::warn!(
            path = %agents_md_path.display(),
            error = %e,
            "Failed to write AGENTS.md — skipping"
        );
    }
}

/// Generate AGENTS.md content for a new ward.
///
/// Sections included:
/// - Title (ward name)
/// - Purpose
/// - Directory layout
/// - Conventions
/// - Core modules placeholder
/// - History
fn generate_agents_md(
    ward_name: &str,
    config: &WardAgentsMdConfig,
    directories: &[String],
) -> String {
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let mut out = String::new();

    // Title
    out.push_str(&format!("# {}\n\n", ward_name));

    // Purpose
    out.push_str("## Purpose\n\n");
    out.push_str(&config.purpose);
    out.push_str("\n\n");

    // Directory layout
    out.push_str("## Directory Layout\n\n");
    if directories.is_empty() {
        out.push_str("_No directories configured._\n\n");
    } else {
        for dir in directories {
            out.push_str(&format!("- `{}/`\n", dir));
        }
        out.push('\n');
    }

    // Conventions
    if !config.conventions.is_empty() {
        out.push_str("## Conventions\n\n");
        for convention in &config.conventions {
            out.push_str(&format!("- {}\n", convention));
        }
        out.push('\n');
    }

    // Core modules placeholder
    out.push_str("## Core Modules\n\n");
    out.push_str("_Document key modules and their responsibilities here._\n\n");

    // History
    out.push_str("## History\n\n");
    out.push_str(&format!("- {} — Ward scaffolded\n", date));

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_services::skills::{WardAgentsMdConfig, WardSetup};
    use tempfile::tempdir;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn setup_with_dirs(dirs: &[&str]) -> WardSetup {
        WardSetup {
            directories: dirs.iter().map(|s| s.to_string()).collect(),
            language_skills: vec![],
            spec_guidance: None,
            agents_md: None,
        }
    }

    fn setup_with_agents_md(dirs: &[&str], purpose: &str, conventions: &[&str]) -> WardSetup {
        WardSetup {
            directories: dirs.iter().map(|s| s.to_string()).collect(),
            language_skills: vec![],
            spec_guidance: None,
            agents_md: Some(WardAgentsMdConfig {
                purpose: purpose.to_string(),
                conventions: conventions.iter().map(|s| s.to_string()).collect(),
            }),
        }
    }

    // -----------------------------------------------------------------------
    // 1. scaffold_creates_directories
    // -----------------------------------------------------------------------

    #[test]
    fn scaffold_creates_directories() {
        let dir = tempdir().unwrap();
        let ward_dir = dir.path();

        let setup = setup_with_dirs(&["core", "output", "specs/archive/"]);
        scaffold_ward(ward_dir, "my-ward", &[setup]);

        assert!(ward_dir.join("core").is_dir(), "core/ should exist");
        assert!(ward_dir.join("output").is_dir(), "output/ should exist");
        assert!(
            ward_dir.join("specs/archive").is_dir(),
            "specs/archive/ should exist (nested)"
        );
    }

    // -----------------------------------------------------------------------
    // 2. scaffold_generates_agents_md
    // -----------------------------------------------------------------------

    #[test]
    fn scaffold_generates_agents_md() {
        let dir = tempdir().unwrap();
        let ward_dir = dir.path();

        let setup = setup_with_agents_md(
            &["core", "output"],
            "A ward for financial analysis work.",
            &["Use kebab-case for file names", "All scripts go in core/"],
        );

        scaffold_ward(ward_dir, "financial-analysis", &[setup]);

        let agents_md_path = ward_dir.join("AGENTS.md");
        assert!(agents_md_path.exists(), "AGENTS.md should be created");

        let content = std::fs::read_to_string(&agents_md_path).unwrap();

        assert!(content.contains("# financial-analysis"), "title missing");
        assert!(
            content.contains("A ward for financial analysis work."),
            "purpose missing"
        );
        assert!(
            content.contains("Use kebab-case for file names"),
            "convention missing"
        );
        assert!(
            content.contains("All scripts go in core/"),
            "convention missing"
        );
        assert!(
            content.contains("`core/`"),
            "directory layout missing: core"
        );
        assert!(
            content.contains("`output/`"),
            "directory layout missing: output"
        );
    }

    // -----------------------------------------------------------------------
    // 3. scaffold_does_not_overwrite_existing_agents_md
    // -----------------------------------------------------------------------

    #[test]
    fn scaffold_does_not_overwrite_existing_agents_md() {
        let dir = tempdir().unwrap();
        let ward_dir = dir.path();

        let original = "# preserved content\n\nDo not overwrite me.\n";
        std::fs::write(ward_dir.join("AGENTS.md"), original).unwrap();

        let setup = setup_with_agents_md(&["core"], "New purpose.", &[]);
        scaffold_ward(ward_dir, "my-ward", &[setup]);

        let content = std::fs::read_to_string(ward_dir.join("AGENTS.md")).unwrap();
        assert_eq!(content, original, "AGENTS.md should not be overwritten");
    }

    // -----------------------------------------------------------------------
    // 4. scaffold_merges_multiple_skill_setups
    // -----------------------------------------------------------------------

    #[test]
    fn scaffold_merges_multiple_skill_setups() {
        let dir = tempdir().unwrap();
        let ward_dir = dir.path();

        let setup_a = setup_with_dirs(&["core", "output"]);
        let setup_b = setup_with_dirs(&["specs", "output"]); // "output" is duplicated

        scaffold_ward(ward_dir, "merged-ward", &[setup_a, setup_b]);

        assert!(ward_dir.join("core").is_dir(), "core/ should exist");
        assert!(ward_dir.join("output").is_dir(), "output/ should exist");
        assert!(ward_dir.join("specs").is_dir(), "specs/ should exist");
    }

    // -----------------------------------------------------------------------
    // 5. scaffold_empty_setups_creates_nothing
    // -----------------------------------------------------------------------

    #[test]
    fn scaffold_empty_setups_creates_nothing() {
        let dir = tempdir().unwrap();
        let ward_dir = dir.path();

        scaffold_ward(ward_dir, "empty-ward", &[]);

        // No directories should have been created (besides the tempdir itself)
        let entries: Vec<_> = std::fs::read_dir(ward_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(entries.is_empty(), "no files or dirs should be created");
    }
}
