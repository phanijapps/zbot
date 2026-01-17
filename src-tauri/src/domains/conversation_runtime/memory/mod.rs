// ============================================================================
// MEMORY MANAGEMENT
// Context window management and summarization
// ============================================================================

use crate::domains::conversation_runtime::repository::{Message, MessageRole};

/// Configuration for memory management
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    /// Maximum tokens in context window
    pub max_tokens: i64,
    /// Reserved tokens for system prompt and response
    pub reserved_tokens: i64,
    /// Threshold for triggering summarization
    pub summarization_threshold: f64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_tokens: 4000,
            reserved_tokens: 500,
            summarization_threshold: 0.8,
        }
    }
}

/// Calculate available tokens for messages
pub fn calculate_available_tokens(config: &MemoryConfig) -> i64 {
    config.max_tokens - config.reserved_tokens
}

/// Check if summarization is needed
pub fn needs_summarization(config: &MemoryConfig, current_tokens: i64) -> bool {
    let available = calculate_available_tokens(config);
    (current_tokens as f64) > (available as f64 * config.summarization_threshold)
}

/// Filter messages to fit within token budget
pub fn filter_messages_by_tokens(messages: Vec<Message>, max_tokens: i64) -> Vec<Message> {
    let mut system_messages = Vec::new();
    let mut recent_messages = Vec::new();
    let mut system_tokens = 0i64;
    let mut recent_tokens = 0i64;

    // Separate system messages from other messages
    for msg in &messages {
        if matches!(msg.role, MessageRole::System) {
            system_messages.push(msg.clone());
            system_tokens += msg.token_count;
        }
    }

    // Add recent messages (in reverse) until we hit the limit
    for msg in messages.into_iter().rev() {
        if matches!(msg.role, MessageRole::System) {
            continue;
        }

        if system_tokens + recent_tokens + msg.token_count > max_tokens && !recent_messages.is_empty() {
            break;
        }

        recent_tokens += msg.token_count;
        recent_messages.push(msg);
    }

    // Reverse recent messages to get chronological order
    recent_messages.reverse();

    // Combine: system messages first, then recent messages
    let mut result = system_messages;
    result.extend(recent_messages);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_available_tokens() {
        let config = MemoryConfig::default();
        let available = calculate_available_tokens(&config);
        assert_eq!(available, 3500); // 4000 - 500
    }

    #[test]
    fn test_needs_summarization() {
        let config = MemoryConfig::default();
        // 80% of 3500 = 2800
        assert!(needs_summarization(&config, 2900));
        assert!(!needs_summarization(&config, 2000));
    }

    #[test]
    fn test_filter_messages_by_tokens() {
        let messages = vec![
            Message {
                id: "1".to_string(),
                conversation_id: "conv1".to_string(),
                role: MessageRole::System,
                content: "System prompt".to_string(),
                created_at: "".to_string(),
                token_count: 100,
                tool_calls: None,
                tool_results: None,
            },
            Message {
                id: "2".to_string(),
                conversation_id: "conv1".to_string(),
                role: MessageRole::User,
                content: "Hello".to_string(),
                created_at: "".to_string(),
                token_count: 10,
                tool_calls: None,
                tool_results: None,
            },
            Message {
                id: "3".to_string(),
                conversation_id: "conv1".to_string(),
                role: MessageRole::Assistant,
                content: "Hi there!".to_string(),
                created_at: "".to_string(),
                token_count: 15,
                tool_calls: None,
                tool_results: None,
            },
        ];

        let filtered = filter_messages_by_tokens(messages, 200);

        // System message should always be included
        assert_eq!(filtered[0].id, "1");
        // Recent messages should be included
        assert_eq!(filtered.len(), 3);
    }
}
