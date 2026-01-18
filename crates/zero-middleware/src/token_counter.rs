//! # Token Counter
//!
//! Token estimation utilities for middleware.

use zero_core::Content;

/// Estimate token count for content
///
/// This uses a simple heuristic: ~4 characters per token for English text.
/// For accurate counting, use tiktoken or similar.
pub fn estimate_tokens(content: &Content) -> usize {
    let mut total = 0;

    for part in &content.parts {
        if let zero_core::Part::Text { text } = part {
            // Rough estimate: ~4 characters per token
            total += (text.len() / 4).max(1);
        }
        // Other part types (tool calls, etc.) have variable cost
        // Use a conservative estimate
        else {
            total += 10;
        }
    }

    total
}

/// Estimate token count for multiple messages
pub fn estimate_tokens_batch(messages: &[Content]) -> usize {
    messages.iter().map(estimate_tokens).sum()
}

/// Get context window size for a model
///
/// Returns the maximum context window for known models.
/// For unknown models, returns a conservative default (8192).
pub fn get_context_window(model_name: &str) -> usize {
    // Known model context windows
    let model = model_name.to_lowercase();

    if model.contains("gpt-4-turbo") || model.contains("gpt-4-1106") {
        128000
    } else if model.contains("gpt-4") {
        8192
    } else if model.contains("gpt-3.5-turbo-16k") {
        16384
    } else if model.contains("gpt-3.5") {
        4096
    } else if model.contains("claude-3-opus") || model.contains("claude-3-5-sonnet") {
        200000
    } else if model.contains("claude-3-sonnet") {
        200000
    } else if model.contains("claude-3-haiku") {
        200000
    } else if model.contains("claude-2") {
        100000
    } else {
        // Conservative default
        8192
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::Part;

    #[test]
    fn test_estimate_tokens() {
        let content = Content {
            role: "user".to_string(),
            parts: vec![Part::Text {
                text: "Hello, world!".to_string(),
            }],
        };

        let tokens = estimate_tokens(&content);
        assert!(tokens > 0);
        assert!(tokens < 100); // Should be reasonable
    }

    #[test]
    fn test_estimate_tokens_batch() {
        let messages = vec![
            Content {
                role: "user".to_string(),
                parts: vec![Part::Text {
                    text: "Hello".to_string(),
                }],
            },
            Content {
                role: "assistant".to_string(),
                parts: vec![Part::Text {
                    text: "Hi there!".to_string(),
                }],
            },
        ];

        let tokens = estimate_tokens_batch(&messages);
        assert_eq!(tokens, estimate_tokens(&messages[0]) + estimate_tokens(&messages[1]));
    }

    #[test]
    fn test_get_context_window() {
        assert_eq!(get_context_window("gpt-4-turbo"), 128000);
        assert_eq!(get_context_window("gpt-4"), 8192);
        assert_eq!(get_context_window("gpt-3.5-turbo"), 4096);
        assert_eq!(get_context_window("gpt-3.5-turbo-16k"), 16384);
        assert_eq!(get_context_window("claude-3-opus"), 200000);
        assert_eq!(get_context_window("claude-3-sonnet"), 200000);
        assert_eq!(get_context_window("claude-2"), 100000);
        assert_eq!(get_context_window("unknown-model"), 8192);
    }
}
