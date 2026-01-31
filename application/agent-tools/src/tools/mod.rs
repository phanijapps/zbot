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
mod memory;
mod introspection;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
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
pub use agent::{CreateAgentTool, ListAgentsTool};
pub use web::WebFetchTool;
pub use memory::MemoryTool;
pub use introspection::{ListSkillsTool, ListToolsTool, ListMcpsTool};

// ============================================================================
// TOOL SETTINGS
// ============================================================================

/// Settings for optional tools.
///
/// These settings control which optional tools are enabled beyond the core set.
/// Core tools (shell, read, write, edit, memory, todo) are always enabled.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToolSettings {
    /// Enable grep tool (regex search in files)
    #[serde(default)]
    pub grep: bool,

    /// Enable glob tool (find files by pattern)
    #[serde(default)]
    pub glob: bool,

    /// Enable python tool (run Python scripts)
    #[serde(default)]
    pub python: bool,

    /// Enable web_fetch tool (HTTP requests).
    /// Disabled by default as large responses can cause context explosion.
    #[serde(default)]
    pub web_fetch: bool,

    /// Enable load_skill tool
    #[serde(default)]
    pub load_skill: bool,

    /// Enable UI tools (request_input, show_content)
    #[serde(default)]
    pub ui_tools: bool,

    /// Enable knowledge graph tools (5 tools)
    #[serde(default)]
    pub knowledge_graph: bool,

    /// Enable create_agent tool
    #[serde(default)]
    pub create_agent: bool,

    /// Enable introspection tools (list_skills, list_tools, list_mcps)
    #[serde(default)]
    pub introspection: bool,

    /// Offload large tool results to filesystem instead of keeping in context.
    /// When a tool result exceeds the token threshold, it's saved to a temp file
    /// and the agent is instructed to read it with a CLI tool.
    #[serde(default = "default_offload_enabled")]
    pub offload_large_results: bool,

    /// Token threshold for offloading tool results (default: 5000 tokens ≈ 20000 chars).
    /// Results larger than this are saved to filesystem.
    #[serde(default = "default_offload_threshold")]
    pub offload_threshold_tokens: usize,
}

fn default_offload_threshold() -> usize {
    5000 // ~20000 characters
}

fn default_offload_enabled() -> bool {
    true // Enabled by default to prevent context explosion
}

// ============================================================================
// BUILT-IN TOOLS FACTORY
// ============================================================================

/// Get core tools that are always enabled (6 tools).
///
/// Core tools:
/// - shell: Run any command
/// - read: Read files
/// - write: Write files
/// - edit: Edit files
/// - memory: Persist/recall information
/// - todo: Track task progress
///
/// Note: respond, delegate_to_agent, and list_agents are registered separately
/// in the runner as action tools.
/// Note: web_fetch is now optional due to potential for large responses causing context explosion.
#[must_use]
pub fn core_tools(fs: Arc<dyn FileSystemContext>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ShellTool::new()),
        Arc::new(ReadTool),
        Arc::new(WriteTool::new(fs.clone())),
        Arc::new(EditTool::new(fs.clone())),
        Arc::new(MemoryTool::new(fs.clone())),
        Arc::new(TodoTool::new()),
    ]
}

/// Get optional tools based on settings.
///
/// Returns tools that are enabled in the settings.
#[must_use]
pub fn optional_tools(fs: Arc<dyn FileSystemContext>, settings: &ToolSettings) -> Vec<Arc<dyn Tool>> {
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();

    if settings.grep {
        tools.push(Arc::new(GrepTool));
    }

    if settings.glob {
        tools.push(Arc::new(GlobTool));
    }

    if settings.python {
        tools.push(Arc::new(PythonTool::new(fs.clone())));
    }

    if settings.web_fetch {
        tools.push(Arc::new(WebFetchTool::new()));
    }

    if settings.load_skill {
        tools.push(Arc::new(LoadSkillTool::new(fs.clone())));
    }

    if settings.ui_tools {
        tools.push(Arc::new(RequestInputTool));
        tools.push(Arc::new(ShowContentTool));
    }

    if settings.knowledge_graph {
        tools.push(Arc::new(ListEntitiesTool));
        tools.push(Arc::new(SearchEntitiesTool));
        tools.push(Arc::new(GetEntityRelationshipsTool));
        tools.push(Arc::new(AddEntityTool));
        tools.push(Arc::new(AddRelationshipTool));
    }

    if settings.create_agent {
        tools.push(Arc::new(CreateAgentTool::new(fs.clone())));
    }

    if settings.introspection {
        tools.push(Arc::new(ListSkillsTool::new(fs.clone())));
        tools.push(Arc::new(ListToolsTool::new()));
        tools.push(Arc::new(ListMcpsTool::new(fs.clone())));
    }

    tools
}

/// Get all built-in tools with a file system context.
///
/// This is the legacy function that returns all tools.
/// For new code, prefer using `core_tools()` + `optional_tools()`.
#[must_use]
pub fn builtin_tools_with_fs(fs: Arc<dyn FileSystemContext>) -> Vec<Arc<dyn Tool>> {
    // Return all tools (core + all optional enabled)
    let all_enabled = ToolSettings {
        grep: true,
        glob: true,
        python: true,
        web_fetch: true, // Include in legacy function
        load_skill: true,
        ui_tools: true,
        knowledge_graph: true,
        create_agent: true,
        introspection: true,
        offload_large_results: false, // Not relevant for this legacy function
        offload_threshold_tokens: default_offload_threshold(),
    };

    let mut tools = core_tools(fs.clone());
    tools.extend(optional_tools(fs, &all_enabled));
    tools
}
