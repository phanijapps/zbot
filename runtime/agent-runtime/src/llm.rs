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


pub mod client;
pub mod config;
pub mod embedding;
pub mod local_embedding;
pub mod non_streaming;
pub mod openai;
pub mod openai_embedding;
pub mod rate_limiter;
pub mod retry;
pub mod throttle;

pub use client::{
    ChatResponse, LlmClient, LlmError, StreamCallback, StreamChunk, TokenUsage, ToolCallChunk,
};
pub use config::LlmConfig;
pub use embedding::{
    content_hash, EmbeddingClient, EmbeddingConfig, EmbeddingError, EmbeddingProviderType,
};
pub use local_embedding::LocalEmbeddingClient;
pub use non_streaming::NonStreamingLlmClient;
pub use openai::OpenAiClient;
pub use openai_embedding::OpenAiEmbeddingClient;
pub use rate_limiter::{ProviderRateLimiter, RateLimitedLlmClient};
pub use retry::{RetryPolicy, RetryingLlmClient};
pub use throttle::ThrottledLlmClient;

// Re-export from types
pub use crate::types::{ChatMessage, ToolCall};
