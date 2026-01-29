// ============================================================================
// TOOL MODULES
// ============================================================================

mod file;
mod search;
mod execution;
mod ui;
mod knowledge_graph;
mod agent;
mod web;

use std::sync::Arc;

use zero_core::Tool;
use zero_core::FileSystemContext;

pub use file::{ReadTool, WriteTool, EditTool};
pub use search::{GrepTool, GlobTool};
pub use execution::PythonTool;
pub use execution::ShellTool;
pub use execution::skills::LoadSkillTool;
pub use execution::TodoTool;
pub use ui::{RequestInputTool, ShowContentTool};
pub use knowledge_graph::{
    ListEntitiesTool,
    SearchEntitiesTool,
    GetEntityRelationshipsTool,
    AddEntityTool,
    AddRelationshipTool,
};
pub use agent::CreateAgentTool;
pub use web::WebFetchTool;

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
///
/// # Note
/// Conversation ID is no longer passed to tools. Tools that need it
/// (like WriteTool, EditTool) will read it from the ToolContext's state
/// using the state key "app:conversation_id".
#[must_use]
pub fn builtin_tools_with_fs(fs: Arc<dyn FileSystemContext>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ReadTool),
        Arc::new(WriteTool::new(fs.clone())),
        Arc::new(EditTool::new(fs.clone())),
        Arc::new(GrepTool),
        Arc::new(GlobTool),
        Arc::new(PythonTool::new(fs.clone())),
        Arc::new(ShellTool::new()),
        Arc::new(LoadSkillTool::new(fs.clone())),
        Arc::new(RequestInputTool),
        Arc::new(ShowContentTool),
        // Knowledge Graph tools
        Arc::new(ListEntitiesTool),
        Arc::new(SearchEntitiesTool),
        Arc::new(GetEntityRelationshipsTool),
        Arc::new(AddEntityTool),
        Arc::new(AddRelationshipTool),
        // Agent tools
        Arc::new(CreateAgentTool::new(fs.clone())),
        // TODO list tool
        Arc::new(TodoTool::new()),
        // Web tools
        Arc::new(WebFetchTool::new()),
    ]
}
