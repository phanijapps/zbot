// ============================================================================
// TOOL MODULES
// ============================================================================

mod file;
mod search;
mod execution;
mod ui;

use std::sync::Arc;

use agent_runtime::tools::Tool;
use agent_runtime::tools::builtin::FileSystemContext;

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
#[must_use]
pub fn builtin_tools_with_fs(fs: Arc<dyn FileSystemContext>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ReadTool),
        Arc::new(WriteTool::new(fs.clone())),
        Arc::new(EditTool),
        Arc::new(GrepTool),
        Arc::new(GlobTool),
        Arc::new(PythonTool::new(fs.clone())),
        Arc::new(LoadSkillTool::new(fs.clone())),
        Arc::new(RequestInputTool),
        Arc::new(ShowContentTool),
    ]
}
