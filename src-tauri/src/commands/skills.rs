// ============================================================================
// SKILLS COMMANDS
// Agent skill management with folder-based storage
// ============================================================================

use crate::settings::AppDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Skill data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub instructions: String,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Skill frontmatter stored in SKILL.md
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkillFrontmatter {
    name: String,
    #[serde(rename = "displayName")]
    display_name: String,
    description: String,
    category: String,
}

/// Gets the skills directory path
fn get_skills_dir() -> Result<PathBuf, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    Ok(dirs.skills_dir)
}

/// Gets the staging directory for new skills
fn get_staging_dir() -> Result<PathBuf, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    Ok(dirs.config_dir.join("staging-skills"))
}

/// Check if we're in staging mode (creating new skill)
fn is_staging_mode(skill_id: &str) -> bool {
    skill_id == "staging" || skill_id == "temp"
}

/// Lists all skills from the skills directory
#[tauri::command]
pub async fn list_skills() -> Result<Vec<Skill>, String> {
    let skills_dir = get_skills_dir()?;

    if !skills_dir.exists() {
        return Ok(vec![]);
    }

    let mut skills = Vec::new();

    // Iterate through subdirectories in skills directory
    let entries = fs::read_dir(&skills_dir)
        .map_err(|e| format!("Failed to read skills directory: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip if not a directory
        if !path.is_dir() {
            continue;
        }

        // Look for SKILL.md file
        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }

        // Read and parse skill folder
        if let Ok(skill) = read_skill_folder(&path) {
            skills.push(skill);
        }
    }

    Ok(skills)
}

/// Gets a single skill by ID
#[tauri::command]
pub async fn get_skill(id: String) -> Result<Skill, String> {
    let skills_dir = get_skills_dir()?;
    let skill_dir = skills_dir.join(&id);

    if !skill_dir.exists() {
        return Err(format!("Skill not found: {}", id));
    }

    read_skill_folder(&skill_dir)
}

/// Creates a new skill
#[tauri::command]
pub async fn create_skill(skill: Skill) -> Result<Skill, String> {
    let skills_dir = get_skills_dir()?;

    // Ensure skills directory exists
    fs::create_dir_all(&skills_dir)
        .map_err(|e| format!("Failed to create skills directory: {}", e))?;

    // Create skill directory
    let skill_dir = skills_dir.join(&skill.name);
    fs::create_dir_all(&skill_dir)
        .map_err(|e| format!("Failed to create skill directory: {}", e))?;

    // Create placeholder folders
    let assets_dir = skill_dir.join("assets");
    let resources_dir = skill_dir.join("resources");
    let scripts_dir = skill_dir.join("scripts");
    fs::create_dir_all(&assets_dir)
        .map_err(|e| format!("Failed to create assets directory: {}", e))?;
    fs::create_dir_all(&resources_dir)
        .map_err(|e| format!("Failed to create resources directory: {}", e))?;
    fs::create_dir_all(&scripts_dir)
        .map_err(|e| format!("Failed to create scripts directory: {}", e))?;

    // Write SKILL.md with frontmatter
    let frontmatter = SkillFrontmatter {
        name: skill.name.clone(),
        display_name: skill.display_name.clone(),
        description: skill.description.clone(),
        category: skill.category.clone(),
    };
    let skill_md_content = format!("---\n{}\n---\n\n{}\n",
        serde_yaml::to_string(&frontmatter)
            .map_err(|e| format!("Failed to serialize frontmatter: {}", e))?,
        skill.instructions
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md_content)
        .map_err(|e| format!("Failed to write SKILL.md: {}", e))?;

    // Clear staging if exists
    let staging_dir = get_staging_dir()?;
    let staging_skill = staging_dir.join("SKILL.md");
    if staging_skill.exists() {
        let _ = fs::remove_file(&staging_skill);
    }

    // Return the created skill
    Ok(Skill {
        id: Some(skill.name.clone()),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
        ..skill
    })
}

/// Updates an existing skill
#[tauri::command]
pub async fn update_skill(id: String, skill: Skill) -> Result<Skill, String> {
    let skills_dir = get_skills_dir()?;
    let skill_dir = skills_dir.join(&id);

    if !skill_dir.exists() {
        return Err(format!("Skill not found: {}", id));
    }

    // If name changed, rename directory
    if skill.name != id {
        let new_dir = skills_dir.join(&skill.name);
        fs::rename(&skill_dir, &new_dir)
            .map_err(|e| format!("Failed to rename skill directory: {}", e))?;
    }

    // Use the new directory name if changed
    let target_dir = skills_dir.join(&skill.name);

    // Write SKILL.md with frontmatter
    let frontmatter = SkillFrontmatter {
        name: skill.name.clone(),
        display_name: skill.display_name.clone(),
        description: skill.description.clone(),
        category: skill.category.clone(),
    };
    let skill_md_content = format!("---\n{}\n---\n\n{}\n",
        serde_yaml::to_string(&frontmatter)
            .map_err(|e| format!("Failed to serialize frontmatter: {}", e))?,
        skill.instructions
    );
    fs::write(target_dir.join("SKILL.md"), skill_md_content)
        .map_err(|e| format!("Failed to write SKILL.md: {}", e))?;

    Ok(skill)
}

/// Deletes a skill by removing its directory
#[tauri::command]
pub async fn delete_skill(id: String) -> Result<(), String> {
    let skills_dir = get_skills_dir()?;
    let skill_path = skills_dir.join(&id);

    if !skill_path.exists() {
        return Err(format!("Skill not found: {}", id));
    }

    fs::remove_dir_all(&skill_path)
        .map_err(|e| format!("Failed to delete skill directory: {}", e))?;

    Ok(())
}

/// Reads a skill folder and parses SKILL.md
fn read_skill_folder(skill_dir: &PathBuf) -> Result<Skill, String> {
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_md_path.exists() {
        return Err(format!("SKILL.md not found in {:?}", skill_dir));
    }

    // Read SKILL.md
    let content = fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("Failed to read SKILL.md: {}", e))?;

    // Parse frontmatter and body
    let (frontmatter, instructions) = parse_skill_frontmatter(&content)?;

    // Get skill name from directory name
    let name = skill_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(Skill {
        id: Some(name.clone()),
        name,
        display_name: frontmatter.display_name,
        description: frontmatter.description,
        category: frontmatter.category,
        instructions,
        created_at: Some("1970-01-01T00:00:00Z".to_string()), // TODO: get from file metadata
    })
}

/// File entry in skill folder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFile {
    pub name: String,
    pub path: String,
    #[serde(rename = "isFile")]
    pub is_file: bool,
    #[serde(rename = "isBinary")]
    pub is_binary: bool,
    #[serde(rename = "isProtected")]
    pub is_protected: bool,
    pub size: u64,
}

/// File content response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFileContent {
    pub content: String,
    #[serde(rename = "isBinary")]
    pub is_binary: bool,
    #[serde(rename = "isMarkdown")]
    pub is_markdown: bool,
}

/// List files in a skill folder or staging
#[tauri::command]
pub async fn list_skill_files(skill_id: String) -> Result<Vec<SkillFile>, String> {
    let (base_dir, _is_staging) = if is_staging_mode(&skill_id) {
        let staging_dir = get_staging_dir()?;
        (staging_dir, true)
    } else {
        let skills_dir = get_skills_dir()?;
        let skill_dir = skills_dir.join(&skill_id);
        if !skill_dir.exists() {
            return Err(format!("Skill not found: {}", skill_id));
        }
        (skill_dir, false)
    };

    // For staging, ensure files exist
    if is_staging_mode(&skill_id) {
        fs::create_dir_all(&base_dir)
            .map_err(|e| format!("Failed to create staging directory: {}", e))?;

        // Create default SKILL.md if not exists
        let skill_md_path = base_dir.join("SKILL.md");
        if !skill_md_path.exists() {
            let default_frontmatter = SkillFrontmatter {
                name: "my-skill".to_string(),
                display_name: "My Skill".to_string(),
                description: "A helpful skill".to_string(),
                category: "utility".to_string(),
            };
            let default_content = format!("---\n{}\n---\n\nYou are a helpful skill.\n",
                serde_yaml::to_string(&default_frontmatter)
                    .map_err(|e| format!("Failed to serialize frontmatter: {}", e))?
            );
            fs::write(&skill_md_path, default_content)
                .map_err(|e| format!("Failed to write SKILL.md: {}", e))?;
        }

        // Create placeholder folders if not exist
        fs::create_dir_all(base_dir.join("assets"))
            .map_err(|e| format!("Failed to create assets directory: {}", e))?;
        fs::create_dir_all(base_dir.join("resources"))
            .map_err(|e| format!("Failed to create resources directory: {}", e))?;
        fs::create_dir_all(base_dir.join("scripts"))
            .map_err(|e| format!("Failed to create scripts directory: {}", e))?;
    }

    let mut files = Vec::new();

    // Recursively collect all files and folders
    fn collect_files(dir: &std::path::Path, base_path: &std::path::Path, relative_path: &str, files: &mut Vec<SkillFile>) -> Result<(), String> {
        let entries = fs::read_dir(dir)
            .map_err(|e| format!("Failed to read directory: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Skip hidden files
            if name.starts_with('.') {
                continue;
            }

            let metadata = match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let is_file = metadata.is_file();
            let is_binary = is_binary_file(&name);
            let is_protected = name == "SKILL.md";

            // Build the relative path
            let new_relative_path = if relative_path.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", relative_path, name)
            };

            files.push(SkillFile {
                name: name.clone(),
                path: new_relative_path.clone(),
                is_file,
                is_binary,
                is_protected,
                size: metadata.len(),
            });

            // If it's a directory, recurse into it
            if !is_file {
                collect_files(&path, base_path, &new_relative_path, files)?;
            }
        }
        Ok(())
    }

    collect_files(&base_dir, &base_dir, "", &mut files)?;

    // Sort: folders first, then alphabetically
    files.sort_by(|a, b| {
        if !a.is_file && b.is_file {
            return std::cmp::Ordering::Less;
        }
        if a.is_file && !b.is_file {
            return std::cmp::Ordering::Greater;
        }
        // Protected files first, then alphabetically
        if a.is_protected && !b.is_protected {
            return std::cmp::Ordering::Less;
        }
        if !a.is_protected && b.is_protected {
            return std::cmp::Ordering::Greater;
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });

    Ok(files)
}

/// Read a file's content from a skill folder or staging
#[tauri::command]
pub async fn read_skill_file(skill_id: String, file_path: String) -> Result<SkillFileContent, String> {
    let (base_dir, _) = if is_staging_mode(&skill_id) {
        (get_staging_dir()?, true)
    } else {
        let skills_dir = get_skills_dir()?;
        (skills_dir.join(&skill_id), false)
    };

    let full_path = base_dir.join(&file_path);

    if !full_path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    // Check if it's a binary file
    let is_binary = is_binary_file(&file_path);
    if is_binary {
        return Ok(SkillFileContent {
            content: String::new(),
            is_binary: true,
            is_markdown: false,
        });
    }

    let content = fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let is_markdown = file_path.ends_with(".md");

    Ok(SkillFileContent {
        content,
        is_binary: false,
        is_markdown,
    })
}

/// Write or create a file in a skill folder or staging
#[tauri::command]
pub async fn write_skill_file(skill_id: String, file_path: String, content: String) -> Result<(), String> {
    let (base_dir, _) = if is_staging_mode(&skill_id) {
        (get_staging_dir()?, true)
    } else {
        let skills_dir = get_skills_dir()?;
        (skills_dir.join(&skill_id), false)
    };

    let full_path = base_dir.join(&file_path);

    // Ensure parent directory exists
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {}", e))?;
    }

    fs::write(&full_path, content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

/// Create a folder in a skill directory or staging
#[tauri::command]
pub async fn create_skill_folder(skill_id: String, folder_path: String) -> Result<(), String> {
    let (base_dir, _) = if is_staging_mode(&skill_id) {
        (get_staging_dir()?, true)
    } else {
        let skills_dir = get_skills_dir()?;
        (skills_dir.join(&skill_id), false)
    };

    let full_path = base_dir.join(&folder_path);

    fs::create_dir_all(&full_path)
        .map_err(|e| format!("Failed to create folder: {}", e))?;

    Ok(())
}

/// Delete a file or folder from a skill directory or staging
#[tauri::command]
pub async fn delete_skill_file(skill_id: String, file_path: String) -> Result<(), String> {
    // Prevent deletion of protected files
    if file_path == "SKILL.md" {
        return Err("Cannot delete SKILL.md. It contains the skill's configuration and instructions.".to_string());
    }

    let (base_dir, _) = if is_staging_mode(&skill_id) {
        (get_staging_dir()?, true)
    } else {
        let skills_dir = get_skills_dir()?;
        (skills_dir.join(&skill_id), false)
    };

    let full_path = base_dir.join(&file_path);

    if !full_path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    if full_path.is_dir() {
        fs::remove_dir_all(&full_path)
            .map_err(|e| format!("Failed to delete folder: {}", e))?;
    } else {
        fs::remove_file(&full_path)
            .map_err(|e| format!("Failed to delete file: {}", e))?;
    }

    Ok(())
}

/// Parse YAML frontmatter from SKILL.md content
fn parse_skill_frontmatter(content: &str) -> Result<(SkillFrontmatter, String), String> {
    let frontmatter_regex = regex::Regex::new(r"^---\n([\s\S]*?)\n---\n([\s\S]*)$")
        .map_err(|e| format!("Failed to create regex: {}", e))?;

    let captures = frontmatter_regex.captures(content)
        .ok_or_else(|| "Invalid SKILL.md format: missing frontmatter".to_string())?;

    let yaml_content = captures.get(1).unwrap().as_str();
    let body = captures.get(2).unwrap().as_str();

    let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_content)
        .map_err(|e| format!("Failed to parse frontmatter: {}", e))?;

    // Trim leading newlines from body (the \n after ---)
    let body = body.trim_start_matches('\n').to_string();

    Ok((frontmatter, body))
}

/// Get skill metadata for system prompt generation
#[tauri::command]
pub async fn get_skill_metadata(id: String) -> Result<SkillMetadata, String> {
    let skills_dir = get_skills_dir()?;
    let skill_dir = skills_dir.join(&id);

    if !skill_dir.exists() {
        return Err(format!("Skill not found: {}", id));
    }

    let skill_md_path = skill_dir.join("SKILL.md");
    if !skill_md_path.exists() {
        return Err(format!("SKILL.md not found for skill: {}", id));
    }

    // Read SKILL.md
    let content = fs::read_to_string(&skill_md_path)
        .map_err(|e| format!("Failed to read SKILL.md: {}", e))?;

    // Parse frontmatter
    let (frontmatter, _body) = parse_skill_frontmatter(&content)?;

    // Get the full path to SKILL.md
    let skill_path = skill_md_path
        .to_str()
        .ok_or_else(|| "Failed to convert skill path to string".to_string())?
        .to_string();

    Ok(SkillMetadata {
        name: frontmatter.name,
        display_name: frontmatter.display_name,
        description: frontmatter.description,
        category: frontmatter.category,
        location: skill_path,
    })
}

/// Skill metadata for system prompt generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub location: String,
}

/// Check if a file is binary based on its extension
fn is_binary_file(filename: &str) -> bool {
    const BINARY_EXTENSIONS: &[&str] = &[
        "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
        "zip", "tar", "gz", "rar", "7z",
        "exe", "dll", "so", "dylib",
        "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp",
        "mp3", "mp4", "wav", "avi", "mov", "mkv",
        "ttf", "otf", "woff", "woff2",
    ];

    if let Some(ext) = filename.rsplit('.').next() {
        BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}
