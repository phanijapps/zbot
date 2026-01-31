//! # Tool Filtering
//!
//! Predicate-based tool filtering for MCP servers.

use super::client::McpToolDefinition;

/// Predicate function for filtering tools.
pub type ToolPredicate = Box<dyn Fn(&McpToolDefinition) -> bool + Send + Sync>;

/// Tool filter builder.
pub struct ToolFilter {
    predicates: Vec<ToolPredicate>,
}

impl ToolFilter {
    /// Create a new tool filter.
    pub fn new() -> Self {
        Self {
            predicates: Vec::new(),
        }
    }

    /// Add a name filter (exact match).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.predicates
            .push(Box::new(move |tool: &McpToolDefinition| tool.name == name));
        self
    }

    /// Add a name pattern filter (prefix match).
    pub fn with_name_prefix(mut self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        self.predicates.push(Box::new(move |tool: &McpToolDefinition| {
            tool.name.starts_with(&prefix)
        }));
        self
    }

    /// Add a name pattern filter (suffix match).
    pub fn with_name_suffix(mut self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.predicates.push(Box::new(move |tool: &McpToolDefinition| {
            tool.name.ends_with(&suffix)
        }));
        self
    }

    /// Add a name pattern filter (contains).
    pub fn with_name_contains(mut self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.predicates
            .push(Box::new(move |tool: &McpToolDefinition| tool.name.contains(&pattern)));
        self
    }

    /// Add a description filter (contains).
    pub fn with_description_contains(mut self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        self.predicates
            .push(Box::new(move |tool: &McpToolDefinition| {
                tool.description.to_lowercase().contains(&pattern.to_lowercase())
            }));
        self
    }

    /// Add a custom predicate.
    pub fn with_predicate(mut self, predicate: ToolPredicate) -> Self {
        self.predicates.push(predicate);
        self
    }

    /// Add an "OR" combination - matches if any predicate matches.
    pub fn or(mut self, other: ToolFilter) -> Self {
        let self_predicates = self.predicates;
        let other_predicates = other.predicates;

        self.predicates = vec![Box::new(move |tool: &McpToolDefinition| {
            self_predicates.iter().any(|p| p(tool)) || other_predicates.iter().any(|p| p(tool))
        })];

        self
    }

    /// Apply all predicates to a tool.
    pub fn matches(&self, tool: &McpToolDefinition) -> bool {
        // Empty filter matches everything
        if self.predicates.is_empty() {
            return true;
        }

        // All predicates must match (AND behavior)
        self.predicates.iter().all(|p| p(tool))
    }

    /// Filter a list of tools.
    pub fn filter(&self, tools: Vec<McpToolDefinition>) -> Vec<McpToolDefinition> {
        tools.into_iter().filter(|t| self.matches(t)).collect()
    }
}

impl Default for ToolFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for common tool filters.

/// Accept all tools.
pub fn accept_all() -> ToolFilter {
    ToolFilter::new()
}

/// Accept no tools.
pub fn accept_none() -> ToolFilter {
    ToolFilter::new().with_predicate(Box::new(|_| false))
}

/// Filter tools by name patterns.
pub fn by_names(names: Vec<String>) -> ToolFilter {
    ToolFilter::new().with_predicate(Box::new(move |tool| {
        names.iter().any(|n| tool.name == *n)
    }))
}

/// Filter tools by name prefix.
pub fn by_prefix(prefix: impl Into<String>) -> ToolFilter {
    ToolFilter::new().with_name_prefix(prefix)
}

/// Filter tools excluding certain names.
pub fn exclude_names(names: Vec<String>) -> ToolFilter {
    ToolFilter::new().with_predicate(Box::new(move |tool| {
        !names.iter().any(|n| tool.name == *n)
    }))
}

/// Filter tools to only those that have a specific property in their schema.
pub fn with_property(property: impl Into<String>) -> ToolFilter {
    let prop = property.into();
    ToolFilter::new().with_predicate(Box::new(move |tool| {
        tool.input_schema.get("properties")
            .and_then(|p| p.as_object())
            .map(|props| props.contains_key(&prop))
            .unwrap_or(false)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_tool(name: &str, description: &str) -> McpToolDefinition {
        McpToolDefinition {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "arg1": {"type": "string"}
                }
            }),
        }
    }

    #[test]
    fn test_filter_with_name() {
        let filter = ToolFilter::new().with_name("test_tool");

        assert!(filter.matches(&create_test_tool("test_tool", "A tool")));
        assert!(!filter.matches(&create_test_tool("other_tool", "A tool")));
    }

    #[test]
    fn test_filter_with_prefix() {
        let filter = ToolFilter::new().with_name_prefix("test_");

        assert!(filter.matches(&create_test_tool("test_tool", "A tool")));
        assert!(filter.matches(&create_test_tool("test_another", "A tool")));
        assert!(!filter.matches(&create_test_tool("other_tool", "A tool")));
    }

    #[test]
    fn test_filter_with_suffix() {
        let filter = ToolFilter::new().with_name_suffix("_tool");

        assert!(filter.matches(&create_test_tool("test_tool", "A tool")));
        assert!(filter.matches(&create_test_tool("other_tool", "A tool")));
        assert!(!filter.matches(&create_test_tool("test_helper", "A tool")));
    }

    #[test]
    fn test_filter_with_contains() {
        let filter = ToolFilter::new().with_name_contains("test");

        assert!(filter.matches(&create_test_tool("test_tool", "A tool")));
        assert!(filter.matches(&create_test_tool("my_test_tool", "A tool")));
        assert!(!filter.matches(&create_test_tool("other_tool", "A tool")));
    }

    #[test]
    fn test_filter_with_description() {
        let filter = ToolFilter::new().with_description_contains("file");

        assert!(filter.matches(&create_test_tool("read", "Read a file")));
        assert!(filter.matches(&create_test_tool("write", "Write to file")));
        assert!(!filter.matches(&create_test_tool("calc", "Calculate numbers")));
    }

    #[test]
    fn test_filter_and_behavior() {
        let filter = ToolFilter::new()
            .with_name_prefix("test_")
            .with_description_contains("file");

        assert!(filter.matches(&create_test_tool("test_read", "Read a file")));
        assert!(!filter.matches(&create_test_tool("test_read", "Calculate")));
        assert!(!filter.matches(&create_test_tool("other_read", "Read a file")));
    }

    #[test]
    fn test_filter_or_behavior() {
        let filter = ToolFilter::new()
            .with_name("exact_match")
            .or(ToolFilter::new().with_name_prefix("test_"));

        assert!(filter.matches(&create_test_tool("exact_match", "A tool")));
        assert!(filter.matches(&create_test_tool("test_tool", "A tool")));
        assert!(!filter.matches(&create_test_tool("other_tool", "A tool")));
    }

    #[test]
    fn test_filter_list() {
        let tools = vec![
            create_test_tool("test_tool1", "A tool"),
            create_test_tool("test_tool2", "Another tool"),
            create_test_tool("other_tool", "Different tool"),
        ];

        let filter = ToolFilter::new().with_name_prefix("test_");
        let filtered = filter.filter(tools);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "test_tool1");
        assert_eq!(filtered[1].name, "test_tool2");
    }

    #[test]
    fn test_accept_all() {
        let filter = accept_all();
        assert!(filter.matches(&create_test_tool("any", "tool")));
    }

    #[test]
    fn test_accept_none() {
        let filter = accept_none();
        assert!(!filter.matches(&create_test_tool("any", "tool")));
    }

    #[test]
    fn test_by_names() {
        let filter = by_names(vec!["tool1".to_string(), "tool2".to_string()]);

        assert!(filter.matches(&create_test_tool("tool1", "A")));
        assert!(filter.matches(&create_test_tool("tool2", "B")));
        assert!(!filter.matches(&create_test_tool("tool3", "C")));
    }

    #[test]
    fn test_exclude_names() {
        let filter = exclude_names(vec!["tool1".to_string(), "tool2".to_string()]);

        assert!(!filter.matches(&create_test_tool("tool1", "A")));
        assert!(!filter.matches(&create_test_tool("tool2", "B")));
        assert!(filter.matches(&create_test_tool("tool3", "C")));
    }

    #[test]
    fn test_with_property() {
        let filter = with_property("special_arg");

        let tool_with_prop = McpToolDefinition {
            name: "test".to_string(),
            description: "Test".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "special_arg": {"type": "string"}
                }
            }),
        };

        let tool_without_prop = create_test_tool("test", "Test");

        assert!(filter.matches(&tool_with_prop));
        assert!(!filter.matches(&tool_without_prop));
    }

    #[test]
    fn test_empty_filter_matches_all() {
        let filter = ToolFilter::new();
        assert!(filter.matches(&create_test_tool("any", "tool")));
    }
}
