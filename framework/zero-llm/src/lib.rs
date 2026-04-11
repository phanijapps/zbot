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
