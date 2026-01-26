//! # Workflow Executor
//!
//! Workflow execution engine for AgentZero. This crate provides:
//!
//! - Loading workflow definitions from the file system
//! - Building executable agent graphs from workflow configurations
//! - Executing workflows using the zero-agent framework
//! - Managing workflow state and streaming events
//!
//! ## Architecture
//!
//! ```text
//! Workflow Definition (.workflow/)
//!     │
//!     ├── graph.yaml    → WorkflowGraph (nodes, edges, pattern)
//!     └── layout.json   → UI positioning (not used in execution)
//!
//! Subagent Configs (.subagents/)
//!     │
//!     └── {name}/
//!         ├── config.yaml → SubagentConfig
//!         └── AGENTS.md   → System instructions
//!
//! Orchestrator Config (root)
//!     │
//!     ├── config.yaml   → OrchestratorConfig
//!     └── AGENTS.md     → Orchestrator instructions
//!
//!           ↓ WorkflowLoader
//!
//! WorkflowDefinition
//!     │
//!     ├── orchestrator: OrchestratorConfig
//!     ├── subagents: Vec<SubagentConfig>
//!     └── graph: WorkflowGraph
//!
//!           ↓ WorkflowBuilder
//!
//! ExecutableWorkflow
//!     │
//!     └── root_agent: Arc<dyn Agent>  (composed from workflow pattern)
//!
//!           ↓ WorkflowExecutor
//!
//! EventStream (streaming execution results)
//! ```

pub mod config;
pub mod error;
pub mod graph;
pub mod loader;
pub mod builder;
pub mod executor;

// Re-exports for convenience
pub use config::{
    OrchestratorConfig,
    SubagentConfig,
    WorkflowDefinition,
};
pub use error::{WorkflowError, Result};
pub use graph::{
    WorkflowGraph,
    WorkflowNode,
    WorkflowEdge,
    WorkflowPattern,
    NodeType,
};
pub use loader::WorkflowLoader;
pub use builder::{
    WorkflowBuilder,
    WorkflowBuilderConfig,
    ExecutableWorkflow,
    LlmFactory,
    ToolsetFactory,
};
pub use executor::{WorkflowExecutor, ExecutionOptions, ExecutionResult};
