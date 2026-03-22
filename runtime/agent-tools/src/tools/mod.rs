// ============================================================================
// TOOL MODULES
// ============================================================================

mod file;
mod search;
mod execution;
mod ui;
mod agent;
mod web;
mod memory;
mod introspection;
mod ward;
mod connectors;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use zero_core::Tool;
use zero_core::FileSystemContext;
use zero_core::MemoryFactStore;

pub use file::{ReadTool, WriteTool, EditTool};
pub use search::{GrepTool, GlobTool};
pub use execution::ApplyPatchTool;
pub use execution::ExecutionGraphTool;
pub use execution::PythonTool;
pub use execution::ShellTool;
pub use execution::skills::LoadSkillTool;
pub use execution::TodoTool;
pub use execution::UpdatePlanTool;
pub use ui::{RequestInputTool, ShowContentTool};
pub use agent::{CreateAgentTool, ListAgentsTool};
pub use web::WebFetchTool;
pub use memory::{MemoryTool, MemoryStore, MemoryEntry};
pub use introspection::{ListSkillsTool, ListToolsTool, ListMcpsTool};
pub use ward::WardTool;
pub use connectors::QueryResourceTool;

// ============================================================================
// TOOL SETTINGS
// ============================================================================

/// Settings for optional tools.
///
/// These settings control which optional tools are enabled beyond the core set.
///
/// Core tools (always enabled):
/// - shell: Primary execution — commands
/// - apply_patch: File creation/editing/deletion via patch format
/// - memory: Persist/recall information
/// - ward: Project directory management
/// - update_plan: Lightweight task checklist
/// - list_skills, load_skill: Skill discovery
/// - grep: Structured file content search
///
/// Note: respond, delegate_to_agent, and list_agents are registered separately
/// in the runner as action tools.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ToolSettings {
    /// Enable python tool (run Python scripts)
    #[serde(default)]
    pub python: bool,

    /// Enable web_fetch tool (HTTP requests).
    /// Disabled by default as large responses can cause context explosion.
    #[serde(default)]
    pub web_fetch: bool,

    /// Enable UI tools (request_input, show_content)
    #[serde(default)]
    pub ui_tools: bool,

    /// Enable create_agent tool
    #[serde(default)]
    pub create_agent: bool,

    /// Enable introspection tools (list_tools, list_mcps)
    /// Note: list_skills is now a core tool
    #[serde(default)]
    pub introspection: bool,

    /// Enable file tools (read, write, edit, glob) as separate tools.
    /// When false (default), the model uses shell + apply_patch instead.
    #[serde(default)]
    pub file_tools: bool,

    /// Enable the heavyweight todos tool (SQLite-like task persistence).
    /// When false (default), the lightweight update_plan tool is used instead.
    #[serde(default)]
    pub todos: bool,

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

/// Get core tools — the minimal, high-signal set (8 tools).
///
/// Shell for commands, apply_patch for file operations.
/// Separate read/write/edit/glob tools are optional (moved to optional_tools).
///
/// Core tools:
/// - shell: Primary execution — commands
/// - apply_patch: File creation/editing/deletion via patch format
/// - memory: Persistent KV store
/// - ward: Project directory management
/// - update_plan: Lightweight task checklist
/// - list_skills, load_skill: Skill discovery
/// - grep: Structured file content search
#[must_use]
pub fn core_tools(
    fs: Arc<dyn FileSystemContext>,
    fact_store: Option<Arc<dyn MemoryFactStore>>,
) -> Vec<Arc<dyn Tool>> {
    vec![
        // Primary execution tool
        Arc::new(ShellTool::new()),
        // First-class file creation/editing/deletion via patch format
        Arc::new(ApplyPatchTool::new(fs.clone())),
        // Persistent memory (with optional DB-backed fact store)
        Arc::new(MemoryTool::new(fs.clone(), fact_store)),
        // Ward management (named project directories)
        Arc::new(WardTool::new(fs.clone())),
        // Lightweight plan tracking
        Arc::new(UpdatePlanTool::new()),
        // DAG workflow engine for multi-step orchestration
        Arc::new(ExecutionGraphTool::new()),
        // Skill discovery (high priority - encourages delegation)
        Arc::new(ListSkillsTool::new(fs.clone())),
        Arc::new(LoadSkillTool::new(fs.clone())),
        // File search (structured output beats raw shell rg)
        Arc::new(GrepTool),
    ]
}

/// Get optional tools based on settings.
///
/// Includes file tools (read/write/edit/glob), todos, python, web_fetch, etc.
#[must_use]
pub fn optional_tools(fs: Arc<dyn FileSystemContext>, settings: &ToolSettings) -> Vec<Arc<dyn Tool>> {
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();

    // File tools — separate read/write/edit/glob (opt-in)
    if settings.file_tools {
        tools.push(Arc::new(ReadTool));
        tools.push(Arc::new(WriteTool::new(fs.clone())));
        tools.push(Arc::new(EditTool::new(fs.clone())));
        tools.push(Arc::new(GlobTool));
    }

    // Heavyweight todos (opt-in, replaced by update_plan in core)
    if settings.todos {
        tools.push(Arc::new(TodoTool::new()));
    }

    if settings.python {
        tools.push(Arc::new(PythonTool::new(fs.clone())));
    }

    if settings.web_fetch {
        tools.push(Arc::new(WebFetchTool::new()));
    }

    if settings.ui_tools {
        tools.push(Arc::new(RequestInputTool));
        tools.push(Arc::new(ShowContentTool));
    }

    if settings.create_agent {
        tools.push(Arc::new(CreateAgentTool::new(fs.clone())));
    }

    if settings.introspection {
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
        python: true,
        web_fetch: true,
        ui_tools: true,
        create_agent: true,
        introspection: true,
        file_tools: true,
        todos: true,
        offload_large_results: false, // Not relevant for this legacy function
        offload_threshold_tokens: default_offload_threshold(),
    };

    let mut tools = core_tools(fs.clone(), None);
    tools.extend(optional_tools(fs, &all_enabled));
    tools
}
