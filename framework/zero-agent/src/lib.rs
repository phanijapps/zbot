//! # Zero Agent
//!
//! Agent implementations for the Zero framework.

pub mod llm_agent;
pub mod orchestrator;
pub mod workflow;

// Re-export from zero-core
pub use zero_core::{AfterAgentCallback, Agent, BeforeAgentCallback};

// Re-export from our modules
pub use llm_agent::{LlmAgent, LlmAgentBuilder};

// Re-export workflow agents
pub use workflow::{
    ConditionalAgent, CustomAgent, CustomAgentBuilder, LlmConditionalAgent,
    LlmConditionalAgentBuilder, LoopAgent, ParallelAgent, SequentialAgent,
};

// Re-export orchestrator
pub use orchestrator::{
    ExecutionTrace, OrchestratorAgent, OrchestratorBuilder, OrchestratorConfig, TaskGraph,
    TaskNode, TaskStatus, TraceEvent, TraceEventKind,
};
