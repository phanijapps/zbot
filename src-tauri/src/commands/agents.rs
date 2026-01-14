// ============================================================================
// AGENTS COMMANDS
// AI agent management with folder-based storage
// ============================================================================

use crate::settings::AppDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Agent data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub model: String,
    pub temperature: f64,
    #[serde(rename = "maxTokens", default = "default_max_tokens")]
    pub max_tokens: u32,
    pub instructions: String,
    pub mcps: Vec<String>,
    pub skills: Vec<String>,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Default value for maxTokens
fn default_max_tokens() -> u32 {
    2000
}

/// Agent configuration stored in config.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentConfig {
    name: String,
    #[serde(rename = "displayName")]
    display_name: String,
    description: String,
    #[serde(rename = "providerId")]
    provider_id: String,
    model: String,
    temperature: f64,
    #[serde(rename = "maxTokens", default = "default_max_tokens")]
    max_tokens: u32,
    skills: Vec<String>,
    mcps: Vec<String>,
}

/// Gets the agents directory path
fn get_agents_dir() -> Result<PathBuf, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    Ok(dirs.config_dir.join("agents"))
}

/// Gets the staging directory for new agents
fn get_staging_dir() -> Result<PathBuf, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    Ok(dirs.config_dir.join("staging"))
}

/// Lists all agents from the agents directory
#[tauri::command]
pub async fn list_agents() -> Result<Vec<Agent>, String> {
    let agents_dir = get_agents_dir()?;

    if !agents_dir.exists() {
        return Ok(vec![]);
    }

    let mut agents = Vec::new();

    // Iterate through subdirectories in agents directory
    let entries = fs::read_dir(&agents_dir)
        .map_err(|e| format!("Failed to read agents directory: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip if not a directory
        if !path.is_dir() {
            continue;
        }

        // Look for config.yaml file
        let config_yaml = path.join("config.yaml");
        if !config_yaml.exists() {
            continue;
        }

        // Read and parse agent folder
        if let Ok(agent) = read_agent_folder(&path) {
            agents.push(agent);
        }
    }

    Ok(agents)
}

/// Gets a single agent by ID
#[tauri::command]
pub async fn get_agent(id: String) -> Result<Agent, String> {
    let agents_dir = get_agents_dir()?;
    let agent_dir = agents_dir.join(&id);

    if !agent_dir.exists() {
        return Err(format!("Agent not found: {}", id));
    }

    read_agent_folder(&agent_dir)
}

/// Creates a new agent
#[tauri::command]
pub async fn create_agent(agent: Agent) -> Result<Agent, String> {
    let agents_dir = get_agents_dir()?;

    // Ensure agents directory exists
    fs::create_dir_all(&agents_dir)
        .map_err(|e| format!("Failed to create agents directory: {}", e))?;

    // Create agent directory
    let agent_dir = agents_dir.join(&agent.name);
    fs::create_dir_all(&agent_dir)
        .map_err(|e| format!("Failed to create agent directory: {}", e))?;

    // Write config.yaml
    let config = AgentConfig {
        name: agent.name.clone(),
        display_name: agent.display_name.clone(),
        description: agent.description.clone(),
        provider_id: agent.provider_id.clone(),
        model: agent.model.clone(),
        temperature: agent.temperature,
        max_tokens: agent.max_tokens,
        skills: agent.skills.clone(),
        mcps: agent.mcps.clone(),
    };
    let config_yaml = serde_yaml::to_string(&config)
        .map_err(|e| format!("Failed to serialize config.yaml: {}", e))?;
    fs::write(agent_dir.join("config.yaml"), config_yaml)
        .map_err(|e| format!("Failed to write config.yaml: {}", e))?;

    // Write AGENTS.md (just the instructions, no frontmatter)
    let agents_md_content = format!("{}\n", agent.instructions);
    fs::write(agent_dir.join("AGENTS.md"), agents_md_content)
        .map_err(|e| format!("Failed to write AGENTS.md: {}", e))?;

    // Clear staging if exists
    let staging_dir = get_staging_dir()?;
    let staging_config = staging_dir.join("config.yaml");
    let staging_agents = staging_dir.join("AGENTS.md");
    if staging_config.exists() {
        let _ = fs::remove_file(&staging_config);
    }
    if staging_agents.exists() {
        let _ = fs::remove_file(&staging_agents);
    }

    // Return the created agent
    Ok(Agent {
        id: agent.name.clone(),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
        ..agent
    })
}

/// Updates an existing agent
#[tauri::command]
pub async fn update_agent(id: String, agent: Agent) -> Result<Agent, String> {
    let agents_dir = get_agents_dir()?;
    let agent_dir = agents_dir.join(&id);

    if !agent_dir.exists() {
        return Err(format!("Agent not found: {}", id));
    }

    // If name changed, rename directory
    if agent.name != id {
        let new_dir = agents_dir.join(&agent.name);
        fs::rename(&agent_dir, &new_dir)
            .map_err(|e| format!("Failed to rename agent directory: {}", e))?;
    }

    // Use the new directory name if changed
    let target_dir = agents_dir.join(&agent.name);

    // Write config.yaml
    let config = AgentConfig {
        name: agent.name.clone(),
        display_name: agent.display_name.clone(),
        description: agent.description.clone(),
        provider_id: agent.provider_id.clone(),
        model: agent.model.clone(),
        temperature: agent.temperature,
        max_tokens: agent.max_tokens,
        skills: agent.skills.clone(),
        mcps: agent.mcps.clone(),
    };
    let config_yaml = serde_yaml::to_string(&config)
        .map_err(|e| format!("Failed to serialize config.yaml: {}", e))?;
    fs::write(target_dir.join("config.yaml"), config_yaml)
        .map_err(|e| format!("Failed to write config.yaml: {}", e))?;

    // Write AGENTS.md (just the instructions, no frontmatter)
    let agents_md_content = format!("{}\n", agent.instructions);
    fs::write(target_dir.join("AGENTS.md"), agents_md_content)
        .map_err(|e| format!("Failed to write AGENTS.md: {}", e))?;

    Ok(agent)
}

/// Deletes an agent by removing its directory
#[tauri::command]
pub async fn delete_agent(id: String) -> Result<(), String> {
    let agents_dir = get_agents_dir()?;
    let agent_path = agents_dir.join(&id);

    if !agent_path.exists() {
        return Err(format!("Agent not found: {}", id));
    }

    fs::remove_dir_all(&agent_path)
        .map_err(|e| format!("Failed to delete agent directory: {}", e))?;

    Ok(())
}

/// Reads an agent folder and parses config.yaml and AGENTS.md
fn read_agent_folder(agent_dir: &PathBuf) -> Result<Agent, String> {
    let config_path = agent_dir.join("config.yaml");
    let agents_md_path = agent_dir.join("AGENTS.md");

    if !config_path.exists() {
        return Err(format!("config.yaml not found in {:?}", agent_dir));
    }

    if !agents_md_path.exists() {
        return Err(format!("AGENTS.md not found in {:?}", agent_dir));
    }

    // Read config.yaml
    let config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.yaml: {}", e))?;
    let config: AgentConfig = serde_yaml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config.yaml: {}", e))?;

    // Read AGENTS.md (just the instructions)
    let instructions = fs::read_to_string(&agents_md_path)
        .map_err(|e| format!("Failed to read AGENTS.md: {}", e))?;

    // Get agent name from directory name
    let name = agent_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(Agent {
        id: name.clone(),
        name,
        display_name: config.display_name,
        description: config.description,
        provider_id: config.provider_id,
        model: config.model,
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        instructions,
        mcps: config.mcps,
        skills: config.skills,
        created_at: Some("1970-01-01T00:00:00Z".to_string()), // TODO: get from file metadata
    })
}

/// File entry in agent folder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFile {
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
pub struct AgentFileContent {
    pub content: String,
    #[serde(rename = "isBinary")]
    pub is_binary: bool,
    #[serde(rename = "isMarkdown")]
    pub is_markdown: bool,
}

/// Check if we're in staging mode (creating new agent)
fn is_staging_mode(agent_id: &str) -> bool {
    agent_id == "staging" || agent_id == "temp"
}

/// Recursively collect files from a directory
fn collect_files(dir: &PathBuf, base_path: &PathBuf, relative_path: &str, files: &mut Vec<AgentFile>) -> Result<(), String> {
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
        let is_protected = name == "config.yaml" || name == "AGENTS.md";

        // Build the relative path
        let new_relative_path = if relative_path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", relative_path, name)
        };

        files.push(AgentFile {
            name: name.clone(),
            path: new_relative_path.clone(),
            is_file,
            is_binary,
            is_protected,
            size: metadata.len(),
        });

        // Recursively scan subdirectories
        if !is_file {
            collect_files(&path, base_path, &new_relative_path, files)?;
        }
    }

    Ok(())
}

/// List files in an agent folder or staging
#[tauri::command]
pub async fn list_agent_files(agent_id: String) -> Result<Vec<AgentFile>, String> {
    let (base_dir, is_staging) = if is_staging_mode(&agent_id) {
        let staging_dir = get_staging_dir()?;
        (staging_dir, true)
    } else {
        let agents_dir = get_agents_dir()?;
        let agent_dir = agents_dir.join(&agent_id);
        if !agent_dir.exists() {
            return Err(format!("Agent not found: {}", agent_id));
        }
        (agent_dir, false)
    };

    // For staging, ensure files exist
    if is_staging {
        fs::create_dir_all(&base_dir)
            .map_err(|e| format!("Failed to create staging directory: {}", e))?;

        // Create default config.yaml if not exists
        let config_path = base_dir.join("config.yaml");
        if !config_path.exists() {
            let default_config = AgentConfig {
                name: "my-agent".to_string(),
                display_name: "My Agent".to_string(),
                description: "A helpful AI assistant".to_string(),
                provider_id: "".to_string(),
                model: "".to_string(),
                temperature: 0.7,
                max_tokens: 2000,
                skills: vec![],
                mcps: vec![],
            };
            let config_yaml = serde_yaml::to_string(&default_config)
                .map_err(|e| format!("Failed to serialize config.yaml: {}", e))?;
            fs::write(&config_path, config_yaml)
                .map_err(|e| format!("Failed to write config.yaml: {}", e))?;
        }

        // Create default AGENTS.md if not exists
        let agents_md_path = base_dir.join("AGENTS.md");
        if !agents_md_path.exists() {
            fs::write(&agents_md_path, "You are a helpful AI assistant.\n")
                .map_err(|e| format!("Failed to write AGENTS.md: {}", e))?;
        }
    }

    let mut files = Vec::new();
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

/// Read a file's content from an agent folder or staging
#[tauri::command]
pub async fn read_agent_file(agent_id: String, file_path: String) -> Result<AgentFileContent, String> {
    let (base_dir, _) = if is_staging_mode(&agent_id) {
        (get_staging_dir()?, true)
    } else {
        let agents_dir = get_agents_dir()?;
        (agents_dir.join(&agent_id), false)
    };

    let full_path = base_dir.join(&file_path);

    if !full_path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    // Check if it's a binary file
    let is_binary = is_binary_file(&file_path);
    if is_binary {
        return Ok(AgentFileContent {
            content: String::new(),
            is_binary: true,
            is_markdown: false,
        });
    }

    let content = fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let is_markdown = file_path.ends_with(".md");

    Ok(AgentFileContent {
        content,
        is_binary: false,
        is_markdown,
    })
}

/// Write or create a file in an agent folder or staging
#[tauri::command]
pub async fn write_agent_file(agent_id: String, file_path: String, content: String) -> Result<(), String> {
    // Prevent writing to protected files through this method
    if file_path == "config.yaml" {
        return Err("Cannot modify config.yaml directly. Use the agent configuration form.".to_string());
    }

    let (base_dir, _) = if is_staging_mode(&agent_id) {
        (get_staging_dir()?, true)
    } else {
        let agents_dir = get_agents_dir()?;
        (agents_dir.join(&agent_id), false)
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

/// Create a folder in an agent directory or staging
#[tauri::command]
pub async fn create_agent_folder(agent_id: String, folder_path: String) -> Result<(), String> {
    let (base_dir, _) = if is_staging_mode(&agent_id) {
        (get_staging_dir()?, true)
    } else {
        let agents_dir = get_agents_dir()?;
        (agents_dir.join(&agent_id), false)
    };

    let full_path = base_dir.join(&folder_path);

    fs::create_dir_all(&full_path)
        .map_err(|e| format!("Failed to create folder: {}", e))?;

    Ok(())
}

/// Delete a file or folder from an agent directory or staging
#[tauri::command]
pub async fn delete_agent_file(agent_id: String, file_path: String) -> Result<(), String> {
    // Prevent deletion of protected files
    if file_path == "config.yaml" {
        return Err("Cannot delete config.yaml. It contains the agent's configuration.".to_string());
    }
    if file_path == "AGENTS.md" {
        return Err("Cannot delete AGENTS.md. It contains the agent's instructions.".to_string());
    }

    let (base_dir, _) = if is_staging_mode(&agent_id) {
        (get_staging_dir()?, true)
    } else {
        let agents_dir = get_agents_dir()?;
        (agents_dir.join(&agent_id), false)
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

/// Upload/copy a file to an agent folder or staging
#[tauri::command]
pub async fn upload_agent_file(agent_id: String, source_path: String, dest_path: String) -> Result<(), String> {
    let (base_dir, _) = if is_staging_mode(&agent_id) {
        (get_staging_dir()?, true)
    } else {
        let agents_dir = get_agents_dir()?;
        (agents_dir.join(&agent_id), false)
    };

    let dest_full = base_dir.join(&dest_path);

    // Ensure parent directory exists
    if let Some(parent) = dest_full.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {}", e))?;
    }

    fs::copy(&source_path, &dest_full)
        .map_err(|e| format!("Failed to copy file: {}", e))?;

    Ok(())
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
