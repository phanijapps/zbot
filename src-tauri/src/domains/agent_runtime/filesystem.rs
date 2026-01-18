// ============================================================================
// TAURI FILESYSTEM CONTEXT
// AppDirs-based file system context for Tauri
// ============================================================================

//! Tauri-specific file system context implementation

use std::path::{Path, PathBuf};
use std::sync::Arc;
use zero_app::FileSystemContext;

use crate::settings::AppDirs;

/// Tauri-specific file system context
pub struct TauriFileSystemContext {
    dirs: Arc<AppDirs>,
    conversation_id: Option<String>,
}

impl TauriFileSystemContext {
    /// Create a new Tauri file system context from a reference
    pub fn new(dirs: AppDirs) -> Self {
        Self {
            dirs: Arc::new(dirs),
            conversation_id: None,
        }
    }

    /// Set the conversation ID
    pub fn with_conversation(mut self, id: String) -> Self {
        self.conversation_id = Some(id);
        self
    }

    /// Get a reference to the AppDirs
    pub fn dirs(&self) -> &AppDirs {
        &self.dirs
    }

    /// Get a clone of the AppDirs Arc
    pub fn dirs_arc(&self) -> Arc<AppDirs> {
        Arc::clone(&self.dirs)
    }
}

impl FileSystemContext for TauriFileSystemContext {
    fn conversation_dir(&self, conversation_id: &str) -> Option<PathBuf> {
        let conv_id = if let Some(id) = &self.conversation_id {
            id.as_str()
        } else {
            conversation_id
        };
        // Use conversation_logs_dir for scoped conversation files
        Some(self.dirs.conversation_logs_dir.join(conv_id))
    }

    fn outputs_dir(&self) -> Option<PathBuf> {
        Some(self.dirs.outputs_dir.clone())
    }

    fn skills_dir(&self) -> Option<PathBuf> {
        Some(self.dirs.skills_dir.clone())
    }

    fn python_executable(&self) -> Option<PathBuf> {
        // Use Python from venv if available
        let python_path = if cfg!(target_os = "windows") {
            self.dirs.venv_dir.join("Scripts").join("python.exe")
        } else {
            self.dirs.venv_dir.join("bin").join("python")
        };
        if python_path.exists() {
            Some(python_path)
        } else {
            None
        }
    }
}

/// Create Tauri file system context from AppDirs
pub fn create_fs_context(dirs: AppDirs, conversation_id: Option<String>) -> TauriFileSystemContext {
    let ctx = TauriFileSystemContext::new(dirs);
    if let Some(id) = conversation_id {
        ctx.with_conversation(id)
    } else {
        ctx
    }
}
