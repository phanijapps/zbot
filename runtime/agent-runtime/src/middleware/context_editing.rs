// ============================================================================
// CONTEXT EDITING MIDDLEWARE
// Manage conversation context by clearing older tool call outputs
//
// Inspired by LangChain's context editing middleware:
// https://docs.langchain.com/oss/javascript/langchain/middleware/built-in#context-editing
// ============================================================================

//! # Context Editing Middleware
//!
//! Manage conversation context by clearing older tool call outputs.

use crate::types::{ChatMessage, StreamEvent, ToolCall};
use crate::middleware::traits::{PreProcessMiddleware, MiddlewareContext, MiddlewareEffect};
use crate::middleware::config::ContextEditingConfig;
use crate::middleware::token_counter::estimate_total_tokens;
use serde_json::json;

/// Context editing middleware
///
/// Clears older tool call outputs when token limits are reached,
/// while preserving the most recent tool results.
pub struct ContextEditingMiddleware {
    /// Configuration
    config: ContextEditingConfig,
}

impl ContextEditingMiddleware {
    /// Create a new context editing middleware
    pub fn new(config: ContextEditingConfig) -> Self {
        Self {
            config,
        }
    }

    /// Find tool result messages that should be cleared
    fn find_tool_results_to_clear(&self, messages: &[ChatMessage]) -> Vec<usize> {
        let mut tool_result_indices = Vec::new();

        // Find all tool role messages (these contain tool results)
        for (idx, message) in messages.iter().enumerate() {
            if message.role == "tool" {
                // Check if this tool is in the exclude list
                if let Some(tool_call_id) = &message.tool_call_id {
                    // Find the corresponding tool call to get the tool name
                    let tool_name = self.find_tool_name_for_call(messages, idx, tool_call_id);

                    if let Some(name) = tool_name {
                        if self.config.exclude_tools.contains(&name) {
                            continue; // Don't clear this tool
                        }
                    }
                }

                tool_result_indices.push(idx);
            }
        }

        // Determine how many to keep vs clear
        let to_keep = self.config.keep_tool_results as usize;

        if tool_result_indices.len() > to_keep {
            // Clear all but the most recent N tool results
            tool_result_indices[..tool_result_indices.len() - to_keep].to_vec()
        } else {
            Vec::new() // Nothing to clear
        }
    }

    /// Find the tool name for a given tool call ID
    fn find_tool_name_for_call(
        &self,
        messages: &[ChatMessage],
        tool_result_idx: usize,
        tool_call_id: &str,
    ) -> Option<String> {
        // Search backwards from the tool result to find the assistant message
        // with the matching tool call
        for message in messages[..tool_result_idx].iter().rev() {
            if let Some(tool_calls) = &message.tool_calls {
                for tool_call in tool_calls {
                    if tool_call.id == *tool_call_id {
                        return Some(tool_call.name.clone());
                    }
                }
            }
        }
        None
    }

    /// Clear tool results by replacing content with placeholder
    fn clear_tool_results(&self, messages: &mut Vec<ChatMessage>, indices_to_clear: &[usize]) {
        for idx in indices_to_clear {
            if let Some(message) = messages.get_mut(*idx) {
                if message.role == "tool" {
                    message.content = self.config.placeholder.clone();

                    // Optionally clear tool call inputs from the assistant message
                    if self.config.clear_tool_inputs {
                        self.clear_tool_call_inputs(messages, *idx);
                    }
                }
            }
        }
    }

    /// Clear tool call inputs (arguments) from assistant messages
    fn clear_tool_call_inputs(&self, messages: &mut Vec<ChatMessage>, tool_result_idx: usize) {
        // First, extract the tool_call_id we need to find
        let tool_call_id_to_clear = messages.get(tool_result_idx)
            .and_then(|msg| msg.tool_call_id.as_ref())
            .map(|id| id.clone());

        if let Some(tool_call_id) = tool_call_id_to_clear {
            // Now we can mutably iterate through messages
            for message in messages.iter_mut() {
                if let Some(tool_calls) = &mut message.tool_calls {
                    for i in 0..tool_calls.len() {
                        if tool_calls[i].id == tool_call_id {
                            // Replace with a new tool call with empty arguments
                            tool_calls[i] = ToolCall::new(
                                tool_calls[i].id.clone(),
                                tool_calls[i].name.clone(),
                                json!( {})
                            );
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Calculate how many tokens would be reclaimed
    fn calculate_tokens_to_reclaim(&self, messages: &[ChatMessage], indices: &[usize]) -> usize {
        indices
            .iter()
            .filter_map(|idx| messages.get(*idx))
            .map(|msg| estimate_total_tokens(&[msg.clone()]))
            .sum()
    }
}

impl Default for ContextEditingMiddleware {
    fn default() -> Self {
        Self::new(ContextEditingConfig::default())
    }
}

#[async_trait::async_trait]
impl PreProcessMiddleware for ContextEditingMiddleware {
    fn name(&self) -> &'static str {
        "context_editing"
    }

    fn clone_box(&self) -> Box<dyn PreProcessMiddleware> {
        Box::new(Self {
            config: self.config.clone(),
        })
    }

    fn enabled(&self) -> bool {
        self.config.enabled
    }

    async fn process(
        &self,
        messages: Vec<ChatMessage>,
        _context: &MiddlewareContext,
    ) -> Result<MiddlewareEffect, String> {
        // Check if we should trigger context editing
        let current_tokens = estimate_total_tokens(&messages);

        if current_tokens < self.config.trigger_tokens {
            return Ok(MiddlewareEffect::Proceed);
        }

        // Find tool results to clear
        let indices_to_clear = self.find_tool_results_to_clear(&messages);

        if indices_to_clear.is_empty() {
            return Ok(MiddlewareEffect::Proceed);
        }

        // Check if we meet the minimum reclaim threshold
        let tokens_to_reclaim = self.calculate_tokens_to_reclaim(&messages, &indices_to_clear);
        if tokens_to_reclaim < self.config.min_reclaim {
            return Ok(MiddlewareEffect::Proceed);
        }

        // Clone messages for modification
        let mut modified_messages = messages.clone();

        // Clear the tool results
        self.clear_tool_results(&mut modified_messages, &indices_to_clear);

        // Create event about context editing
        let event = StreamEvent::Token {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            content: format!(
                "[Cleared {} tool results (reclaimed ~{} tokens)]",
                indices_to_clear.len(),
                tokens_to_reclaim
            ),
        };

        Ok(MiddlewareEffect::EmitAndModify {
            event,
            messages: modified_messages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_messages_with_tool_calls() -> Vec<ChatMessage> {
        let tool1 = ToolCall::new(
            "call_1".to_string(),
            "search".to_string(),
            json!({"query": "test"}),
        );

        let tool2 = ToolCall::new(
            "call_2".to_string(),
            "calculator".to_string(),
            json!({"expression": "1+1"}),
        );

        vec![
            ChatMessage {
                role: "user".to_string(),
                content: "Search for something".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "".to_string(),
                tool_calls: Some(vec![tool1, tool2]),
                tool_call_id: None,
            },
            ChatMessage {
                role: "tool".to_string(),
                content: "Search result: lots of text here that should be cleared when context editing kicks in".to_string(),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
            ChatMessage {
                role: "tool".to_string(),
                content: "2".to_string(),
                tool_calls: None,
                tool_call_id: Some("call_2".to_string()),
            },
        ]
    }

    #[test]
    fn test_find_tool_results_to_clear() {
        let config = ContextEditingConfig {
            enabled: true,
            trigger_tokens: 100,
            keep_tool_results: 1,
            min_reclaim: 0,
            clear_tool_inputs: false,
            exclude_tools: vec![],
            placeholder: "[cleared]".to_string(),
        };

        let middleware = ContextEditingMiddleware::new(config);
        let messages = create_test_messages_with_tool_calls();

        let indices = middleware.find_tool_results_to_clear(&messages);
        // Should clear the first tool result, keep the last one
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], 2); // Index of first tool result
    }

    #[test]
    fn test_exclude_tools() {
        let config = ContextEditingConfig {
            enabled: true,
            trigger_tokens: 100,
            keep_tool_results: 0, // Clear all
            min_reclaim: 0,
            clear_tool_inputs: false,
            exclude_tools: vec!["search".to_string()], // Don't clear search results
            placeholder: "[cleared]".to_string(),
        };

        let middleware = ContextEditingMiddleware::new(config);
        let messages = create_test_messages_with_tool_calls();

        let indices = middleware.find_tool_results_to_clear(&messages);
        // Should only clear calculator, not search
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], 3); // Index of calculator result
    }

    #[test]
    fn test_clear_tool_results() {
        let config = ContextEditingConfig {
            enabled: true,
            trigger_tokens: 100,
            keep_tool_results: 1,
            min_reclaim: 0,
            clear_tool_inputs: false,
            exclude_tools: vec![],
            placeholder: "[cleared]".to_string(),
        };

        let middleware = ContextEditingMiddleware::new(config);
        let mut messages = create_test_messages_with_tool_calls();

        let indices = middleware.find_tool_results_to_clear(&messages);
        middleware.clear_tool_results(&mut messages, &indices);

        // Check that the first tool result was cleared
        assert_eq!(messages[2].content, "[cleared]");
        // Check that the second tool result was NOT cleared
        assert_eq!(messages[3].content, "2");
    }
}
