// ============================================================================
// WORKFLOW LOADER
// Loads workflow data from files and constructs XY Flow compatible structures
// ============================================================================

use super::graph::{WorkflowGraph, WorkflowPattern, NextNode};
use super::layout::{WorkflowLayout, Position};
use crate::commands::agents::{read_agent_folder, Agent};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// ============================================================================
// XY FLOW TYPES (for frontend compatibility)
// ============================================================================

/// Complete workflow data for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowData {
    /// Graph nodes for XY Flow
    pub nodes: Vec<XYFlowNode>,
    /// Graph edges for XY Flow
    pub edges: Vec<XYFlowEdge>,
    /// Orchestrator metadata
    pub orchestrator: OrchestratorInfo,
    /// Workflow pattern
    pub pattern: String,
}

/// XY Flow compatible node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XYFlowNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub position: Position,
    pub data: XYFlowNodeData,
}

/// Node data for XY Flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XYFlowNodeData {
    pub label: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "subagentId", default, skip_serializing_if = "Option::is_none")]
    pub subagent_id: Option<String>,
    #[serde(rename = "providerId", default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(rename = "maxTokens", default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    #[serde(rename = "triggerType", default, skip_serializing_if = "Option::is_none")]
    pub trigger_type: Option<String>,
}

/// XY Flow compatible edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XYFlowEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "animated", default)]
    pub animated: bool,
}

/// Orchestrator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorInfo {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub model: String,
    pub temperature: f64,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    pub instructions: String,
    pub skills: Vec<String>,
    pub mcps: Vec<String>,
}

// ============================================================================
// LOADER FUNCTIONS
// ============================================================================

/// Load complete workflow data from agent directory
pub fn load_workflow(agent_dir: &Path) -> Result<WorkflowData, String> {
    // Load orchestrator agent info
    let orchestrator = load_orchestrator_info(agent_dir)?;

    // Load graph definition
    let graph = WorkflowGraph::load(agent_dir)?;

    // Load or generate layout
    let mut layout = WorkflowLayout::load(agent_dir)?;

    // Get subagent info
    let subagents = load_subagents(agent_dir)?;

    // If layout is empty, generate default
    if layout.nodes.is_empty() && !graph.nodes.is_empty() {
        let node_ids: Vec<String> = graph.get_execution_order();
        layout.generate_default(&node_ids);
    }

    // Build XY Flow nodes
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Add start node
    let start_pos = layout.get_position("start");
    nodes.push(XYFlowNode {
        id: "start".to_string(),
        node_type: "start".to_string(),
        position: if start_pos.x == 0.0 && start_pos.y == 0.0 {
            Position { x: 250.0, y: 50.0 }
        } else {
            start_pos
        },
        data: XYFlowNodeData {
            label: "Start".to_string(),
            display_name: "Start".to_string(),
            trigger_type: Some(graph.start.trigger.clone()),
            description: None,
            subagent_id: None,
            provider_id: None,
            model: None,
            temperature: None,
            max_tokens: None,
            instructions: None,
            role: None,
            optional: None,
        },
    });

    // Add subagent nodes
    for (node_id, graph_node) in &graph.nodes {
        let pos = layout.get_position(node_id);
        let subagent = subagents.iter().find(|s| &s.name == node_id);

        let (display_name, description, provider_id, model, temperature, max_tokens, instructions) =
            if let Some(agent) = subagent {
                (
                    agent.display_name.clone(),
                    Some(agent.description.clone()),
                    Some(agent.provider_id.clone()),
                    Some(agent.model.clone()),
                    Some(agent.temperature),
                    Some(agent.max_tokens),
                    Some(agent.instructions.clone()),
                )
            } else {
                (
                    node_id.replace('_', " "),
                    graph_node.description.clone(),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            };

        nodes.push(XYFlowNode {
            id: format!("subagent-{}", node_id),
            node_type: "subagent".to_string(),
            position: pos,
            data: XYFlowNodeData {
                label: display_name.clone(),
                display_name,
                description,
                subagent_id: Some(node_id.clone()),
                provider_id,
                model,
                temperature,
                max_tokens,
                instructions,
                role: graph_node.role.clone(),
                optional: Some(graph_node.optional),
                trigger_type: None,
            },
        });

        // Create edges to next nodes
        for next_id in graph_node.next.to_vec() {
            if next_id == "end" {
                edges.push(XYFlowEdge {
                    id: format!("edge-{}-end", node_id),
                    source: format!("subagent-{}", node_id),
                    target: "end".to_string(),
                    label: None,
                    animated: false,
                });
            } else {
                edges.push(XYFlowEdge {
                    id: format!("edge-{}-{}", node_id, next_id),
                    source: format!("subagent-{}", node_id),
                    target: format!("subagent-{}", next_id),
                    label: None,
                    animated: graph_node.parallel,
                });
            }
        }
    }

    // Create edges from start to first nodes
    let first_nodes: Vec<String> = graph.nodes.keys()
        .filter(|k| {
            // Nodes that are not referenced as "next" by any other node
            !graph.nodes.values().any(|n| n.next.to_vec().contains(k))
        })
        .cloned()
        .collect();

    for node_id in first_nodes {
        edges.push(XYFlowEdge {
            id: format!("edge-start-{}", node_id),
            source: "start".to_string(),
            target: format!("subagent-{}", node_id),
            label: None,
            animated: false,
        });
    }

    // Add end node
    let end_pos = layout.get_position("end");
    nodes.push(XYFlowNode {
        id: "end".to_string(),
        node_type: "end".to_string(),
        position: if end_pos.x == 0.0 && end_pos.y == 0.0 {
            Position { x: 250.0, y: 600.0 }
        } else {
            end_pos
        },
        data: XYFlowNodeData {
            label: "End".to_string(),
            display_name: "End".to_string(),
            description: None,
            subagent_id: None,
            provider_id: None,
            model: None,
            temperature: None,
            max_tokens: None,
            instructions: None,
            role: None,
            optional: None,
            trigger_type: None,
        },
    });

    // Create edges to end from terminal nodes (nodes with no next)
    for (node_id, graph_node) in &graph.nodes {
        if graph_node.next.is_empty() {
            edges.push(XYFlowEdge {
                id: format!("edge-{}-end", node_id),
                source: format!("subagent-{}", node_id),
                target: "end".to_string(),
                label: None,
                animated: false,
            });
        }
    }

    let pattern = match graph.pattern {
        WorkflowPattern::Pipeline => "pipeline",
        WorkflowPattern::Parallel => "parallel",
        WorkflowPattern::Router => "router",
        WorkflowPattern::Custom => "custom",
    };

    Ok(WorkflowData {
        nodes,
        edges,
        orchestrator,
        pattern: pattern.to_string(),
    })
}

/// Load orchestrator info from config.yaml and AGENTS.md
fn load_orchestrator_info(agent_dir: &Path) -> Result<OrchestratorInfo, String> {
    let agent = read_agent_folder(&agent_dir.to_path_buf())?;

    Ok(OrchestratorInfo {
        name: agent.name,
        display_name: agent.display_name,
        description: agent.description,
        provider_id: agent.provider_id,
        model: agent.model,
        temperature: agent.temperature,
        max_tokens: agent.max_tokens,
        instructions: agent.instructions,
        skills: agent.skills,
        mcps: agent.mcps,
    })
}

/// Load all subagents from .subagents/ directory
fn load_subagents(agent_dir: &Path) -> Result<Vec<Agent>, String> {
    let subagents_dir = agent_dir.join(".subagents");

    if !subagents_dir.exists() {
        return Ok(vec![]);
    }

    let mut subagents = Vec::new();

    let entries = fs::read_dir(&subagents_dir)
        .map_err(|e| format!("Failed to read .subagents directory: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Ok(agent) = read_agent_folder(&path.to_path_buf()) {
                subagents.push(agent);
            }
        }
    }

    Ok(subagents)
}

/// Save workflow data back to files
pub fn save_workflow(agent_dir: &Path, data: &WorkflowData) -> Result<(), String> {
    // Build graph from XY Flow data
    let mut graph = WorkflowGraph::new();

    // Set pattern
    graph.pattern = match data.pattern.as_str() {
        "pipeline" => WorkflowPattern::Pipeline,
        "parallel" => WorkflowPattern::Parallel,
        "router" => WorkflowPattern::Router,
        _ => WorkflowPattern::Custom,
    };

    // Extract nodes and build graph
    for node in &data.nodes {
        if node.node_type == "subagent" {
            if let Some(subagent_id) = &node.data.subagent_id {
                // Find outgoing edges for this node
                let next_nodes: Vec<String> = data.edges.iter()
                    .filter(|e| e.source == node.id)
                    .map(|e| {
                        if e.target == "end" {
                            "end".to_string()
                        } else {
                            e.target.strip_prefix("subagent-").unwrap_or(&e.target).to_string()
                        }
                    })
                    .filter(|t| t != "end")
                    .collect();

                let next = if next_nodes.is_empty() {
                    NextNode::None
                } else if next_nodes.len() == 1 {
                    NextNode::Single(next_nodes[0].clone())
                } else {
                    NextNode::Multiple(next_nodes)
                };

                graph.add_node(subagent_id.clone(), super::graph::WorkflowNode {
                    role: node.data.role.clone(),
                    description: node.data.description.clone(),
                    next,
                    parallel: data.edges.iter()
                        .filter(|e| e.source == node.id)
                        .any(|e| e.animated),
                    optional: node.data.optional.unwrap_or(false),
                });
            }
        }
    }

    // Build layout from positions
    let mut layout = WorkflowLayout::new();
    for node in &data.nodes {
        let key = if node.node_type == "subagent" {
            node.data.subagent_id.clone().unwrap_or(node.id.clone())
        } else {
            node.id.clone()
        };
        layout.set_position(key, node.position);
    }

    // Save both files
    graph.save(agent_dir)?;
    layout.save(agent_dir)?;

    Ok(())
}
