// ============================================================================
// FILE SYSTEM CONTEXT TRAIT
// ============================================================================
// This module defines only the FileSystemContext trait and related types.
// Concrete tool implementations have been moved to the zerotools crate.

use std::path::PathBuf;
use std::sync::Arc;

/// Trait for providing file system context to tools
///
/// This allows the framework to be used with different directory structures
/// without depending on application-specific code like `AppDirs`.
pub trait FileSystemContext: Send + Sync {
    /// Get the conversation directory for a given conversation ID
    fn conversation_dir(&self, conversation_id: &str) -> Option<PathBuf>;

    /// Get the outputs directory
    fn outputs_dir(&self) -> Option<PathBuf>;

    /// Get the skills directory
    fn skills_dir(&self) -> Option<PathBuf>;

    /// Get the Python executable path
    fn python_executable(&self) -> Option<PathBuf>;
}

/// Default file system context that returns None for all paths
/// (for library-only usage without application integration)
#[derive(Debug, Clone, Default)]
pub struct NoFileSystemContext;

impl FileSystemContext for NoFileSystemContext {
    fn conversation_dir(&self, _conversation_id: &str) -> Option<PathBuf> {
        None
    }

    fn outputs_dir(&self) -> Option<PathBuf> {
        None
    }

    fn skills_dir(&self) -> Option<PathBuf> {
        None
    }

    fn python_executable(&self) -> Option<PathBuf> {
        None
    }
}

// ============================================================================
// TOOL CONTEXT WITH FILE SYSTEM
// ============================================================================

use super::context::ToolContext as BaseToolContext;

/// Extended tool context with file system access
pub struct ToolContextWithFs {
    /// Base tool context
    pub base: BaseToolContext,

    /// File system context
    pub fs: Arc<dyn FileSystemContext>,
}

impl ToolContextWithFs {
    /// Create a new tool context with file system
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self {
            base: BaseToolContext::new(),
            fs,
        }
    }

    /// Create with conversation ID
    #[must_use]
    pub fn with_conversation(fs: Arc<dyn FileSystemContext>, conversation_id: String) -> Self {
        Self {
            base: BaseToolContext::with_conversation(conversation_id),
            fs,
        }
    }
}
