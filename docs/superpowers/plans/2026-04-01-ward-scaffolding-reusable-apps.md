# Ward Scaffolding: Skill-Driven Reusable Apps — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make wards skill-driven reusable apps with structured scaffolding, spec-first workflows, core module indexing via language configs, and spec archival.

**Architecture:** Skills declare `ward_setup` in frontmatter. A post-ward-creation middleware scaffolds directories and AGENTS.md. Intent analysis injects ward rules and spec-first guidance. The existing `auto_update_agents_md` function is refactored to read language configs from `~/Documents/zbot/config/wards/*.yaml` instead of hardcoded Python patterns. Spec lifecycle is agent-driven via prompt injection.

**Tech Stack:** Rust (gateway-execution, gateway-services), YAML (serde_yaml), existing test infrastructure (mock LLM clients, tempdir)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `gateway/gateway-services/src/skills.rs` | Modify | Add `WardSetup` to `SkillFrontmatter`, expose parsed ward_setup |
| `gateway/gateway-services/src/lang_config.rs` | Create | Language config loader — reads `config/wards/*.yaml` |
| `gateway/gateway-services/src/lib.rs` | Modify | Export new `lang_config` module |
| `gateway/gateway-services/src/paths.rs` | Modify | Add `ward_lang_configs_dir()` path helper |
| `gateway/gateway-execution/src/middleware/ward_scaffold.rs` | Create | Post-ward-creation scaffolding middleware |
| `gateway/gateway-execution/src/middleware/mod.rs` | Modify | Export new `ward_scaffold` module |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Modify | Add ward rules + spec guidance to `format_intent_injection()`, update LLM prompt for spec-first graph node |
| `gateway/gateway-execution/src/runner.rs` | Modify | Wire ward_scaffold into post-ward-creation path, refactor `auto_update_agents_md` to use language configs |
| `gateway/gateway-execution/src/runner.rs` (helpers) | Modify | Extract `extract_function_signatures`/`extract_first_docstring` into `lang_config` driven versions |
| `~/Documents/zbot/config/wards/python.yaml` | Create | Default Python language config |
| `gateway/gateway-execution/tests/ward_scaffold_tests.rs` | Create | Tests for scaffolding middleware |
| `gateway/gateway-services/tests/lang_config_tests.rs` | Create | Tests for language config loading |

---

### Task 1: Language Config Loader

**Files:**
- Create: `gateway/gateway-services/src/lang_config.rs`
- Modify: `gateway/gateway-services/src/lib.rs`
- Modify: `gateway/gateway-services/src/paths.rs`
- Test: `gateway/gateway-services/tests/lang_config_tests.rs`

- [ ] **Step 1: Add path helper for ward language configs**

In `gateway/gateway-services/src/paths.rs`, add a method to `VaultPaths`:

```rust
/// Path to `config/wards/` — language config directory for ward indexing
pub fn ward_lang_configs_dir(&self) -> PathBuf {
    self.vault_dir.join("config").join("wards")
}
```

- [ ] **Step 2: Write failing test for language config parsing**

Create `gateway/gateway-services/tests/lang_config_tests.rs`:

```rust
use gateway_services::lang_config::{LangConfig, load_lang_config, load_all_lang_configs};
use std::path::Path;
use tempfile::TempDir;

#[test]
fn test_parse_python_config() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("python.yaml");
    std::fs::write(&config_path, r#"
language: python
file_extensions: [".py"]
signature_patterns:
  function: '^def\s+(\w+)\s*\((.*)\)'
  class: '^class\s+(\w+)'
docstring_pattern: '^\s*"""(.+?)"""'
conventions:
  - "Import from core/: `from core.<module> import <function>`"
  - "Use shared .venv at wards root"
"#).unwrap();

    let config = load_lang_config(&config_path).unwrap();
    assert_eq!(config.language, "python");
    assert_eq!(config.file_extensions, vec![".py"]);
    assert!(config.signature_patterns.contains_key("function"));
    assert!(config.signature_patterns.contains_key("class"));
    assert_eq!(config.conventions.len(), 2);
}

#[test]
fn test_load_all_configs_from_dir() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("python.yaml"), r#"
language: python
file_extensions: [".py"]
signature_patterns:
  function: '^def\s+(\w+)\s*\((.*)\)'
conventions: []
"#).unwrap();
    std::fs::write(dir.path().join("r.yaml"), r#"
language: r
file_extensions: [".R", ".r"]
signature_patterns:
  function: '^(\w+)\s*<-\s*function\s*\((.*)\)'
conventions: []
"#).unwrap();

    let configs = load_all_lang_configs(dir.path()).unwrap();
    assert_eq!(configs.len(), 2);
    assert!(configs.iter().any(|c| c.language == "python"));
    assert!(configs.iter().any(|c| c.language == "r"));
}

#[test]
fn test_empty_dir_returns_empty_vec() {
    let dir = TempDir::new().unwrap();
    let configs = load_all_lang_configs(dir.path()).unwrap();
    assert!(configs.is_empty());
}

#[test]
fn test_nonexistent_dir_returns_empty_vec() {
    let configs = load_all_lang_configs(Path::new("/tmp/does-not-exist-lang-configs")).unwrap();
    assert!(configs.is_empty());
}

#[test]
fn test_find_config_for_extension() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("python.yaml"), r#"
language: python
file_extensions: [".py"]
signature_patterns:
  function: '^def\s+(\w+)\s*\((.*)\)'
conventions: []
"#).unwrap();

    let configs = load_all_lang_configs(dir.path()).unwrap();
    let found = LangConfig::find_for_extension(&configs, ".py");
    assert!(found.is_some());
    assert_eq!(found.unwrap().language, "python");

    let not_found = LangConfig::find_for_extension(&configs, ".rs");
    assert!(not_found.is_none());
}

#[test]
fn test_extract_signatures_with_config() {
    let dir = TempDir::new().unwrap();
    let py_file = dir.path().join("example.py");
    std::fs::write(&py_file, r#"
def fetch_ohlcv(ticker: str, period: str) -> pd.DataFrame:
    """Fetch OHLCV data for a ticker."""
    pass

def fetch_fundamentals(ticker):
    """Get fundamental data."""
    pass

class DataFetcher:
    """Main data fetching class."""
    pass
"#).unwrap();

    let config = LangConfig {
        language: "python".to_string(),
        file_extensions: vec![".py".to_string()],
        signature_patterns: {
            let mut m = std::collections::HashMap::new();
            m.insert("function".to_string(), r"^def\s+(\w+)\s*\((.*)?\)".to_string());
            m.insert("class".to_string(), r"^class\s+(\w+)".to_string());
            m
        },
        docstring_pattern: Some(r#"^\s*"""(.+?)""""#.to_string()),
        conventions: vec![],
    };

    let sigs = config.extract_signatures(&py_file);
    assert!(sigs.iter().any(|s| s.contains("fetch_ohlcv")));
    assert!(sigs.iter().any(|s| s.contains("fetch_fundamentals")));
    assert!(sigs.iter().any(|s| s.contains("DataFetcher")));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-services --test lang_config_tests 2>&1 | head -20`
Expected: compilation failure — `lang_config` module doesn't exist

- [ ] **Step 4: Implement LangConfig module**

Create `gateway/gateway-services/src/lang_config.rs`:

```rust
//! # Language Configuration
//!
//! Reads language-specific patterns from `config/wards/*.yaml` for
//! core module signature extraction in ward AGENTS.md auto-indexing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Language configuration for ward core module indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LangConfig {
    /// Language name (e.g., "python", "r", "node")
    pub language: String,
    /// File extensions to match (e.g., [".py"], [".R", ".r"])
    pub file_extensions: Vec<String>,
    /// Regex patterns for extracting signatures.
    /// Keys: "function", "class", etc.
    /// Values: regex strings with capture groups for name and params.
    pub signature_patterns: HashMap<String, String>,
    /// Regex for extracting docstrings (optional).
    #[serde(default)]
    pub docstring_pattern: Option<String>,
    /// Language-specific coding conventions for AGENTS.md.
    #[serde(default)]
    pub conventions: Vec<String>,
}

impl LangConfig {
    /// Find the config that matches a given file extension.
    pub fn find_for_extension<'a>(configs: &'a [LangConfig], ext: &str) -> Option<&'a LangConfig> {
        configs.iter().find(|c| c.file_extensions.iter().any(|e| e == ext))
    }

    /// Extract function/class signatures from a source file using this config's patterns.
    pub fn extract_signatures(&self, file_path: &Path) -> Vec<String> {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        let mut signatures = Vec::new();

        for (_kind, pattern_str) in &self.signature_patterns {
            let re = match regex::Regex::new(pattern_str) {
                Ok(r) => r,
                Err(_) => continue,
            };

            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(m) = re.find(trimmed) {
                    signatures.push(m.as_str().to_string());
                }
            }
        }

        signatures
    }

    /// Extract the first docstring from a source file using this config's docstring_pattern.
    pub fn extract_first_docstring(&self, file_path: &Path) -> Option<String> {
        let pattern_str = self.docstring_pattern.as_ref()?;
        let content = std::fs::read_to_string(file_path).ok()?;
        let re = regex::Regex::new(pattern_str).ok()?;

        for line in content.lines() {
            if let Some(caps) = re.captures(line.trim()) {
                if let Some(m) = caps.get(1) {
                    return Some(m.as_str().to_string());
                }
            }
        }

        None
    }
}

/// Load a single language config from a YAML file.
pub fn load_lang_config(path: &Path) -> Result<LangConfig, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read language config {:?}: {}", path, e))?;
    serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse language config {:?}: {}", path, e))
}

/// Load all language configs from a directory.
/// Returns empty vec if directory doesn't exist (graceful degradation).
pub fn load_all_lang_configs(dir: &Path) -> Result<Vec<LangConfig>, String> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut configs = Vec::new();
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read lang config dir {:?}: {}", dir, e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
            match load_lang_config(&path) {
                Ok(config) => configs.push(config),
                Err(e) => {
                    tracing::warn!("Skipping invalid language config {:?}: {}", path, e);
                }
            }
        }
    }

    Ok(configs)
}
```

- [ ] **Step 5: Export the module**

In `gateway/gateway-services/src/lib.rs`, add:

```rust
pub mod lang_config;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-services --test lang_config_tests -- --nocapture 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add gateway/gateway-services/src/lang_config.rs gateway/gateway-services/src/lib.rs gateway/gateway-services/src/paths.rs gateway/gateway-services/tests/lang_config_tests.rs
git commit -m "feat(services): add language config loader for ward core module indexing

Reads ~/Documents/zbot/config/wards/*.yaml for language-specific
signature extraction patterns. Supports any language via user-created
YAML configs."
```

---

### Task 2: Extend Skill Frontmatter with `ward_setup`

**Files:**
- Modify: `gateway/gateway-services/src/skills.rs:26-34` (SkillFrontmatter struct)
- Test: `gateway/gateway-services/tests/skill_ward_setup_tests.rs`

- [ ] **Step 1: Write failing test for ward_setup parsing**

Create `gateway/gateway-services/tests/skill_ward_setup_tests.rs`:

```rust
use gateway_services::skills::WardSetup;

#[test]
fn test_parse_skill_with_ward_setup() {
    let yaml = r#"
name: financial-analysis
description: Stock, options, and market analysis
ward_setup:
  directories:
    - core/
    - output/
    - specs/
    - specs/archive/
    - memory-bank/
  language_skills:
    - python
  spec_guidance: |
    Financial analysis specs must cover:
    - Data sources with rate limits
    - Calculation methodology
  agents_md:
    purpose: "Reusable financial analysis workspace"
    conventions:
      - "All reusable code in core/"
      - "Output files in output/"
"#;

    let fm: gateway_services::skills::SkillFrontmatterPublic = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(fm.name, "financial-analysis");
    let ws = fm.ward_setup.unwrap();
    assert_eq!(ws.directories, vec!["core/", "output/", "specs/", "specs/archive/", "memory-bank/"]);
    assert_eq!(ws.language_skills, vec!["python"]);
    assert!(ws.spec_guidance.unwrap().contains("Data sources"));
    assert_eq!(ws.agents_md.unwrap().purpose, "Reusable financial analysis workspace");
    assert_eq!(ws.agents_md.unwrap().conventions.len(), 2);
}

#[test]
fn test_parse_skill_without_ward_setup() {
    let yaml = r#"
name: coding
description: General coding skill
"#;

    let fm: gateway_services::skills::SkillFrontmatterPublic = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(fm.name, "coding");
    assert!(fm.ward_setup.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-services --test skill_ward_setup_tests 2>&1 | head -20`
Expected: compilation failure — `WardSetup` and `SkillFrontmatterPublic` don't exist

- [ ] **Step 3: Add WardSetup types and extend SkillFrontmatter**

In `gateway/gateway-services/src/skills.rs`, add these types after the existing `SkillFrontmatter` struct (line 34):

```rust
/// Ward setup configuration from skill frontmatter.
/// Declares how a skill scaffolds a ward on first creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WardSetup {
    /// Directories to create on first ward creation
    #[serde(default)]
    pub directories: Vec<String>,
    /// Referenced language skills (informational)
    #[serde(default)]
    pub language_skills: Vec<String>,
    /// Domain-specific guidance for spec writing
    #[serde(default)]
    pub spec_guidance: Option<String>,
    /// Seed content for AGENTS.md
    #[serde(default)]
    pub agents_md: Option<WardAgentsMdConfig>,
}

/// Seed content for AGENTS.md in a new ward.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WardAgentsMdConfig {
    /// Purpose description for the ward
    pub purpose: String,
    /// Coding conventions
    #[serde(default)]
    pub conventions: Vec<String>,
}

/// Public version of SkillFrontmatter with ward_setup exposed.
/// Used by external consumers (ward scaffolding, tests).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatterPublic {
    pub name: String,
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    pub description: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub ward_setup: Option<WardSetup>,
}
```

Update the existing private `SkillFrontmatter` struct (line 26-34) to also include `ward_setup`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkillFrontmatter {
    name: String,
    #[serde(rename = "displayName", default)]
    display_name: Option<String>,
    description: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    ward_setup: Option<WardSetup>,
}
```

Add a method to `SkillService` to get the ward_setup for a skill:

```rust
/// Get ward_setup config for a skill by ID, if it has one.
pub async fn get_ward_setup(&self, id: &str) -> Result<Option<WardSetup>, String> {
    let skill_dir = self.skills_dir.join(id);
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_md_path.exists() {
        return Err(format!("Skill not found: {}", id));
    }

    let content = std::fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("Failed to read SKILL.md: {}", e))?;

    let (frontmatter, _) = self.parse_frontmatter(&content)?;
    Ok(frontmatter.ward_setup)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-services --test skill_ward_setup_tests -- --nocapture 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 5: Verify existing skill tests still pass**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-services 2>&1 | tail -10`
Expected: All existing tests pass (ward_setup is `Option`, so backwards compatible)

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-services/src/skills.rs gateway/gateway-services/tests/skill_ward_setup_tests.rs
git commit -m "feat(services): add ward_setup to skill frontmatter

Skills can now declare directories, language_skills, spec_guidance,
and agents_md seed content in their SKILL.md frontmatter."
```

---

### Task 3: Ward Scaffolding Middleware

**Files:**
- Create: `gateway/gateway-execution/src/middleware/ward_scaffold.rs`
- Modify: `gateway/gateway-execution/src/middleware/mod.rs`
- Test: `gateway/gateway-execution/tests/ward_scaffold_tests.rs`

- [ ] **Step 1: Write failing test for ward scaffolding**

Create `gateway/gateway-execution/tests/ward_scaffold_tests.rs`:

```rust
use gateway_execution::middleware::ward_scaffold::scaffold_ward;
use gateway_services::skills::{WardSetup, WardAgentsMdConfig};
use tempfile::TempDir;

#[test]
fn test_scaffold_creates_directories() {
    let dir = TempDir::new().unwrap();
    let ward_dir = dir.path().join("test-ward");
    std::fs::create_dir_all(&ward_dir).unwrap();

    let setup = WardSetup {
        directories: vec![
            "core/".to_string(),
            "output/".to_string(),
            "specs/".to_string(),
            "specs/archive/".to_string(),
            "memory-bank/".to_string(),
        ],
        language_skills: vec!["python".to_string()],
        spec_guidance: None,
        agents_md: None,
    };

    scaffold_ward(&ward_dir, "test-ward", &[setup]);

    assert!(ward_dir.join("core").is_dir());
    assert!(ward_dir.join("output").is_dir());
    assert!(ward_dir.join("specs").is_dir());
    assert!(ward_dir.join("specs/archive").is_dir());
    assert!(ward_dir.join("memory-bank").is_dir());
}

#[test]
fn test_scaffold_generates_agents_md() {
    let dir = TempDir::new().unwrap();
    let ward_dir = dir.path().join("financial-analysis");
    std::fs::create_dir_all(&ward_dir).unwrap();

    let setup = WardSetup {
        directories: vec!["core/".to_string()],
        language_skills: vec!["python".to_string()],
        spec_guidance: Some("Cover data sources and rate limits".to_string()),
        agents_md: Some(WardAgentsMdConfig {
            purpose: "Reusable financial analysis workspace".to_string(),
            conventions: vec![
                "All reusable code in core/".to_string(),
                "Output files in output/".to_string(),
            ],
        }),
    };

    scaffold_ward(&ward_dir, "financial-analysis", &[setup]);

    let agents_md = std::fs::read_to_string(ward_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("# financial-analysis"));
    assert!(agents_md.contains("Reusable financial analysis workspace"));
    assert!(agents_md.contains("All reusable code in core/"));
    assert!(agents_md.contains("Output files in output/"));
}

#[test]
fn test_scaffold_does_not_overwrite_existing_agents_md() {
    let dir = TempDir::new().unwrap();
    let ward_dir = dir.path().join("existing-ward");
    std::fs::create_dir_all(&ward_dir).unwrap();
    std::fs::write(ward_dir.join("AGENTS.md"), "# Custom content\nDo not overwrite").unwrap();

    let setup = WardSetup {
        directories: vec!["core/".to_string()],
        language_skills: vec![],
        spec_guidance: None,
        agents_md: Some(WardAgentsMdConfig {
            purpose: "New purpose".to_string(),
            conventions: vec![],
        }),
    };

    scaffold_ward(&ward_dir, "existing-ward", &[setup]);

    let content = std::fs::read_to_string(ward_dir.join("AGENTS.md")).unwrap();
    assert!(content.contains("# Custom content"));
    assert!(!content.contains("New purpose"));
}

#[test]
fn test_scaffold_merges_multiple_skill_setups() {
    let dir = TempDir::new().unwrap();
    let ward_dir = dir.path().join("multi-skill");
    std::fs::create_dir_all(&ward_dir).unwrap();

    let setup1 = WardSetup {
        directories: vec!["core/".to_string(), "output/".to_string()],
        language_skills: vec!["python".to_string()],
        spec_guidance: None,
        agents_md: Some(WardAgentsMdConfig {
            purpose: "Multi-purpose ward".to_string(),
            conventions: vec!["Use core/ for shared code".to_string()],
        }),
    };

    let setup2 = WardSetup {
        directories: vec!["specs/".to_string(), "specs/archive/".to_string()],
        language_skills: vec![],
        spec_guidance: Some("Write detailed specs".to_string()),
        agents_md: None,
    };

    scaffold_ward(&ward_dir, "multi-skill", &[setup1, setup2]);

    assert!(ward_dir.join("core").is_dir());
    assert!(ward_dir.join("output").is_dir());
    assert!(ward_dir.join("specs").is_dir());
    assert!(ward_dir.join("specs/archive").is_dir());
}

#[test]
fn test_scaffold_empty_setups_creates_nothing() {
    let dir = TempDir::new().unwrap();
    let ward_dir = dir.path().join("empty-ward");
    std::fs::create_dir_all(&ward_dir).unwrap();

    scaffold_ward(&ward_dir, "empty-ward", &[]);

    // Only the ward dir itself should exist, no AGENTS.md
    let entries: Vec<_> = std::fs::read_dir(&ward_dir).unwrap().collect();
    assert!(entries.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-execution --test ward_scaffold_tests 2>&1 | head -20`
Expected: compilation failure — `ward_scaffold` module doesn't exist

- [ ] **Step 3: Implement ward scaffolding middleware**

Create `gateway/gateway-execution/src/middleware/ward_scaffold.rs`:

```rust
//! # Ward Scaffolding
//!
//! Scaffolds ward directory structure based on skill `ward_setup` frontmatter.
//! Runs after ward creation (when ward tool returns `action: "created"`).
//! Does NOT overwrite existing AGENTS.md or directories.

use std::path::Path;

use gateway_services::skills::{WardSetup, WardAgentsMdConfig};

/// Scaffold a ward directory based on one or more skill `ward_setup` configs.
///
/// - Creates directories from all setups (merged, deduplicated)
/// - Generates AGENTS.md from the first setup that provides `agents_md` (does NOT overwrite existing)
/// - Skips gracefully if setups is empty
pub fn scaffold_ward(ward_dir: &Path, ward_name: &str, setups: &[WardSetup]) {
    if setups.is_empty() {
        return;
    }

    // Merge directories from all setups (deduplicated)
    let mut all_dirs: Vec<String> = Vec::new();
    for setup in setups {
        for dir in &setup.directories {
            if !all_dirs.contains(dir) {
                all_dirs.push(dir.clone());
            }
        }
    }

    // Create directories
    for dir in &all_dirs {
        let dir_path = ward_dir.join(dir.trim_end_matches('/'));
        if !dir_path.exists() {
            if let Err(e) = std::fs::create_dir_all(&dir_path) {
                tracing::warn!(
                    ward = %ward_name,
                    dir = %dir,
                    error = %e,
                    "Failed to create ward directory"
                );
            }
        }
    }

    // Generate AGENTS.md from first setup that provides agents_md config
    let agents_md_path = ward_dir.join("AGENTS.md");
    if !agents_md_path.exists() {
        let agents_md_config = setups.iter().find_map(|s| s.agents_md.as_ref());
        if let Some(config) = agents_md_config {
            let content = generate_agents_md(ward_name, config, &all_dirs);
            if let Err(e) = std::fs::write(&agents_md_path, &content) {
                tracing::warn!(
                    ward = %ward_name,
                    error = %e,
                    "Failed to write AGENTS.md"
                );
            }
        }
    }
}

/// Generate AGENTS.md content from ward setup config.
fn generate_agents_md(ward_name: &str, config: &WardAgentsMdConfig, directories: &[String]) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let mut sections = Vec::new();

    sections.push(format!("# {}\n", ward_name));
    sections.push(format!("\n## Purpose\n{}\n", config.purpose));

    // Directory layout
    if !directories.is_empty() {
        sections.push("\n## Directory Layout\n".to_string());
        for dir in directories {
            sections.push(format!("- `{}`\n", dir));
        }
    }

    // Conventions
    if !config.conventions.is_empty() {
        sections.push("\n## Conventions\n".to_string());
        for conv in &config.conventions {
            sections.push(format!("- {}\n", conv));
        }
    }

    // Core modules placeholder
    sections.push("\n## Core Modules\n*(auto-indexed after each session)*\n".to_string());

    // History
    sections.push(format!("\n## History\n- {}: Ward created\n", today));

    sections.join("")
}
```

- [ ] **Step 4: Export the module**

In `gateway/gateway-execution/src/middleware/mod.rs`, add:

```rust
pub mod ward_scaffold;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-execution --test ward_scaffold_tests -- --nocapture 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/middleware/ward_scaffold.rs gateway/gateway-execution/src/middleware/mod.rs gateway/gateway-execution/tests/ward_scaffold_tests.rs
git commit -m "feat(execution): add ward scaffolding middleware

Scaffolds ward directories and AGENTS.md based on skill ward_setup
frontmatter. Merges multiple skill setups, never overwrites existing
AGENTS.md."
```

---

### Task 4: Wire Scaffolding into Runner

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs:1292-1296` (after intent analysis injection)
- Modify: `gateway/gateway-execution/src/runner.rs:882-890` (after execution completes)

- [ ] **Step 1: Store intent analysis result for ward scaffolding**

In `runner.rs`, after the intent analysis succeeds (line 1294-1296), store the analysis in a variable accessible to the ward scaffolding path. Add after line 1296:

```rust
// Store recommended skills for ward scaffolding
if analysis.ward_recommendation.action == "create_new" {
    let skills_for_scaffold: Vec<String> = analysis.recommended_skills.clone();
    // Store in extra_initial_state so ward tool can trigger scaffolding
    extra_initial_state.insert(
        "ward_scaffold_skills".to_string(),
        serde_json::to_value(&skills_for_scaffold).unwrap_or_default(),
    );
    // Store spec_guidance from recommended skills
    let mut spec_guidances = Vec::new();
    for skill_name in &skills_for_scaffold {
        if let Ok(Some(ws)) = self.skill_service.get_ward_setup(skill_name).await {
            if let Some(ref guidance) = ws.spec_guidance {
                spec_guidances.push(guidance.clone());
            }
        }
    }
    if !spec_guidances.is_empty() {
        extra_initial_state.insert(
            "ward_spec_guidance".to_string(),
            serde_json::json!(spec_guidances.join("\n\n")),
        );
    }
}
```

- [ ] **Step 2: Call scaffolding when ward is created**

In `runner.rs`, find where `auto_update_agents_md` is called (line 882-890). Before `auto_update_agents_md`, add ward scaffolding for new wards. The scaffolding should run when the ward is first created in a session.

Add a new function that the runner calls after execution, before `auto_update_agents_md`:

```rust
/// Scaffold ward structure from skill ward_setup configs.
/// Only runs for newly created wards (not re-entry).
async fn scaffold_ward_from_skills(
    paths: &gateway_services::SharedVaultPaths,
    skill_service: &gateway_services::SkillService,
    ward_id: &str,
    recommended_skills: &[String],
) {
    let ward_dir = paths.vault_dir().join("wards").join(ward_id);
    if !ward_dir.exists() {
        return;
    }

    let mut setups = Vec::new();
    for skill_name in recommended_skills {
        match skill_service.get_ward_setup(skill_name).await {
            Ok(Some(ws)) => setups.push(ws),
            Ok(None) => {} // Skill has no ward_setup — skip
            Err(e) => {
                tracing::warn!(skill = %skill_name, error = %e, "Failed to read skill ward_setup");
            }
        }
    }

    if !setups.is_empty() {
        gateway_execution::middleware::ward_scaffold::scaffold_ward(&ward_dir, ward_id, &setups);
        tracing::info!(ward = %ward_id, skill_count = setups.len(), "Ward scaffolded from skill configs");
    }
}
```

- [ ] **Step 3: Call scaffold_ward_from_skills in the post-execution path**

In the post-execution section (around line 882-890), add the scaffolding call before `auto_update_agents_md`. The scaffolding is idempotent (skips existing dirs/files), so it's safe to call on every execution — but it only creates dirs that don't exist:

```rust
// Scaffold ward structure from skill configs (idempotent — skips existing)
if let Some(ref ward_id) = session_ward {
    // Read recommended skills from execution logs
    let recommended_skills = log_service
        .get_intent_log(execution_id)
        .ok()
        .flatten()
        .and_then(|log| {
            log.metadata.as_ref()
                .and_then(|m| m.get("recommended_skills"))
                .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
        })
        .unwrap_or_default();

    if !recommended_skills.is_empty() {
        scaffold_ward_from_skills(&paths, &skill_service, ward_id, &recommended_skills).await;
    }
}

// Auto-update ward AGENTS.md after root execution completes (existing code)
if let Some(ref ward_id) = session_ward {
    auto_update_agents_md(paths.vault_dir(), ward_id);
}
```

- [ ] **Step 4: Verify compilation**

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | tail -20`
Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs
git commit -m "feat(runner): wire ward scaffolding into execution pipeline

After intent analysis recommends skills, stores them for scaffolding.
Post-execution calls scaffold_ward_from_skills before auto_update_agents_md."
```

---

### Task 5: Refactor `auto_update_agents_md` to Use Language Configs

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs:2006-2280` (auto_update_agents_md and helper functions)

- [ ] **Step 1: Write failing test for language-config-driven signature extraction**

Add to `gateway/gateway-execution/tests/ward_scaffold_tests.rs`:

```rust
use gateway_services::lang_config::LangConfig;
use std::collections::HashMap;

#[test]
fn test_auto_update_agents_md_uses_lang_config() {
    let dir = TempDir::new().unwrap();
    let ward_dir = dir.path().join("wards").join("test-ward");
    let core_dir = ward_dir.join("core");
    std::fs::create_dir_all(&core_dir).unwrap();

    // Create a Python file in core/
    std::fs::write(core_dir.join("data_fetcher.py"), r#"
"""Fetch market data from various sources."""

def fetch_ohlcv(ticker: str, period: str) -> pd.DataFrame:
    """Fetch OHLCV data."""
    pass

def fetch_fundamentals(ticker: str) -> dict:
    """Get fundamental metrics."""
    pass
"#).unwrap();

    // Create a lang config
    let lang_configs_dir = dir.path().join("config").join("wards");
    std::fs::create_dir_all(&lang_configs_dir).unwrap();
    std::fs::write(lang_configs_dir.join("python.yaml"), r#"
language: python
file_extensions: [".py"]
signature_patterns:
  function: '^def\s+(\w+)\s*\((.*)?\)'
  class: '^class\s+(\w+)'
docstring_pattern: '^\s*"""(.+?)"""'
conventions:
  - "Import from core/: `from core.<module> import <function>`"
"#).unwrap();

    // Create a minimal AGENTS.md so auto_update has something to preserve
    std::fs::write(ward_dir.join("AGENTS.md"), "# test-ward\n\n## Purpose\nTest ward\n").unwrap();

    // Call auto_update_agents_md with lang configs path
    gateway_execution::runner::auto_update_agents_md_with_lang_configs(
        dir.path(),
        "test-ward",
        &lang_configs_dir,
    );

    let content = std::fs::read_to_string(ward_dir.join("AGENTS.md")).unwrap();
    assert!(content.contains("fetch_ohlcv"));
    assert!(content.contains("fetch_fundamentals"));
    assert!(content.contains("core/data_fetcher.py"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-execution --test ward_scaffold_tests test_auto_update_agents_md_uses_lang_config 2>&1 | head -20`
Expected: compilation failure — `auto_update_agents_md_with_lang_configs` doesn't exist

- [ ] **Step 3: Refactor auto_update_agents_md**

In `runner.rs`, create a new public function `auto_update_agents_md_with_lang_configs` that replaces the hardcoded Python patterns with language config lookups. The existing `auto_update_agents_md` becomes a wrapper that uses the default lang configs path.

Replace the `extract_function_signatures` call in the Core Modules section (lines 2032-2067) with language-config-driven extraction:

```rust
/// Auto-update AGENTS.md using language configs for signature extraction.
pub fn auto_update_agents_md_with_lang_configs(
    vault_dir: &std::path::Path,
    ward_id: &str,
    lang_configs_dir: &std::path::Path,
) {
    let ward_dir = vault_dir.join("wards").join(ward_id);
    let agents_md_path = ward_dir.join("AGENTS.md");

    if !ward_dir.exists() || ward_id == "scratch" {
        return;
    }

    // Load language configs
    let lang_configs = gateway_services::lang_config::load_all_lang_configs(lang_configs_dir)
        .unwrap_or_default();

    // ... (same structure as existing auto_update_agents_md, but replace
    //  extract_function_signatures/extract_first_docstring with lang_config methods)
    // In the Core Modules section, iterate over all files in core/,
    // find matching LangConfig by extension, use config.extract_signatures()
    // and config.extract_first_docstring()
    // Fall back to existing Python-hardcoded patterns if no lang config matches
}
```

The key change in the Core Modules section:

```rust
// ── Core Modules with function signatures ──
let core_dir = ward_dir.join("core");
if core_dir.exists() {
    if let Ok(entries) = std::fs::read_dir(&core_dir) {
        let mut modules: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file() && !e.file_name().to_string_lossy().starts_with('.'))
            .collect();
        modules.sort_by_key(|e| e.file_name());

        if !modules.is_empty() {
            sections.push("\n## Core Modules\n".to_string());
            for entry in &modules {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| format!(".{}", e))
                    .unwrap_or_default();

                let config = LangConfig::find_for_extension(&lang_configs, &ext);

                sections.push(format!("### core/{}\n", name));

                // Docstring
                let desc = if let Some(cfg) = config {
                    cfg.extract_first_docstring(&path).unwrap_or_default()
                } else {
                    extract_first_docstring(&path) // fallback to hardcoded Python
                };
                if !desc.is_empty() {
                    sections.push(format!("{}\n", desc));
                }

                // Signatures
                let sigs = if let Some(cfg) = config {
                    cfg.extract_signatures(&path)
                } else {
                    extract_function_signatures(&path) // fallback
                };
                for sig in &sigs {
                    sections.push(format!("- `{}`\n", sig));
                }
                sections.push("\n".to_string());
            }
        }
    }
}
```

Update the existing `auto_update_agents_md` to delegate:

```rust
fn auto_update_agents_md(vault_dir: &std::path::Path, ward_id: &str) {
    let lang_configs_dir = vault_dir.join("config").join("wards");
    auto_update_agents_md_with_lang_configs(vault_dir, ward_id, &lang_configs_dir);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-execution --test ward_scaffold_tests -- --nocapture 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 5: Verify existing tests still pass**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-execution 2>&1 | tail -10`
Expected: All existing tests pass

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs gateway/gateway-execution/tests/ward_scaffold_tests.rs
git commit -m "refactor(runner): use language configs for core module indexing

auto_update_agents_md now reads config/wards/*.yaml for signature
extraction patterns. Falls back to hardcoded Python patterns when
no config matches."
```

---

### Task 6: Update Intent Analysis — Ward Rules & Spec-First Graph

**Files:**
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs:90-144` (INTENT_ANALYSIS_PROMPT)
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs:153-196` (format_intent_injection)
- Test: `gateway/gateway-execution/tests/intent_analysis_tests.rs`

- [ ] **Step 1: Write failing test for ward rule injection**

Add to `gateway/gateway-execution/tests/intent_analysis_tests.rs`:

```rust
#[test]
fn test_format_intent_injection_includes_ward_rules() {
    let analysis = IntentAnalysis {
        primary_intent: "financial analysis".to_string(),
        hidden_intents: vec![],
        recommended_skills: vec!["financial-analysis".to_string()],
        recommended_agents: vec![],
        ward_recommendation: WardRecommendation {
            action: "create_new".to_string(),
            ward_name: "financial-analysis".to_string(),
            subdirectory: Some("stocks/spy".to_string()),
            structure: Default::default(),
            reason: "Domain match".to_string(),
        },
        execution_strategy: ExecutionStrategy {
            approach: "graph".to_string(),
            graph: None,
            explanation: "Complex task".to_string(),
        },
        rewritten_prompt: String::new(),
    };

    let injection = format_intent_injection(&analysis, None);
    assert!(injection.contains("Ward Rule:"));
    assert!(injection.contains("ALL code must be written inside a ward"));
    assert!(injection.contains("Spec Lifecycle:"));
    assert!(injection.contains("specs/archive/"));
}

#[test]
fn test_format_intent_injection_includes_spec_guidance() {
    let analysis = IntentAnalysis {
        primary_intent: "financial analysis".to_string(),
        hidden_intents: vec![],
        recommended_skills: vec![],
        recommended_agents: vec![],
        ward_recommendation: WardRecommendation {
            action: "create_new".to_string(),
            ward_name: "financial-analysis".to_string(),
            subdirectory: None,
            structure: Default::default(),
            reason: "test".to_string(),
        },
        execution_strategy: ExecutionStrategy {
            approach: "simple".to_string(),
            graph: None,
            explanation: "Simple task".to_string(),
        },
        rewritten_prompt: String::new(),
    };

    let spec_guidance = Some("Cover data sources with rate limits\nInclude calculation methodology".to_string());
    let injection = format_intent_injection(&analysis, spec_guidance.as_deref());
    assert!(injection.contains("Domain Spec Guidance:"));
    assert!(injection.contains("Cover data sources"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-execution --test intent_analysis_tests test_format_intent_injection_includes_ward_rules 2>&1 | head -20`
Expected: Fails — `format_intent_injection` doesn't accept `spec_guidance` parameter

- [ ] **Step 3: Update format_intent_injection**

In `intent_analysis.rs`, modify `format_intent_injection` (line 153) to accept optional spec guidance and append ward rules:

```rust
/// Format an `IntentAnalysis` as a markdown section suitable for appending
/// to the agent's system prompt / instructions.
///
/// `spec_guidance` is optional domain-specific guidance from the skill's ward_setup.
pub fn format_intent_injection(analysis: &IntentAnalysis, spec_guidance: Option<&str>) -> String {
    let mut out = String::from("\n\n## Intent Analysis\n\n");

    // ... (existing code for primary_intent, hidden_intents, ward, skills, agents, approach)

    // Ward rules — always included
    out.push_str(r#"
**Ward Rule:** ALL code must be written inside a ward. If you need to write code:
1. Enter the recommended ward (or create if new)
2. Read AGENTS.md to understand what exists in core/
3. Check if existing core/ modules already solve your need — reuse, don't recreate
4. If new functionality: write a spec in specs/<domain>/<name>.md first, then implement
5. After implementing: archive spec to specs/archive/, update AGENTS.md with new core modules

**Spec Lifecycle:**
- Active specs live in specs/
- After implementing a spec, archive it to specs/archive/
- Archived specs are searchable via knowledge graph for future context

**Spec Quality:**
Write specs detailed enough that a different agent can implement them without asking questions:
- Purpose: what this does and why
- Inputs/Outputs: exact data structures, types, formats
- Dependencies: which core/ modules to import, external packages needed
- Implementation detail: algorithm, data flow, error cases
- Integration: how this connects to other specs in this run
"#);

    // Domain-specific spec guidance from skill ward_setup
    if let Some(guidance) = spec_guidance {
        out.push_str(&format!("\n**Domain Spec Guidance:**\n{}\n", guidance));
    }

    out
}
```

- [ ] **Step 4: Update all callers of format_intent_injection**

In `runner.rs` line 1294-1296, update the call to pass spec guidance:

```rust
// Collect spec guidance from recommended skills' ward_setup
let spec_guidance = {
    let mut guidances = Vec::new();
    for skill_name in &analysis.recommended_skills {
        if let Ok(Some(ws)) = self.skill_service.get_ward_setup(skill_name).await {
            if let Some(ref g) = ws.spec_guidance {
                guidances.push(g.clone());
            }
        }
    }
    if guidances.is_empty() { None } else { Some(guidances.join("\n\n")) }
};

agent_for_build.instructions.push_str(
    &format_intent_injection(&analysis, spec_guidance.as_deref()),
);
```

- [ ] **Step 5: Update INTENT_ANALYSIS_PROMPT for spec-first graph node**

In `intent_analysis.rs`, update the `INTENT_ANALYSIS_PROMPT` (line 90) to add spec-first guidance:

Add to the `## Rules` section:

```
- When approach is "graph" and the task involves writing code:
  - The FIRST node must be a spec-writing node (id: "specs", agent: "root", skills: ["coding"])
  - This node reads AGENTS.md, existing core modules, and writes detailed specs in specs/<domain>/*.md
  - Subsequent nodes implement against those specs
  - Do NOT combine spec writing with implementation in the same node
```

- [ ] **Step 6: Run all intent analysis tests**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-execution --test intent_analysis_tests -- --nocapture 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 7: Verify full workspace compilation**

Run: `cd /home/videogamer/projects/agentzero && cargo check --workspace 2>&1 | tail -10`
Expected: No errors

- [ ] **Step 8: Commit**

```bash
git add gateway/gateway-execution/src/middleware/intent_analysis.rs gateway/gateway-execution/src/runner.rs gateway/gateway-execution/tests/intent_analysis_tests.rs
git commit -m "feat(intent): add ward rules, spec-first graph node, and spec guidance injection

format_intent_injection now includes universal ward rules (code must
live in wards, spec-first workflow, spec archival). Skills with
spec_guidance get domain-specific hints injected. Graph tasks must
start with a spec-writing node."
```

---

### Task 7: Create Default Python Language Config

**Files:**
- Create: `~/Documents/zbot/config/wards/python.yaml`

- [ ] **Step 1: Create the config directory and file**

```bash
mkdir -p ~/Documents/zbot/config/wards
```

Write `~/Documents/zbot/config/wards/python.yaml`:

```yaml
language: python
file_extensions: [".py"]
signature_patterns:
  function: '^def\s+(\w+)\s*\((.*)?\)'
  class: '^class\s+(\w+)'
docstring_pattern: '^\s*"""(.+?)"""'
conventions:
  - "Import from core/: `from core.<module> import <function>`"
  - "Use shared .venv at wards root"
  - "Max 100 lines per file, one concern per module"
  - "Use apply_patch for all file operations"
```

- [ ] **Step 2: Verify the config loads correctly**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-services --test lang_config_tests -- --nocapture 2>&1 | tail -10`
Expected: All tests still pass (they use temp dirs, not the real config)

- [ ] **Step 3: Commit**

```bash
git add ~/Documents/zbot/config/wards/python.yaml
git commit -m "feat: add default Python language config for ward indexing

Ships python.yaml with signature patterns for def/class extraction.
Users can add more languages by dropping YAML files in config/wards/."
```

---

### Task 8: Slim Down Ward Tool AGENTS.md Generation

**Files:**
- Modify: `runtime/agent-tools/src/tools/ward.rs:66-135`

- [ ] **Step 1: Write test to verify ward tool creates minimal AGENTS.md**

The ward tool should create a minimal AGENTS.md when no skill ward_setup is available. The middleware handles the enriched version. Update the existing test in `ward.rs` (line 520-533):

```rust
#[test]
fn test_create_agents_md_minimal() {
    let ward_dir = TempDir::new().unwrap();
    let ward_path = ward_dir.path().to_path_buf();

    WardTool::write_agents_md(&ward_path, "test-project", None, None);

    let content = std::fs::read_to_string(ward_path.join("AGENTS.md")).unwrap();
    assert!(content.contains("# test-project"));
    assert!(content.contains("## Purpose"));
    // Should NOT contain hardcoded Python conventions
    assert!(!content.contains("yfinance"));
    assert!(!content.contains("pandas"));
}
```

- [ ] **Step 2: Simplify write_agents_md**

In `ward.rs`, simplify `write_agents_md` (lines 84-135) to produce a minimal AGENTS.md without hardcoded Python conventions:

```rust
fn write_agents_md(
    ward_dir: &std::path::Path,
    ward_name: &str,
    purpose: Option<&str>,
    structure: Option<&str>,
) {
    let agents_md_path = ward_dir.join(WARD_AGENTS_MD);
    if agents_md_path.exists() {
        return;
    }

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let default_purpose = format!("Domain workspace for {} projects.", ward_name);
    let purpose = purpose.unwrap_or(&default_purpose);

    let mut content = format!(
        "# {name}\n\n## Purpose\n{purpose}\n",
        name = ward_name,
        purpose = purpose,
    );

    if let Some(structure) = structure {
        content.push_str(&format!("\n## Directory Layout\n{}\n", structure));
    }

    content.push_str(&format!(
        "\n## Core Modules\n*(auto-indexed after each session)*\n\n## History\n- {}: Ward created\n",
        today
    ));

    if let Err(e) = std::fs::write(&agents_md_path, content) {
        tracing::warn!("Failed to create AGENTS.md in ward '{}': {}", ward_name, e);
    }
}
```

- [ ] **Step 3: Run ward tool tests**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p agent-tools -- ward --nocapture 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-tools/src/tools/ward.rs
git commit -m "refactor(ward): slim down AGENTS.md generation

Ward tool now creates minimal AGENTS.md without hardcoded Python
conventions. Skill-driven scaffolding middleware handles enrichment."
```

---

### Task 9: Full Integration Test

**Files:**
- Test: `gateway/gateway-execution/tests/ward_scaffold_tests.rs` (extend)

- [ ] **Step 1: Write end-to-end scaffolding test**

Add to `gateway/gateway-execution/tests/ward_scaffold_tests.rs`:

```rust
/// End-to-end: skill ward_setup → scaffold → auto_update_agents_md → verify
#[test]
fn test_full_ward_lifecycle() {
    let dir = TempDir::new().unwrap();
    let vault_dir = dir.path();
    let ward_dir = vault_dir.join("wards").join("financial-analysis");
    std::fs::create_dir_all(&ward_dir).unwrap();

    // 1. Create language config
    let lang_configs_dir = vault_dir.join("config").join("wards");
    std::fs::create_dir_all(&lang_configs_dir).unwrap();
    std::fs::write(lang_configs_dir.join("python.yaml"), r#"
language: python
file_extensions: [".py"]
signature_patterns:
  function: '^def\s+(\w+)\s*\((.*)?\)'
  class: '^class\s+(\w+)'
docstring_pattern: '^\s*"""(.+?)"""'
conventions:
  - "Import from core/"
"#).unwrap();

    // 2. Scaffold from skill ward_setup
    let setup = WardSetup {
        directories: vec![
            "core/".to_string(),
            "output/".to_string(),
            "specs/".to_string(),
            "specs/archive/".to_string(),
            "memory-bank/".to_string(),
        ],
        language_skills: vec!["python".to_string()],
        spec_guidance: Some("Cover data sources and rate limits".to_string()),
        agents_md: Some(WardAgentsMdConfig {
            purpose: "Reusable financial analysis workspace".to_string(),
            conventions: vec![
                "All reusable code in core/".to_string(),
                "Output files in output/".to_string(),
            ],
        }),
    };

    scaffold_ward(&ward_dir, "financial-analysis", &[setup]);

    // Verify directories created
    assert!(ward_dir.join("core").is_dir());
    assert!(ward_dir.join("output").is_dir());
    assert!(ward_dir.join("specs").is_dir());
    assert!(ward_dir.join("specs/archive").is_dir());
    assert!(ward_dir.join("memory-bank").is_dir());

    // Verify AGENTS.md created with purpose and conventions
    let agents_md = std::fs::read_to_string(ward_dir.join("AGENTS.md")).unwrap();
    assert!(agents_md.contains("Reusable financial analysis workspace"));
    assert!(agents_md.contains("All reusable code in core/"));

    // 3. Simulate agent writing a core module
    std::fs::write(ward_dir.join("core").join("data_fetcher.py"), r#"
"""Fetch market data from various sources."""

def fetch_ohlcv(ticker: str, period: str) -> pd.DataFrame:
    """Fetch OHLCV data for a given ticker."""
    pass

def fetch_fundamentals(ticker: str) -> dict:
    """Get fundamental financial metrics."""
    pass
"#).unwrap();

    // 4. Run auto_update_agents_md (simulates post-execution hook)
    gateway_execution::runner::auto_update_agents_md_with_lang_configs(
        vault_dir,
        "financial-analysis",
        &lang_configs_dir,
    );

    // 5. Verify AGENTS.md now includes core module index
    let updated_agents_md = std::fs::read_to_string(ward_dir.join("AGENTS.md")).unwrap();
    assert!(updated_agents_md.contains("core/data_fetcher.py"));
    assert!(updated_agents_md.contains("fetch_ohlcv"));
    assert!(updated_agents_md.contains("fetch_fundamentals"));
}
```

- [ ] **Step 2: Run the full test**

Run: `cd /home/videogamer/projects/agentzero && cargo test -p gateway-execution --test ward_scaffold_tests test_full_ward_lifecycle -- --nocapture 2>&1 | tail -20`
Expected: PASS

- [ ] **Step 3: Run full workspace tests**

Run: `cd /home/videogamer/projects/agentzero && cargo test --workspace 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/tests/ward_scaffold_tests.rs
git commit -m "test: add full ward lifecycle integration test

Verifies skill ward_setup → scaffold → core module → auto_update
AGENTS.md pipeline end-to-end with language config driven indexing."
```

---

### Task 10: Update Component Documentation

**Files:**
- Create: `memory-bank/components/ward-scaffolding/overview.md`
- Modify: `memory-bank/components/index.md`

- [ ] **Step 1: Create ward scaffolding component docs**

Create `memory-bank/components/ward-scaffolding/overview.md`:

```markdown
# Ward Scaffolding — Component Overview

## What It Is

Ward scaffolding is a **post-ward-creation middleware** that structures ward directories based on skill `ward_setup` frontmatter. It makes wards reusable apps that grow over time.

## When It Runs

- After ward creation (`__ward_changed__: true`, `action: "created"`)
- Post-execution: `auto_update_agents_md` re-indexes core modules
- Scaffolding is idempotent — safe to run on every execution

## What It Does

1. **Reads skill `ward_setup`** — from recommended skills' SKILL.md frontmatter
2. **Creates directories** — `core/`, `output/`, `specs/`, `specs/archive/`, `memory-bank/`
3. **Generates AGENTS.md** — from skill's `agents_md` config (purpose, conventions)
4. **Indexes core modules** — scans `core/` using language configs, updates AGENTS.md
5. **Injects ward rules** — via `format_intent_injection()` into agent prompt

## Key Design Decisions

- **Skills drive structure** — not hardcoded. Different skills scaffold different directories.
- **Language configs externalized** — `config/wards/*.yaml` for signature extraction patterns.
- **Spec-first workflow** — injected as prompt guidance, enforced in graph as first node.
- **AGENTS.md is the living README** — auto-updated with core module API index after each session.
- **Specs are ephemeral** — active in `specs/`, archived to `specs/archive/` after implementation.

## Related Files

| File | Purpose |
|------|---------|
| `gateway/gateway-execution/src/middleware/ward_scaffold.rs` | Scaffolding middleware |
| `gateway/gateway-services/src/lang_config.rs` | Language config loader |
| `gateway/gateway-services/src/skills.rs` | WardSetup types in skill frontmatter |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Ward rules + spec guidance injection |
| `gateway/gateway-execution/src/runner.rs` | Wiring + auto_update_agents_md |
| `~/Documents/zbot/config/wards/*.yaml` | Language pattern configs |
```

- [ ] **Step 2: Update component index**

In `memory-bank/components/index.md`, add a row to the table:

```markdown
| Ward Scaffolding | [ward-scaffolding/overview.md](ward-scaffolding/overview.md) | Post-ward-creation: skill-driven directory scaffolding, AGENTS.md generation, core module indexing via language configs. |
```

- [ ] **Step 3: Commit**

```bash
git add memory-bank/components/ward-scaffolding/overview.md memory-bank/components/index.md
git commit -m "docs: add ward scaffolding component documentation"
```
