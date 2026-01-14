# Skills Commands Module

## Overview

This module handles all skill-related operations in the Tauri backend. Skills extend AI agent capabilities with specialized instructions and can include supporting files. Each skill is stored as a folder containing `SKILL.md` (frontmatter + markdown instructions) and optional asset folders.

## Storage Structure

```
~/.config/zeroagent/skills/
├── skill-1/
│   └── SKILL.md          # YAML frontmatter + markdown instructions
│   ├── assets/           # Optional: images, diagrams
│   ├── resources/        # Optional: reference materials
│   └── scripts/          # Optional: executable scripts
└── skill-2/
    └── SKILL.md
    └── resources/
        └── reference.pdf
```

## SKILL.md Format

Skills use YAML frontmatter for metadata followed by markdown instructions:

```yaml
---
name: web-search
displayName: Web Search
description: Search the web for current information
category: research
---

You are a web search skill. When asked to search:
1. Formulate a clear search query
2. Execute the search
3. Summarize the top results
...
```

## Key Data Structures

### Skill (Public API)
```rust
pub struct Skill {
    pub id: Option<String>,          // Unique identifier (folder name)
    pub name: String,                 // URL-friendly name
    pub display_name: String,         // Human-readable name
    pub description: String,          // Short description
    pub category: String,             // Category (utility, coding, etc.)
    pub instructions: String,         // System instructions (markdown only)
    pub created_at: Option<String>,   // ISO timestamp
}
```

### SkillFile (File Explorer)
```rust
pub struct SkillFile {
    pub name: String,          // File/folder name
    pub path: String,          // Relative path from skill root
    pub is_file: bool,         // True = file, False = folder
    pub is_binary: bool,       // True = binary file (no preview)
    pub is_protected: bool,    // True = cannot be deleted (SKILL.md)
    pub size: u64,             // File size in bytes
}
```

## Key Functions

### Frontmatter Parsing

**Problem**: SKILL.md contains both YAML metadata and markdown instructions. Need to split and parse correctly.

**Solution**: Use regex to extract frontmatter and body:

```rust
fn parse_skill_frontmatter(content: &str) -> Result<(SkillFrontmatter, String), String> {
    let frontmatter_regex = Regex::new(r"^---\n(.+?)\n---\n(.*)$").unwrap();

    if let Some(captures) = frontmatter_regex.captures(content) {
        let yaml_str = captures.get(1).unwrap().as_str();
        let body = captures.get(2).unwrap().as_str();

        // Trim leading newlines from body (the \n after ---)
        let body = body.trim_start_matches('\n').to_string();

        let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_str)
            .map_err(|e| format!("Failed to parse frontmatter: {}", e))?;

        Ok((frontmatter, body))
    } else {
        // No frontmatter - treat entire content as instructions
        Ok((SkillFrontmatter::default(), content.to_string()))
    }
}
```

**Learnings**:
- Regex `^---\n(.+?)\n---\n(.*)$` captures frontmatter (non-greedy) and body
- Use `trim_start_matches('\n')` to remove extra newline after closing `---`
- Default frontmatter allows skills without metadata (graceful degradation)
- `\n` in regex matches platform line endings on Unix, use `\r?\n` for cross-platform

### Recursive File Scanning

**Problem**: Skills can have files in nested subdirectories (assets/, resources/, scripts/). Initial implementation only scanned root level.

**Solution**: Same pattern as agents - `collect_files` helper:

```rust
fn collect_files(dir: &PathBuf, base_path: &PathBuf, relative_path: &str, files: &mut Vec<SkillFile>) -> Result<(), String> {
    let entries = fs::read_dir(dir)?;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        let metadata = fs::metadata(&path)?;
        let is_file = metadata.is_file();

        // Build relative path
        let new_relative_path = if relative_path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", relative_path, name)
        };

        files.push(SkillFile { /* ... */ });

        // Recurse for directories
        if !is_file {
            collect_files(&path, base_path, &new_relative_path, files)?;
        }
    }
    Ok(())
}
```

**Learnings**:
- Reuse identical pattern from agents.rs for consistency
- Hidden file filtering is important (`.git`, `.DS_Store`, etc.)
- Path building is identical: accumulate relative path as we recurse

### Default Folder Creation

**Problem**: New skills should start with placeholder folders (assets, resources, scripts) for better UX.

**Solution**: Create folders during skill creation:

```rust
pub async fn create_skill(skill: Skill) -> Result<Skill, String> {
    let skills_dir = get_skills_dir()?;
    let skill_dir = skills_dir.join(&skill.name);

    // Create skill directory
    fs::create_dir_all(&skill_dir)?;

    // Create default placeholder folders
    fs::create_dir_all(skill_dir.join("assets"))?;
    fs::create_dir_all(skill_dir.join("resources"))?;
    fs::create_dir_all(skill_dir.join("scripts"))?;

    // Write SKILL.md with frontmatter + instructions
    let skill_md_content = format!(
        "---\n{}\n---\n\n{}\n",
        serde_yaml::to_string(&SkillFrontmatter { /* ... */ })?,
        skill.instructions
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md_content)?;

    Ok(Skill { /* ... */ })
}
```

**Learnings**:
- `create_dir_all` is safe - no error if directory exists
- Placeholders guide users toward expected structure
- Don't create `.gitkeep` - empty folders work fine on most systems

### Protected Files

**Problem**: `SKILL.md` should not be deletable as it contains essential skill metadata.

**Solution**: Mark as protected in file listing and enforce in delete command:

```rust
// When listing files
let is_protected = name == "SKILL.md";

// When deleting files
pub async fn delete_skill_file(skill_id: String, file_path: String) -> Result<(), String> {
    if file_path == "SKILL.md" {
        return Err("Cannot delete SKILL.md. It contains the skill's metadata and instructions.".to_string());
    }
    // ... proceed with deletion
}
```

**Learnings**:
- Single protected file for skills (vs 2 for agents)
- Clear error message explains importance of file
- Frontend should also hide delete button for consistency

### Category System

**Problem**: Skills need to be categorized for filtering and organization.

**Solution**: Define fixed set of categories with defaults:

```rust
const SKILL_CATEGORIES: &[&str] = &[
    "utility", "coding", "writing", "analysis",
    "communication", "productivity", "research",
    "creative", "automation", "other",
];

// Frontend enum matches backend
pub enum SkillCategory {
    Utility,
    Coding,
    Writing,
    // ...
}
```

**Learnings**:
- Fixed categories enable consistent filtering UI
- "other" category catches uncategorized skills
- Categories can be extended over time (additive changes)

## Error Handling Pattern

```rust
#[tauri::command]
pub async fn read_skill_file(skill_id: String, file_path: String) -> Result<SkillFileContent, String> {
    let skills_dir = get_skills_dir()?;
    let skill_dir = skills_dir.join(&skill_id);
    let full_path = skill_dir.join(&file_path);

    if !full_path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    // Binary file check
    if is_binary_file(&file_path) {
        return Ok(SkillFileContent {
            content: String::new(),
            is_binary: true,
            is_markdown: false,
        });
    }

    let content = fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    Ok(SkillFileContent { content, is_binary: false, is_markdown: file_path.ends_with(".md") })
}
```

**Learnings**:
- Always validate skill directory exists before accessing files
- Early return for binary files (don't try to read as text)
- Detect markdown by extension for frontend hinting

## Commands Reference

| Command | Description |
|---------|-------------|
| `list_skills()` | List all skills |
| `get_skill(id)` | Get single skill by ID |
| `create_skill(skill)` | Create new skill with folders |
| `update_skill(id, skill)` | Update existing skill |
| `delete_skill(id)` | Delete skill (removes folder) |
| `list_skill_files(id)` | List files in skill folder (recursive) |
| `read_skill_file(id, path)` | Read file content |
| `write_skill_file(id, path, content)` | Write/create file |
| `create_skill_folder(id, path)` | Create folder |
| `delete_skill_file(id, path)` | Delete file/folder |

## Differences from Agents Module

| Aspect | Agents | Skills |
|--------|--------|--------|
| **Main File** | `AGENTS.md` (plain markdown) | `SKILL.md` (frontmatter + markdown) |
| **Config** | Separate `config.yaml` | Embedded in frontmatter |
| **Protected Files** | 2 (config.yaml, AGENTS.md) | 1 (SKILL.md) |
| **Default Folders** | None | assets/, resources/, scripts/ |
| **Categorization** | None | Fixed categories |

## Future Considerations

1. **Skill Marketplace**: Share skills as ZIP files with metadata
2. **Versioning**: Track skill versions for compatibility
3. **Dependencies**: Skills could depend on other skills
4. **Validation**: Schema validation for frontmatter fields
5. **Testing**: Test harness for skill execution
