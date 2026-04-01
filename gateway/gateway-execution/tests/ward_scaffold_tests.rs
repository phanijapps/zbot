//! Integration tests for the ward scaffolding middleware.
//!
//! These tests exercise `scaffold_ward` through the public crate interface
//! and verify file-system side effects using a temporary directory.

use gateway_execution::middleware::ward_scaffold::scaffold_ward;
use gateway_execution::runner::auto_update_agents_md_with_lang_configs;
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

// ---------------------------------------------------------------------------
// 6. test_full_ward_lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_full_ward_lifecycle() {
    // 1. Create vault root
    let vault_tmp = tempdir().unwrap();
    let vault_dir = vault_tmp.path();

    let ward_id = "financial-analysis";

    // 2. Create the ward directory inside the vault: {vault}/wards/financial-analysis/
    let ward_dir = vault_dir.join("wards").join(ward_id);
    std::fs::create_dir_all(&ward_dir).unwrap();

    // 3. Create language config directory: {vault}/config/wards/
    let lang_configs_dir = vault_dir.join("config").join("wards");
    std::fs::create_dir_all(&lang_configs_dir).unwrap();

    // 4. Write python.yaml with function/class signature patterns
    let python_yaml = r#"language: python
file_extensions:
  - py
signature_patterns:
  function: "(?m)^(def \\w+\\([^)]*\\))"
  class: "(?m)^(class \\w+[^:]*):"
docstring_pattern: '(?s)"""(.*?)"""'
conventions:
  - "Import from core/"
"#;
    std::fs::write(lang_configs_dir.join("python.yaml"), python_yaml).unwrap();

    // 5. Call scaffold_ward with WardSetup containing directories and agents_md config
    let setup = WardSetup {
        directories: vec!["core".to_string(), "output".to_string(), "specs".to_string()],
        language_skills: vec![],
        spec_guidance: None,
        agents_md: Some(WardAgentsMdConfig {
            purpose: "A ward for financial analysis and market data processing.".to_string(),
            conventions: vec![
                "Import from core/".to_string(),
                "Use kebab-case for output file names".to_string(),
            ],
        }),
    };

    scaffold_ward(&ward_dir, ward_id, &[setup]);

    // 6. Verify all directories were created
    assert!(ward_dir.join("core").is_dir(), "core/ should exist");
    assert!(ward_dir.join("output").is_dir(), "output/ should exist");
    assert!(ward_dir.join("specs").is_dir(), "specs/ should exist");

    // 7. Verify AGENTS.md was created with correct purpose/conventions
    let agents_md_path = ward_dir.join("AGENTS.md");
    assert!(agents_md_path.exists(), "AGENTS.md should be created");

    let initial_content = std::fs::read_to_string(&agents_md_path).unwrap();
    assert!(
        initial_content.contains("financial-analysis"),
        "ward id missing from AGENTS.md"
    );
    assert!(
        initial_content.contains("A ward for financial analysis and market data processing."),
        "purpose missing from AGENTS.md"
    );
    assert!(
        initial_content.contains("Import from core/"),
        "first convention missing from AGENTS.md"
    );
    assert!(
        initial_content.contains("Use kebab-case for output file names"),
        "second convention missing from AGENTS.md"
    );

    // 8. Simulate an agent writing a core module
    let python_src = r#""""Fetch market data from various sources."""

def fetch_ohlcv(ticker, period):
    """Fetch OHLCV data for a given ticker."""
    pass

def fetch_fundamentals(ticker):
    """Get fundamental financial metrics."""
    pass
"#;
    std::fs::write(ward_dir.join("core").join("data_fetcher.py"), python_src).unwrap();

    // 9. Call auto_update_agents_md_with_lang_configs to simulate the post-execution hook
    auto_update_agents_md_with_lang_configs(vault_dir, ward_id, &lang_configs_dir);

    // 10. Verify the updated AGENTS.md contains the core module index with function names
    let updated_content = std::fs::read_to_string(&agents_md_path).unwrap();
    assert!(
        updated_content.contains("data_fetcher"),
        "core module filename should appear in updated AGENTS.md"
    );
    assert!(
        updated_content.contains("fetch_ohlcv"),
        "fetch_ohlcv function should be indexed in updated AGENTS.md"
    );
    assert!(
        updated_content.contains("fetch_fundamentals"),
        "fetch_fundamentals function should be indexed in updated AGENTS.md"
    );
}
