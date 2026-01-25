// ============================================================================
// SUBAGENT TOOL
// Tool that wraps a subagent for execution by an orchestrator
// ============================================================================

use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{json, Value};

use zero_app::prelude::*;
use zero_core::ZeroError;

// Type alias for Result with String error type (for Tauri compatibility)
type TResult<T> = std::result::Result<T, String>;

// ============================================================================
// SUBAGENT TOOL
// ============================================================================

/// Tool that executes a subagent with isolated context.
///
/// When called, this tool creates a fresh executor for the subagent,
/// injects the context+task+goal into the system prompt, and returns
/// only the final result (not the subagent's conversation history).
pub struct SubagentTool {
    /// Leaked string for 'static lifetime (required by Tool trait)
    name: &'static str,
    /// Leaked string for 'static lifetime
    description: &'static str,
    /// Parent agent ID (orchestrator)
    parent_agent_id: String,
    /// Subagent ID
    subagent_id: String,
}

impl SubagentTool {
    /// Create a new subagent tool
    ///
    /// # Arguments
    /// * `parent_agent_id` - The parent/orchestrator agent ID
    /// * `subagent_id` - The subagent ID (folder name in .subagents/)
    /// * `description` - Description of what this subagent does
    pub fn new(
        parent_agent_id: String,
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
            subagent_id,
        }
    }

    /// Execute the subagent and return only the final result
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
        tracing::info!("  Context: {}", context.chars().take(100).collect::<String>());
        if context.len() > 100 { tracing::info!("  ... ({} chars total)", context.len()); }
        tracing::info!("  Task: {}", task.chars().take(100).collect::<String>());
        if task.len() > 100 { tracing::info!("  ... ({} chars total)", task.len()); }
        tracing::info!("  Goal: {}", goal.chars().take(100).collect::<String>());
        if goal.len() > 100 { tracing::info!("  ... ({} chars total)", goal.len()); }
        tracing::info!("========================================");

        // Create a fresh executor for the subagent
        let executor = create_subagent_executor(
            &self.parent_agent_id,
            &self.subagent_id,
            context,
            task,
            goal,
        ).await?;

        tracing::info!("Subagent executor created, running...");

        // Run the subagent with a simple prompt to trigger execution
        // The actual work is defined by the injected context+task+goal
        let user_message = String::from("Execute your task based on the provided context.");

        // Collect all events and extract the final assistant response
        let events = executor.run(user_message).await?;

        tracing::info!("Subagent execution completed, extracting final result...");

        // Extract the final text response from events
        let mut final_response = String::new();
        for event in events {
            if let Some(content) = event.content {
                if content.role == "assistant" {
                    for part in content.parts {
                        if let zero_app::Part::Text { text } = part {
                            final_response.push_str(&text);
                        }
                    }
                }
            }
        }

        tracing::info!("SUBAGENT TOOL RESULT: {}", final_response.chars().take(200).collect::<String>());
        if final_response.len() > 200 {
            tracing::info!("  ... ({} chars total)", final_response.len());
        }
        tracing::info!("========================================");

        Ok(final_response)
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
