//! Integration tests for the ward scaffolding middleware.
//!
//! These tests exercise `scaffold_ward` through the public crate interface
//! and verify file-system side effects using a temporary directory.

use gateway_execution::middleware::ward_scaffold::scaffold_ward;
use gateway_execution::runner::auto_update_agents_md_with_lang_configs;
use gateway_services::skills::{WardAgentsMdConfig, WardSetup};
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// test_full_ward_lifecycle
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
        directories: vec![
            "core".to_string(),
            "output".to_string(),
            "specs".to_string(),
        ],
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
