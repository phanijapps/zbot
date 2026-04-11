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


pub mod config;
pub mod context_editing;
pub mod pipeline;
pub mod summarization;
pub mod token_counter;
pub mod traits;

// Re-exports for convenience
pub use config::{
    ContextEditingConfig, KeepPolicy, MiddlewareConfig, SummarizationConfig, TriggerCondition,
};
pub use context_editing::ContextEditingMiddleware;
pub(crate) use context_editing::compress_old_assistant_messages;
pub use pipeline::MiddlewarePipeline;
pub use summarization::SummarizationMiddleware;
pub use traits::{
    EventMiddleware, ExecutionState, MiddlewareContext, MiddlewareEffect, PreProcessMiddleware,
    SkillInfo,
};
