# Task 08: Agent Delegation Framework

## Context

The user's vision includes agents that can delegate tasks to subagents in a fire-and-forget pattern. When a subagent completes, it sends a callback message to the parent agent's conversation.

### Key Concepts
1. **Fire-and-forget**: Parent delegates and continues (doesn't block)
2. **Callback on completion**: Subagent result triggers message to parent
3. **Task-scoped context**: Parent passes relevant context slice to subagent
4. **Context preservation**: Each agent maintains its own conversation history

---

## Specifications (BDD)

### Feature: Delegate Tool

```gherkin
Feature: Delegate to Subagent
  As an agent (root or intermediate)
  I want to delegate tasks to specialized subagents
  So that I can handle complex multi-step workflows

  Background:
    Given agent "root" exists
    And agent "research-agent" exists
    And agent "writing-agent" exists
    And "root" has subagents ["research-agent", "writing-agent"]

  Scenario: Basic delegation
    Given I am the root agent
    When I call delegate_to_agent tool with:
      | agent_id | research-agent                    |
      | task     | Find information about quantum computing |
    Then a new conversation is created for research-agent
    And research-agent is invoked with task message
    And delegate tool returns immediately:
      """
      {
        "status": "delegated",
        "subagent": "research-agent",
        "subagent_conversation_id": "sub-conv-uuid"
      }
      """
    And I (root) can continue processing

  Scenario: Delegation with context
    Given I am processing a user request about "travel planning"
    When I delegate to "research-agent" with:
      | task    | Find flights from NYC to LAX |
      | context | {"budget": 500, "dates": "March 15-20"} |
    Then research-agent receives task message including context
    And context is prefixed to task message

  Scenario: Subagent completion triggers callback
    Given research-agent is working on a delegated task
    And delegation has parent_conversation_id "parent-conv-123"
    When research-agent completes with result "Found 5 relevant papers"
    Then a callback message is sent to parent-conv-123:
      """
      [Subagent research-agent completed]
      Task: Find information about quantum computing
      Result: Found 5 relevant papers
      """
    And root agent processes this as a new message
    And root agent can use the result or delegate further

  Scenario: Subagent uses respond tool
    Given research-agent is processing a delegated task
    When research-agent uses respond tool
    Then response goes to ORIGINAL hook (user's channel)
    Not to the parent agent

  Scenario: Delegation not allowed to unknown agent
    Given I am root agent
    When I try to delegate to "unknown-agent"
    Then delegate tool returns error:
      | error | Agent not found or delegation not allowed |

  Scenario: Chain of delegation
    Given root delegates to research-agent
    And research-agent delegates to data-agent
    When data-agent completes
    Then callback goes to research-agent's conversation
    And when research-agent completes
    Then callback goes to root's conversation
```

### Feature: Agent Relationships

```gherkin
Feature: Agent Subagent Relationships
  As an administrator
  I want to define which agents can delegate to which
  So that I can control the agent hierarchy

  Scenario: Define subagents in agent config
    Given agent config for "root":
      """
      id: root
      name: Root Agent
      subagents:
        - research-agent
        - writing-agent
        - scheduling-agent
      """
    Then root can delegate to research-agent
    And root can delegate to writing-agent
    And root cannot delegate to unknown-agent

  Scenario: Subagent can have its own subagents
    Given agent config for "research-agent":
      """
      id: research-agent
      subagents:
        - web-search-agent
        - database-agent
      """
    Then research-agent can delegate to web-search-agent
    But root cannot directly delegate to web-search-agent
```

---

## Implementation

### File: `application/gateway/src/agents/config.yaml` (schema update)

Add subagents field to agent config:

```yaml
id: root
name: Root Agent
provider_id: anthropic
model: claude-3-5-sonnet
subagents:
  - research-agent
  - writing-agent
  - scheduling-agent
```

### File: `application/gateway/src/services/agents.rs` (update)

Add subagents to Agent struct:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    // ... existing fields ...

    /// List of agent IDs this agent can delegate to
    #[serde(default)]
    pub subagents: Vec<String>,
}

impl AgentService {
    /// Check if an agent can delegate to another
    pub fn can_delegate(&self, from_agent_id: &str, to_agent_id: &str) -> bool {
        if let Some(agent) = self.get(from_agent_id) {
            agent.subagents.contains(&to_agent_id.to_string())
        } else {
            false
        }
    }

    /// Get available subagents for an agent
    pub fn get_subagents(&self, agent_id: &str) -> Vec<String> {
        self.get(agent_id)
            .map(|a| a.subagents.clone())
            .unwrap_or_default()
    }
}
```

### File: `application/gateway/src/delegation/mod.rs` (new)

```rust
mod context;
mod handler;

pub use context::DelegationContext;
pub use handler::DelegationHandler;
```

### File: `application/gateway/src/delegation/context.rs`

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Context for a delegated task
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DelegationContext {
    /// ID of the parent agent that delegated
    pub parent_agent_id: String,

    /// Conversation ID of the parent (for callback)
    pub parent_conversation_id: String,

    /// The original task description
    pub task: String,

    /// Task-scoped context passed from parent
    pub task_context: Option<Value>,

    /// Whether to send callback when complete
    pub callback_on_complete: bool,

    /// Original hook context (for respond tool)
    pub original_hook_context: crate::hooks::HookContext,
}
```

### File: `application/gateway/src/delegation/handler.rs`

```rust
use super::DelegationContext;
use crate::services::RuntimeService;
use std::sync::Arc;

pub struct DelegationHandler {
    runtime: Arc<RuntimeService>,
}

impl DelegationHandler {
    pub fn new(runtime: Arc<RuntimeService>) -> Self {
        Self { runtime }
    }

    /// Handle subagent completion - send callback to parent
    pub async fn handle_completion(
        &self,
        delegation: &DelegationContext,
        result: &str,
    ) -> Result<(), String> {
        if !delegation.callback_on_complete {
            return Ok(());
        }

        // Format callback message
        let callback_message = format!(
            "[Subagent Completed]\n\
             Task: {}\n\
             Result: {}",
            delegation.task,
            result
        );

        tracing::info!(
            parent_agent = %delegation.parent_agent_id,
            parent_conversation = %delegation.parent_conversation_id,
            "Sending delegation callback to parent"
        );

        // Send message to parent's conversation
        // Use original hook context so responses still go to user
        self.runtime.invoke_with_hook(
            &delegation.parent_agent_id,
            &delegation.parent_conversation_id,
            &callback_message,
            delegation.original_hook_context.clone(),
        ).await
    }
}
```

### File: `application/agent-runtime/src/tools/delegate.rs` (new)

```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::tools::{Tool, ToolContext, ToolError, ToolResult};

/// Tool for delegating tasks to subagents
pub struct DelegateTool;

impl DelegateTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for DelegateTool {
    fn name(&self) -> &str {
        "delegate_to_agent"
    }

    fn description(&self) -> &str {
        "Delegate a task to a specialized subagent. The subagent will work on the task \
         independently, and you will receive a callback message when it completes. \
         Use this for complex tasks that require specialized knowledge."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "agent_id": {
                    "type": "string",
                    "description": "ID of the subagent to delegate to"
                },
                "task": {
                    "type": "string",
                    "description": "Description of the task for the subagent"
                },
                "context": {
                    "type": "object",
                    "description": "Optional context to pass to the subagent"
                }
            },
            "required": ["agent_id", "task"]
        })
    }

    async fn execute(&self, context: &ToolContext, args: Value) -> ToolResult {
        let target_agent_id = args.get("agent_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgument("agent_id is required".into()))?;

        let task = args.get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgument("task is required".into()))?;

        let task_context = args.get("context").cloned();

        // Get current agent info from context
        let current_agent_id = context.get_state::<String>("agent_id")
            .ok_or_else(|| ToolError::ExecutionError("No agent_id in context".into()))?;

        let current_conversation_id = context.get_state::<String>("conversation_id")
            .ok_or_else(|| ToolError::ExecutionError("No conversation_id in context".into()))?;

        let hook_context = context.get_state::<crate::hooks::HookContext>("hook_context")
            .ok_or_else(|| ToolError::ExecutionError("No hook_context in context".into()))?;

        // Get agent service to check delegation is allowed
        let agent_service = context.get_state::<Arc<crate::services::AgentService>>("agent_service")
            .ok_or_else(|| ToolError::ExecutionError("No agent_service in context".into()))?;

        if !agent_service.can_delegate(&current_agent_id, target_agent_id) {
            return Err(ToolError::ExecutionError(format!(
                "Agent {} cannot delegate to {}. Check subagents configuration.",
                current_agent_id, target_agent_id
            )));
        }

        // Create delegation context
        let delegation = crate::delegation::DelegationContext {
            parent_agent_id: current_agent_id.clone(),
            parent_conversation_id: current_conversation_id.clone(),
            task: task.to_string(),
            task_context: task_context.clone(),
            callback_on_complete: true,
            original_hook_context: hook_context.clone(),
        };

        // Create subagent conversation ID
        let subagent_conversation_id = format!(
            "{}-sub-{}",
            current_conversation_id,
            Uuid::new_v4()
        );

        // Format task message with context
        let task_message = if let Some(ctx) = &task_context {
            format!(
                "[Delegated Task]\nContext: {}\n\nTask: {}",
                serde_json::to_string_pretty(ctx).unwrap_or_default(),
                task
            )
        } else {
            format!("[Delegated Task]\n{}", task)
        };

        // Get runtime and spawn subagent execution
        let runtime = context.get_state::<Arc<crate::services::RuntimeService>>("runtime")
            .ok_or_else(|| ToolError::ExecutionError("No runtime in context".into()))?;

        let delegation_handler = context.get_state::<Arc<crate::delegation::DelegationHandler>>("delegation_handler")
            .ok_or_else(|| ToolError::ExecutionError("No delegation_handler in context".into()))?;

        // Fire and forget - spawn async task
        let target_id = target_agent_id.to_string();
        let sub_conv_id = subagent_conversation_id.clone();
        let msg = task_message.clone();
        let del = delegation.clone();
        let rt = runtime.clone();
        let dh = delegation_handler.clone();

        tokio::spawn(async move {
            tracing::info!(
                parent = %del.parent_agent_id,
                subagent = %target_id,
                conversation = %sub_conv_id,
                "Starting delegated task"
            );

            // Execute subagent
            match rt.invoke_with_delegation(&target_id, &sub_conv_id, &msg, del.clone()).await {
                Ok(result) => {
                    // Send callback to parent
                    if let Err(e) = dh.handle_completion(&del, &result).await {
                        tracing::error!(error = %e, "Failed to send delegation callback");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Delegated task failed");
                    // Send error callback to parent
                    let error_result = format!("Error: {}", e);
                    if let Err(e) = dh.handle_completion(&del, &error_result).await {
                        tracing::error!(error = %e, "Failed to send error callback");
                    }
                }
            }
        });

        Ok(json!({
            "status": "delegated",
            "subagent": target_agent_id,
            "subagent_conversation_id": subagent_conversation_id
        }))
    }
}

impl Default for DelegateTool {
    fn default() -> Self {
        Self::new()
    }
}
```

### File: `application/gateway/src/execution/runner.rs` (additions)

Add `invoke_with_delegation` method:

```rust
impl ExecutionRunner {
    /// Invoke agent as a delegated task (returns result, doesn't stream)
    pub async fn invoke_with_delegation(
        &self,
        agent_id: &str,
        conversation_id: &str,
        message: &str,
        delegation: DelegationContext,
    ) -> Result<String, String> {
        // Use original hook context for respond tool
        let hook_context = delegation.original_hook_context.clone();

        // ... similar to invoke_with_hook but accumulates result ...

        let mut accumulated_response = String::new();

        // Execute and collect response
        let callback = |event: StreamEvent| {
            if let StreamEvent::Token { content, .. } = event {
                accumulated_response.push_str(&content);
            }
        };

        // ... execute agent ...

        Ok(accumulated_response)
    }
}
```

---

## Verification

### Unit Tests

```rust
#[test]
fn test_can_delegate() {
    let agent_service = create_test_agent_service();

    // Root has research-agent as subagent
    assert!(agent_service.can_delegate("root", "research-agent"));

    // Root doesn't have unknown as subagent
    assert!(!agent_service.can_delegate("root", "unknown-agent"));

    // Research-agent can't delegate to root (not in its subagents)
    assert!(!agent_service.can_delegate("research-agent", "root"));
}

#[tokio::test]
async fn test_delegate_tool_validates_subagent() {
    let tool = DelegateTool::new();
    let context = create_context_with_agent("root");

    // Try to delegate to non-subagent
    let result = tool.execute(&context, json!({
        "agent_id": "not-a-subagent",
        "task": "Do something"
    })).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot delegate"));
}

#[tokio::test]
async fn test_delegation_callback() {
    let handler = DelegationHandler::new(/* ... */);

    let delegation = DelegationContext {
        parent_agent_id: "root".into(),
        parent_conversation_id: "parent-conv".into(),
        task: "Research topic".into(),
        task_context: None,
        callback_on_complete: true,
        original_hook_context: create_web_hook_context(),
    };

    let result = handler.handle_completion(&delegation, "Found 5 results").await;
    assert!(result.is_ok());

    // Parent conversation should have received callback message
}
```

### Integration Test

```bash
# Create agents with subagent relationships
# agents/root/config.yaml:
# subagents: ["research-agent"]

# agents/research-agent/config.yaml:
# (no subagents)

# Invoke root with a task that requires delegation
curl -X POST http://localhost:18791/api/hooks/web/invoke \
  -H "Content-Type: application/json" \
  -d '{
    "source_id": "test-user",
    "message": "I need you to research quantum computing and then summarize the findings"
  }'

# Root agent should:
# 1. Delegate research to research-agent
# 2. Receive callback when research-agent completes
# 3. Summarize and respond
```

---

## Dependencies

- Task 01-07 complete
- Agent config schema update

## Outputs

- `application/gateway/src/delegation/mod.rs`
- `application/gateway/src/delegation/context.rs`
- `application/gateway/src/delegation/handler.rs`
- `application/agent-runtime/src/tools/delegate.rs`
- Modified: `services/agents.rs`, `execution/runner.rs`

## Summary

This completes the hook framework and agent delegation system:

1. **Tasks 01-03**: Built-in hooks (CLI, Web, Cron)
2. **Tasks 04-06**: External hook registration, invocation, callbacks
3. **Task 07**: Respond tool for unified response routing
4. **Task 08**: Agent delegation with fire-and-forget pattern
