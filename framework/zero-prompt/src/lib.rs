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

#![warn(missing_docs)]
#![warn(clippy::all)]

mod template;
mod error;

pub use error::{PromptError, Result};
pub use template::{Template, TemplateRenderer, inject_session_state};
