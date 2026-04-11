//! # Tool Registry
//!
//! Registry for managing tools.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use zero_core::{Result, Tool, ToolPredicate, Toolset};

/// Tool registry for managing available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Check if a tool exists.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// List all tools.
    pub fn list(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }

    /// Get tool names.
    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Filter tools by predicate.
    pub fn filter(&self, predicate: ToolPredicate) -> Vec<Arc<dyn Tool>> {
        self.tools
            .values()
            .filter(|tool| predicate(tool.as_ref()))
            .cloned()
            .collect()
    }

    /// Get the number of tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Toolset for ToolRegistry {
    fn name(&self) -> &str {
        "registry"
    }

    async fn tools(&self) -> Result<Vec<Arc<dyn Tool>>> {
        Ok(self.list())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::Value;
    use zero_core::Result;
    use zero_core::ToolContext;

    #[derive(Debug)]
    struct MockTool {
        name: String,
        description: String,
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        async fn execute(&self, _ctx: Arc<dyn ToolContext>, _args: Value) -> Result<Value> {
            Ok(Value::Null)
        }
    }

    #[test]
    fn test_registry_new() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry = ToolRegistry::new();
        let tool = Arc::new(MockTool {
            name: "test".to_string(),
            description: "Test tool".to_string(),
        });

        registry.register(tool.clone());
        assert_eq!(registry.len(), 1);
        assert!(registry.contains("test"));

        let retrieved = registry.get("test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name(), "test");
    }

    #[test]
    fn test_registry_list() {
        let mut registry = ToolRegistry::new();
        let tool1 = Arc::new(MockTool {
            name: "tool1".to_string(),
            description: "Tool 1".to_string(),
        });
        let tool2 = Arc::new(MockTool {
            name: "tool2".to_string(),
            description: "Tool 2".to_string(),
        });

        registry.register(tool1);
        registry.register(tool2);

        let tools = registry.list();
        assert_eq!(tools.len(), 2);
        let mut names = registry.names();
        names.sort(); // Sort for consistent comparison since HashMap doesn't guarantee order
        assert_eq!(names, vec!["tool1", "tool2"]);
    }
}
