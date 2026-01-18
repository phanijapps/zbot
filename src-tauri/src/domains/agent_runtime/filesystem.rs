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
///
/// Note: conversation_id is no longer stored here. It is passed directly
/// to conversation_dir() by tools that read it from session state.
pub struct TauriFileSystemContext {
    dirs: Arc<AppDirs>,
}

impl TauriFileSystemContext {
    /// Create a new Tauri file system context from a reference
    pub fn new(dirs: AppDirs) -> Self {
        Self {
            dirs: Arc::new(dirs),
        }
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
        // Use conversation_logs_dir for scoped conversation files
        Some(self.dirs.conversation_logs_dir.join(conversation_id))
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
pub fn create_fs_context(dirs: AppDirs) -> TauriFileSystemContext {
    TauriFileSystemContext::new(dirs)
}
