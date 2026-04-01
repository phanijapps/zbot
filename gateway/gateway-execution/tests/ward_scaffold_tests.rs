//! Integration tests for the ward scaffolding middleware.
//!
//! These tests exercise `scaffold_ward` through the public crate interface
//! and verify file-system side effects using a temporary directory.

use gateway_execution::middleware::ward_scaffold::scaffold_ward;
use gateway_services::skills::{WardAgentsMdConfig, WardSetup};
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 1. scaffold_creates_directories
// ---------------------------------------------------------------------------

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
        "specs/archive/ should exist as a nested directory"
    );
}

// ---------------------------------------------------------------------------
// 2. scaffold_generates_agents_md
// ---------------------------------------------------------------------------

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

    assert!(
        content.contains("# financial-analysis"),
        "title missing in AGENTS.md"
    );
    assert!(
        content.contains("A ward for financial analysis work."),
        "purpose missing in AGENTS.md"
    );
    assert!(
        content.contains("Use kebab-case for file names"),
        "first convention missing"
    );
    assert!(
        content.contains("All scripts go in core/"),
        "second convention missing"
    );
    assert!(content.contains("`core/`"), "core directory entry missing");
    assert!(content.contains("`output/`"), "output directory entry missing");
}

// ---------------------------------------------------------------------------
// 3. scaffold_does_not_overwrite_existing_agents_md
// ---------------------------------------------------------------------------

#[test]
fn scaffold_does_not_overwrite_existing_agents_md() {
    let dir = tempdir().unwrap();
    let ward_dir = dir.path();

    let original = "# preserved content\n\nDo not overwrite me.\n";
    std::fs::write(ward_dir.join("AGENTS.md"), original).unwrap();

    let setup = setup_with_agents_md(&["core"], "This should not appear.", &[]);
    scaffold_ward(ward_dir, "my-ward", &[setup]);

    let content = std::fs::read_to_string(ward_dir.join("AGENTS.md")).unwrap();
    assert_eq!(
        content, original,
        "AGENTS.md must not be overwritten when it already exists"
    );
}

// ---------------------------------------------------------------------------
// 4. scaffold_merges_multiple_skill_setups
// ---------------------------------------------------------------------------

#[test]
fn scaffold_merges_multiple_skill_setups() {
    let dir = tempdir().unwrap();
    let ward_dir = dir.path();

    let setup_a = setup_with_dirs(&["core", "output"]);
    let setup_b = setup_with_dirs(&["specs", "output"]); // "output" is a duplicate

    scaffold_ward(ward_dir, "merged-ward", &[setup_a, setup_b]);

    assert!(ward_dir.join("core").is_dir(), "core/ should exist");
    assert!(ward_dir.join("output").is_dir(), "output/ should exist");
    assert!(ward_dir.join("specs").is_dir(), "specs/ should exist");

    // Ensure no extra entries were created (output counted once)
    let entries: Vec<_> = std::fs::read_dir(ward_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 3, "exactly three directories should exist");
}

// ---------------------------------------------------------------------------
// 5. scaffold_empty_setups_creates_nothing
// ---------------------------------------------------------------------------

#[test]
fn scaffold_empty_setups_creates_nothing() {
    let dir = tempdir().unwrap();
    let ward_dir = dir.path();

    scaffold_ward(ward_dir, "empty-ward", &[]);

    let entries: Vec<_> = std::fs::read_dir(ward_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        entries.is_empty(),
        "empty setups must not create any files or directories"
    );
}
