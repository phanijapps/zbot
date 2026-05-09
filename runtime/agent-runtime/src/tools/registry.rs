// ============================================================================
// TOOL REGISTRY
// Registry of available tools
// ============================================================================

use std::sync::Arc;

use zero_core::Tool;

/// Registry of all available tools
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.push(tool);
    }

    /// Register multiple tools
    pub fn register_all(&mut self, tools: Vec<Arc<dyn Tool>>) {
        for tool in tools {
            self.register(tool);
        }
    }

    /// Get all registered tools
    #[must_use]
    pub fn get_all(&self) -> &[Arc<dyn Tool>] {
        &self.tools
    }

    /// Find a tool by name
    #[must_use]
    pub fn find(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name).cloned()
    }

    /// Check if a tool exists
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    /// Get the number of registered tools
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::Value;
    use zero_core::ToolContext as ZcToolContext;

    struct DummyTool {
        n: &'static str,
    }

    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &'static str {
            self.n
        }
        fn description(&self) -> &'static str {
            "dummy"
        }
        fn parameters_schema(&self) -> Option<Value> {
            None
        }
        async fn execute(
            &self,
            _ctx: Arc<dyn ZcToolContext>,
            _args: Value,
        ) -> zero_core::Result<Value> {
            Ok(Value::Null)
        }
    }

    fn tool(n: &'static str) -> Arc<dyn Tool> {
        Arc::new(DummyTool { n })
    }

    #[test]
    fn empty_registry_state() {
        let r = ToolRegistry::new();
        assert_eq!(r.len(), 0);
        assert!(r.is_empty());
        assert!(r.get_all().is_empty());
        assert!(r.find("any").is_none());
        assert!(!r.contains("any"));
    }

    #[test]
    fn default_is_empty() {
        let r = ToolRegistry::default();
        assert!(r.is_empty());
    }

    #[test]
    fn register_one_then_lookup() {
        let mut r = ToolRegistry::new();
        r.register(tool("alpha"));
        assert_eq!(r.len(), 1);
        assert!(!r.is_empty());
        assert!(r.contains("alpha"));
        assert!(r.find("alpha").is_some());
        assert!(r.find("missing").is_none());
        assert_eq!(r.get_all().len(), 1);
    }

    #[test]
    fn register_all_appends_in_order() {
        let mut r = ToolRegistry::new();
        r.register(tool("first"));
        r.register_all(vec![tool("second"), tool("third")]);
        assert_eq!(r.len(), 3);
        assert!(r.contains("first"));
        assert!(r.contains("second"));
        assert!(r.contains("third"));
        assert_eq!(r.get_all()[0].name(), "first");
        assert_eq!(r.get_all()[2].name(), "third");
    }

    #[test]
    fn find_returns_first_match_clone() {
        let mut r = ToolRegistry::new();
        r.register(tool("dup"));
        r.register(tool("dup"));
        let got = r.find("dup").expect("present");
        assert_eq!(got.name(), "dup");
        // The registry still has both entries
        assert_eq!(r.len(), 2);
    }
}
