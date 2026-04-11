//! # Zero Prompt
//!
//! Prompt and template management for the Zero framework.
//!
//! ## Features
//!
//! - **Template Injection**: Inject session state and variables into prompt templates
//! - **Placeholder Syntax**: Support for required and optional placeholders
//! - **Variable Validation**: Type-safe variable resolution
//! - **State Integration**: Seamless integration with zero-core state management

mod error;
mod template;

pub use error::{PromptError, Result};
pub use template::{inject_session_state, Template, TemplateRenderer};
