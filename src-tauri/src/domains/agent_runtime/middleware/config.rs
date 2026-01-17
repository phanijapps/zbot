// ============================================================================
// MIDDLEWARE CONFIGURATION
// Configuration structures for middleware
// ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for all middleware from agent config.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareConfig {
    /// Summarization middleware configuration
    #[serde(default)]
    pub summarization: Option<SummarizationConfig>,

    /// Context editing middleware configuration
    #[serde(default)]
    pub context_editing: Option<ContextEditingConfig>,

    /// Additional custom middleware configurations
    #[serde(flatten)]
    pub custom: HashMap<String, serde_yaml::Value>,
}

impl Default for MiddlewareConfig {
    fn default() -> Self {
        Self {
            summarization: None,
            context_editing: None,
            custom: HashMap::new(),
        }
    }
}

/// Trigger conditions for middleware activation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TriggerCondition {
    /// Trigger when token count reaches this value
    pub tokens: Option<usize>,

    /// Trigger when message count reaches this value
    pub messages: Option<usize>,

    /// Trigger when fraction of context window is reached (0.0-1.0)
    pub fraction: Option<f64>,
}

impl TriggerCondition {
    pub fn should_trigger(&self, tokens: usize, messages: usize, context_window: usize) -> bool {
        if let Some(trigger_tokens) = self.tokens {
            if tokens >= trigger_tokens {
                return true;
            }
        }

        if let Some(trigger_messages) = self.messages {
            if messages >= trigger_messages {
                return true;
            }
        }

        if let Some(trigger_fraction) = self.fraction {
            let fraction = tokens as f64 / context_window.max(1) as f64;
            if fraction >= trigger_fraction {
                return true;
            }
        }

        false
    }

    /// Check if any trigger condition is set (for validation)
    pub fn is_valid(&self) -> bool {
        self.tokens.is_some() || self.messages.is_some() || self.fraction.is_some()
    }
}

/// Keep policy for middleware (what to preserve after processing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeepPolicy {
    /// Number of messages to keep
    pub messages: Option<usize>,

    /// Number of tokens to keep
    pub tokens: Option<usize>,

    /// Fraction of context window to keep (0.0-1.0)
    pub fraction: Option<f64>,
}

impl KeepPolicy {
    /// Get the number of items to keep based on policy
    pub fn to_keep_count(&self, total_items: usize, context_window: usize) -> usize {
        if let Some(keep_messages) = self.messages {
            return keep_messages.min(total_items);
        }

        if let Some(keep_tokens) = self.tokens {
            // Rough estimate: ~100 tokens per message on average
            return (keep_tokens / 100).min(total_items);
        }

        if let Some(keep_fraction) = self.fraction {
            return ((total_items as f64) * keep_fraction) as usize;
        }

        // Default: keep 20 messages
        20.min(total_items)
    }

    /// Check if any keep policy is set (for validation)
    pub fn is_valid(&self) -> bool {
        self.messages.is_some() || self.tokens.is_some() || self.fraction.is_some()
    }
}

/// Summarization middleware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizationConfig {
    /// Whether summarization is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Model to use for summarization (None = use agent's model)
    ///
    /// If not specified, defaults to the agent's model.
    /// You can specify a different/cheaper model like "gpt-4o-mini" for cost savings.
    pub model: Option<String>,

    /// Provider for summarization model (None = use agent's provider)
    ///
    /// If not specified, defaults to the agent's provider.
    /// You can specify a different provider if needed.
    pub provider: Option<String>,

    /// Trigger conditions for when to summarize
    #[serde(default)]
    pub trigger: TriggerCondition,

    /// Keep policy for what to preserve after summarization
    #[serde(default = "default_keep_policy")]
    pub keep: KeepPolicy,

    /// Maximum tokens to include when generating the summary
    #[serde(default = "default_summary_max_tokens")]
    pub summary_max_tokens: usize,

    /// Custom prompt template for summarization
    pub summary_prompt: Option<String>,

    /// Prefix to add to the summary message
    #[serde(default = "default_summary_prefix")]
    pub summary_prefix: String,

    /// Custom token counting function name (reserved for future use)
    #[serde(skip)]
    pub custom_token_counter: Option<String>,
}

impl Default for SummarizationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: None,
            provider: None,
            trigger: TriggerCondition {
                tokens: Some(8000),
                messages: None,
                fraction: None,
            },
            keep: default_keep_policy(),
            summary_max_tokens: 16000,
            summary_prompt: None,
            summary_prefix: default_summary_prefix(),
            custom_token_counter: None,
        }
    }
}

/// Context editing middleware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEditingConfig {
    /// Whether context editing is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Trigger when token count reaches this value
    #[serde(default = "default_trigger_tokens")]
    pub trigger_tokens: usize,

    /// Number of recent tool results to preserve
    #[serde(default = "default_keep_tool_results")]
    pub keep_tool_results: usize,

    /// Minimum number of tokens to reclaim when editing
    #[serde(default = "default_min_reclaim")]
    pub min_reclaim: usize,

    /// Whether to clear tool call arguments (not just results)
    #[serde(default)]
    pub clear_tool_inputs: bool,

    /// Tool names to exclude from clearing
    #[serde(default)]
    pub exclude_tools: Vec<String>,

    /// Placeholder text to insert for cleared outputs
    #[serde(default = "default_placeholder")]
    pub placeholder: String,
}

impl Default for ContextEditingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            trigger_tokens: 10000,
            keep_tool_results: 3,
            min_reclaim: 0,
            clear_tool_inputs: false,
            exclude_tools: Vec::new(),
            placeholder: default_placeholder(),
        }
    }
}

// Default functions

fn default_enabled() -> bool {
    false
}

fn default_keep_policy() -> KeepPolicy {
    KeepPolicy {
        messages: Some(10),
        tokens: None,
        fraction: None,
    }
}

fn default_summary_max_tokens() -> usize {
    16000
}

fn default_summary_prefix() -> String {
    "[Previous conversation summarized]".to_string()
}

fn default_trigger_tokens() -> usize {
    10000
}

fn default_keep_tool_results() -> usize {
    3
}

fn default_min_reclaim() -> usize {
    0
}

fn default_placeholder() -> String {
    "[cleared]".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_condition() {
        let trigger = TriggerCondition {
            tokens: Some(100),
            messages: Some(10),
            fraction: Some(0.5),
        };

        // Should trigger on tokens
        assert!(trigger.should_trigger(100, 5, 1000));

        // Should trigger on messages
        assert!(trigger.should_trigger(50, 10, 1000));

        // Should trigger on fraction (50% of 1000 = 500)
        assert!(trigger.should_trigger(500, 5, 1000));

        // Should not trigger
        assert!(!trigger.should_trigger(50, 5, 1000));
    }

    #[test]
    fn test_keep_policy() {
        let keep = KeepPolicy {
            messages: Some(10),
            tokens: None,
            fraction: None,
        };

        assert_eq!(keep.to_keep_count(100, 100000), 10);
        assert_eq!(keep.to_keep_count(5, 100000), 5);
    }

    #[test]
    fn test_deserialize_summarization_config() {
        let yaml = r#"
enabled: true
model: gpt-4o-mini
provider: openai
trigger:
  tokens: 8000
  messages: 15
keep:
  messages: 10
summary_max_tokens: 16000
"#;

        let config: SummarizationConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.model, Some("gpt-4o-mini".to_string()));
        assert_eq!(config.trigger.tokens, Some(8000));
        assert_eq!(config.keep.messages, Some(10));
    }
}
