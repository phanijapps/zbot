//! # Prompt Errors
//!
//! Error types for prompt operations.

use thiserror::Error;

/// Prompt-specific errors.
#[derive(Debug, Error)]
pub enum PromptError {
    /// Template parsing error
    #[error("Template parse error: {message}")]
    ParseError { message: String },

    /// Variable not found error
    #[error("Variable not found: {name}")]
    VariableNotFound { name: String },

    /// Invalid variable name error
    #[error("Invalid variable name: {name}")]
    InvalidVariable { name: String },

    /// Render error
    #[error("Template render error: {message}")]
    RenderError { message: String },
}

/// Result type for prompt operations.
pub type Result<T> = std::result::Result<T, PromptError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PromptError::VariableNotFound {
            name: "test_var".to_string(),
        };
        assert!(err.to_string().contains("test_var"));
    }

    #[test]
    fn test_invalid_variable() {
        let err = PromptError::InvalidVariable {
            name: "123invalid".to_string(),
        };
        assert!(err.to_string().contains("123invalid"));
    }
}
