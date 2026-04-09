//! # Skills Service
//!
//! Manages skill configurations stored as folders with SKILL.md files.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Skill data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub instructions: String,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Ward setup configuration from skill frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WardSetup {
    #[serde(default)]
    pub directories: Vec<String>,
    /// Referenced language skills (informational — not auto-loaded).
    #[serde(default)]
    pub language_skills: Vec<String>,
    #[serde(default)]
    pub spec_guidance: Option<String>,
    #[serde(default)]
    pub agents_md: Option<WardAgentsMdConfig>,
}

/// Seed content for AGENTS.md in a new ward.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WardAgentsMdConfig {
    pub purpose: String,
    #[serde(default)]
    pub conventions: Vec<String>,
}

/// Skill frontmatter stored in SKILL.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    pub description: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub ward_setup: Option<WardSetup>,
}

pub struct SkillService {
    skills_dir: PathBuf,
    cache: Arc<RwLock<Option<Vec<Skill>>>>,
}

impl SkillService {
    /// Create a new skill service.
    pub fn new(skills_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Preload skills into cache on startup.
    pub async fn preload(&self) -> Result<(), String> {
        let skills = self.load_all_skills()?;
        *self.cache.write().await = Some(skills);
        tracing::info!("Preloaded {} skills into cache", self.cache.read().await.as_ref().map(|s| s.len()).unwrap_or(0));
        Ok(())
    }

    /// List all skills (from cache if available).
    pub async fn list(&self) -> Result<Vec<Skill>, String> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(skills) = cache.as_ref() {
                return Ok(skills.clone());
            }
        }

        // Load from disk and cache
        let skills = self.load_all_skills()?;
        {
            let mut cache = self.cache.write().await;
            *cache = Some(skills.clone());
        }
        Ok(skills)
    }

    /// Load all skills from disk (bypasses cache).
    fn load_all_skills(&self) -> Result<Vec<Skill>, String> {
        if !self.skills_dir.exists() {
            return Ok(vec![]);
        }

        let mut skills = Vec::new();

        let entries = fs::read_dir(&self.skills_dir)
            .map_err(|e| format!("Failed to read skills directory: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            if let Ok(skill) = self.read_skill_folder(&path) {
                skills.push(skill);
            }
        }

        Ok(skills)
    }

    /// Get a skill by ID.
    pub async fn get(&self, id: &str) -> Result<Skill, String> {
        let skill_dir = self.skills_dir.join(id);

        if !skill_dir.exists() {
            return Err(format!("Skill not found: {}", id));
        }

        self.read_skill_folder(&skill_dir)
    }

    /// Create a new skill.
    pub async fn create(&self, skill: Skill) -> Result<Skill, String> {
        fs::create_dir_all(&self.skills_dir)
            .map_err(|e| format!("Failed to create skills directory: {}", e))?;

        let skill_dir = self.skills_dir.join(&skill.name);
        fs::create_dir_all(&skill_dir)
            .map_err(|e| format!("Failed to create skill directory: {}", e))?;

        // Create placeholder folders
        fs::create_dir_all(skill_dir.join("assets")).ok();
        fs::create_dir_all(skill_dir.join("resources")).ok();
        fs::create_dir_all(skill_dir.join("scripts")).ok();

        // Write SKILL.md
        self.write_skill_md(&skill_dir, &skill)?;

        // Invalidate cache
        self.invalidate_cache().await;

        Ok(Skill {
            id: skill.name.clone(),
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            ..skill
        })
    }

    /// Update an existing skill.
    pub async fn update(&self, id: &str, skill: Skill) -> Result<Skill, String> {
        let skill_dir = self.skills_dir.join(id);

        if !skill_dir.exists() {
            return Err(format!("Skill not found: {}", id));
        }

        // If name changed, rename directory
        if skill.name != id {
            let new_dir = self.skills_dir.join(&skill.name);
            fs::rename(&skill_dir, &new_dir)
                .map_err(|e| format!("Failed to rename skill directory: {}", e))?;
        }

        let target_dir = self.skills_dir.join(&skill.name);
        self.write_skill_md(&target_dir, &skill)?;

        // Invalidate cache
        self.invalidate_cache().await;

        Ok(skill)
    }

    /// Delete a skill.
    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let skill_path = self.skills_dir.join(id);

        if !skill_path.exists() {
            return Err(format!("Skill not found: {}", id));
        }

        fs::remove_dir_all(&skill_path)
            .map_err(|e| format!("Failed to delete skill directory: {}", e))?;

        // Invalidate cache
        self.invalidate_cache().await;

        Ok(())
    }

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

    /// Invalidate the skill cache.
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }

    fn read_skill_folder(&self, skill_dir: &Path) -> Result<Skill, String> {
        let skill_md_path = skill_dir.join("SKILL.md");

        if !skill_md_path.exists() {
            return Err(format!("SKILL.md not found in {:?}", skill_dir));
        }

        let content = fs::read_to_string(&skill_md_path)
            .map_err(|e| format!("Failed to read SKILL.md: {}", e))?;

        let (frontmatter, instructions) = self.parse_frontmatter(&content)?;

        let name = skill_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let display_name = frontmatter
            .display_name
            .clone()
            .unwrap_or_else(|| self.format_name(&name));

        Ok(Skill {
            id: name.clone(),
            name,
            display_name,
            description: frontmatter.description,
            category: frontmatter.category.unwrap_or_else(|| "general".to_string()),
            instructions,
            created_at: None,
        })
    }

    fn write_skill_md(&self, skill_dir: &Path, skill: &Skill) -> Result<(), String> {
        // Preserve existing ward_setup from the current SKILL.md, if any.
        // The Skill struct does not carry ward_setup, so a naive write would silently
        // strip it.  Read the existing file and extract the field before overwriting.
        let existing_ward_setup = {
            let path = skill_dir.join("SKILL.md");
            if path.exists() {
                std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|content| self.parse_frontmatter(&content).ok())
                    .and_then(|(fm, _)| fm.ward_setup)
            } else {
                None
            }
        };

        let frontmatter = SkillFrontmatter {
            name: skill.name.clone(),
            display_name: if skill.display_name.is_empty() {
                None
            } else {
                Some(skill.display_name.clone())
            },
            description: skill.description.clone(),
            category: if skill.category.is_empty() {
                None
            } else {
                Some(skill.category.clone())
            },
            ward_setup: existing_ward_setup,
        };

        let content = format!(
            "---\n{}\n---\n\n{}\n",
            serde_yaml::to_string(&frontmatter)
                .map_err(|e| format!("Failed to serialize frontmatter: {}", e))?,
            skill.instructions
        );

        fs::write(skill_dir.join("SKILL.md"), content)
            .map_err(|e| format!("Failed to write SKILL.md: {}", e))?;

        Ok(())
    }

    fn parse_frontmatter(&self, content: &str) -> Result<(SkillFrontmatter, String), String> {
        let frontmatter_regex = regex::Regex::new(r"^---\r?\n([\s\S]*?)\r?\n---\r?\n([\s\S]*)$")
            .map_err(|e| format!("Failed to create regex: {}", e))?;

        let captures = frontmatter_regex
            .captures(content)
            .ok_or_else(|| "Invalid SKILL.md format: missing frontmatter".to_string())?;

        let yaml_content = captures.get(1).unwrap().as_str();
        let body = captures.get(2).unwrap().as_str();

        let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_content)
            .map_err(|e| format!("Failed to parse frontmatter: {}", e))?;

        let body = body.trim_start_matches(['\r', '\n']).to_string();

        Ok((frontmatter, body))
    }

    fn format_name(&self, name: &str) -> String {
        name.split('-')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_service(dir: &TempDir) -> SkillService {
        SkillService::new(dir.path().to_path_buf())
    }

    /// Writing a skill back (simulating an update) must not strip ward_setup.
    #[tokio::test]
    async fn test_write_skill_preserves_ward_setup() {
        let tmp = TempDir::new().expect("tempdir");
        let service = make_service(&tmp);

        // Create skill directory and an initial SKILL.md that contains ward_setup.
        let skill_dir = tmp.path().join("my-skill");
        fs::create_dir_all(&skill_dir).expect("create skill dir");

        let initial_md = r#"---
name: my-skill
description: A test skill
ward_setup:
  directories:
    - src
  language_skills:
    - rust
---

Do something useful.
"#;
        fs::write(skill_dir.join("SKILL.md"), initial_md).expect("write initial SKILL.md");

        // Build a Skill struct (no ward_setup field — mirrors what the API provides).
        let skill = Skill {
            id: "my-skill".to_string(),
            name: "my-skill".to_string(),
            display_name: "My Skill".to_string(),
            description: "A test skill".to_string(),
            category: "general".to_string(),
            instructions: "Do something useful.".to_string(),
            created_at: None,
        };

        // Simulate an update write.
        service
            .write_skill_md(&skill_dir, &skill)
            .expect("write_skill_md");

        // Read back and verify ward_setup survived.
        let written = fs::read_to_string(skill_dir.join("SKILL.md")).expect("read back");
        let (fm, _body) = service
            .parse_frontmatter(&written)
            .expect("parse written frontmatter");

        let ward_setup = fm.ward_setup.expect("ward_setup must be preserved");
        assert!(
            ward_setup.directories.contains(&"src".to_string()),
            "directories must be preserved; got {:?}",
            ward_setup.directories
        );
        assert!(
            ward_setup.language_skills.contains(&"rust".to_string()),
            "language_skills must be preserved; got {:?}",
            ward_setup.language_skills
        );
    }
}
