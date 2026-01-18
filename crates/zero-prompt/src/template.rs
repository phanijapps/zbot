//! # Template System
//!
//! Template rendering with variable injection.

use crate::error::{PromptError, Result};
use regex::Regex;
use std::sync::OnceLock;
use zero_core::CallbackContext;

/// Regex pattern to match template placeholders like {variable} or {artifact.file_name}
/// Matches: { + any chars except {} + optional ? + }
/// This is more permissive so we can validate the content and provide better errors.
static PLACEHOLDER_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_placeholder_regex() -> &'static Regex {
    PLACEHOLDER_REGEX.get_or_init(|| {
        // Match: { + any chars except {} + optional ? + }
        Regex::new(r"\{[^{}]*\??\}").expect("Invalid regex pattern")
    })
}

/// Checks if a string is a valid identifier (like Python's str.isidentifier())
/// Must start with letter or underscore, followed by letters, digits, or underscores
fn is_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();
    let first = chars.next().unwrap();

    if !first.is_alphabetic() && first != '_' {
        return false;
    }

    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Checks if a variable name is a valid state name.
/// Supports prefixes: app:, user:, temp:
fn is_valid_state_name(var_name: &str) -> bool {
    let parts: Vec<&str> = var_name.split(':').collect();

    match parts.len() {
        1 => is_identifier(var_name),
        2 => {
            let prefix = format!("{}:", parts[0]);
            let valid_prefixes = ["app:", "user:", "temp:"];
            valid_prefixes.contains(&prefix.as_str()) && is_identifier(parts[1])
        }
        _ => false,
    }
}

/// Replaces a single placeholder match with its resolved value.
/// Handles {var}, {var?}, and {artifact.name} syntax.
async fn replace_match(ctx: &dyn CallbackContext, match_str: &str) -> Result<String> {
    // Trim curly braces: "{var_name}" -> "var_name"
    let var_name = match_str.trim_matches(|c| c == '{' || c == '}').trim();

    // Check if optional (ends with ?)
    let (var_name, optional) =
        if let Some(name) = var_name.strip_suffix('?') {
            (name, true)
        } else {
            (var_name, false)
        };

    // Handle artifact.{name} pattern
    if let Some(_file_name) = var_name.strip_prefix("artifact.") {
        // For now, we don't have artifact support
        // Return empty string for optional artifacts, error otherwise
        if optional {
            Ok(String::new())
        } else {
            Err(PromptError::VariableNotFound {
                name: var_name.to_string(),
            })
        }
    } else if is_valid_state_name(var_name) {
        // Handle session state variable
        let state_value = ctx.get_state(var_name);

        match state_value {
            Some(value) => {
                // Convert value to string
                if let Some(s) = value.as_str() {
                    Ok(s.to_string())
                } else {
                    Ok(value.to_string())
                }
            }
            None => {
                if optional {
                    Ok(String::new())
                } else {
                    Err(PromptError::VariableNotFound {
                        name: var_name.to_string(),
                    })
                }
            }
        }
    } else {
        // Not a valid variable name - return original match as literal
        Ok(match_str.to_string())
    }
}

/// Template for prompt rendering with variable injection.
#[derive(Clone, Debug)]
pub struct Template {
    /// The raw template string
    raw: String,
}

impl Template {
    /// Create a new template from a string.
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            raw: template.into(),
        }
    }

    /// Get the raw template string.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Parse placeholders from the template.
    ///
    /// Returns a list of variable names (without the curly braces).
    pub fn parse_placeholders(&self) -> Vec<String> {
        let regex = get_placeholder_regex();
        let mut placeholders = Vec::new();

        for captures in regex.find_iter(&self.raw) {
            let match_str = captures.as_str();
            let var_name = match_str
                .trim_matches(|c| c == '{' || c == '}')
                .trim()
                .trim_end_matches('?');
            placeholders.push(var_name.to_string());
        }

        placeholders
    }

    /// Validate that all required placeholders have valid names.
    ///
    /// Returns an error if any placeholder has an invalid name.
    pub fn validate(&self) -> Result<()> {
        let regex = get_placeholder_regex();

        for captures in regex.find_iter(&self.raw) {
            let match_str = captures.as_str();
            let var_name = match_str
                .trim_matches(|c| c == '{' || c == '}')
                .trim();

            // Skip artifact references - they're handled differently
            if var_name.starts_with("artifact.") {
                continue;
            }

            // Remove optional marker
            let var_name = var_name.trim_end_matches('?');

            if !is_valid_state_name(var_name) {
                return Err(PromptError::InvalidVariable {
                    name: var_name.to_string(),
                });
            }
        }

        Ok(())
    }
}

impl From<String> for Template {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for Template {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Renders templates with variable injection.
pub struct TemplateRenderer {
    _priv: (),
}

impl TemplateRenderer {
    /// Create a new renderer.
    pub fn new() -> Self {
        Self { _priv: () }
    }

    /// Render a template with session state injection.
    ///
    /// Supports the following placeholder syntax:
    /// - `{var_name}` - Required session state variable (errors if missing)
    /// - `{var_name?}` - Optional variable (empty string if missing)
    /// - `{artifact.file_name}` - Artifact content insertion (future support)
    /// - `{app:var}`, `{user:var}`, `{temp:var}` - Prefixed state variables
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let template = Template::new("Hello {user_name}, your score is {score}");
    /// let result = renderer.render(ctx, &template).await?;
    /// // Result: "Hello Alice, your score is 100"
    /// ```
    pub async fn render(
        &self,
        ctx: &dyn CallbackContext,
        template: &Template,
    ) -> Result<String> {
        let regex = get_placeholder_regex();
        // Pre-allocate 20% extra capacity to reduce reallocations when placeholders expand
        let mut result = String::with_capacity((template.raw.len() as f32 * 1.2) as usize);
        let mut last_end = 0;

        for captures in regex.find_iter(&template.raw) {
            let match_range = captures.range();

            // Append text between last match and this one
            result.push_str(&template.raw[last_end..match_range.start]);

            // Get the replacement for the current match
            let match_str = captures.as_str();
            let replacement = replace_match(ctx, match_str).await?;
            result.push_str(&replacement);

            last_end = match_range.end;
        }

        // Append any remaining text
        result.push_str(&template.raw[last_end..]);

        Ok(result)
    }
}

impl Default for TemplateRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Injects session state values into a template string.
///
/// This is a convenience function that creates a template and renders it.
///
/// # Arguments
///
/// * `ctx` - The callback context with state access
/// * `template_str` - The template string with placeholders
///
/// # Returns
///
/// The rendered template with variables replaced.
///
/// # Example
///
/// ```ignore
/// let result = inject_session_state(ctx, "Hello {user:name}").await?;
/// // Returns: "Hello Alice" if user:name is "Alice"
/// ```
pub async fn inject_session_state(
    ctx: &dyn CallbackContext,
    template_str: &str,
) -> Result<String> {
    let template = Template::new(template_str);
    let renderer = TemplateRenderer::new();
    renderer.render(ctx, &template).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::{CallbackContext, ReadonlyContext};
    use zero_core::types::Content;
    use serde_json::Value;

    struct MockContext;

    impl ReadonlyContext for MockContext {
        fn invocation_id(&self) -> &str { "test" }
        fn agent_name(&self) -> &str { "test" }
        fn user_id(&self) -> &str { "test" }
        fn app_name(&self) -> &str { "test" }
        fn session_id(&self) -> &str { "test" }
        fn branch(&self) -> &str { "test" }
        fn user_content(&self) -> &Content {
            static CONTENT: Content = Content {
                role: String::new(),
                parts: Vec::new(),
            };
            &CONTENT
        }
    }

    impl CallbackContext for MockContext {
        fn get_state(&self, key: &str) -> Option<Value> {
            match key {
                "user_name" => Some(Value::String("Alice".to_string())),
                "score" => Some(Value::Number(100.into())),
                "app:theme" => Some(Value::String("dark".to_string())),
                _ => None,
            }
        }

        fn set_state(&self, _key: String, _value: Value) {
            // No-op for tests
        }
    }

    #[tokio::test]
    async fn test_template_basic() {
        let template = Template::new("Hello {user_name}");
        let renderer = TemplateRenderer::new();
        let ctx = MockContext;

        let result = renderer.render(&ctx, &template).await.unwrap();
        assert_eq!(result, "Hello Alice");
    }

    #[test]
    fn test_is_identifier() {
        assert!(is_identifier("valid_name"));
        assert!(is_identifier("_private"));
        assert!(is_identifier("name123"));
        assert!(!is_identifier("123invalid"));
        assert!(!is_identifier(""));
        assert!(!is_identifier("with-dash"));
    }

    #[test]
    fn test_is_valid_state_name() {
        assert!(is_valid_state_name("valid_var"));
        assert!(is_valid_state_name("app:config"));
        assert!(is_valid_state_name("user:preference"));
        assert!(is_valid_state_name("temp:data"));
        assert!(!is_valid_state_name("invalid:prefix"));
        assert!(!is_valid_state_name("app:invalid-name"));
        assert!(!is_valid_state_name("too:many:parts"));
    }

    #[test]
    fn test_template_parse_placeholders() {
        let template = Template::new("Hello {user_name}, your score is {score}");
        let placeholders = template.parse_placeholders();
        assert_eq!(placeholders, vec!["user_name", "score"]);
    }

    #[test]
    fn test_template_validate() {
        let template = Template::new("Hello {user_name}");
        assert!(template.validate().is_ok());

        let invalid = Template::new("Hello {123invalid}");
        assert!(invalid.validate().is_err());
    }

    #[tokio::test]
    async fn test_optional_variable() {
        let template = Template::new("Hello {missing_var?}");
        let renderer = TemplateRenderer::new();
        let ctx = MockContext;

        let result = renderer.render(&ctx, &template).await.unwrap();
        assert_eq!(result, "Hello ");
    }

    #[tokio::test]
    async fn test_required_variable_missing() {
        let template = Template::new("Hello {missing_var}");
        let renderer = TemplateRenderer::new();
        let ctx = MockContext;

        let result = renderer.render(&ctx, &template).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_prefixed_variable() {
        let template = Template::new("Theme: {app:theme}");
        let renderer = TemplateRenderer::new();
        let ctx = MockContext;

        let result = renderer.render(&ctx, &template).await.unwrap();
        assert_eq!(result, "Theme: dark");
    }

    #[tokio::test]
    async fn test_inject_session_state() {
        let ctx = MockContext;
        let result = inject_session_state(&ctx, "Hello {user_name}").await.unwrap();
        assert_eq!(result, "Hello Alice");
    }

    #[test]
    fn test_template_from_string() {
        let template = Template::from("Hello {world}");
        assert_eq!(template.as_str(), "Hello {world}");

        let template = Template::from("Hello {world}".to_string());
        assert_eq!(template.as_str(), "Hello {world}");
    }
}
