//! # Zero LLM
//!
//! LLM abstractions and OpenAI client for the Zero framework.

pub mod llm;
pub mod config;
pub mod encoding;
pub mod openai;
pub mod openai_encoder;

// Re-export from zero-core
pub use zero_core::types::Content;

// Re-export from our modules
pub use llm::{Llm, LlmRequest, LlmResponse, LlmResponseChunk, LlmResponseStream, ToolCall, TokenUsage, ToolDefinition};
pub use config::LlmConfig;
pub use openai::OpenAiLlm;
pub use encoding::{ProviderEncoder, EncodingError};
pub use openai_encoder::{OpenAiEncoder, EncoderCapabilities};
