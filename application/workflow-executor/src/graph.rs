//! Workflow graph types
//!
//! These types represent the visual workflow graph:
//! - WorkflowGraph: The complete graph with nodes and edges
//! - WorkflowNode: Individual nodes (start, end, subagent, conditional)
//! - WorkflowEdge: Connections between nodes with optional conditions

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::error::{Result, WorkflowError};

/// Workflow execution pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowPattern {
    /// Sequential pipeline: nodes execute in order
    Pipeline,
    /// Parallel execution: specified nodes run concurrently
    Parallel,
    /// Router pattern: orchestrator routes to appropriate subagent
    Router,
    /// Custom pattern: follows graph edges explicitly
    Custom,
}

impl Default for WorkflowPattern {
    fn default() -> Self {
        Self::Pipeline
    }
}

/// Type of workflow node
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    /// Start node - entry point for the workflow
    Start,
    /// End node - workflow completion
    End,
    /// Subagent node - delegates to a specific subagent
    Subagent,
    /// Conditional gateway - branches based on conditions
    Conditional,
    /// Orchestrator node (legacy - for migration only)
    Orchestrator,
}

/// A node in the workflow graph
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowNode {
    /// Unique node identifier
    pub id: String,

    /// Node type
    #[serde(rename = "type")]
    pub node_type: NodeType,

    /// Display label
    #[serde(default)]
    pub label: String,

    /// For subagent nodes: the subagent ID
    #[serde(default)]
    pub subagent_id: Option<String>,

    /// For conditional nodes: the condition expression
    #[serde(default)]
    pub condition: Option<String>,

    /// For conditional nodes: branch definitions
    #[serde(default)]
    pub branches: Vec<ConditionalBranch>,

    /// Visual position (x coordinate)
    #[serde(default)]
    pub x: f64,

    /// Visual position (y coordinate)
    #[serde(default)]
    pub y: f64,

    /// Additional node data
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
}

/// Branch definition for conditional nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalBranch {
    /// Branch identifier
    pub id: String,

    /// Branch display name
    pub name: String,

    /// Condition expression (JavaScript-like)
    pub condition: String,

    /// Target node ID for this branch
    #[serde(default)]
    pub target_node_id: Option<String>,
}

/// An edge connecting two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEdge {
    /// Unique edge identifier
    pub id: String,

    /// Source node ID
    pub source: String,

    /// Target node ID
    pub target: String,

    /// Optional label (used as condition for conditional edges)
    #[serde(default)]
    pub label: Option<String>,

    /// For conditional edges: the condition expression
    #[serde(default)]
    pub condition: Option<String>,

    /// Source handle (for nodes with multiple outputs)
    #[serde(default)]
    pub source_handle: Option<String>,

    /// Target handle (for nodes with multiple inputs)
    #[serde(default)]
    pub target_handle: Option<String>,
}

/// Complete workflow graph
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowGraph {
    /// Graph version for compatibility
    #[serde(default = "default_version")]
    pub version: u32,

    /// Workflow execution pattern
    #[serde(default)]
    pub pattern: WorkflowPattern,

    /// All nodes in the graph
    #[serde(default)]
    pub nodes: Vec<WorkflowNode>,

    /// All edges connecting nodes
    #[serde(default)]
    pub edges: Vec<WorkflowEdge>,
}

fn default_version() -> u32 {
    1
}

impl WorkflowGraph {
    /// Create a new empty workflow graph
    pub fn new() -> Self {
        Self::default()
    }

    /// Find a node by ID
    pub fn find_node(&self, id: &str) -> Option<&WorkflowNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Find the start node
    pub fn start_node(&self) -> Option<&WorkflowNode> {
        self.nodes.iter().find(|n| n.node_type == NodeType::Start)
    }

    /// Find all end nodes
    pub fn end_nodes(&self) -> Vec<&WorkflowNode> {
        self.nodes.iter().filter(|n| n.node_type == NodeType::End).collect()
    }

    /// Find all subagent nodes
    pub fn subagent_nodes(&self) -> Vec<&WorkflowNode> {
        self.nodes.iter().filter(|n| n.node_type == NodeType::Subagent).collect()
    }

    /// Get outgoing edges from a node
    pub fn outgoing_edges(&self, node_id: &str) -> Vec<&WorkflowEdge> {
        self.edges.iter().filter(|e| e.source == node_id).collect()
    }

    /// Get incoming edges to a node
    pub fn incoming_edges(&self, node_id: &str) -> Vec<&WorkflowEdge> {
        self.edges.iter().filter(|e| e.target == node_id).collect()
    }

    /// Get the next nodes after a given node
    pub fn next_nodes(&self, node_id: &str) -> Vec<&WorkflowNode> {
        self.outgoing_edges(node_id)
            .iter()
            .filter_map(|e| self.find_node(&e.target))
            .collect()
    }

    /// Get the previous nodes before a given node
    pub fn previous_nodes(&self, node_id: &str) -> Vec<&WorkflowNode> {
        self.incoming_edges(node_id)
            .iter()
            .filter_map(|e| self.find_node(&e.source))
            .collect()
    }

    /// Validate the workflow graph
    pub fn validate(&self) -> Result<()> {
        // Must have exactly one start node
        let start_nodes: Vec<_> = self.nodes.iter()
            .filter(|n| n.node_type == NodeType::Start)
            .collect();

        if start_nodes.is_empty() {
            return Err(WorkflowError::MissingStartNode);
        }
        if start_nodes.len() > 1 {
            return Err(WorkflowError::InvalidGraph(
                "Multiple start nodes found".to_string()
            ));
        }

        // Must have at least one end node
        if self.end_nodes().is_empty() {
            return Err(WorkflowError::MissingEndNode);
        }

        // Validate all edges reference existing nodes
        let node_ids: HashSet<_> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        for edge in &self.edges {
            if !node_ids.contains(edge.source.as_str()) {
                return Err(WorkflowError::InvalidEdge {
                    from: edge.source.clone(),
                    to: edge.target.clone(),
                    reason: "Source node does not exist".to_string(),
                });
            }
            if !node_ids.contains(edge.target.as_str()) {
                return Err(WorkflowError::InvalidEdge {
                    from: edge.source.clone(),
                    to: edge.target.clone(),
                    reason: "Target node does not exist".to_string(),
                });
            }
        }

        // Validate subagent nodes have subagent_id
        for node in &self.nodes {
            if node.node_type == NodeType::Subagent && node.subagent_id.is_none() {
                return Err(WorkflowError::InvalidGraph(format!(
                    "Subagent node '{}' missing subagent_id",
                    node.id
                )));
            }
        }

        // Check for cycles (optional - some patterns allow cycles)
        if self.pattern != WorkflowPattern::Custom {
            self.check_cycles()?;
        }

        Ok(())
    }

    /// Check for cycles in the graph using DFS
    fn check_cycles(&self) -> Result<()> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node in &self.nodes {
            if !visited.contains(&node.id) {
                if self.has_cycle_util(&node.id, &mut visited, &mut rec_stack) {
                    return Err(WorkflowError::CycleDetected(node.id.clone()));
                }
            }
        }

        Ok(())
    }

    fn has_cycle_util(
        &self,
        node_id: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        visited.insert(node_id.to_string());
        rec_stack.insert(node_id.to_string());

        for edge in self.outgoing_edges(node_id) {
            if !visited.contains(&edge.target) {
                if self.has_cycle_util(&edge.target, visited, rec_stack) {
                    return true;
                }
            } else if rec_stack.contains(&edge.target) {
                return true;
            }
        }

        rec_stack.remove(node_id);
        false
    }

    /// Get execution order for pipeline pattern (topological sort)
    pub fn execution_order(&self) -> Result<Vec<String>> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();

        // Start from start node
        if let Some(start) = self.start_node() {
            self.topological_sort(&start.id, &mut visited, &mut temp_visited, &mut order)?;
        }

        order.reverse();
        Ok(order)
    }

    fn topological_sort(
        &self,
        node_id: &str,
        visited: &mut HashSet<String>,
        temp_visited: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<()> {
        if temp_visited.contains(node_id) {
            return Err(WorkflowError::CycleDetected(node_id.to_string()));
        }
        if visited.contains(node_id) {
            return Ok(());
        }

        temp_visited.insert(node_id.to_string());

        for edge in self.outgoing_edges(node_id) {
            self.topological_sort(&edge.target, visited, temp_visited, order)?;
        }

        temp_visited.remove(node_id);
        visited.insert(node_id.to_string());
        order.push(node_id.to_string());

        Ok(())
    }
}

impl WorkflowNode {
    /// Create a new start node
    pub fn start(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            node_type: NodeType::Start,
            label: "Start".to_string(),
            subagent_id: None,
            condition: None,
            branches: Vec::new(),
            x: 0.0,
            y: 0.0,
            data: HashMap::new(),
        }
    }

    /// Create a new end node
    pub fn end(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            node_type: NodeType::End,
            label: "End".to_string(),
            subagent_id: None,
            condition: None,
            branches: Vec::new(),
            x: 0.0,
            y: 0.0,
            data: HashMap::new(),
        }
    }

    /// Create a new subagent node
    pub fn subagent(id: impl Into<String>, subagent_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            node_type: NodeType::Subagent,
            label: label.into(),
            subagent_id: Some(subagent_id.into()),
            condition: None,
            branches: Vec::new(),
            x: 0.0,
            y: 0.0,
            data: HashMap::new(),
        }
    }

    /// Create a new conditional node
    pub fn conditional(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            node_type: NodeType::Conditional,
            label: label.into(),
            subagent_id: None,
            condition: None,
            branches: Vec::new(),
            x: 0.0,
            y: 0.0,
            data: HashMap::new(),
        }
    }
}

impl WorkflowEdge {
    /// Create a new edge
    pub fn new(id: impl Into<String>, source: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            label: None,
            condition: None,
            source_handle: None,
            target_handle: None,
        }
    }

    /// Add a label to the edge
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Add a condition to the edge
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_pipeline_validation() {
        let mut graph = WorkflowGraph::new();
        graph.pattern = WorkflowPattern::Pipeline;

        graph.nodes.push(WorkflowNode::start("start-1"));
        graph.nodes.push(WorkflowNode::subagent("agent-1", "processor", "Processor"));
        graph.nodes.push(WorkflowNode::end("end-1"));

        graph.edges.push(WorkflowEdge::new("e1", "start-1", "agent-1"));
        graph.edges.push(WorkflowEdge::new("e2", "agent-1", "end-1"));

        assert!(graph.validate().is_ok());
    }

    #[test]
    fn test_missing_start_node() {
        let mut graph = WorkflowGraph::new();
        graph.nodes.push(WorkflowNode::end("end-1"));

        assert!(matches!(graph.validate(), Err(WorkflowError::MissingStartNode)));
    }

    #[test]
    fn test_missing_end_node() {
        let mut graph = WorkflowGraph::new();
        graph.nodes.push(WorkflowNode::start("start-1"));

        assert!(matches!(graph.validate(), Err(WorkflowError::MissingEndNode)));
    }

    #[test]
    fn test_execution_order() {
        let mut graph = WorkflowGraph::new();
        graph.pattern = WorkflowPattern::Pipeline;

        graph.nodes.push(WorkflowNode::start("start"));
        graph.nodes.push(WorkflowNode::subagent("a", "agent-a", "Agent A"));
        graph.nodes.push(WorkflowNode::subagent("b", "agent-b", "Agent B"));
        graph.nodes.push(WorkflowNode::end("end"));

        graph.edges.push(WorkflowEdge::new("e1", "start", "a"));
        graph.edges.push(WorkflowEdge::new("e2", "a", "b"));
        graph.edges.push(WorkflowEdge::new("e3", "b", "end"));

        let order = graph.execution_order().unwrap();
        assert_eq!(order, vec!["start", "a", "b", "end"]);
    }
}
