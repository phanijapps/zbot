// ============================================================================
// WORKFLOW GRAPH
// Types and functions for .workflow/graph.yaml
// ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ============================================================================
// GRAPH TYPES
// ============================================================================

/// Workflow graph definition (stored in .workflow/graph.yaml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowGraph {
    /// Schema version for migration support
    #[serde(default = "default_version")]
    pub version: u32,

    /// Flow pattern hint: pipeline, parallel, router, custom
    #[serde(default = "default_pattern")]
    pub pattern: WorkflowPattern,

    /// Start node configuration
    #[serde(default)]
    pub start: StartConfig,

    /// Subagent nodes and their connections
    #[serde(default)]
    pub nodes: HashMap<String, WorkflowNode>,

    /// End node configuration
    #[serde(default)]
    pub end: EndConfig,

    /// Optional conditional routing rules (for router pattern)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<ConditionRule>,
}

/// Workflow pattern types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowPattern {
    Pipeline,
    Parallel,
    Router,
    Custom,
}

/// Start node configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StartConfig {
    /// Trigger type: user_message, scheduled, webhook, manual
    #[serde(default = "default_trigger")]
    pub trigger: String,
}

/// End node configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EndConfig {
    /// Which node provides the final output
    #[serde(default)]
    pub output: Option<String>,
}

/// Workflow node (subagent in the flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    /// Role hint: input_processor, generator, enhancer, output_formatter, etc.
    #[serde(default)]
    pub role: Option<String>,

    /// Human-readable description of this node's purpose
    #[serde(default)]
    pub description: Option<String>,

    /// Next node(s) to execute after this one
    /// Can be a single string or array for parallel execution
    #[serde(default)]
    pub next: NextNode,

    /// Whether nodes in `next` should execute in parallel
    #[serde(default)]
    pub parallel: bool,

    /// Whether this node can be skipped
    #[serde(default)]
    pub optional: bool,
}

/// Next node specification (single or multiple)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum NextNode {
    #[default]
    None,
    Single(String),
    Multiple(Vec<String>),
}

impl NextNode {
    pub fn is_empty(&self) -> bool {
        match self {
            NextNode::None => true,
            NextNode::Single(s) => s.is_empty(),
            NextNode::Multiple(v) => v.is_empty(),
        }
    }

    pub fn to_vec(&self) -> Vec<String> {
        match self {
            NextNode::None => vec![],
            NextNode::Single(s) => vec![s.clone()],
            NextNode::Multiple(v) => v.clone(),
        }
    }
}

/// Conditional routing rule (for router pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionRule {
    /// Condition description (evaluated by orchestrator)
    #[serde(rename = "when")]
    pub condition: String,

    /// Node to route to if condition matches
    #[serde(rename = "route_to", default, skip_serializing_if = "Option::is_none")]
    pub route_to: Option<String>,

    /// Nodes to skip if condition matches
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<String>,
}

// ============================================================================
// DEFAULT FUNCTIONS
// ============================================================================

fn default_version() -> u32 {
    1
}

fn default_pattern() -> WorkflowPattern {
    WorkflowPattern::Pipeline
}

fn default_trigger() -> String {
    "user_message".to_string()
}

// ============================================================================
// GRAPH OPERATIONS
// ============================================================================

impl WorkflowGraph {
    /// Create a new empty workflow graph
    pub fn new() -> Self {
        Self {
            version: 1,
            pattern: WorkflowPattern::Pipeline,
            start: StartConfig::default(),
            nodes: HashMap::new(),
            end: EndConfig::default(),
            conditions: vec![],
        }
    }

    /// Load workflow graph from .workflow/graph.yaml
    pub fn load(agent_dir: &Path) -> Result<Self, String> {
        let graph_path = agent_dir.join(".workflow").join("graph.yaml");

        if !graph_path.exists() {
            // Return empty graph if no workflow exists
            return Ok(Self::new());
        }

        let content = fs::read_to_string(&graph_path)
            .map_err(|e| format!("Failed to read graph.yaml: {}", e))?;

        serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse graph.yaml: {}", e))
    }

    /// Save workflow graph to .workflow/graph.yaml
    pub fn save(&self, agent_dir: &Path) -> Result<(), String> {
        let workflow_dir = agent_dir.join(".workflow");

        // Ensure .workflow directory exists
        fs::create_dir_all(&workflow_dir)
            .map_err(|e| format!("Failed to create .workflow directory: {}", e))?;

        let graph_path = workflow_dir.join("graph.yaml");
        let content = serde_yaml::to_string(self)
            .map_err(|e| format!("Failed to serialize graph.yaml: {}", e))?;

        // Add header comment
        let final_content = format!(
            "# Workflow Graph Definition\n# Pattern: {:?}\n\n{}",
            self.pattern, content
        );

        fs::write(&graph_path, final_content)
            .map_err(|e| format!("Failed to write graph.yaml: {}", e))?;

        Ok(())
    }

    /// Check if this is an orchestrator agent (has workflow)
    pub fn is_orchestrator(&self) -> bool {
        !self.nodes.is_empty()
    }

    /// Get execution order based on flow connections
    pub fn get_execution_order(&self) -> Vec<String> {
        let mut order = Vec::new();
        let mut visited = std::collections::HashSet::new();

        // Find starting nodes (nodes not referenced as "next" by any other node)
        let mut referenced: std::collections::HashSet<String> = std::collections::HashSet::new();
        for node in self.nodes.values() {
            for next in node.next.to_vec() {
                referenced.insert(next);
            }
        }

        let starting_nodes: Vec<String> = self
            .nodes
            .keys()
            .filter(|k| !referenced.contains(*k))
            .cloned()
            .collect();

        // BFS traversal
        let mut queue: std::collections::VecDeque<String> = starting_nodes.into_iter().collect();

        while let Some(node_id) = queue.pop_front() {
            if visited.contains(&node_id) {
                continue;
            }
            visited.insert(node_id.clone());
            order.push(node_id.clone());

            if let Some(node) = self.nodes.get(&node_id) {
                for next in node.next.to_vec() {
                    if !visited.contains(&next) {
                        queue.push_back(next);
                    }
                }
            }
        }

        order
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, id: String, node: WorkflowNode) {
        self.nodes.insert(id, node);
    }

    /// Remove a node from the graph
    pub fn remove_node(&mut self, id: &str) {
        self.nodes.remove(id);

        // Also remove references to this node from other nodes
        for node in self.nodes.values_mut() {
            node.next = match &node.next {
                NextNode::Single(s) if s == id => NextNode::None,
                NextNode::Multiple(v) => {
                    let filtered: Vec<String> = v.iter().filter(|s| *s != id).cloned().collect();
                    if filtered.is_empty() {
                        NextNode::None
                    } else if filtered.len() == 1 {
                        NextNode::Single(filtered[0].clone())
                    } else {
                        NextNode::Multiple(filtered)
                    }
                }
                other => other.clone(),
            };
        }
    }
}

impl Default for WorkflowGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_graph_yaml() {
        let yaml = r#"
version: 1
pattern: pipeline

start:
  trigger: user_message

nodes:
  inventory_checker:
    role: input_processor
    next: recipe_finder

  recipe_finder:
    role: generator
    next:
      - ingredient_substituter
      - instruction_formatter
    parallel: true

  ingredient_substituter:
    role: enhancer
    optional: true
    next: instruction_formatter

  instruction_formatter:
    role: output_formatter

end:
  output: instruction_formatter
"#;

        let graph: WorkflowGraph = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(graph.version, 1);
        assert_eq!(graph.pattern, WorkflowPattern::Pipeline);
        assert_eq!(graph.nodes.len(), 4);

        let inventory = graph.nodes.get("inventory_checker").unwrap();
        assert_eq!(inventory.role, Some("input_processor".to_string()));

        let recipe = graph.nodes.get("recipe_finder").unwrap();
        assert!(recipe.parallel);
        assert_eq!(recipe.next.to_vec().len(), 2);
    }

    #[test]
    fn test_execution_order() {
        let mut graph = WorkflowGraph::new();

        graph.add_node("a".to_string(), WorkflowNode {
            role: None,
            description: None,
            next: NextNode::Single("b".to_string()),
            parallel: false,
            optional: false,
        });

        graph.add_node("b".to_string(), WorkflowNode {
            role: None,
            description: None,
            next: NextNode::Single("c".to_string()),
            parallel: false,
            optional: false,
        });

        graph.add_node("c".to_string(), WorkflowNode {
            role: None,
            description: None,
            next: NextNode::None,
            parallel: false,
            optional: false,
        });

        let order = graph.get_execution_order();
        assert_eq!(order, vec!["a", "b", "c"]);
    }
}
