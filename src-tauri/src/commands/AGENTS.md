# Agent Commands Module

## Overview

This module handles all agent-related operations in the Tauri backend. Agents are AI assistants with specific instructions, providers, models, and optional MCP servers and skills. Each agent is stored as a folder containing `config.yaml` (metadata) and `AGENTS.md` (instructions).

## Storage Structure

```
~/.config/zeroagent/agents/
├── agent-1/
│   ├── config.yaml       # Agent metadata (name, provider, model, etc.)
│   ├── AGENTS.md         # Agent instructions (markdown)
│   └── [user files]      # Additional files in subfolders
└── agent-2/
    ├── config.yaml
    ├── AGENTS.md
    └── assets/
        └── image.png
```

## Key Data Structures

### Agent (Public API)
```rust
pub struct Agent {
    pub id: Option<String>,          // Unique identifier (folder name)
    pub name: String,                 // URL-friendly name
    pub display_name: String,         // Human-readable name
    pub description: String,          // Short description
    pub provider_id: String,          // Reference to provider
    pub model: String,                // Model identifier
    pub temperature: f64,             // LLM temperature (0.0-1.0)
    pub max_tokens: u32,              // Max tokens in response
    pub instructions: String,         // System instructions
    pub mcps: Vec<String>,            // MCP server IDs
    pub skills: Vec<String>,          // Skill IDs
    pub created_at: Option<String>,   // ISO timestamp
}
```

### AgentFile (File Explorer)
```rust
pub struct AgentFile {
    pub name: String,          // File/folder name
    pub path: String,          // Relative path (e.g., "assets/image.png")
    pub is_file: bool,         // True = file, False = folder
    pub is_binary: bool,       // True = binary file (no preview)
    pub is_protected: bool,    // True = cannot be deleted
    pub size: u64,             // File size in bytes
}
```

## Key Functions

### Recursive File Scanning

**Problem**: Initial implementation only scanned files at root level of agent folder. Files in subdirectories were not visible in the file explorer.

**Solution**: Added `collect_files` helper that recursively traverses all subdirectories:

```rust
fn collect_files(dir: &PathBuf, base_path: &PathBuf, relative_path: &str, files: &mut Vec<AgentFile>) -> Result<(), String> {
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

        // Build relative path (e.g., "assets/image.png")
        let new_relative_path = if relative_path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", relative_path, name)
        };

        files.push(AgentFile { /* ... */ });

        // Recursively scan subdirectories
        if !is_file {
            collect_files(&path, base_path, &new_relative_path, files)?;
        }
    }
    Ok(())
}
```

**Learnings**:
- Recursive functions need `Result<>` return type to propagate errors
- Use `fs::read_dir()` with `.flatten()` to handle permission errors gracefully
- Always skip hidden files (starting with `.`) to avoid system files
- Build relative path incrementally: `folder/subfolder/file.ext`

### Staging Mode

**Problem**: New agents need a place to store files before the agent is officially created. If user cancels creation, files should be discarded.

**Solution**: Use a special "staging" directory that's cleaned up on save or cancel:

```rust
fn is_staging_mode(agent_id: &str) -> bool {
    agent_id == "staging" || agent_id == "temp"
}

pub async fn list_agent_files(agent_id: String) -> Result<Vec<AgentFile>, String> {
    let (base_dir, is_staging) = if is_staging_mode(&agent_id) {
        (get_staging_dir()?, true)
    } else {
        (agents_dir.join(&agent_id), false)
    };

    // For staging, ensure default files exist
    if is_staging {
        fs::create_dir_all(&base_dir)?;
        // Create default config.yaml, AGENTS.md if not exists
    }
    // ...
}
```

**Learnings**:
- Use multiple identifiers (`"staging"`, `"temp"`) for flexibility
- Create default files lazily (only if they don't exist)
- Staging directory is separate from agents directory to avoid pollution

### Protected Files

**Problem**: System files (`config.yaml`, `AGENTS.md`) should not be deletable through the file explorer.

**Solution**: Mark files as protected and enforce checks:

```rust
// When listing files
let is_protected = name == "config.yaml" || name == "AGENTS.md";

// When deleting files
pub async fn delete_agent_file(agent_id: String, file_path: String) -> Result<(), String> {
    if file_path == "config.yaml" {
        return Err("Cannot delete config.yaml...".to_string());
    }
    if file_path == "AGENTS.md" {
        return Err("Cannot delete AGENTS.md...".to_string());
    }
    // ... proceed with deletion
}
```

**Learnings**:
- Protect on both frontend (hide delete button) and backend (return error)
- Provide clear error messages explaining why deletion is blocked
- Protected files are identified by name, not path (simpler)

### Binary File Detection

**Problem**: Some file types (images, PDFs, etc.) cannot be displayed as text and should be marked as binary.

**Solution**: Check file extension against known binary extensions:

```rust
fn is_binary_file(filename: &str) -> bool {
    const BINARY_EXTENSIONS: &[&str] = &[
        "pdf", "doc", "docx", "xls", "xlsx",
        "zip", "tar", "gz",
        "png", "jpg", "jpeg", "gif", "webp",
        "mp3", "mp4", "wav",
        // ...
    ];

    if let Some(ext) = filename.rsplit('.').next() {
        BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}
```

**Learnings**:
- Use `rsplit('.')` to get extension (handles multiple dots correctly)
- Case-insensitive comparison for cross-platform compatibility
- Return false for files without extensions (assume text)

## Error Handling Pattern

All commands use `Result<T, String>` for error handling:

```rust
#[tauri::command]
pub async fn read_agent_file(agent_id: String, file_path: String) -> Result<AgentFileContent, String> {
    let full_path = base_dir.join(&file_path);

    if !full_path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    let content = fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    Ok(AgentFileContent { content, ... })
}
```

**Learnings**:
- Use `?` operator to propagate errors with context
- `map_err()` converts std errors to String with descriptive messages
- Always validate file existence before operations

## Commands Reference

| Command | Description |
|---------|-------------|
| `list_agents()` | List all agents |
| `get_agent(id)` | Get single agent by ID |
| `create_agent(agent)` | Create new agent |
| `update_agent(id, agent)` | Update existing agent |
| `delete_agent(id)` | Delete agent (removes folder) |
| `list_agent_files(id)` | List files in agent folder |
| `read_agent_file(id, path)` | Read file content |
| `write_agent_file(id, path, content)` | Write/create file |
| `create_agent_folder(id, path)` | Create folder |
| `delete_agent_file(id, path)` | Delete file/folder |

## Future Considerations

1. **File Watching**: Could notify frontend when files change externally
2. **Search**: Add full-text search across agent instructions
3. **Import/Export**: Allow packaging agents as shareable ZIP files
4. **Versioning**: Track changes to agent instructions over time
