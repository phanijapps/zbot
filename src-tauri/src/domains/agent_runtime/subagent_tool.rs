// ============================================================================
// SUBAGENT TOOL
// Tool that wraps a subagent for execution by an orchestrator
// Now with streaming event relay to parent's channel
// ============================================================================

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use zero_app::prelude::*;
use zero_core::ZeroError;

use super::event_emitter;
use super::executor_v2::ZeroAppStreamEvent;

// Type alias for Result with String error type (for Tauri compatibility)
type TResult<T> = std::result::Result<T, String>;

// ============================================================================
// SUBAGENT TOOL
// ============================================================================

/// Tool that executes a subagent with isolated context.
///
/// When called, this tool creates a fresh executor for the subagent,
/// injects the context+task+goal into the system prompt, and streams
/// events back to the parent's event channel for visibility.
pub struct SubagentTool {
    /// Leaked string for 'static lifetime (required by Tool trait)
    name: &'static str,
    /// Leaked string for 'static lifetime
    description: &'static str,
    /// Parent agent ID (orchestrator)
    parent_agent_id: String,
    /// Parent session ID (for event relay)
    parent_session_id: String,
    /// Subagent ID
    subagent_id: String,
}

impl SubagentTool {
    /// Create a new subagent tool
    ///
    /// # Arguments
    /// * `parent_agent_id` - The parent/orchestrator agent ID
    /// * `parent_session_id` - The parent's session ID for event relay
    /// * `subagent_id` - The subagent ID (folder name in .subagents/)
    /// * `description` - Description of what this subagent does
    pub fn new(
        parent_agent_id: String,
        parent_session_id: String,
        subagent_id: String,
        description: String,
    ) -> Self {
        // Leak the name for 'static lifetime (required by Tool trait)
        let name = Box::leak(subagent_id.clone().into_boxed_str());

        // Leak the description for 'static lifetime
        let description = if description.is_empty() {
            format!("Execute the {} subagent", subagent_id)
        } else {
            description
        };
        let description = Box::leak(description.into_boxed_str());

        Self {
            name,
            description,
            parent_agent_id,
            parent_session_id,
            subagent_id,
        }
    }

    /// Execute the subagent with streaming events relayed to parent
    async fn execute_subagent(
        &self,
        context: String,
        task: String,
        goal: String,
    ) -> TResult<String> {
        // Import create_subagent_executor to avoid circular dependency
        use super::executor_v2::create_subagent_executor;

        // Log the tool call clearly
        tracing::info!("========================================");
        tracing::info!("SUBAGENT TOOL CALLED: {}", self.subagent_id);
        tracing::info!("  Parent Orchestrator: {}", self.parent_agent_id);
        tracing::info!("  Parent Session: {}", self.parent_session_id);
        tracing::info!("  Context: {}", context.chars().take(100).collect::<String>());
        if context.len() > 100 { tracing::info!("  ... ({} chars total)", context.len()); }
        tracing::info!("  Task: {}", task.chars().take(100).collect::<String>());
        if task.len() > 100 { tracing::info!("  ... ({} chars total)", task.len()); }
        tracing::info!("  Goal: {}", goal.chars().take(100).collect::<String>());
        if goal.len() > 100 { tracing::info!("  ... ({} chars total)", goal.len()); }
        tracing::info!("========================================");

        // Emit subagent start event
        let _ = event_emitter::emit_subagent_event(
            &self.parent_session_id,
            &self.subagent_id,
            &self.subagent_id,
            "subagent_start",
            json!({
                "type": "subagent_start",
                "timestamp": chrono::Utc::now().timestamp_millis(),
                "task": task.chars().take(200).collect::<String>(),
            }),
        ).await;

        // Create a fresh executor for the subagent
        let executor = create_subagent_executor(
            &self.parent_agent_id,
            &self.subagent_id,
            context,
            task,
            goal,
        ).await?;

        tracing::info!("Subagent executor created, running with streaming...");

        // Run the subagent with streaming and relay events
        let user_message = String::from("Execute your task based on the provided context.");

        // Collect final response while streaming events
        let final_response = Arc::new(Mutex::new(String::new()));
        let final_response_clone = final_response.clone();

        // Track tool calls for activity
        let tool_start_times: Arc<Mutex<std::collections::HashMap<String, Instant>>> =
            Arc::new(Mutex::new(std::collections::HashMap::new()));
        let tool_start_times_clone = tool_start_times.clone();

        let parent_session_id = self.parent_session_id.clone();
        let subagent_id = self.subagent_id.clone();

        // Use run_stream to get real-time events
        executor.run_stream(user_message, move |event| {
            let parent_session_id = parent_session_id.clone();
            let subagent_id = subagent_id.clone();
            let final_response = final_response_clone.clone();
            let tool_start_times = tool_start_times_clone.clone();

            // Spawn async task to handle event
            tokio::spawn(async move {
                match event {
                    ZeroAppStreamEvent::Content { delta } => {
                        // Accumulate content for final response
                        let mut response = final_response.lock().await;
                        response.push_str(&delta);

                        // Relay content event to parent
                        let _ = event_emitter::emit_subagent_event(
                            &parent_session_id,
                            &subagent_id,
                            &subagent_id,
                            "token",
                            json!({
                                "type": "subagent_token",
                                "timestamp": chrono::Utc::now().timestamp_millis(),
                                "content": delta,
                            }),
                        ).await;
                    }
                    ZeroAppStreamEvent::ToolCall { id, name, arguments } => {
                        // Track start time
                        {
                            let mut times = tool_start_times.lock().await;
                            times.insert(id.clone(), Instant::now());
                        }

                        // Relay tool call event
                        let _ = event_emitter::emit_subagent_event(
                            &parent_session_id,
                            &subagent_id,
                            &subagent_id,
                            "tool_call_start",
                            json!({
                                "type": "subagent_tool_call",
                                "timestamp": chrono::Utc::now().timestamp_millis(),
                                "toolId": id,
                                "toolName": name,
                                "status": "running",
                                "args": arguments,
                            }),
                        ).await;
                    }
                    ZeroAppStreamEvent::ToolResponse { id, response } => {
                        // Calculate duration
                        let duration_ms = {
                            let times = tool_start_times.lock().await;
                            times.get(&id).map(|start| start.elapsed().as_millis() as u64)
                        };

                        // Truncate response for preview
                        let result_preview: String = response.chars().take(200).collect();

                        // Check for special markers in the response
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                            // Check for todo_update marker - relay to parent for centralization
                            if parsed.get("__todo_update").and_then(|v| v.as_bool()).unwrap_or(false) {
                                let _ = event_emitter::emit_subagent_event(
                                    &parent_session_id,
                                    &subagent_id,
                                    &subagent_id,
                                    "todo_update",
                                    json!({
                                        "type": "todo_update",
                                        "timestamp": chrono::Utc::now().timestamp_millis(),
                                        "todos": parsed.get("todos"),
                                        "fromSubagent": true,
                                    }),
                                ).await;
                            }
                        }

                        // Relay tool response event
                        let _ = event_emitter::emit_subagent_event(
                            &parent_session_id,
                            &subagent_id,
                            &subagent_id,
                            "tool_result",
                            json!({
                                "type": "subagent_tool_result",
                                "timestamp": chrono::Utc::now().timestamp_millis(),
                                "toolId": id,
                                "status": "success",
                                "durationMs": duration_ms,
                                "resultPreview": result_preview,
                            }),
                        ).await;
                    }
                    ZeroAppStreamEvent::IterationUpdate { current, max } => {
                        // Relay iteration update
                        let _ = event_emitter::emit_subagent_event(
                            &parent_session_id,
                            &subagent_id,
                            &subagent_id,
                            "iteration_update",
                            json!({
                                "type": "subagent_iteration",
                                "timestamp": chrono::Utc::now().timestamp_millis(),
                                "current": current,
                                "max": max,
                            }),
                        ).await;
                    }
                    ZeroAppStreamEvent::Error { message } => {
                        // Relay error event
                        let _ = event_emitter::emit_subagent_event(
                            &parent_session_id,
                            &subagent_id,
                            &subagent_id,
                            "error",
                            json!({
                                "type": "subagent_error",
                                "timestamp": chrono::Utc::now().timestamp_millis(),
                                "message": message,
                            }),
                        ).await;
                    }
                    _ => {
                        // Other events (Complete, Stopped, ContinuationPrompt)
                        // Can be handled if needed
                    }
                }
            });
        }).await?;

        // Get final response
        let result = final_response.lock().await.clone();

        tracing::info!("SUBAGENT TOOL RESULT: {}", result.chars().take(200).collect::<String>());
        if result.len() > 200 {
            tracing::info!("  ... ({} chars total)", result.len());
        }
        tracing::info!("========================================");

        // Emit subagent end event
        let _ = event_emitter::emit_subagent_event(
            &self.parent_session_id,
            &self.subagent_id,
            &self.subagent_id,
            "subagent_end",
            json!({
                "type": "subagent_end",
                "timestamp": chrono::Utc::now().timestamp_millis(),
                "resultPreview": result.chars().take(200).collect::<String>(),
            }),
        ).await;

        Ok(result)
    }
}

#[async_trait]
impl Tool for SubagentTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "context": {
                    "type": "string",
                    "description": "Summary of relevant information from the conversation"
                },
                "task": {
                    "type": "string",
                    "description": "Specific task for the subagent to accomplish"
                },
                "goal": {
                    "type": "string",
                    "description": "Overall goal/vision for context"
                }
            },
            "required": ["context", "task", "goal"]
        }))
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> zero_core::Result<Value> {
        // Extract parameters
        let context = args.get("context")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'context' parameter".to_string()))?
            .to_string();

        let task = args.get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'task' parameter".to_string()))?
            .to_string();

        let goal = args.get("goal")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'goal' parameter".to_string()))?
            .to_string();

        // Execute the subagent
        let result = self.execute_subagent(context, task, goal).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Subagent execution failed: {}", e)))?;

        // Return only the final result (no conversation history)
        Ok(json!({
            "result": result
        }))
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_tool_name() {
        let tool = SubagentTool::new(
            "parent".to_string(),
            "session-123".to_string(),
            "test-subagent".to_string(),
            "A test subagent".to_string(),
        );

        assert_eq!(tool.name(), "test-subagent");
        assert_eq!(tool.description(), "A test subagent");
    }

    #[test]
    fn test_subagent_tool_schema() {
        let tool = SubagentTool::new(
            "parent".to_string(),
            "session-123".to_string(),
            "test-subagent".to_string(),
            "A test subagent".to_string(),
        );

        let schema = tool.parameters_schema().unwrap();
        let props = schema.get("properties").unwrap().as_object().unwrap();

        assert!(props.contains_key("context"));
        assert!(props.contains_key("task"));
        assert!(props.contains_key("goal"));

        let required = schema.get("required").unwrap().as_array().unwrap();
        assert_eq!(required.len(), 3);
    }
}
