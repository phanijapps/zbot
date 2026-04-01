//! Tests for WardSetup frontmatter parsing in skills.

use gateway_services::{SkillFrontmatter, WardAgentsMdConfig, WardSetup};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_public_frontmatter(yaml: &str) -> SkillFrontmatter {
    serde_yaml::from_str(yaml).expect("YAML should parse into SkillFrontmatter")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_full_ward_setup_parses_correctly() {
    let yaml = r#"
name: rust-dev
displayName: Rust Development
description: Helps with Rust development tasks.
category: engineering
ward_setup:
  directories:
    - src
    - tests
    - benches
  language_skills:
    - rust-development
    - cargo-tooling
  spec_guidance: "Follow the architecture spec in docs/spec.md"
  agents_md:
    purpose: "Guide the agent for Rust-first projects."
    conventions:
      - "Use clippy for linting"
      - "Prefer idiomatic Rust patterns"
"#;

    let fm = parse_public_frontmatter(yaml);

    assert_eq!(fm.name, "rust-dev");
    assert_eq!(fm.display_name.as_deref(), Some("Rust Development"));
    assert_eq!(fm.description, "Helps with Rust development tasks.");
    assert_eq!(fm.category.as_deref(), Some("engineering"));

    let ward = fm.ward_setup.expect("ward_setup should be present");
    assert_eq!(ward.directories, vec!["src", "tests", "benches"]);
    assert_eq!(ward.language_skills, vec!["rust-development", "cargo-tooling"]);
    assert_eq!(
        ward.spec_guidance.as_deref(),
        Some("Follow the architecture spec in docs/spec.md")
    );

    let agents_md = ward.agents_md.expect("agents_md should be present");
    assert_eq!(agents_md.purpose, "Guide the agent for Rust-first projects.");
    assert_eq!(
        agents_md.conventions,
        vec!["Use clippy for linting", "Prefer idiomatic Rust patterns"]
    );
}

#[test]
fn test_skill_without_ward_setup_is_backwards_compatible() {
    let yaml = r#"
name: general-helper
description: A general-purpose helper skill.
"#;

    let fm = parse_public_frontmatter(yaml);

    assert_eq!(fm.name, "general-helper");
    assert_eq!(fm.description, "A general-purpose helper skill.");
    assert!(fm.ward_setup.is_none(), "ward_setup should be None when not present");
}

#[test]
fn test_ward_agents_md_config_round_trips() {
    let config = WardAgentsMdConfig {
        purpose: "Lead a Python-focused ward.".to_string(),
        conventions: vec!["Use black for formatting".to_string(), "Type-hint everything".to_string()],
    };

    let yaml = serde_yaml::to_string(&config).expect("serialize should succeed");
    let round_tripped: WardAgentsMdConfig =
        serde_yaml::from_str(&yaml).expect("deserialize should succeed");

    assert_eq!(round_tripped.purpose, config.purpose);
    assert_eq!(round_tripped.conventions, config.conventions);
}

#[test]
fn test_empty_ward_setup_fields_default_correctly() {
    let yaml = r#"
name: minimal-skill
description: Minimal skill with empty ward_setup.
ward_setup: {}
"#;

    let fm = parse_public_frontmatter(yaml);
    let ward = fm.ward_setup.expect("ward_setup should be present");

    assert!(ward.directories.is_empty(), "directories should default to empty vec");
    assert!(ward.language_skills.is_empty(), "language_skills should default to empty vec");
    assert!(ward.spec_guidance.is_none(), "spec_guidance should default to None");
    assert!(ward.agents_md.is_none(), "agents_md should default to None");
}

#[test]
fn test_ward_setup_without_agents_md() {
    let yaml = r#"
name: python-dev
description: Python development skill.
ward_setup:
  directories:
    - src
    - tests
  language_skills:
    - python-development
  spec_guidance: "Follow PEP 8"
"#;

    let fm = parse_public_frontmatter(yaml);
    let ward = fm.ward_setup.expect("ward_setup should be present");

    assert_eq!(ward.directories, vec!["src", "tests"]);
    assert_eq!(ward.language_skills, vec!["python-development"]);
    assert_eq!(ward.spec_guidance.as_deref(), Some("Follow PEP 8"));
    assert!(ward.agents_md.is_none(), "agents_md should be None when not specified");
}

#[test]
fn test_ward_agents_md_conventions_default_to_empty() {
    let yaml = r#"
name: ts-skill
description: TypeScript skill.
ward_setup:
  agents_md:
    purpose: "TypeScript-first development."
"#;

    let fm = parse_public_frontmatter(yaml);
    let ward = fm.ward_setup.expect("ward_setup should be present");
    let agents_md = ward.agents_md.expect("agents_md should be present");

    assert_eq!(agents_md.purpose, "TypeScript-first development.");
    assert!(agents_md.conventions.is_empty(), "conventions should default to empty vec");
}

#[test]
fn test_get_ward_setup_method_with_tempdir() {
    use std::fs;
    use gateway_services::SkillService;

    let tmp = tempfile::tempdir().expect("create tempdir");
    let skills_dir = tmp.path().to_path_buf();

    // Create a skill folder with ward_setup
    let skill_dir = skills_dir.join("rust-dev");
    fs::create_dir_all(&skill_dir).expect("create skill dir");

    let skill_md = r#"---
name: rust-dev
description: Rust development skill.
ward_setup:
  directories:
    - src
    - tests
  language_skills:
    - rust-development
---

This skill helps with Rust development.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).expect("write SKILL.md");

    let service = SkillService::new(skills_dir.clone());

    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let ward = rt
        .block_on(service.get_ward_setup("rust-dev"))
        .expect("get_ward_setup should succeed");

    let ward = ward.expect("ward_setup should be Some");
    assert_eq!(ward.directories, vec!["src", "tests"]);
    assert_eq!(ward.language_skills, vec!["rust-development"]);
}

#[test]
fn test_get_ward_setup_returns_none_for_skill_without_it() {
    use std::fs;
    use gateway_services::SkillService;

    let tmp = tempfile::tempdir().expect("create tempdir");
    let skills_dir = tmp.path().to_path_buf();

    let skill_dir = skills_dir.join("plain-skill");
    fs::create_dir_all(&skill_dir).expect("create skill dir");

    let skill_md = r#"---
name: plain-skill
description: A plain skill without ward_setup.
---

Instructions here.
"#;
    fs::write(skill_dir.join("SKILL.md"), skill_md).expect("write SKILL.md");

    let service = SkillService::new(skills_dir.clone());

    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let ward = rt
        .block_on(service.get_ward_setup("plain-skill"))
        .expect("get_ward_setup should succeed");

    assert!(ward.is_none(), "ward_setup should be None for skill without it");
}

#[test]
fn test_get_ward_setup_errors_for_missing_skill() {
    use gateway_services::SkillService;

    let tmp = tempfile::tempdir().expect("create tempdir");
    let service = SkillService::new(tmp.path().to_path_buf());

    let rt = tokio::runtime::Runtime::new().expect("create runtime");
    let result = rt.block_on(service.get_ward_setup("nonexistent-skill"));

    assert!(result.is_err(), "should return Err for missing skill");
    assert!(result.unwrap_err().contains("Skill not found"));
}
