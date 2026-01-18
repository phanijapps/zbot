//! # File System Context
//!
//! File system abstraction for tool execution.

use std::path::PathBuf;

/// Trait for providing file system context to tools
///
/// This allows the framework to be used with different directory structures
/// without depending on application-specific code.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_file_system_context() {
        let ctx = NoFileSystemContext;
        assert!(ctx.conversation_dir("test").is_none());
        assert!(ctx.outputs_dir().is_none());
        assert!(ctx.skills_dir().is_none());
        assert!(ctx.python_executable().is_none());
    }
}
