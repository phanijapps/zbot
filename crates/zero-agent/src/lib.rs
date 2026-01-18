//! # Zero Agent
//!
//! Agent implementations for the Zero framework.

pub mod llm_agent;
pub mod workflow;

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
