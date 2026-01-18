// ============================================================================
// TOOL MODULES
// ============================================================================

mod file;
mod search;
mod execution;
mod ui;

use std::sync::Arc;

use zero_core::Tool;
use zero_core::FileSystemContext;

pub use file::{ReadTool, WriteTool, EditTool};
pub use search::{GrepTool, GlobTool};
pub use execution::PythonTool;
pub use execution::skills::LoadSkillTool;
pub use ui::{RequestInputTool, ShowContentTool};

// ============================================================================
// BUILT-IN TOOLS FACTORY
// ============================================================================

/// Get all built-in tools with a file system context
///
/// This function creates all the built-in tools with the provided
/// file system context. Tools that don't need file system access
/// are created without context.
///
/// # Arguments
/// * `fs` - File system context
/// * `conversation_id` - Optional conversation ID for tools that need it
#[must_use]
pub fn builtin_tools_with_fs(fs: Arc<dyn FileSystemContext>, conversation_id: Option<String>) -> Vec<Arc<dyn Tool>> {
    // Create WriteTool with conversation_id if provided
    let write_tool = if let Some(ref conv_id) = conversation_id {
        Arc::new(WriteTool::with_conversation(fs.clone(), Some(conv_id.clone())))
    } else {
        Arc::new(WriteTool::new(fs.clone()))
    };

    // Create EditTool with conversation_id if provided
    let edit_tool = if let Some(ref conv_id) = conversation_id {
        Arc::new(EditTool::with_context(fs.clone(), Some(conv_id.clone())))
    } else {
        Arc::new(EditTool::new(fs.clone()))
    };

    vec![
        Arc::new(ReadTool),
        write_tool,
        edit_tool,
        Arc::new(GrepTool),
        Arc::new(GlobTool),
        Arc::new(PythonTool::new(fs.clone())),
        Arc::new(LoadSkillTool::new(fs.clone())),
        Arc::new(RequestInputTool),
        Arc::new(ShowContentTool),
    ]
}
