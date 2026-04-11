//! # LLM Configuration
//!
//! Configuration types for LLM clients.

use serde::{Deserialize, Serialize};

/// Configuration for an LLM client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// API key for authentication.
    pub api_key: String,

    /// Model to use (e.g., "gpt-4o-mini").
    pub model: String,

    /// Optional custom base URL for OpenAI-compatible APIs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Optional organization ID (for OpenAI).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,

    /// Default temperature for requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Default max tokens for requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

impl LlmConfig {
    /// Create a new config with API key and model.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            base_url: None,
            organization_id: None,
            temperature: None,
            max_tokens: None,
        }
    }

    /// Create a config for an OpenAI-compatible API.
    ///
    /// # Example
    ///
    /// ```
    /// use zero_llm::LlmConfig;
    ///
    /// // DeepSeek
    /// let config = LlmConfig::compatible(
    ///     "sk-...",
    ///     "https://api.deepseek.com",
    ///     "deepseek-chat"
    /// );
    ///
    /// // Groq
    /// let config = LlmConfig::compatible(
    ///     "gsk_...",
    ///     "https://api.groq.com/openai/v1",
    ///     "llama-3.3-70b-versatile"
    /// );
    /// ```
    pub fn compatible(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            base_url: Some(base_url.into()),
            organization_id: None,
            temperature: None,
            max_tokens: None,
        }
    }

    /// Set organization ID.
    pub fn with_organization_id(mut self, id: impl Into<String>) -> Self {
        self.organization_id = Some(id.into());
        self
    }

    /// Set default temperature.
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set default max tokens.
    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Get the base URL to use for requests.
    pub fn base_url(&self) -> &str {
        self.base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = LlmConfig::new("sk-test", "gpt-4o-mini");
        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.base_url(), "https://api.openai.com/v1");
    }

    #[test]
    fn test_config_compatible() {
        let config = LlmConfig::compatible("sk-test", "https://api.example.com", "model");
        assert_eq!(config.base_url, Some("https://api.example.com".to_string()));
        assert_eq!(config.base_url(), "https://api.example.com");
    }

    #[test]
    fn test_config_builder() {
        let config = LlmConfig::new("key", "model")
            .with_temperature(0.7)
            .with_max_tokens(1000);

        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.max_tokens, Some(1000));
    }
}
