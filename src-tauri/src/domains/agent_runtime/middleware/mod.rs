// ============================================================================
// MIDDLEWARE MODULE
// Modular middleware system for agent execution
//
// Inspired by LangChain JS middleware architecture
// https://docs.langchain.com/oss/javascript/langchain/middleware/built-in
// ============================================================================

pub mod pipeline;
pub mod traits;
pub mod config;
pub mod summarization;
pub mod context_editing;
pub mod token_counter;

// Re-exports for convenience
pub use pipeline::MiddlewarePipeline;
pub use traits::{PreProcessMiddleware, EventMiddleware, MiddlewareContext, MiddlewareEffect};
pub use config::{MiddlewareConfig, TriggerCondition, KeepPolicy, SummarizationConfig, ContextEditingConfig};
pub use summarization::SummarizationMiddleware;
pub use context_editing::ContextEditingMiddleware;
