//! # State Management
//!
//! In-memory key-value state storage with key prefix support.

use std::collections::HashMap;
use serde_json::Value;
use zero_core::{Result, ZeroError};
use zero_core::context::State;

/// In-memory state storage.
///
/// Supports key prefixes for scoping:
/// - `user:` - User-scoped state
/// - `app:` - Application-scoped state
/// - `temp:` - Temporary state (cleared between turns)
#[derive(Debug, Clone, Default)]
pub struct InMemoryState {
    data: HashMap<String, Value>,
}

impl InMemoryState {
    /// Create a new empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create state from a HashMap.
    pub fn from_map(data: HashMap<String, Value>) -> Self {
        Self { data }
    }

    /// Get a value by key with prefix support.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    /// Set a value.
    pub fn set(&mut self, key: String, value: Value) {
        self.data.insert(key, value);
    }

    /// Get all values.
    pub fn all(&self) -> &HashMap<String, Value> {
        &self.data
    }

    /// Get all values with a specific prefix.
    ///
    /// # Example
    ///
    /// ```
    /// use zero_session::InMemoryState;
    ///
    /// let mut state = InMemoryState::new();
    /// state.set("user:name".to_string(), serde_json::json!("Alice"));
    /// state.set("user:theme".to_string(), serde_json::json!("dark"));
    /// state.set("app:version".to_string(), serde_json::json!("1.0"));
    ///
    /// let user_data = state.get_by_prefix("user:");
    /// assert_eq!(user_data.len(), 2);
    /// ```
    pub fn get_by_prefix(&self, prefix: &str) -> HashMap<String, Value> {
        self.data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Clear all values with a specific prefix.
    ///
    /// Useful for clearing temporary state between turns.
    pub fn clear_prefix(&mut self, prefix: &str) {
        self.data.retain(|k, _| !k.starts_with(prefix));
    }

    /// Merge another state into this one.
    pub fn merge(&mut self, other: InMemoryState) {
        for (key, value) in other.data {
            self.data.insert(key, value);
        }
    }
}

impl State for InMemoryState {
    fn get(&self, key: &str) -> Option<Value> {
        self.data.get(key).cloned()
    }

    fn set(&mut self, key: String, value: Value) {
        self.data.insert(key, value);
    }

    fn all(&self) -> HashMap<String, Value> {
        self.data.clone()
    }
}

/// Helper function to validate and normalize state keys.
///
/// Ensures keys use proper prefixes (user:, app:, temp:).
pub fn validate_key(key: &str) -> Result<String> {
    let has_prefix = key.starts_with("user:")
        || key.starts_with("app:")
        || key.starts_with("temp:");

    if !has_prefix {
        return Err(ZeroError::Config(format!(
            "State key must have prefix (user:, app:, temp:), got: {}",
            key
        )));
    }

    Ok(key.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::KEY_PREFIX_USER;
    use zero_core::KEY_PREFIX_APP;

    #[test]
    fn test_state_new() {
        let state = InMemoryState::new();
        assert!(state.all().is_empty());
    }

    #[test]
    fn test_state_get_set() {
        let mut state = InMemoryState::new();
        state.set("key".to_string(), serde_json::json!("value"));
        assert_eq!(state.get("key"), Some(&serde_json::json!("value")));
    }

    #[test]
    fn test_state_trait() {
        let mut state = InMemoryState::new();
        <dyn State>::set(&mut state, "test".to_string(), serde_json::json!(42));
        assert_eq!(<dyn State>::get(&state, "test"), Some(serde_json::json!(42)));
    }

    #[test]
    fn test_state_prefixes() {
        let mut state = InMemoryState::new();
        state.set(format!("{}name", KEY_PREFIX_USER), serde_json::json!("Alice"));
        state.set(format!("{}theme", KEY_PREFIX_USER), serde_json::json!("dark"));
        state.set(format!("{}version", KEY_PREFIX_APP), serde_json::json!("1.0"));

        let user_data = state.get_by_prefix(KEY_PREFIX_USER);
        assert_eq!(user_data.len(), 2);
    }

    #[test]
    fn test_state_clear_prefix() {
        let mut state = InMemoryState::new();
        state.set("temp:scratch".to_string(), serde_json::json!("data"));
        state.set("user:name".to_string(), serde_json::json!("Alice"));

        state.clear_prefix("temp:");
        assert!(state.get("temp:scratch").is_none());
        assert_eq!(state.get("user:name"), Some(&serde_json::json!("Alice")));
    }

    #[test]
    fn test_state_merge() {
        let mut state1 = InMemoryState::new();
        state1.set("key1".to_string(), serde_json::json!("value1"));

        let mut state2 = InMemoryState::new();
        state2.set("key2".to_string(), serde_json::json!("value2"));

        state1.merge(state2);
        assert_eq!(state1.get("key1"), Some(&serde_json::json!("value1")));
        assert_eq!(state1.get("key2"), Some(&serde_json::json!("value2")));
    }

    #[test]
    fn test_validate_key_valid() {
        assert!(validate_key("user:name").is_ok());
        assert!(validate_key("app:version").is_ok());
        assert!(validate_key("temp:scratch").is_ok());
    }

    #[test]
    fn test_validate_key_invalid() {
        assert!(validate_key("name").is_err());
        assert!(validate_key("config:setting").is_err());
    }
}
