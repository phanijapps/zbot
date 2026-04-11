#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]
#![allow(missing_docs)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::unnecessary_wraps)]
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
