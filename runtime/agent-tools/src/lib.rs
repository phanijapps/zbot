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
    builtin_tools_with_fs, core_tools, optional_tools,
    // Composite re-exports
    ListAgentsTool, MemoryEntry, MemoryStore, QueryResourceTool, ToolSettings,
    // Individual tools for lean subagent registries
    ShellTool, ApplyPatchTool, MemoryTool, LoadSkillTool, GrepTool, ReadTool, GlobTool,
    // Orchestrator tools
    WardTool, UpdatePlanTool, SetSessionTitleTool,
};

// Re-export from zero-core
pub use zero_core::{Tool, FileSystemContext};
