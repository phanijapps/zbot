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
        Self {
            tools: Vec::new(),
        }
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
