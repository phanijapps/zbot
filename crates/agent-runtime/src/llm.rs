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

pub use client::{
    LlmClient, LlmModel, ChatResponse, StreamChunk, StreamCallback, ToolCallChunk
};
pub use config::LlmConfig;
pub use openai::OpenAiClient;

// Re-export from types
pub use crate::types::{ChatMessage, ToolCall};
