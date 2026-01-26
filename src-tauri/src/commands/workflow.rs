// ============================================================================
// WORKFLOW COMMANDS
// Visual workflow IDE commands that sync XY Flow graphs with .subagents/ folders
// ============================================================================

use crate::commands::agents::{read_agent_folder, Agent, AgentConfig};
use crate::commands::agents::save_subagent;
use crate::domains::vault::manager::get_active_vault_path;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// TYPES FOR XY FLOW INTEGRATION
// ============================================================================

/// XY Flow Node structure (matches @xyflow/react Node type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub position: WorkflowPosition,
    pub data: WorkflowNodeData,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WorkflowPosition {
    pub x: f64,
    pub y: f64,
}

/// Node data - uses a map to accommodate different node types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNodeData {
    pub label: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// XY Flow Edge structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Complete workflow graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowGraph {
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    /// Orchestrator configuration (flow-level)
    pub orchestrator: Option<OrchestratorConfig>,
}

/// Orchestrator configuration (applies to the entire flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub model: String,
    pub temperature: f64,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(rename = "systemInstructions")]
    pub system_instructions: String,
    pub mcps: Vec<String>,
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub middleware: Option<String>,
}

/// Subagent configuration (extracted from node data)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubagentConfig {
    #[serde(rename = "displayName")]
    display_name: String,
    pub description: String,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub model: String,
    pub temperature: f64,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(rename = "systemPrompt")]
    pub system_prompt: String,
    #[serde(rename = "subagentId")]
    subagent_id: String,
    pub mcps: Vec<String>,
    pub skills: Vec<String>,
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub node_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub node_id: String,
    pub message: String,
}

/// Workflow layout storage - persists node positions and edges
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkflowLayout {
    #[serde(default)]
    positions: HashMap<String, WorkflowPosition>,
    #[serde(default)]
    edges: Vec<WorkflowEdge>,
}

impl WorkflowLayout {
    fn new() -> Self {
        Self {
            positions: HashMap::new(),
            edges: Vec::new(),
        }
    }

    fn save(&self, path: &PathBuf) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize layout: {}", e))?;
        fs::write(path, json)
            .map_err(|e| format!("Failed to write layout file: {}", e))?;
        Ok(())
    }

    fn load(path: &PathBuf) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read layout file: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse layout file: {}", e))
    }
}

// ============================================================================
// WORKFLOW COMMANDS
// ============================================================================

/// Get the orchestrator structure as a visual graph
/// Reads from .subagents/ folder and AGENTS.md to construct the workflow
#[tauri::command]
pub async fn get_orchestrator_structure(agent_id: String) -> Result<WorkflowGraph, String> {
    let vault_path = get_active_vault_path().await?;
    // Agents are in vault_path/agents/
    let agents_dir = vault_path.join("agents");
    let agent_dir = agents_dir.join(&agent_id);

    if !agent_dir.exists() {
        return Err(format!("Agent not found: {}", agent_id));
    }

    let subagents_dir = agent_dir.join(".subagents");
    let agents_md_path = agent_dir.join("AGENTS.md");
    let config_path = agent_dir.join("config.yaml");
    let layout_path = agent_dir.join(".workflow-layout.json");

    // Load saved layout (positions and edges)
    let layout = WorkflowLayout::load(&layout_path)?;

    // Read agent config for orchestrator settings
    let (orchestrator_config, display_name, description) = if config_path.exists() {
        let config_content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config.yaml: {}", e))?;
        let config: AgentConfig = serde_yaml::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config.yaml: {}", e))?;

        let orchestrator = OrchestratorConfig {
            display_name: config.display_name.clone(),
            description: config.description.clone(),
            provider_id: config.provider_id,
            model: config.model,
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            system_instructions: String::new(), // Will be read from AGENTS.md
            mcps: config.mcps,
            skills: config.skills,
            middleware: None, // Middleware is embedded in YAML
        };
        (Some(orchestrator), config.display_name, config.description)
    } else {
        (None, "Orchestrator".to_string(), "No description".to_string())
    };

    // Read system instructions from AGENTS.md
    let system_instructions = if agents_md_path.exists() {
        fs::read_to_string(&agents_md_path)
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Update orchestrator config with system instructions
    let orchestrator_config = orchestrator_config.map(|mut c| {
        c.system_instructions = system_instructions;
        c
    });

    let mut nodes = Vec::new();
    let mut subagent_node_ids = Vec::new();

    // Check if layout has start/end nodes (new workflow format) or orchestrator node (legacy)
    let has_start_end_nodes = layout.positions.keys().any(|k| k.starts_with("start-") || k.starts_with("end-"));

    if has_start_end_nodes {
        // Load start and end nodes from saved layout
        for (node_id, position) in &layout.positions {
            if node_id.starts_with("start-") {
                let mut map = serde_json::Map::new();
                map.insert("label".to_string(), json!(display_name.clone()));
                map.insert("displayName".to_string(), json!(display_name.clone()));
                map.insert("description".to_string(), json!(description.clone()));
                if let Some(orc) = &orchestrator_config {
                    map.insert("providerId".to_string(), json!(orc.provider_id.clone()));
                    map.insert("model".to_string(), json!(orc.model.clone()));
                }
                nodes.push(WorkflowNode {
                    id: node_id.clone(),
                    node_type: "start".to_string(),
                    position: *position,
                    data: WorkflowNodeData { label: display_name.clone(), extra: map },
                });
            } else if node_id.starts_with("end-") {
                let mut map = serde_json::Map::new();
                map.insert("label".to_string(), json!("End"));
                nodes.push(WorkflowNode {
                    id: node_id.clone(),
                    node_type: "end".to_string(),
                    position: *position,
                    data: WorkflowNodeData { label: "End".to_string(), extra: map },
                });
            }
        }
    } else {
        // Legacy: Create orchestrator node
        let orchestrator_id = format!("orchestrator-{}", agent_id);
        let orchestrator_position = layout.positions.get(&orchestrator_id)
            .copied()
            .unwrap_or(WorkflowPosition { x: 100.0, y: 100.0 });
        nodes.push(WorkflowNode {
            id: orchestrator_id.clone(),
            node_type: "orchestrator".to_string(),
            position: orchestrator_position,
            data: {
                let mut map = serde_json::Map::new();
                map.insert("label".to_string(), json!(display_name));
                map.insert("displayName".to_string(), json!(display_name));
                map.insert("description".to_string(), json!(description));
                if let Some(orc) = &orchestrator_config {
                    map.insert("providerId".to_string(), json!(orc.provider_id));
                    map.insert("model".to_string(), json!(orc.model));
                }
                WorkflowNodeData { label: display_name, extra: map }
            },
        });
    }

    // Read subagents from .subagents/ folder
    if subagents_dir.exists() {
        let entries = fs::read_dir(&subagents_dir)
            .map_err(|e| format!("Failed to read .subagents directory: {}", e))?;

        let y_offset = 300.0;
        let x_offset = 100.0;

        // First, check if we have saved node IDs in the layout that match subagent folders
        let mut subagent_id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        // Build a map from subagent folder name to saved node ID (if exists in layout)
        for (node_id, _pos) in &layout.positions {
            if node_id.starts_with("subagent-") {
                let folder_name = node_id.strip_prefix("subagent-").unwrap_or("");
                subagent_id_map.insert(folder_name.to_string(), node_id.clone());
            }
        }

        for (index, entry) in entries.flatten().enumerate() {
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let config_yaml = path.join("config.yaml");
            if !config_yaml.exists() {
                continue;
            }

            // Read subagent config
            if let Ok(subagent) = read_agent_folder(&path) {
                // Use the saved node ID from layout if it exists, otherwise generate a new one
                let subagent_node_id = subagent_id_map.get(&subagent.name)
                    .cloned()
                    .unwrap_or_else(|| format!("subagent-{}", subagent.name));

                // Track subagent node IDs for default edge generation
                subagent_node_ids.push(subagent_node_id.clone());

                // Use saved position or calculate default
                let saved_position = layout.positions.get(&subagent_node_id);
                let position = if let Some(pos) = saved_position {
                    *pos
                } else {
                    let x = x_offset + (index as f64 % 3.0) * 250.0;
                    let y = y_offset + (index as f64 / 3.0).floor() * 200.0;
                    WorkflowPosition { x, y }
                };

                let mut node_data = serde_json::Map::new();
                node_data.insert("label".to_string(), json!(subagent.display_name.clone()));
                node_data.insert("displayName".to_string(), json!(subagent.display_name));
                node_data.insert("description".to_string(), json!(subagent.description));
                node_data.insert("subagentId".to_string(), json!(subagent.name));
                node_data.insert("providerId".to_string(), json!(subagent.provider_id));
                node_data.insert("model".to_string(), json!(subagent.model));
                node_data.insert("temperature".to_string(), json!(subagent.temperature));
                node_data.insert("maxTokens".to_string(), json!(subagent.max_tokens));
                node_data.insert("systemPrompt".to_string(), json!(subagent.instructions));
                node_data.insert("mcps".to_string(), json!(subagent.mcps));
                node_data.insert("skills".to_string(), json!(subagent.skills));

                nodes.push(WorkflowNode {
                    id: subagent_node_id.clone(),
                    node_type: "subagent".to_string(),
                    position,
                    data: WorkflowNodeData {
                        label: subagent.display_name.clone(),
                        extra: node_data,
                    },
                });
            }
        }
    }

    // Use saved edges if available, otherwise generate default edges
    let edges = if !layout.edges.is_empty() {
        // Use saved edges from layout
        layout.edges.clone()
    } else {
        // Generate default edges from start/orchestrator node to each subagent
        // Find the start or orchestrator node
        let source_node_id = nodes.iter()
            .find(|n| n.node_type == "start" || n.node_type == "orchestrator")
            .map(|n| n.id.clone())
            .unwrap_or_else(|| format!("orchestrator-{}", agent_id));

        subagent_node_ids.iter()
            .map(|subagent_node_id| WorkflowEdge {
                id: format!("edge-{}-{}", source_node_id, subagent_node_id),
                source: source_node_id.clone(),
                target: subagent_node_id.clone(),
                label: Some("delegates to".to_string()),
            })
            .collect()
    };

    Ok(WorkflowGraph {
        nodes,
        edges,
        orchestrator: orchestrator_config,
    })
}

/// Save the orchestrator structure
/// Creates .subagents/ folders from the visual graph and generates AGENTS.md
#[tauri::command]
pub async fn save_orchestrator_structure(
    agent_id: String,
    graph: WorkflowGraph,
) -> Result<(), String> {
    let vault_path = get_active_vault_path().await?;
    // Agents are in vault_path/agents/
    let agents_dir = vault_path.join("agents");
    let agent_dir = agents_dir.join(&agent_id);
    let layout_path = agent_dir.join(".workflow-layout.json");

    if !agent_dir.exists() {
        return Err(format!("Agent not found: {}", agent_id));
    }

    // Save node positions and edges to layout file
    // The key must match what get_orchestrator_structure expects:
    // - Orchestrator: "orchestrator-{agent_id}"
    // - Subagent: "subagent-{folder_name}" where folder_name comes from subagentId field
    let mut layout = WorkflowLayout::new();
    for node in &graph.nodes {
        let layout_key = if node.node_type == "orchestrator" {
            format!("orchestrator-{}", agent_id)
        } else if node.node_type == "subagent" {
            // Get the subagentId from node data - this should be the folder name
            // If subagentId already has "subagent-" prefix, strip it first
            let folder_name = node.data.extra.get("subagentId")
                .and_then(|v| v.as_str())
                .unwrap_or(&node.id);

            // Remove "subagent-" prefix if present to get just the folder name
            let clean_name = folder_name.strip_prefix("subagent-")
                .unwrap_or(folder_name);

            format!("subagent-{}", clean_name)
        } else {
            node.id.clone()
        };
        layout.positions.insert(layout_key, node.position.clone());
    }
    // Save edges from the graph
    layout.edges = graph.edges.clone();
    layout.save(&layout_path)?;

    // Update orchestrator config if provided
    if let Some(orchestrator) = &graph.orchestrator {
        update_agent_orchestrator_config(agent_id.clone(), orchestrator).await?;
    }

    // Process subagent nodes and create .subagents/ folders
    let mut existing_subagents = std::collections::HashSet::new();

    for node in &graph.nodes {
        if node.node_type != "subagent" {
            continue;
        }

        // Extract subagent config from node data
        let subagent_config = extract_subagent_config(&node.data)?;

        // Track this subagent
        existing_subagents.insert(subagent_config.subagent_id.clone());

        // Create/update the subagent
        let agent = Agent {
            id: String::new(),
            name: subagent_config.subagent_id.clone(),
            display_name: subagent_config.display_name,
            description: subagent_config.description,
            agent_type: Some("llm".to_string()),
            provider_id: subagent_config.provider_id,
            model: subagent_config.model,
            temperature: subagent_config.temperature,
            max_tokens: subagent_config.max_tokens,
            thinking_enabled: false,
            voice_recording_enabled: false,
            system_instruction: Some(subagent_config.system_prompt.clone()),
            instructions: subagent_config.system_prompt,
            mcps: subagent_config.mcps,
            skills: subagent_config.skills,
            middleware: None,
            created_at: None,
        };

        save_subagent(agent_id.clone(), agent).await?;
    }

    // Remove subagents that are no longer in the graph
    let subagents_dir = agent_dir.join(".subagents");
    if subagents_dir.exists() {
        let entries = fs::read_dir(&subagents_dir)
            .map_err(|e| format!("Failed to read .subagents directory: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !existing_subagents.contains(name) {
                        // Remove this subagent
                        fs::remove_dir_all(&path)
                            .map_err(|e| format!("Failed to remove subagent {}: {}", name, e))?;
                    }
                }
            }
        }
    }

    // Generate AGENTS.md from the graph
    generate_agents_md(agent_dir, &graph)?;

    Ok(())
}

/// Validate a workflow graph
#[tauri::command]
pub async fn validate_workflow(graph: WorkflowGraph) -> Result<ValidationResult, String> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut node_ids = std::collections::HashSet::new();

    // Check for duplicate node IDs
    for node in &graph.nodes {
        if !node_ids.insert(&node.id) {
            errors.push(ValidationError {
                node_id: node.id.clone(),
                message: "Duplicate node ID".to_string(),
            });
        }
    }

    // Check for orphaned edges
    for edge in &graph.edges {
        let source_exists = node_ids.contains(&edge.source);
        let target_exists = node_ids.contains(&edge.target);

        if !source_exists {
            errors.push(ValidationError {
                node_id: edge.id.clone(),
                message: format!("Edge source node '{}' not found", edge.source),
            });
        }

        if !target_exists {
            errors.push(ValidationError {
                node_id: edge.id.clone(),
                message: format!("Edge target node '{}' not found", edge.target),
            });
        }
    }

    // Validate orchestrator configuration
    if let Some(ref orchestrator) = graph.orchestrator {
        if orchestrator.provider_id.is_empty() {
            errors.push(ValidationError {
                node_id: "orchestrator".to_string(),
                message: "Orchestrator provider ID is required".to_string(),
            });
        }

        if orchestrator.model.is_empty() {
            errors.push(ValidationError {
                node_id: "orchestrator".to_string(),
                message: "Orchestrator model is required".to_string(),
            });
        }
    }

    // Validate subagent nodes
    for node in &graph.nodes {
        if node.node_type == "subagent" {
            let subagent_id = node.data.extra.get("subagentId")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if subagent_id.is_empty() {
                errors.push(ValidationError {
                    node_id: node.id.clone(),
                    message: "Subagent ID is required".to_string(),
                });
            }

            let provider_id = node.data.extra.get("providerId")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if provider_id.is_empty() {
                warnings.push(ValidationWarning {
                    node_id: node.id.clone(),
                    message: "Subagent provider ID not set".to_string(),
                });
            }

            let model = node.data.extra.get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if model.is_empty() {
                warnings.push(ValidationWarning {
                    node_id: node.id.clone(),
                    message: "Subagent model not set".to_string(),
                });
            }
        }
    }

    Ok(ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    })
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Extract subagent configuration from node data
fn extract_subagent_config(data: &WorkflowNodeData) -> Result<SubagentConfig, String> {
    let get_str = |key: &str| -> String {
        data.extra.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let get_vec_str = |key: &str| -> Vec<String> {
        data.extra.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };

    let get_f64 = |key: &str, default: f64| -> f64 {
        data.extra.get(key)
            .and_then(|v| v.as_f64())
            .unwrap_or(default)
    };

    let get_u32 = |key: &str, default: u32| -> u32 {
        data.extra.get(key)
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(default)
    };

    Ok(SubagentConfig {
        display_name: get_str("displayName"),
        description: get_str("description"),
        provider_id: get_str("providerId"),
        model: get_str("model"),
        temperature: get_f64("temperature", 0.7),
        max_tokens: get_u32("maxTokens", 4096),
        system_prompt: get_str("systemPrompt"),
        subagent_id: get_str("subagentId"),
        mcps: get_vec_str("mcps"),
        skills: get_vec_str("skills"),
    })
}

/// Update agent's orchestrator configuration
async fn update_agent_orchestrator_config(
    agent_id: String,
    orchestrator: &OrchestratorConfig,
) -> Result<(), String> {
    let vault_path = get_active_vault_path().await?;
    // Agents are in vault_path/agents/
    let agents_dir = vault_path.join("agents");
    let agent_dir = agents_dir.join(&agent_id);
    let config_path = agent_dir.join("config.yaml");

    // Read existing config or create default
    let mut config = if config_path.exists() {
        let config_content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config.yaml: {}", e))?;
        serde_yaml::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config.yaml: {}", e))?
    } else {
        AgentConfig {
            name: agent_id.clone(),
            display_name: orchestrator.display_name.clone(),
            description: orchestrator.description.clone(),
            agent_type: None,
            provider_id: orchestrator.provider_id.clone(),
            model: orchestrator.model.clone(),
            temperature: orchestrator.temperature,
            max_tokens: orchestrator.max_tokens,
            thinking_enabled: false,
            voice_recording_enabled: true,
            skills: orchestrator.skills.clone(),
            mcps: orchestrator.mcps.clone(),
            // Don't save system_instruction - AGENTS.md is the source of truth
            system_instruction: None,
        }
    };

    // Update with orchestrator config (metadata only - instructions go in AGENTS.md)
    config.display_name = orchestrator.display_name.clone();
    config.description = orchestrator.description.clone();
    config.provider_id = orchestrator.provider_id.clone();
    config.model = orchestrator.model.clone();
    config.temperature = orchestrator.temperature;
    config.max_tokens = orchestrator.max_tokens;
    config.skills = orchestrator.skills.clone();
    config.mcps = orchestrator.mcps.clone();
    // Don't save system_instruction to config.yaml - AGENTS.md is the source of truth
    config.system_instruction = None;

    // Write updated config
    let config_yaml = serde_yaml::to_string(&config)
        .map_err(|e| format!("Failed to serialize config.yaml: {}", e))?;

    let final_yaml = if let Some(ref middleware) = orchestrator.middleware {
        format!("{}\n{}", config_yaml.trim_end(), middleware.trim_end())
    } else {
        config_yaml
    };

    fs::write(config_path, final_yaml)
        .map_err(|e| format!("Failed to write config.yaml: {}", e))?;

    Ok(())
}

/// Write AGENTS.md with the orchestrator's instructions
/// This preserves user-authored content - no auto-generated boilerplate
fn generate_agents_md(agent_dir: PathBuf, graph: &WorkflowGraph) -> Result<(), String> {
    // Write the user's instructions directly - AGENTS.md is the source of truth
    // Don't add auto-generated "## Your Team" sections - the user maintains their own format
    let content = if let Some(ref orchestrator) = graph.orchestrator {
        if orchestrator.system_instructions.is_empty() {
            // If no instructions, don't overwrite existing AGENTS.md
            return Ok(());
        }
        format!("{}\n", orchestrator.system_instructions)
    } else {
        // No orchestrator config, don't overwrite
        return Ok(());
    };

    // Write AGENTS.md
    fs::write(agent_dir.join("AGENTS.md"), content)
        .map_err(|e| format!("Failed to write AGENTS.md: {}", e))?;

    Ok(())
}

// ============================================================================
// WORKFLOW EXECUTION
// ============================================================================

use std::sync::atomic::{AtomicBool, Ordering};
use once_cell::sync::Lazy;

/// Global map of active workflow executions for cancellation support
static ACTIVE_EXECUTIONS: Lazy<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<AtomicBool>>>> =
    Lazy::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Stop a running workflow execution
#[tauri::command]
pub async fn stop_workflow(invocation_id: String) -> Result<(), String> {
    tracing::info!("Stopping workflow execution: {}", invocation_id);

    if let Ok(mut executions) = ACTIVE_EXECUTIONS.lock() {
        if let Some(cancelled) = executions.get(&invocation_id) {
            cancelled.store(true, Ordering::SeqCst);
            tracing::info!("Workflow {} marked for cancellation", invocation_id);
        }
        executions.remove(&invocation_id);
    }

    Ok(())
}

/// Execute a workflow with streaming events
///
/// This command:
/// 1. Loads the workflow definition from the agents directory
/// 2. Builds an executable workflow using LLM and toolset factories
/// 3. Executes the workflow with the given user message
/// 4. Streams events to the frontend via Tauri events
#[tauri::command]
pub async fn execute_workflow(
    app: tauri::AppHandle,
    agent_id: String,
    message: String,
    session_id: Option<String>,
    invocation_id: Option<String>,
) -> Result<Value, String> {
    use crate::domains::agent_runtime::workflow_integration::{
        load_and_build_workflow,
        stream_workflow_events,
    };
    use workflow_executor::{WorkflowExecutor, ExecutionOptions};

    // Use provided invocation_id or generate one
    let invocation_id = invocation_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    tracing::info!("Executing workflow: {} with message: {} (invocation: {})", agent_id, message, invocation_id);

    let vault_path = get_active_vault_path().await?;
    let agents_dir = vault_path.join("agents");

    // Load and build the workflow
    let (executable, _definition) = load_and_build_workflow(&agents_dir, &agent_id).await?;

    // Create executor
    let executor = WorkflowExecutor::new(executable);

    // Generate session ID if not provided
    let session_id = session_id.unwrap_or_else(|| {
        format!("workflow-{}-{}", agent_id, uuid::Uuid::new_v4())
    });

    // Create execution options
    let options = ExecutionOptions::new()
        .with_session_id(session_id.clone())
        .with_user_id("user")
        .with_app_name("agentzero")
        .with_max_iterations(50);

    // Execute the workflow - this returns the event stream
    let result = executor.execute(&message, options).await
        .map_err(|e| format!("Workflow execution failed: {}", e))?;

    let workflow_id = result.workflow_id.clone();
    let invocation_id_clone = invocation_id.clone();

    // Create cancellation flag and register this execution
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    if let Ok(mut executions) = ACTIVE_EXECUTIONS.lock() {
        executions.insert(invocation_id.clone(), cancelled.clone());
    }

    // Spawn streaming as background task so we can return immediately
    // This allows the frontend to set up listeners before events start flowing
    let app_clone = app.clone();
    let invocation_id_for_cleanup = invocation_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = stream_workflow_events(
            &app_clone,
            result.events,
            &workflow_id,
            &invocation_id_clone,
            cancelled,
        ).await {
            tracing::error!("Workflow streaming error: {}", e);
        }
        tracing::info!("Workflow execution completed: {}", invocation_id_clone);

        // Clean up from active executions
        if let Ok(mut executions) = ACTIVE_EXECUTIONS.lock() {
            executions.remove(&invocation_id_for_cleanup);
        }
    });

    // Return immediately so frontend can receive events
    Ok(json!({
        "workflow_id": agent_id,
        "invocation_id": invocation_id,
        "session_id": session_id,
        "response": "",
        "done": false
    }))
}
