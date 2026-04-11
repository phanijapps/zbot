//! # Zero LLM
//!
//! LLM abstractions and OpenAI client for the Zero framework.

pub mod config;
pub mod encoding;
pub mod llm;
pub mod openai;
pub mod openai_encoder;

// Re-export from zero-core
pub use zero_core::types::Content;

// Re-export from our modules
pub use config::LlmConfig;
pub use encoding::{EncodingError, ProviderEncoder};
pub use llm::{
    Llm, LlmRequest, LlmResponse, LlmResponseChunk, LlmResponseStream, TokenUsage, ToolCall,
    ToolDefinition,
};
pub use openai::OpenAiLlm;
pub use openai_encoder::{EncoderCapabilities, OpenAiEncoder};
