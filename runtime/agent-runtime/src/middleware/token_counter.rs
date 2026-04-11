// ============================================================================
// TOKEN COUNTER
// Estimate token counts for messages
// ============================================================================

//! # Token Counter
//!
//! Token estimation utilities for middleware.

use crate::types::ChatMessage;

/// Estimate token count for a string
///
/// Uses a simple heuristic: ~4 characters per token for English text
/// This is approximate but works well for most use cases
#[must_use] 
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    // Rough estimate: 4 characters per token
    // This varies by language and model but is a reasonable default
    (text.len() / 4).max(1)
}

/// Estimate token count for a single message
#[must_use] 
pub fn estimate_message_tokens(message: &ChatMessage) -> usize {
    let mut tokens = estimate_tokens(&message.text_content());

    // Add overhead for tool calls
    if let Some(tool_calls) = &message.tool_calls {
        for tool_call in tool_calls {
            // Tool name + arguments
            tokens += estimate_tokens(&tool_call.name);
            tokens += estimate_tokens(&tool_call.arguments.to_string());
        }
    }

    // Role and other metadata overhead (~10 tokens)
    tokens.saturating_add(10)
}

/// Estimate total token count for all messages
pub fn estimate_total_tokens(messages: &[ChatMessage]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

/// Estimate fraction of context window used
#[must_use] 
pub fn estimate_context_fraction(tokens: usize, context_window: usize) -> f64 {
    if context_window == 0 {
        return 0.0;
    }
    tokens as f64 / context_window as f64
}

/// Get default context window size for common models
#[must_use] 
pub fn get_model_context_window(model: &str) -> usize {
    // Common context window sizes
    match model {
        // GPT-4 models
        m if m.starts_with("gpt-4") => 128_000,
        m if m.contains("gpt-4o") => 128_000,

        // GPT-3.5 models
        m if m.starts_with("gpt-3.5") => 16_385,

        // Claude models
        m if m.contains("claude-3-5-sonnet") => 200_000,
        m if m.contains("claude-3-5-haiku") => 200_000,
        m if m.contains("claude-3-opus") => 200_000,
        m if m.contains("claude-3-sonnet") => 200_000,
        m if m.contains("claude-3-haiku") => 200_000,

        // DeepSeek models
        m if m.contains("deepseek") => 128_000,

        // Gemini models
        m if m.contains("gemini-2") => 1_000_000,
        m if m.contains("gemini-1.5") => 1_000_000,
        m if m.contains("gemini-1") => 32_000,

        // LLaMA models
        m if m.contains("llama-3") => 128_000,
        m if m.contains("llama-2") => 4_096,

        // GLM models (glm-4, glm-4.5, glm-4.6, glm-4.7, glm-5, etc.)
        m if m.contains("glm") => 128_000,

        // Mistral models
        m if m.contains("mistral-large") => 128_000,
        m if m.contains("mistral-7b") => 32_768,
        m if m.contains("mixtral") => 32_768,

        // Default fallback
        _ => 8_192,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hello"), 1); // 5 chars / 4 = 1.25 -> 1
        assert_eq!(estimate_tokens("hello world"), 2); // 11 chars / 4 = 2.75 -> 2
        assert_eq!(estimate_tokens(&"a".repeat(100)), 25);
    }

    #[test]
    fn test_model_context_windows() {
        assert!(get_model_context_window("gpt-4o") >= 100_000);
        assert!(get_model_context_window("claude-3-5-sonnet") >= 100_000);
        assert!(get_model_context_window("deepseek-chat") >= 100_000);
        assert!(get_model_context_window("unknown") == 8_192);
    }

    #[test]
    fn test_context_fraction() {
        assert_eq!(estimate_context_fraction(0, 100_000), 0.0);
        assert_eq!(estimate_context_fraction(50_000, 100_000), 0.5);
        assert_eq!(estimate_context_fraction(100_000, 100_000), 1.0);
    }
}
