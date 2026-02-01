//! # Workflow Agents
//!
//! Composable agents for complex workflows.
//!
//! ## Agent Types
//!
//! - [`SequentialAgent`] - Execute agents in order (A → B → C)
//! - [`ParallelAgent`] - Execute agents concurrently
//! - [`LoopAgent`] - Iterate until exit condition
//! - [`ConditionalAgent`] - Rule-based routing
//! - [`LlmConditionalAgent`] - LLM-based classification and routing
//! - [`CustomAgent`] - Custom async logic without LLM

mod sequential_agent;
mod parallel_agent;
mod loop_agent;
mod conditional_agent;
mod llm_conditional_agent;
mod custom_agent;

pub use sequential_agent::SequentialAgent;
pub use parallel_agent::ParallelAgent;
pub use loop_agent::LoopAgent;
pub use conditional_agent::ConditionalAgent;
pub use llm_conditional_agent::{LlmConditionalAgent, LlmConditionalAgentBuilder};
pub use custom_agent::{CustomAgent, CustomAgentBuilder};
