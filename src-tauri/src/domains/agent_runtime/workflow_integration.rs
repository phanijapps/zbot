// ============================================================================
// WORKFLOW INTEGRATION
// Bridges workflow-executor crate with Tauri backend
// ============================================================================

use std::sync::Arc;
use futures::StreamExt;
use serde_json::json;
use tauri::{AppHandle, Emitter};

use workflow_executor::{
    WorkflowBuilder, WorkflowLoader,
    ExecutableWorkflow, WorkflowError,
    LlmFactory, ToolsetFactory,
    WorkflowDefinition,
};
use zero_core::{Event, Part, Tool, Toolset};
use zero_app::prelude::{Llm, LlmConfig, OpenAiLlm};

use crate::settings::AppDirs;
use crate::domains::agent_runtime::filesystem::TauriFileSystemContext;
use agent_tools::builtin_tools_with_fs;

// ============================================================================
// LLM FACTORY
// ============================================================================

/// Create an LLM factory that uses the existing provider credentials system
pub fn create_llm_factory() -> LlmFactory {
    Arc::new(|provider_id: &str, model: &str| {
        // We need to load credentials synchronously
        let provider_id = provider_id.to_string();
        let model = model.to_string();

        let (api_key, base_url) = load_provider_credentials_sync(&provider_id)
            .map_err(|e| WorkflowError::LlmConfig(e))?;

        let config = LlmConfig {
            api_key,
            base_url: Some(base_url),
            model,
            organization_id: None,
            temperature: None,
            max_tokens: None,
        };

        let llm = OpenAiLlm::new(config)
            .map_err(|e: zero_app::prelude::ZeroError| WorkflowError::LlmConfig(e.to_string()))?;

        Ok(Arc::new(llm) as Arc<dyn Llm>)
    })
}

/// Load provider credentials synchronously
fn load_provider_credentials_sync(provider_id: &str) -> Result<(String, String), String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    let providers_file = dirs.config_dir.join("providers.json");

    let content = std::fs::read_to_string(&providers_file)
        .map_err(|e| format!("Failed to read providers file: {}", e))?;

    let providers: Vec<serde_json::Value> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse providers: {}", e))?;

    let provider = providers
        .into_iter()
        .find(|p| p.get("id").and_then(|i| i.as_str()) == Some(provider_id))
        .ok_or_else(|| format!("Provider not found: {}", provider_id))?;

    let api_key = provider.get("apiKey")
        .and_then(|k| k.as_str())
        .ok_or_else(|| "Provider missing apiKey".to_string())?
        .to_string();

    let base_url = provider.get("baseUrl")
        .and_then(|u| u.as_str())
        .ok_or_else(|| "Provider missing baseUrl".to_string())?
        .to_string();

    Ok((api_key, base_url))
}

// ============================================================================
// TOOLSET FACTORY
// ============================================================================

/// Create a toolset factory that provides builtin tools
pub fn create_toolset_factory() -> ToolsetFactory {
    Arc::new(|_mcps: &[String], _skills: &[String], _tools: &[String]| {
        // Get app dirs
        let app_dirs = AppDirs::get()
            .map_err(|e| WorkflowError::Framework(e.to_string()))?;

        // Create filesystem context
        let fs_context = TauriFileSystemContext::new(app_dirs);

        // Get builtin tools
        let builtin_tools = builtin_tools_with_fs(Arc::new(fs_context));

        // Create a simple toolset wrapper
        let toolset = WorkflowToolset::new(builtin_tools);

        Ok(Arc::new(toolset) as Arc<dyn Toolset>)
    })
}

/// Simple toolset wrapper for workflow execution
struct WorkflowToolset {
    tools: Vec<Arc<dyn Tool>>,
}

impl WorkflowToolset {
    fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        Self { tools }
    }
}

#[async_trait::async_trait]
impl Toolset for WorkflowToolset {
    fn name(&self) -> &str {
        "workflow_tools"
    }

    async fn tools(&self) -> zero_core::Result<Vec<Arc<dyn Tool>>> {
        Ok(self.tools.clone())
    }
}

// ============================================================================
// EVENT STREAMING
// ============================================================================

/// Stream workflow events to the Tauri frontend
pub async fn stream_workflow_events(
    app: &AppHandle,
    events: zero_core::EventStream,
    workflow_id: &str,
    invocation_id: &str,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<String, String> {
    use futures::pin_mut;
    use std::sync::atomic::Ordering;

    let event_name = format!("workflow-stream://{}", invocation_id);
    let node_event_name = format!("workflow-node://{}", workflow_id);

    let mut final_response = String::new();
    pin_mut!(events);

    while let Some(event_result) = events.next().await {
        // Check if execution was cancelled
        if cancelled.load(Ordering::SeqCst) {
            tracing::info!("Workflow execution cancelled: {}", invocation_id);
            let _ = app.emit(&event_name, json!({
                "type": "cancelled",
                "message": "Execution stopped by user",
                "timestamp": chrono::Utc::now().timestamp_millis(),
            }));
            return Ok(final_response);
        }

        match event_result {
            Ok(event) => {
                // Map event to Tauri payload
                let payload = map_event_to_payload(&event);

                // Emit main event
                if let Err(e) = app.emit(&event_name, payload) {
                    tracing::warn!("Failed to emit workflow event: {}", e);
                }

                // Extract and track final response text
                if let Some(content) = &event.content {
                    for part in &content.parts {
                        if let Part::Text { text } = part {
                            final_response.push_str(text);
                        }
                    }
                }

                // Emit node status for visualization (if we can extract node info)
                if let Some(node_status) = extract_node_status(&event) {
                    if let Err(e) = app.emit(&node_event_name, node_status) {
                        tracing::warn!("Failed to emit node status: {}", e);
                    }
                }
            }
            Err(e) => {
                // Emit error event
                let _ = app.emit(&event_name, json!({
                    "type": "error",
                    "error": e.to_string(),
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                }));

                return Err(e.to_string());
            }
        }
    }

    // Emit completion event
    let _ = app.emit(&event_name, json!({
        "type": "done",
        "finalMessage": final_response,
        "timestamp": chrono::Utc::now().timestamp_millis(),
    }));

    Ok(final_response)
}

/// Map a zero_core::Event to a Tauri-compatible JSON payload
fn map_event_to_payload(event: &Event) -> serde_json::Value {
    // Check for agent lifecycle events first
    if let Some(lifecycle) = event.metadata.get("agent_lifecycle") {
        if let Some(agent_id) = event.metadata.get("agent_id") {
            let event_type = if lifecycle.as_str() == Some("start") {
                "agent_start"
            } else if lifecycle.as_str() == Some("end") {
                "agent_end"
            } else {
                "unknown"
            };

            return json!({
                "type": event_type,
                "agentId": agent_id,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            });
        }
    }

    if let Some(content) = &event.content {
        for part in &content.parts {
            match part {
                Part::Text { text } => {
                    return json!({
                        "type": "token",
                        "content": text,
                        "timestamp": chrono::Utc::now().timestamp_millis(),
                    });
                }
                Part::FunctionCall { id, name, args } => {
                    return json!({
                        "type": "tool_call_start",
                        "toolId": id,
                        "toolName": name,
                        "args": args,
                        "timestamp": chrono::Utc::now().timestamp_millis(),
                    });
                }
                Part::FunctionResponse { id, response } => {
                    return json!({
                        "type": "tool_result",
                        "toolId": id,
                        "result": response,
                        "timestamp": chrono::Utc::now().timestamp_millis(),
                    });
                }
                Part::Binary { .. } => {
                    // Binary parts not yet supported in streaming
                }
            }
        }
    }

    if event.turn_complete {
        return json!({
            "type": "turn_complete",
            "turnComplete": true,
            "timestamp": chrono::Utc::now().timestamp_millis(),
        });
    }

    json!({
        "type": "unknown",
        "timestamp": chrono::Utc::now().timestamp_millis(),
    })
}

/// Extract node status from event for workflow visualization
/// Returns node ID and status if determinable
fn extract_node_status(event: &Event) -> Option<serde_json::Value> {
    // Check for agent lifecycle events (emitted by SequentialAgent)
    if let Some(lifecycle) = event.metadata.get("agent_lifecycle") {
        if let Some(agent_id) = event.metadata.get("agent_id") {
            let agent_id_str = agent_id.as_str().unwrap_or("");
            // Node IDs are formatted as "subagent-{agent_id}"
            let node_id = format!("subagent-{}", agent_id_str);

            let status = if lifecycle.as_str() == Some("start") {
                "running"
            } else if lifecycle.as_str() == Some("end") {
                "completed"
            } else {
                return None;
            };

            return Some(json!({
                "nodeId": node_id,
                "status": status,
                "agentId": agent_id_str,
                "message": format!("Agent {} {}", agent_id_str, if status == "running" { "started" } else { "completed" }),
            }));
        }
    }

    None
}

// ============================================================================
// WORKFLOW BUILDER HELPER
// ============================================================================

/// Create a configured WorkflowBuilder
pub fn create_workflow_builder() -> WorkflowBuilder {
    WorkflowBuilder::new()
        .with_llm_factory(create_llm_factory())
        .with_toolset_factory(create_toolset_factory())
}

/// Load and build a workflow from the agents directory
pub async fn load_and_build_workflow(
    agents_dir: &std::path::Path,
    workflow_name: &str,
) -> Result<(ExecutableWorkflow, WorkflowDefinition), String> {
    let loader = WorkflowLoader::new(agents_dir);

    let definition = loader.load(workflow_name).await
        .map_err(|e| format!("Failed to load workflow: {}", e))?;

    let builder = create_workflow_builder();

    let executable = builder.build(definition.clone()).await
        .map_err(|e| format!("Failed to build workflow: {}", e))?;

    Ok((executable, definition))
}
