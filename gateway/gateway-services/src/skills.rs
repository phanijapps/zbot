//! # Skills Service
//!
//! Manages skill configurations stored as folders with SKILL.md files.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

/// Origin of a loaded skill. Determines whether mutation operations
/// (create / update / delete) are allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillSource {
    /// User-owned skill under `<vault>/skills/`. Mutable.
    Vault,
    /// Externally-installed skill under `$HOME/.agents/skills/`. Read-only
    /// from this service — managed by whatever installer dropped it there.
    Agent,
}

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
    /// Where this skill was loaded from. Defaults to `Vault` when missing
    /// (e.g. older serialized payloads) so existing consumers stay correct.
    #[serde(default = "default_source")]
    pub source: SkillSource,
}

fn default_source() -> SkillSource {
    SkillSource::Vault
}

/// Per-skill filesystem facts the incremental reindexer needs. Returned
/// from [`SkillService::list_for_index`]. Not part of the HTTP API — it
/// stays in the service crate so we don't pollute the public `Skill`
/// shape with internal mtime/path fields.
#[derive(Debug, Clone)]
pub struct SkillFileInfo {
    /// Skill identifier (directory name).
    pub id: String,
    /// Origin root.
    pub source: SkillSource,
    /// Absolute path to `<root>/<id>/SKILL.md`.
    pub file_path: PathBuf,
    /// `SKILL.md` mtime in seconds since the Unix epoch.
    pub mtime_unix: i64,
    /// `SKILL.md` size in bytes — used to break mtime-granularity ties on
    /// filesystems that record mtime at 1-second resolution.
    pub size_bytes: u64,
    /// Display-ready content for the embedding (matches what `index_resources`
    /// already writes today: `name | description | category: <cat>`).
    pub indexed_content: String,
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

/// Skills service that loads from one or more roots in priority order.
///
/// The first root is the **mutable vault root** — all create / update /
/// delete operations target it. Subsequent roots are read-only sources
/// (e.g. `$HOME/.agents/skills/`). When a skill name appears in multiple
/// roots, the one from the highest-priority (earliest) root wins; the
/// shadowed copy is silently skipped.
pub struct SkillService {
    /// Skills roots in priority order. `skills_dirs[0]` is the writable
    /// vault root; everything after it is read-only.
    skills_dirs: Vec<PathBuf>,
    cache: Arc<RwLock<Option<Vec<Skill>>>>,
}

impl SkillService {
    /// Create a service with a single (writable) skills root.
    /// Convenience for tests and call sites that haven't migrated to
    /// multi-root.
    pub fn new(skills_dir: PathBuf) -> Self {
        Self::with_roots(vec![skills_dir])
    }

    /// Create a service with an ordered list of skills roots. The first
    /// root is treated as the writable vault; the rest are read-only.
    /// Panics if `skills_dirs` is empty.
    pub fn with_roots(skills_dirs: Vec<PathBuf>) -> Self {
        assert!(
            !skills_dirs.is_empty(),
            "SkillService requires at least one root (the writable vault)"
        );
        Self {
            skills_dirs,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Returns the writable vault root — the first entry in `skills_dirs`.
    /// Used by mutation methods that always target the vault.
    fn vault_root(&self) -> &PathBuf {
        &self.skills_dirs[0]
    }

    /// Maps a skills root path to its `SkillSource`. The vault root is
    /// always `Source::Vault`; everything else is `Source::Agent`.
    fn source_for_root(&self, root: &Path) -> SkillSource {
        if root == self.vault_root().as_path() {
            SkillSource::Vault
        } else {
            SkillSource::Agent
        }
    }

    /// Preload skills into cache on startup.
    pub async fn preload(&self) -> Result<(), String> {
        let skills = self.load_all_skills()?;
        *self.cache.write().await = Some(skills);
        tracing::info!(
            "Preloaded {} skills into cache",
            self.cache
                .read()
                .await
                .as_ref()
                .map(|s| s.len())
                .unwrap_or(0)
        );
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
    ///
    /// Walks every configured root in priority order. The first occurrence
    /// of each skill ID (directory name) wins; later occurrences are
    /// silently skipped (with a `tracing::info!` line so the shadow is
    /// visible in logs).
    fn load_all_skills(&self) -> Result<Vec<Skill>, String> {
        let mut skills = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for root in &self.skills_dirs {
            self.load_root_into(root, &mut skills, &mut seen);
        }

        Ok(skills)
    }

    /// List skills with the disk metadata the incremental reindexer needs:
    /// canonical id (after dedup), origin source, absolute path, and mtime.
    /// Bypasses the in-memory cache so the result always reflects the
    /// filesystem at call time — important for the reindex diff.
    pub fn list_for_index(&self) -> Vec<SkillFileInfo> {
        let mut out = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for root in &self.skills_dirs {
            self.collect_root_for_index(root, &mut out, &mut seen);
        }
        out
    }

    fn collect_root_for_index(
        &self,
        root: &Path,
        out: &mut Vec<SkillFileInfo>,
        seen: &mut HashSet<String>,
    ) {
        if !root.exists() {
            return;
        }
        let entries = match fs::read_dir(root) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to read skills root {:?}: {}", root, e);
                return;
            }
        };
        let source = self.source_for_root(root);
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            let id = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            match Self::file_info(&id, source, &path, &skill_md) {
                Ok(info) => out.push(info),
                Err(e) => tracing::warn!("Skipping skill {:?} for index: {}", path, e),
            }
        }
    }

    fn file_info(
        id: &str,
        source: SkillSource,
        skill_dir: &Path,
        skill_md: &Path,
    ) -> Result<SkillFileInfo, String> {
        let meta = fs::metadata(skill_md).map_err(|e| format!("stat failed: {e}"))?;
        let mtime_unix = meta
            .modified()
            .map_err(|e| format!("mtime read failed: {e}"))?
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let size_bytes = meta.len();

        // Read just the frontmatter to recover description + category for
        // the embedding content. Cheap (file is already in page cache).
        let content = fs::read_to_string(skill_md).map_err(|e| format!("read failed: {e}"))?;
        let (frontmatter, _) = parse_skill_frontmatter(&content)?;
        let description = frontmatter.description;
        let category = frontmatter
            .category
            .unwrap_or_else(|| "general".to_string());
        let _ = skill_dir;

        let indexed_content = format!("{} | {} | category: {}", id, description, category);
        Ok(SkillFileInfo {
            id: id.to_string(),
            source,
            file_path: skill_md.to_path_buf(),
            mtime_unix,
            size_bytes,
            indexed_content,
        })
    }

    /// Walk a single root and append non-shadowed skills to `out`.
    /// Errors at this level are logged, not bubbled — a missing or
    /// unreadable agent root must not block vault loading.
    fn load_root_into(&self, root: &Path, out: &mut Vec<Skill>, seen: &mut HashSet<String>) {
        if !root.exists() {
            return;
        }
        let entries = match fs::read_dir(root) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to read skills root {:?}: {}", root, e);
                return;
            }
        };
        let source = self.source_for_root(root);

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if !path.join("SKILL.md").exists() {
                continue;
            }
            let id = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };
            if !seen.insert(id.clone()) {
                tracing::info!(
                    "Skill {} from {:?} shadowed by higher-priority root",
                    id,
                    root
                );
                continue;
            }
            match self.read_skill_folder(&path, source) {
                Ok(skill) => out.push(skill),
                Err(e) => tracing::warn!("Failed to load skill {:?}: {}", path, e),
            }
        }
    }

    /// Locate a skill by ID across all configured roots. Returns the
    /// directory path and the source root the skill belongs to.
    fn find_skill(&self, id: &str) -> Option<(PathBuf, SkillSource)> {
        for root in &self.skills_dirs {
            let candidate = root.join(id);
            if candidate.join("SKILL.md").exists() {
                return Some((candidate, self.source_for_root(root)));
            }
        }
        None
    }

    /// Get a skill by ID. Searches every root; agent-dir skills load
    /// successfully but cannot be mutated.
    pub async fn get(&self, id: &str) -> Result<Skill, String> {
        let (dir, source) = self
            .find_skill(id)
            .ok_or_else(|| format!("Skill not found: {}", id))?;
        self.read_skill_folder(&dir, source)
    }

    /// Create a new skill. Always writes to the vault root. Returns an
    /// error if a skill with the same name already exists in the vault.
    /// Skills shadowed in `~/.agents/skills/` are unaffected — the new
    /// vault entry will shadow them on the next list.
    pub async fn create(&self, skill: Skill) -> Result<Skill, String> {
        let vault = self.vault_root();
        fs::create_dir_all(vault)
            .map_err(|e| format!("Failed to create skills directory: {}", e))?;

        let skill_dir = vault.join(&skill.name);
        fs::create_dir_all(&skill_dir)
            .map_err(|e| format!("Failed to create skill directory: {}", e))?;

        // Create placeholder folders
        fs::create_dir_all(skill_dir.join("assets")).ok();
        fs::create_dir_all(skill_dir.join("resources")).ok();
        fs::create_dir_all(skill_dir.join("scripts")).ok();

        self.write_skill_md(&skill_dir, &skill)?;
        self.invalidate_cache().await;

        Ok(Skill {
            id: skill.name.clone(),
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            source: SkillSource::Vault,
            ..skill
        })
    }

    /// Update an existing skill. Refuses to mutate skills that live in
    /// a read-only root.
    pub async fn update(&self, id: &str, skill: Skill) -> Result<Skill, String> {
        let (current_dir, source) = self
            .find_skill(id)
            .ok_or_else(|| format!("Skill not found: {}", id))?;
        ensure_writable(source, "update", id)?;

        // If name changed, rename directory (always within the vault).
        let vault = self.vault_root();
        let target_dir = if skill.name != id {
            let new_dir = vault.join(&skill.name);
            fs::rename(&current_dir, &new_dir)
                .map_err(|e| format!("Failed to rename skill directory: {}", e))?;
            new_dir
        } else {
            current_dir
        };

        self.write_skill_md(&target_dir, &skill)?;
        self.invalidate_cache().await;

        Ok(Skill {
            source: SkillSource::Vault,
            ..skill
        })
    }

    /// Delete a skill. Refuses to mutate skills that live in a read-only
    /// root.
    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let (skill_path, source) = self
            .find_skill(id)
            .ok_or_else(|| format!("Skill not found: {}", id))?;
        ensure_writable(source, "delete", id)?;

        fs::remove_dir_all(&skill_path)
            .map_err(|e| format!("Failed to delete skill directory: {}", e))?;
        self.invalidate_cache().await;
        Ok(())
    }

    /// Get ward_setup config for a skill by ID, if it has one. Works for
    /// skills in any root (read-only path).
    pub async fn get_ward_setup(&self, id: &str) -> Result<Option<WardSetup>, String> {
        let (skill_dir, _source) = self
            .find_skill(id)
            .ok_or_else(|| format!("Skill not found: {}", id))?;
        let content = std::fs::read_to_string(skill_dir.join("SKILL.md"))
            .map_err(|e| format!("Failed to read SKILL.md: {}", e))?;
        let (frontmatter, _) = self.parse_frontmatter(&content)?;
        Ok(frontmatter.ward_setup)
    }

    /// Invalidate the skill cache.
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }

    fn read_skill_folder(&self, skill_dir: &Path, source: SkillSource) -> Result<Skill, String> {
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
            category: frontmatter
                .category
                .unwrap_or_else(|| "general".to_string()),
            instructions,
            created_at: None,
            source,
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
        parse_skill_frontmatter(content)
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

/// Parse YAML frontmatter delimited by `---` lines from a SKILL.md
/// document. Returns the parsed struct and the trimmed body.
fn parse_skill_frontmatter(content: &str) -> Result<(SkillFrontmatter, String), String> {
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

/// Returns an error if `source` is read-only. Used by mutation methods
/// to refuse writes targeting the agent-installed root.
fn ensure_writable(source: SkillSource, op: &str, id: &str) -> Result<(), String> {
    match source {
        SkillSource::Vault => Ok(()),
        SkillSource::Agent => Err(format!(
            "Cannot {op} skill '{id}': it is managed by ~/.agents/skills (read-only)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_service(dir: &TempDir) -> SkillService {
        SkillService::new(dir.path().to_path_buf())
    }

    /// Build a SKILL.md for a skill named `name` with optional description.
    fn write_skill_at(root: &Path, name: &str, description: &str) {
        let skill_dir = root.join(name);
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        let md = format!("---\nname: {name}\ndescription: {description}\n---\n\nbody\n");
        fs::write(skill_dir.join("SKILL.md"), md).expect("write SKILL.md");
    }

    /// Sanity: empty constructor list panics — there must always be a vault root.
    #[test]
    #[should_panic(expected = "at least one root")]
    fn with_roots_empty_panics() {
        let _ = SkillService::with_roots(vec![]);
    }

    /// Skills load from both roots and their `source` field reflects the
    /// originating root.
    #[tokio::test]
    async fn load_two_roots_tags_source_correctly() {
        let vault = TempDir::new().expect("vault tmp");
        let agent = TempDir::new().expect("agent tmp");
        write_skill_at(vault.path(), "vault-skill", "from vault");
        write_skill_at(agent.path(), "agent-skill", "from agent");

        let service =
            SkillService::with_roots(vec![vault.path().to_path_buf(), agent.path().to_path_buf()]);
        let mut skills = service.list().await.expect("list");
        skills.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].id, "agent-skill");
        assert_eq!(skills[0].source, SkillSource::Agent);
        assert_eq!(skills[1].id, "vault-skill");
        assert_eq!(skills[1].source, SkillSource::Vault);
    }

    /// Vault wins when both roots define the same skill name. The agent
    /// copy is silently shadowed; only the vault entry is returned.
    #[tokio::test]
    async fn vault_wins_on_collision() {
        let vault = TempDir::new().expect("vault tmp");
        let agent = TempDir::new().expect("agent tmp");
        write_skill_at(vault.path(), "shared", "vault copy");
        write_skill_at(agent.path(), "shared", "agent copy");

        let service =
            SkillService::with_roots(vec![vault.path().to_path_buf(), agent.path().to_path_buf()]);
        let skills = service.list().await.expect("list");
        assert_eq!(skills.len(), 1, "shadow must be silent — only one row");
        assert_eq!(skills[0].id, "shared");
        assert_eq!(skills[0].source, SkillSource::Vault);
        assert_eq!(skills[0].description, "vault copy");
    }

    /// When the vault has no shadowing entry, the agent-dir skill loads.
    #[tokio::test]
    async fn agent_only_when_no_shadow() {
        let vault = TempDir::new().expect("vault tmp");
        let agent = TempDir::new().expect("agent tmp");
        write_skill_at(agent.path(), "agent-only", "lives only in agent dir");

        let service =
            SkillService::with_roots(vec![vault.path().to_path_buf(), agent.path().to_path_buf()]);
        let skills = service.list().await.expect("list");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "agent-only");
        assert_eq!(skills[0].source, SkillSource::Agent);
    }

    /// Missing agent root is tolerated — vault skills still load.
    #[tokio::test]
    async fn missing_agent_dir_is_tolerated() {
        let vault = TempDir::new().expect("vault tmp");
        write_skill_at(vault.path(), "vault-only", "only in vault");
        let nonexistent = vault.path().join("does-not-exist-123");

        let service = SkillService::with_roots(vec![vault.path().to_path_buf(), nonexistent]);
        let skills = service.list().await.expect("list");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "vault-only");
        assert_eq!(skills[0].source, SkillSource::Vault);
    }

    /// `update` against a skill that lives in the read-only agent root
    /// returns an error mentioning the read-only nature.
    #[tokio::test]
    async fn update_rejects_agent_root_skill() {
        let vault = TempDir::new().expect("vault tmp");
        let agent = TempDir::new().expect("agent tmp");
        write_skill_at(agent.path(), "managed", "managed skill");
        let service =
            SkillService::with_roots(vec![vault.path().to_path_buf(), agent.path().to_path_buf()]);

        let updated = Skill {
            id: "managed".to_string(),
            name: "managed".to_string(),
            display_name: "Managed".to_string(),
            description: "would-be edit".to_string(),
            category: "general".to_string(),
            instructions: "...".to_string(),
            created_at: None,
            source: SkillSource::Agent,
        };
        let err = service
            .update("managed", updated)
            .await
            .expect_err("must reject");
        assert!(err.contains("read-only"), "got: {err}");
    }

    /// `delete` against an agent-root skill returns an error and leaves
    /// the file in place.
    #[tokio::test]
    async fn delete_rejects_agent_root_skill() {
        let vault = TempDir::new().expect("vault tmp");
        let agent = TempDir::new().expect("agent tmp");
        write_skill_at(agent.path(), "managed", "managed skill");
        let service =
            SkillService::with_roots(vec![vault.path().to_path_buf(), agent.path().to_path_buf()]);

        let err = service.delete("managed").await.expect_err("must reject");
        assert!(err.contains("read-only"), "got: {err}");
        assert!(
            agent.path().join("managed").join("SKILL.md").exists(),
            "the file must still be on disk"
        );
    }

    /// `create` always lands in the vault root, even when an agent-root
    /// skill with the same name already exists. After creation the vault
    /// copy shadows the agent copy on subsequent lists.
    #[tokio::test]
    async fn create_lands_in_vault_and_shadows_agent_copy() {
        let vault = TempDir::new().expect("vault tmp");
        let agent = TempDir::new().expect("agent tmp");
        write_skill_at(agent.path(), "shared", "agent copy");
        let service =
            SkillService::with_roots(vec![vault.path().to_path_buf(), agent.path().to_path_buf()]);

        let new_skill = Skill {
            id: String::new(),
            name: "shared".to_string(),
            display_name: "Shared".to_string(),
            description: "vault copy".to_string(),
            category: "general".to_string(),
            instructions: "vault body".to_string(),
            created_at: None,
            source: SkillSource::Vault,
        };
        let created = service.create(new_skill).await.expect("create ok");
        assert_eq!(created.source, SkillSource::Vault);
        assert!(vault.path().join("shared").join("SKILL.md").exists());

        let listed = service.list().await.expect("list");
        assert_eq!(listed.len(), 1, "shadow rule still applies after create");
        assert_eq!(listed[0].source, SkillSource::Vault);
        assert_eq!(listed[0].description, "vault copy");
    }

    /// Deleting the vault copy must promote the previously-shadowed
    /// agent copy on the next list (cache invalidation behaves).
    #[tokio::test]
    async fn deleting_vault_promotes_agent_copy() {
        let vault = TempDir::new().expect("vault tmp");
        let agent = TempDir::new().expect("agent tmp");
        write_skill_at(vault.path(), "shared", "vault copy");
        write_skill_at(agent.path(), "shared", "agent copy");
        let service =
            SkillService::with_roots(vec![vault.path().to_path_buf(), agent.path().to_path_buf()]);

        // Prime cache with the vault copy.
        let initial = service.list().await.expect("list");
        assert_eq!(initial[0].source, SkillSource::Vault);

        service.delete("shared").await.expect("vault delete ok");

        let after = service.list().await.expect("list");
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].source, SkillSource::Agent);
        assert_eq!(after[0].description, "agent copy");
    }

    /// `get` works for skills in the agent root (read-only path).
    #[tokio::test]
    async fn get_returns_agent_skill() {
        let vault = TempDir::new().expect("vault tmp");
        let agent = TempDir::new().expect("agent tmp");
        write_skill_at(agent.path(), "managed", "agent-managed skill");
        let service =
            SkillService::with_roots(vec![vault.path().to_path_buf(), agent.path().to_path_buf()]);
        let s = service.get("managed").await.expect("get ok");
        assert_eq!(s.source, SkillSource::Agent);
        assert_eq!(s.description, "agent-managed skill");
    }

    /// `list_for_index` returns one entry per visible skill, dedup'd, with
    /// the file metadata the reindexer needs.
    #[tokio::test]
    async fn list_for_index_returns_dedup_with_mtime() {
        let vault = TempDir::new().expect("vault tmp");
        let agent = TempDir::new().expect("agent tmp");
        write_skill_at(vault.path(), "shared", "vault copy");
        write_skill_at(agent.path(), "shared", "agent copy"); // shadowed
        write_skill_at(agent.path(), "agent-only", "managed");

        let service =
            SkillService::with_roots(vec![vault.path().to_path_buf(), agent.path().to_path_buf()]);
        let mut info = service.list_for_index();
        info.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(info.len(), 2);

        assert_eq!(info[0].id, "agent-only");
        assert_eq!(info[0].source, SkillSource::Agent);
        assert!(info[0].mtime_unix > 0);
        assert!(info[0].size_bytes > 0);
        assert!(info[0].file_path.ends_with("agent-only/SKILL.md"));
        assert!(info[0].indexed_content.contains("agent-only"));

        assert_eq!(info[1].id, "shared");
        assert_eq!(info[1].source, SkillSource::Vault);
        assert!(info[1].file_path.starts_with(vault.path()));
        assert!(info[1].indexed_content.contains("vault copy"));
    }

    /// Single-root constructor still works (backwards compat path).
    #[tokio::test]
    async fn single_root_constructor_still_works() {
        let dir = TempDir::new().expect("tmp");
        write_skill_at(dir.path(), "solo", "solo skill");
        let service = SkillService::new(dir.path().to_path_buf());
        let skills = service.list().await.expect("list");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].source, SkillSource::Vault);
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
            source: SkillSource::Vault,
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
