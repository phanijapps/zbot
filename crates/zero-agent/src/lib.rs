//! # Zero Agent
//!
//! Agent implementations for the Zero framework.

pub mod llm_agent;
pub mod workflow;
pub mod orchestrator;

// Re-export from zero-core
pub use zero_core::{Agent, BeforeAgentCallback, AfterAgentCallback};

// Re-export from our modules
pub use llm_agent::{LlmAgent, LlmAgentBuilder};

// Re-export workflow agents
pub use workflow::{
    SequentialAgent,
    ParallelAgent,
    LoopAgent,
    ConditionalAgent,
    LlmConditionalAgent,
    LlmConditionalAgentBuilder,
    CustomAgent,
    CustomAgentBuilder,
};

// Re-export orchestrator
pub use orchestrator::{
    OrchestratorAgent,
    OrchestratorBuilder,
    OrchestratorConfig,
    TaskGraph,
    TaskNode,
    TaskStatus,
    ExecutionTrace,
    TraceEvent,
    TraceEventKind,
};
