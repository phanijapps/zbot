#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]
#![allow(missing_docs)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::unnecessary_wraps)]
// ============================================================================
// APP-TOOLS - Built-in Tools for z-Bot Application
// ============================================================================

//! # App Tools
//!
//! Built-in tool implementations for the z-Bot application.
//!
//! This crate provides concrete tool implementations that use
//! the abstractions defined in zero-core.

mod tools;

pub use tools::{
    ApplyPatchTool,
    EditFileTool,
    GlobTool,
    GrepTool,
    // Composite re-exports
    ListAgentsTool,
    LoadSkillTool,
    MemoryEntry,
    MemoryStore,
    MemoryTool,
    // Multimodal vision fallback
    MultimodalAnalyzeTool,
    QueryResourceTool,
    ReadTool,
    SetSessionTitleTool,
    // Individual tools for lean subagent registries
    ShellTool,
    ToolSettings,
    UpdatePlanTool,
    // Orchestrator tools
    WardTool,
    WriteFileTool,
    builtin_tools_with_fs,
    core_tools,
    optional_tools,
};

// Re-export from zero-core
pub use zero_core::{FileSystemContext, Tool};
