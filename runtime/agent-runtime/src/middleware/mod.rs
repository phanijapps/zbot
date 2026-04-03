// ============================================================================
// MIDDLEWARE MODULE
// Modular middleware system for agent execution
//
// Inspired by LangChain JS middleware architecture
// https://docs.langchain.com/oss/javascript/langchain/middleware/built-in
// ============================================================================

//! # Middleware Module
//!
//! Extensible middleware pipeline for preprocessing messages and handling events.
//!
//! ## Module Structure
//!
//! - [`pipeline`]: Middleware pipeline orchestration
//! - [`traits`]: Core middleware traits
//! - [`config`]: Configuration types
//! - [`summarization`]: Conversation summarization middleware
//! - [`context_editing`]: Context editing middleware
//! - [`token_counter`]: Token estimation utilities
//!
//! ## Middleware Types
//!
//! - **PreProcessMiddleware**: Processes messages before sending to LLM
//! - **EventMiddleware**: Handles events during execution
//!
//! ## Built-in Middleware
//!
//! - `SummarizationMiddleware`: Compresses long conversations
//! - `ContextEditingMiddleware`: Clears old tool results

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod pipeline;
pub mod traits;
pub mod config;
pub mod summarization;
pub mod context_editing;
pub mod token_counter;

// Re-exports for convenience
pub use pipeline::MiddlewarePipeline;
pub use traits::{PreProcessMiddleware, EventMiddleware, MiddlewareContext, MiddlewareEffect, ExecutionState, SkillInfo};
pub use config::{MiddlewareConfig, TriggerCondition, KeepPolicy, SummarizationConfig, ContextEditingConfig};
pub use summarization::SummarizationMiddleware;
pub use context_editing::ContextEditingMiddleware;
pub(crate) use context_editing::{compress_old_assistant_messages, compress_assistant_message};
