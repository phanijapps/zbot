// ============================================================================
// MIDDLEWARE CONFIGURATION
// Configuration structures for middleware
// ============================================================================

//! # Middleware Configuration
//!
//! Configuration types for middleware components.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for all middleware from agent config.yaml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn to_keep_count(&self, total_items: usize, _context_window: usize) -> usize {
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
    #[must_use]
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

    // =========================================================================
    // SKILL-AWARE EDITING OPTIONS
    // =========================================================================
    /// Whether to use skill-specific placeholder messages when unloading skills.
    /// When true, unloaded skills show a message like:
    /// "[Skill 'skill-name' was loaded and unloaded. Reload with `load_skill(skill`=\"skill-name\") if needed.]"
    /// When false, uses the generic placeholder for all tool results.
    #[serde(default = "default_skill_aware_placeholders")]
    pub skill_aware_placeholders: bool,

    /// Whether to cascade unload resources when a skill's SKILL.md is unloaded.
    /// When true, all resource files loaded by the skill are also unloaded.
    /// When false, only the SKILL.md content is cleared; resources remain until
    /// selected for clearing by the regular tool result clearing logic.
    #[serde(default = "default_cascade_unload")]
    pub cascade_unload: bool,

    /// Custom template for skill placeholder messages.
    /// Available variables: {`skill_name`}
    /// Default: "[Skill '{`skill_name`}' was loaded and unloaded. Reload with `load_skill(skill`=\"{`skill_name`}\") if needed.]"
    #[serde(default)]
    pub skill_placeholder_template: Option<String>,

    /// Custom template for skill resource placeholder messages.
    /// Available variables: {`skill_name`}
    /// Default: "[Resource from skill '{`skill_name`}' was unloaded.]"
    #[serde(default)]
    pub resource_placeholder_template: Option<String>,
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
            skill_aware_placeholders: true,
            cascade_unload: true,
            skill_placeholder_template: None,
            resource_placeholder_template: None,
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

fn default_skill_aware_placeholders() -> bool {
    true
}

fn default_cascade_unload() -> bool {
    true
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

        assert_eq!(keep.to_keep_count(100, 100_000), 10);
        assert_eq!(keep.to_keep_count(5, 100_000), 5);
    }

    #[test]
    fn test_deserialize_summarization_config() {
        let yaml = r"
enabled: true
model: gpt-4o-mini
provider: openai
trigger:
  tokens: 8000
  messages: 15
keep:
  messages: 10
summary_max_tokens: 16000
";

        let config: SummarizationConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.model, Some("gpt-4o-mini".to_string()));
        assert_eq!(config.trigger.tokens, Some(8000));
        assert_eq!(config.keep.messages, Some(10));
    }

    #[test]
    fn trigger_condition_messages_only() {
        let t = TriggerCondition {
            tokens: None,
            messages: Some(5),
            fraction: None,
        };
        assert!(t.is_valid());
        assert!(t.should_trigger(0, 5, 1000));
        assert!(!t.should_trigger(100, 4, 1000));
    }

    #[test]
    fn trigger_condition_fraction_only() {
        let t = TriggerCondition {
            tokens: None,
            messages: None,
            fraction: Some(0.25),
        };
        assert!(t.is_valid());
        assert!(t.should_trigger(250, 0, 1000)); // 25%
        assert!(!t.should_trigger(100, 0, 1000)); // 10%
    }

    #[test]
    fn trigger_condition_zero_window_does_not_panic() {
        let t = TriggerCondition {
            tokens: None,
            messages: None,
            fraction: Some(0.5),
        };
        // Window 0 → max(1) avoids div by zero. 1 >= 0.5 → triggers.
        assert!(t.should_trigger(1, 0, 0));
    }

    #[test]
    fn trigger_condition_invalid_when_empty() {
        let t = TriggerCondition::default();
        assert!(!t.is_valid());
        assert!(!t.should_trigger(0, 0, 1000));
    }

    #[test]
    fn keep_policy_tokens_branch() {
        let k = KeepPolicy {
            messages: None,
            tokens: Some(500),
            fraction: None,
        };
        // 500 / 100 = 5, capped by total
        assert_eq!(k.to_keep_count(20, 100_000), 5);
        assert_eq!(k.to_keep_count(3, 100_000), 3);
        assert!(k.is_valid());
    }

    #[test]
    fn keep_policy_fraction_branch() {
        let k = KeepPolicy {
            messages: None,
            tokens: None,
            fraction: Some(0.25),
        };
        assert_eq!(k.to_keep_count(20, 100_000), 5);
        assert!(k.is_valid());
    }

    #[test]
    fn keep_policy_default_is_20_or_total() {
        let k = KeepPolicy {
            messages: None,
            tokens: None,
            fraction: None,
        };
        assert_eq!(k.to_keep_count(50, 100_000), 20);
        assert_eq!(k.to_keep_count(5, 100_000), 5);
        assert!(!k.is_valid());
    }

    #[test]
    fn defaults_for_summarization_and_context_editing() {
        let s = SummarizationConfig::default();
        assert!(!s.enabled);
        assert_eq!(s.summary_max_tokens, 16000);
        assert_eq!(s.summary_prefix, "[Previous conversation summarized]");
        assert!(s.model.is_none());
        assert!(s.provider.is_none());

        let c = ContextEditingConfig::default();
        assert!(!c.enabled);
        assert_eq!(c.trigger_tokens, 10000);
        assert_eq!(c.keep_tool_results, 3);
        assert_eq!(c.placeholder, "[cleared]");
        assert!(c.skill_aware_placeholders);
        assert!(c.cascade_unload);
    }

    #[test]
    fn middleware_config_default_is_all_none() {
        let m = MiddlewareConfig::default();
        assert!(m.summarization.is_none());
        assert!(m.context_editing.is_none());
        assert!(m.custom.is_empty());
    }

    #[test]
    fn deserialize_context_editing_config() {
        let yaml = r"
enabled: true
trigger_tokens: 5000
keep_tool_results: 5
clear_tool_inputs: true
exclude_tools: [search]
";
        let cfg: ContextEditingConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.trigger_tokens, 5000);
        assert_eq!(cfg.keep_tool_results, 5);
        assert!(cfg.clear_tool_inputs);
        assert_eq!(cfg.exclude_tools, vec!["search".to_string()]);
    }
}
