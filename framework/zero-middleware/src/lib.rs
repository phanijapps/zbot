//! # Zero Middleware
//!
//! Extensible middleware pipeline for agent execution.
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
pub use pipeline::MiddlewarePipeline;
pub use summarization::SummarizationMiddleware;
pub use traits::{EventMiddleware, MiddlewareContext, MiddlewareEffect, PreProcessMiddleware};
