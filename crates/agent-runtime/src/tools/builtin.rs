// ============================================================================
// BUILT-IN TOOLS
// Default tools provided by the framework
// ============================================================================

use std::sync::Arc;

use super::super::tools::Tool;

/// Get all built-in tools
///
/// TODO: Extract from src-tauri/src/domains/agent_runtime/tools.rs
#[must_use]
pub fn builtin_tools() -> Vec<Arc<dyn Tool>> {
    // Built-in tools will include:
    // - read_file: Read file contents
    // - write_file: Write to a file
    // - edit_file: Edit a file with search/replace
    // - grep_files: Search for text in files
    // - glob_files: Find files by pattern
    // - python_execute: Execute Python code
    // - load_skill: Load a skill file
    // And more...

    Vec::new()
}
