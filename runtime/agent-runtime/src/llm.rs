// ============================================================================
// LLM CLIENT MODULE
// Abstract interface for LLM providers
// ============================================================================

//! # LLM Client Module
//!
//! Abstract interface for interacting with various LLM providers.
//!
//! ## Submodules
//!
//! - [`client`]: Core LLM client trait and types
//! - [`config`]: LLM client configuration
//! - [`openai`]: OpenAI-compatible client implementation

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod client;
pub mod config;
pub mod openai;
pub mod retry;

pub use client::{
    LlmClient, ChatResponse, StreamChunk, StreamCallback, ToolCallChunk, TokenUsage
};
pub use config::LlmConfig;
pub use openai::OpenAiClient;
pub use retry::{RetryingLlmClient, RetryPolicy};

// Re-export from types
pub use crate::types::{ChatMessage, ToolCall};
