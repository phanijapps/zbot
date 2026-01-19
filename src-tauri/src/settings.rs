// ============================================================================
// SETTINGS MODULE
// Manages application configuration, directory structure, and persistence
// ============================================================================

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application settings structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Appearance settings
    pub appearance: AppearanceSettings,
    /// Performance settings
    pub performance: PerformanceSettings,
    /// Notification settings
    pub notifications: NotificationSettings,
    /// Privacy settings
    pub privacy: PrivacySettings,
    /// Default provider settings
    pub default_provider: String,
}

/// Appearance settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceSettings {
    /// Dark mode enabled
    pub dark_mode: bool,
    /// Theme selection (default, purple, blue, green)
    pub theme: String,
    /// Font size (small, medium, large)
    pub font_size: String,
}

/// Performance settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    /// Hardware acceleration enabled
    pub hardware_acceleration: bool,
    /// Stream responses as they generate
    pub stream_responses: bool,
}

/// Notification settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// Desktop notifications enabled
    pub desktop_notifications: bool,
    /// Sound effects enabled
    pub sound_effects: bool,
}

/// Privacy settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySettings {
    /// Save chat history locally
    pub save_chat_history: bool,
    /// Analytics enabled
    pub analytics: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            appearance: AppearanceSettings {
                dark_mode: true,
                theme: "default".to_string(),
                font_size: "medium".to_string(),
            },
            performance: PerformanceSettings {
                hardware_acceleration: true,
                stream_responses: true,
            },
            notifications: NotificationSettings {
                desktop_notifications: true,
                sound_effects: false,
            },
            privacy: PrivacySettings {
                save_chat_history: true,
                analytics: false,
            },
            default_provider: "openai".to_string(),
        }
    }
}

/// Application directory structure manager
pub struct AppDirs {
    /// Config directory (~/.config/zeroagent on Linux)
    pub config_dir: PathBuf,
    /// Settings file path
    pub settings_file: PathBuf,
    /// LanceDB database path (legacy, kept for compatibility)
    pub database_path: PathBuf,
    /// Agents directory (configs)
    pub agents_dir: PathBuf,
    /// Agents data directory (attachments, documents, archives)
    pub agents_data_dir: PathBuf,
    /// Database directory for Agent Channel SQLite database
    pub db_dir: PathBuf,
    /// Skills directory
    pub skills_dir: PathBuf,
    /// Python virtual environment directory
    pub venv_dir: PathBuf,
    /// Conversation logs directory (logs/<conv-id>/) - legacy
    pub conversation_logs_dir: PathBuf,
    /// Outputs directory (~/Documents/ZeroAgent/outputs/)
    pub outputs_dir: PathBuf,
}

impl AppDirs {
    /// Get the application directories for the current platform
    pub fn get() -> Result<Self> {
        let config_dir = Self::get_config_dir()?;

        // Get documents directory for outputs
        let documents_dir = dirs::document_dir()
            .unwrap_or_else(|| config_dir.clone());
        let outputs_dir = documents_dir.join("ZeroAgent").join("outputs");

        Ok(Self {
            settings_file: config_dir.join("settings.yaml"),
            database_path: config_dir.join("zero_lance.db"),
            agents_dir: config_dir.join("agents"),
            agents_data_dir: config_dir.join("agents_data"),
            db_dir: config_dir.join("db"),
            skills_dir: config_dir.join("skills"),
            venv_dir: config_dir.join("venv"),
            conversation_logs_dir: config_dir.join("logs"),
            outputs_dir,
            config_dir,
        })
    }

    /// Get the config directory based on the platform
    fn get_config_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config directory")?
            .join("zeroagent");

        Ok(config_dir)
    }

    /// Initialize all application directories and files
    pub fn initialize(&self) -> Result<()> {
        // Create config directory
        fs::create_dir_all(&self.config_dir)
            .context("Failed to create config directory")?;

        // Create agents directory
        fs::create_dir_all(&self.agents_dir)
            .context("Failed to create agents directory")?;

        // Create agents data directory
        fs::create_dir_all(&self.agents_data_dir)
            .context("Failed to create agents data directory")?;

        // Create database directory
        fs::create_dir_all(&self.db_dir)
            .context("Failed to create database directory")?;

        // Create skills directory
        fs::create_dir_all(&self.skills_dir)
            .context("Failed to create skills directory")?;

        // Create venv directory
        fs::create_dir_all(&self.venv_dir)
            .context("Failed to create venv directory")?;

        // Create conversation logs directory
        fs::create_dir_all(&self.conversation_logs_dir)
            .context("Failed to create conversation logs directory")?;

        // Create outputs directory
        fs::create_dir_all(&self.outputs_dir)
            .context("Failed to create outputs directory")?;

        // Create LanceDB database file if it doesn't exist
        if !self.database_path.exists() {
            self.initialize_database()?;
        }

        // Create Python venv if Python is available and venv doesn't exist
        self.initialize_python_venv()?;

        Ok(())
    }

    /// Initialize LanceDB database
    fn initialize_database(&self) -> Result<()> {
        // For now, create an empty SQLite database file as a placeholder
        // LanceDB will properly initialize it when first used
        fs::File::create(&self.database_path)
            .context("Failed to create database file")?;

        println!("Created LanceDB database at: {:?}", self.database_path);
        Ok(())
    }

    /// Initialize Python virtual environment if Python is available
    fn initialize_python_venv(&self) -> Result<()> {
        // Check if venv already exists
        let venv_marker = self.venv_dir.join("pyvenv.cfg");
        if venv_marker.exists() {
            return Ok(());
        }

        // Check if python3 or python command exists
        let python_cmd = self.find_python_command();

        if let Some(cmd) = python_cmd {
            println!("Found Python at: {}, creating venv...", cmd);

            // Create venv using the detected Python
            let output = std::process::Command::new(&cmd)
                .arg("-m")
                .arg("venv")
                .arg(&self.venv_dir)
                .output();

            match output {
                Ok(result) => {
                    if result.status.success() {
                        println!("Python venv created successfully at: {:?}", self.venv_dir);
                    } else {
                        let stderr = String::from_utf8_lossy(&result.stderr);
                        eprintln!("Failed to create Python venv: {}", stderr);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to execute Python command: {}", e);
                }
            }
        } else {
            println!("Python not found, skipping venv creation");
        }

        Ok(())
    }

    /// Find available Python command (python3 or python)
    fn find_python_command(&self) -> Option<String> {
        // Try python3 first, then python, then python3.13
        let candidates = vec!["python3", "python", "python3.13"];

        for cmd in candidates {
            // Check if command exists by trying to run it with --version
            // Note: Python outputs version to stderr, not stdout
            if let Ok(result) = std::process::Command::new(cmd)
                .arg("--version")
                .stderr(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .output()
            {
                if result.status.success() {
                    // Python outputs to stderr, check both streams
                    let version = if result.stderr.is_empty() {
                        String::from_utf8_lossy(&result.stdout).to_string()
                    } else {
                        String::from_utf8_lossy(&result.stderr).to_string()
                    };
                    println!("Found Python: {} ({})", cmd, version.trim());
                    return Some(cmd.to_string());
                }
            }
        }

        None
    }

    /// Load settings from the settings file
    pub fn load_settings(&self) -> Result<Settings> {
        if !self.settings_file.exists() {
            // Return default settings if file doesn't exist
            return Ok(Settings::default());
        }

        let content = fs::read_to_string(&self.settings_file)
            .context("Failed to read settings file")?;

        let settings: Settings = serde_yaml::from_str(&content)
            .context("Failed to parse settings YAML")?;

        Ok(settings)
    }

    /// Save settings to the settings file
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.settings_file.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create settings directory")?;
        }

        let yaml = serde_yaml::to_string(settings)
            .context("Failed to serialize settings")?;

        fs::write(&self.settings_file, yaml)
            .context("Failed to write settings file")?;

        Ok(())
    }

    /// Reset settings to defaults
    pub fn reset_settings(&self) -> Result<()> {
        let default_settings = Settings::default();
        self.save_settings(&default_settings)
    }

    /// Get storage usage information
    pub fn get_storage_info(&self) -> Result<StorageInfo> {
        let total_used = Self::get_dir_size(&self.config_dir)?;

        // Calculate size of individual components
        let database_size = if self.database_path.exists() {
            Self::get_file_size(&self.database_path)?
        } else {
            0
        };

        let agents_size = Self::get_dir_size(&self.agents_dir)?;
        let skills_size = Self::get_dir_size(&self.skills_dir)?;

        Ok(StorageInfo {
            total_used,
            database_size,
            agents_size,
            skills_size,
        })
    }

    /// Clear all data (except settings)
    pub fn clear_all_data(&self) -> Result<()> {
        // Remove database
        if self.database_path.exists() {
            fs::remove_file(&self.database_path)
                .context("Failed to remove database")?;
        }

        // Remove agents directory
        if self.agents_dir.exists() {
            fs::remove_dir_all(&self.agents_dir)
                .context("Failed to remove agents directory")?;
            fs::create_dir_all(&self.agents_dir)
                .context("Failed to recreate agents directory")?;
        }

        // Remove skills directory
        if self.skills_dir.exists() {
            fs::remove_dir_all(&self.skills_dir)
                .context("Failed to remove skills directory")?;
            fs::create_dir_all(&self.skills_dir)
                .context("Failed to recreate skills directory")?;
        }

        Ok(())
    }

    /// Get the size of a directory in bytes
    fn get_dir_size(path: &PathBuf) -> Result<u64> {
        let mut total = 0u64;

        if path.exists() {
            let entries = fs::read_dir(path)
                .context("Failed to read directory")?;

            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    total += Self::get_file_size(&path)?;
                } else if path.is_dir() {
                    total += Self::get_dir_size(&path)?;
                }
            }
        }

        Ok(total)
    }

    /// Get the size of a file in bytes
    fn get_file_size(path: &PathBuf) -> Result<u64> {
        Ok(fs::metadata(path)
            .context("Failed to get file metadata")?
            .len())
    }

    /// Get the directory path for a specific conversation
    pub fn conversation_dir(&self, conversation_id: &str) -> PathBuf {
        self.conversation_logs_dir.join(conversation_id)
    }

    /// Create the directory structure for a conversation
    /// Creates: logs/<conv-id>/, logs/<conv-id>/scratchpad/, logs/<conv-id>/attachments/
    pub fn create_conversation_dir(&self, conversation_id: &str) -> Result<()> {
        let conv_dir = self.conversation_dir(conversation_id);

        // Create main conversation directory
        fs::create_dir_all(&conv_dir)
            .context("Failed to create conversation directory")?;

        // Create scratchpad directory
        let scratchpad_dir = conv_dir.join("scratchpad");
        fs::create_dir_all(&scratchpad_dir)
            .context("Failed to create scratchpad directory")?;

        // Create attachments directory
        let attachments_dir = conv_dir.join("attachments");
        fs::create_dir_all(&attachments_dir)
            .context("Failed to create attachments directory")?;

        // Create empty memory.md file
        let memory_file = conv_dir.join("memory.md");
        if !memory_file.exists() {
            fs::write(&memory_file, "")
                .context("Failed to create memory.md")?;
        }

        Ok(())
    }

    /// Delete the directory for a specific conversation
    pub fn delete_conversation_dir(&self, conversation_id: &str) -> Result<()> {
        let conv_dir = self.conversation_dir(conversation_id);

        if conv_dir.exists() {
            fs::remove_dir_all(&conv_dir)
                .context("Failed to remove conversation directory")?;
        }

        Ok(())
    }

    // =========================================================================
    // Agent Data Directory Helpers (Agent Channel Model)
    // =========================================================================

    /// Get the data directory for a specific agent
    pub fn agent_data_dir(&self, agent_id: &str) -> PathBuf {
        self.agents_data_dir.join(agent_id)
    }

    /// Get the attachments directory for a specific agent
    /// Organized by month: agents_data/{agent_id}/attachments/YYYY-MM/
    pub fn agent_attachments_dir(&self, agent_id: &str) -> PathBuf {
        self.agent_data_dir(agent_id).join("attachments")
    }

    /// Get the attachments directory for a specific agent and month
    pub fn agent_attachments_month_dir(&self, agent_id: &str, year_month: &str) -> PathBuf {
        self.agent_attachments_dir(agent_id).join(year_month)
    }

    /// Get the documents directory for a specific agent
    pub fn agent_documents_dir(&self, agent_id: &str) -> PathBuf {
        self.agent_data_dir(agent_id).join("documents")
    }

    /// Get the knowledge graph directory for a specific agent
    pub fn agent_knowledge_graph_dir(&self, agent_id: &str) -> PathBuf {
        self.agent_data_dir(agent_id).join("knowledge_graph")
    }

    /// Get the archive directory for a specific agent (Parquet archives)
    pub fn agent_archive_dir(&self, agent_id: &str) -> PathBuf {
        self.agent_data_dir(agent_id).join("archive")
    }

    /// Create the directory structure for a new agent
    /// Creates: agents_data/{agent-id}/attachments/, documents/, knowledge_graph/, archive/
    pub fn create_agent_data_dirs(&self, agent_id: &str) -> Result<()> {
        let agent_dir = self.agent_data_dir(agent_id);

        // Create main agent data directory
        fs::create_dir_all(&agent_dir)
            .context("Failed to create agent data directory")?;

        // Create attachments directory
        fs::create_dir_all(self.agent_attachments_dir(agent_id))
            .context("Failed to create agent attachments directory")?;

        // Create documents directory
        fs::create_dir_all(self.agent_documents_dir(agent_id))
            .context("Failed to create agent documents directory")?;

        // Create knowledge graph directory
        fs::create_dir_all(self.agent_knowledge_graph_dir(agent_id))
            .context("Failed to create agent knowledge graph directory")?;

        // Create archive directory
        fs::create_dir_all(self.agent_archive_dir(agent_id))
            .context("Failed to create agent archive directory")?;

        Ok(())
    }

    /// Get the Agent Channel database path
    pub fn agent_channels_db_path(&self) -> PathBuf {
        self.db_dir.join("agent_channels.db")
    }
}

/// Storage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    /// Total used space in bytes
    pub total_used: u64,
    /// Database size in bytes
    pub database_size: u64,
    /// Agents directory size in bytes
    pub agents_size: u64,
    /// Skills directory size in bytes
    pub skills_size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert!(settings.appearance.dark_mode);
        assert_eq!(settings.appearance.theme, "default");
        assert!(settings.performance.hardware_acceleration);
    }

    #[test]
    fn test_serialize_deserialize() {
        let settings = Settings::default();
        let yaml = serde_yaml::to_string(&settings).unwrap();
        let deserialized: Settings = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(settings.appearance.dark_mode, deserialized.appearance.dark_mode);
        assert_eq!(settings.appearance.theme, deserialized.appearance.theme);
    }
}
