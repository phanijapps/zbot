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

mod conditional_agent;
mod custom_agent;
mod llm_conditional_agent;
mod loop_agent;
mod parallel_agent;
mod sequential_agent;

pub use conditional_agent::ConditionalAgent;
pub use custom_agent::{CustomAgent, CustomAgentBuilder};
pub use llm_conditional_agent::{LlmConditionalAgent, LlmConditionalAgentBuilder};
pub use loop_agent::LoopAgent;
pub use parallel_agent::ParallelAgent;
pub use sequential_agent::SequentialAgent;
