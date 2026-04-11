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
