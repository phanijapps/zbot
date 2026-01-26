//! Workflow loader - loads workflow definitions from the file system
//!
//! Directory structure:
//! ```text
//! agents/{workflow-name}/
//! ├── config.yaml              # Orchestrator configuration
//! ├── AGENTS.md                # Orchestrator system instructions
//! ├── .workflow-layout.json    # Visual layout from IDE (positions, edges)
//! └── .subagents/
//!     └── {subagent-name}/
//!         ├── config.yaml      # Subagent configuration
//!         └── AGENTS.md        # Subagent system instructions
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use serde::Deserialize;

use crate::config::{OrchestratorConfig, SubagentConfig, WorkflowDefinition};
use crate::error::{Result, WorkflowError};
use crate::graph::{WorkflowGraph, WorkflowNode, WorkflowEdge, WorkflowPattern, NodeType};

/// Frontend layout file format
#[derive(Debug, Deserialize)]
struct FrontendLayout {
    positions: HashMap<String, Position>,
    #[serde(default)]
    edges: Vec<FrontendEdge>,
}

#[derive(Debug, Deserialize)]
struct Position {
    x: f64,
    y: f64,
}

#[derive(Debug, Deserialize)]
struct FrontendEdge {
    id: String,
    source: String,
    target: String,
    #[serde(default)]
    label: Option<String>,
}

/// Workflow loader for loading workflow definitions from the file system
pub struct WorkflowLoader {
    /// Base directory for agents
    agents_dir: PathBuf,
}

impl WorkflowLoader {
    /// Create a new workflow loader
    pub fn new(agents_dir: impl Into<PathBuf>) -> Self {
        Self {
            agents_dir: agents_dir.into(),
        }
    }

    /// Load a workflow by name
    pub async fn load(&self, workflow_name: &str) -> Result<WorkflowDefinition> {
        let workflow_dir = self.agents_dir.join(workflow_name);

        if !workflow_dir.exists() {
            return Err(WorkflowError::DirectoryNotFound(workflow_dir));
        }

        self.load_from_path(&workflow_dir).await
    }

    /// Load a workflow from a specific directory path
    pub async fn load_from_path(&self, workflow_dir: &Path) -> Result<WorkflowDefinition> {
        let workflow_name = workflow_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        tracing::debug!("Loading workflow '{}' from {:?}", workflow_name, workflow_dir);

        // Load orchestrator config
        let orchestrator = self.load_orchestrator_config(workflow_dir).await?;

        // Load subagents first (we need them to build the graph)
        let subagents = self.load_subagents(workflow_dir).await?;

        // Build workflow graph from subagents and layout
        let graph = self.build_workflow_graph(workflow_dir, &subagents).await?;

        // Validate graph (skip if empty - no subagents)
        if !graph.nodes.is_empty() {
            graph.validate()?;
        }

        Ok(WorkflowDefinition {
            id: workflow_name.clone(),
            name: orchestrator.display_name.clone(),
            orchestrator,
            subagents,
            graph,
            path: workflow_dir.to_path_buf(),
        })
    }

    /// Load orchestrator configuration
    async fn load_orchestrator_config(&self, workflow_dir: &Path) -> Result<OrchestratorConfig> {
        let config_path = workflow_dir.join("config.yaml");
        let agents_md_path = workflow_dir.join("AGENTS.md");

        // Load config.yaml
        let mut config: OrchestratorConfig = if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            serde_yaml::from_str(&content).map_err(|e| WorkflowError::YamlParse {
                path: config_path.clone(),
                message: e.to_string(),
            })?
        } else {
            OrchestratorConfig::default()
        };

        // Load AGENTS.md as system instructions
        if agents_md_path.exists() {
            config.system_instructions = fs::read_to_string(&agents_md_path).await?;
        }

        Ok(config)
    }

    /// Build workflow graph from subagents and frontend layout
    async fn build_workflow_graph(
        &self,
        workflow_dir: &Path,
        subagents: &[SubagentConfig],
    ) -> Result<WorkflowGraph> {
        // If no subagents, return minimal graph
        if subagents.is_empty() {
            return Ok(self.create_minimal_graph());
        }

        // Try to load frontend layout
        let layout_path = workflow_dir.join(".workflow-layout.json");
        let layout = if layout_path.exists() {
            let content = fs::read_to_string(&layout_path).await?;
            serde_json::from_str::<FrontendLayout>(&content).ok()
        } else {
            None
        };

        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // Create start node
        let start_id = layout.as_ref()
            .and_then(|l| l.positions.keys().find(|k| k.starts_with("start")))
            .cloned()
            .unwrap_or_else(|| "start".to_string());

        let start_pos = layout.as_ref()
            .and_then(|l| l.positions.get(&start_id))
            .map(|p| (p.x, p.y))
            .unwrap_or((200.0, 50.0));

        nodes.push(WorkflowNode {
            id: start_id.clone(),
            node_type: NodeType::Start,
            label: "Start".to_string(),
            subagent_id: None,
            condition: None,
            branches: Vec::new(),
            x: start_pos.0,
            y: start_pos.1,
            data: HashMap::new(),
        });

        // Create subagent nodes
        for subagent in subagents {
            let node_id = format!("subagent-{}", subagent.id);

            let pos = layout.as_ref()
                .and_then(|l| l.positions.get(&node_id))
                .map(|p| (p.x, p.y))
                .unwrap_or((200.0, 200.0));

            nodes.push(WorkflowNode {
                id: node_id,
                node_type: NodeType::Subagent,
                label: if subagent.display_name.is_empty() { subagent.id.clone() } else { subagent.display_name.clone() },
                subagent_id: Some(subagent.id.clone()),
                condition: None,
                branches: Vec::new(),
                x: pos.0,
                y: pos.1,
                data: HashMap::new(),
            });
        }

        // Create end node
        let end_id = layout.as_ref()
            .and_then(|l| l.positions.keys().find(|k| k.starts_with("end")))
            .cloned()
            .unwrap_or_else(|| "end".to_string());

        let end_pos = layout.as_ref()
            .and_then(|l| l.positions.get(&end_id))
            .map(|p| (p.x, p.y))
            .unwrap_or((200.0, 500.0));

        nodes.push(WorkflowNode {
            id: end_id.clone(),
            node_type: NodeType::End,
            label: "End".to_string(),
            subagent_id: None,
            condition: None,
            branches: Vec::new(),
            x: end_pos.0,
            y: end_pos.1,
            data: HashMap::new(),
        });

        // Use edges from layout if available
        if let Some(ref layout) = layout {
            for edge in &layout.edges {
                edges.push(WorkflowEdge {
                    id: edge.id.clone(),
                    source: edge.source.clone(),
                    target: edge.target.clone(),
                    label: edge.label.clone(),
                    condition: None,
                    source_handle: None,
                    target_handle: None,
                });
            }
        } else {
            // Create default pipeline edges if no layout
            // Start -> first subagent -> ... -> last subagent -> End
            if !subagents.is_empty() {
                let first_subagent = format!("subagent-{}", subagents[0].id);
                edges.push(WorkflowEdge::new(
                    format!("e-start-{}", first_subagent),
                    start_id.clone(),
                    first_subagent.clone(),
                ));

                for i in 0..subagents.len() - 1 {
                    let from = format!("subagent-{}", subagents[i].id);
                    let to = format!("subagent-{}", subagents[i + 1].id);
                    edges.push(WorkflowEdge::new(
                        format!("e-{}-{}", from, to),
                        from,
                        to,
                    ));
                }

                let last_subagent = format!("subagent-{}", subagents.last().unwrap().id);
                edges.push(WorkflowEdge::new(
                    format!("e-{}-end", last_subagent),
                    last_subagent,
                    end_id,
                ));
            }
        }

        Ok(WorkflowGraph {
            version: 1,
            pattern: WorkflowPattern::Pipeline,
            nodes,
            edges,
        })
    }

    /// Create a minimal graph with just start and end
    fn create_minimal_graph(&self) -> WorkflowGraph {
        WorkflowGraph {
            version: 1,
            pattern: WorkflowPattern::Pipeline,
            nodes: vec![
                WorkflowNode::start("start"),
                WorkflowNode::end("end"),
            ],
            edges: vec![
                WorkflowEdge::new("e-start-end", "start", "end"),
            ],
        }
    }

    /// Load all subagents from .subagents directory
    async fn load_subagents(&self, workflow_dir: &Path) -> Result<Vec<SubagentConfig>> {
        let subagents_dir = workflow_dir.join(".subagents");

        if !subagents_dir.exists() {
            tracing::debug!("No .subagents directory found at {:?}", subagents_dir);
            return Ok(Vec::new());
        }

        let mut subagents = Vec::new();
        let mut entries = fs::read_dir(&subagents_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                match self.load_subagent_config(&path).await {
                    Ok(config) => {
                        tracing::debug!("Loaded subagent: {}", config.id);
                        subagents.push(config);
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to load subagent from {:?}: {}",
                            path,
                            e
                        );
                    }
                }
            }
        }

        tracing::info!("Loaded {} subagents", subagents.len());
        Ok(subagents)
    }

    /// Load a single subagent configuration
    async fn load_subagent_config(&self, subagent_dir: &Path) -> Result<SubagentConfig> {
        let subagent_id = subagent_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let config_path = subagent_dir.join("config.yaml");
        let agents_md_path = subagent_dir.join("AGENTS.md");

        // Load config.yaml
        let mut config: SubagentConfig = if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            serde_yaml::from_str(&content).map_err(|e| WorkflowError::YamlParse {
                path: config_path.clone(),
                message: e.to_string(),
            })?
        } else {
            SubagentConfig::default()
        };

        // Set the ID from folder name
        config.id = subagent_id;

        // Load AGENTS.md as system prompt
        if agents_md_path.exists() {
            config.system_prompt = fs::read_to_string(&agents_md_path).await?;
        }

        Ok(config)
    }

    /// List all available workflows
    pub async fn list_workflows(&self) -> Result<Vec<String>> {
        let mut workflows = Vec::new();

        if !self.agents_dir.exists() {
            return Ok(workflows);
        }

        let mut entries = fs::read_dir(&self.agents_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                // Check if this directory has a .subagents subdirectory (indicates a workflow)
                let subagents_dir = path.join(".subagents");
                if subagents_dir.exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        workflows.push(name.to_string());
                    }
                }
            }
        }

        Ok(workflows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_loader_nonexistent_dir() {
        let loader = WorkflowLoader::new("/nonexistent/path");
        let result = loader.load("test").await;
        assert!(matches!(result, Err(WorkflowError::DirectoryNotFound(_))));
    }
}
