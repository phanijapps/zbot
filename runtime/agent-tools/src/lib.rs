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
    // Knowledge graph query types
    EntityInfo,
    GlobTool,
    // Goal tool (agent intent lifecycle)
    GoalAccess,
    GoalSummary,
    GoalTool,
    GraphQueryTool,
    GraphStorageAccess,
    GrepTool,
    // Ingestion tool (enqueue text for background extraction)
    IngestTool,
    IngestionAccess,
    // Composite re-exports
    ListAgentsTool,
    ListMcpsTool,
    ListSkillsTool,
    LoadSkillTool,
    MemoryEntry,
    MemoryStore,
    MemoryTool,
    // Multimodal vision fallback
    MultimodalAnalyzeTool,
    NeighborInfo,
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
