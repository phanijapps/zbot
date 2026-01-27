// ============================================================================
// TODO LIST TOOL
// Manages a TODO list stored in session state
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use zero_core::{Result, Tool, ToolContext, ZeroError};

/// State key for TODO list
const TODO_LIST_KEY: &str = "app:todo_list";

// ============================================================================
// TODO DATA TYPES
// ============================================================================

/// A single TODO item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    /// Unique identifier
    pub id: String,
    /// Agent ID that created this TODO
    pub agent_id: String,
    /// Agent name for display
    pub agent_name: String,
    /// Whether this is from the orchestrator (vs subagent)
    pub is_orchestrator: bool,
    /// Title/summary of the task
    pub title: String,
    /// Detailed description (optional)
    pub description: Option<String>,
    /// Whether the task is completed
    pub completed: bool,
    /// Priority level: "low", "medium", "high"
    pub priority: String,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    /// Completion timestamp (ISO 8601, if completed)
    pub completed_at: Option<String>,
}

impl Todo {
    /// Create a new TODO item with agent identity
    pub fn new(
        title: String,
        description: Option<String>,
        priority: String,
        agent_id: String,
        agent_name: String,
        is_orchestrator: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id,
            agent_name,
            is_orchestrator,
            title,
            description,
            completed: false,
            priority,
            created_at: Utc::now().to_rfc3339(),
            completed_at: None,
        }
    }
}

/// The complete TODO list
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoList {
    /// All TODO items
    pub items: Vec<Todo>,
    /// Last update timestamp
    pub last_updated: String,
}

impl TodoList {
    /// Create a new empty TODO list
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            last_updated: Utc::now().to_rfc3339(),
        }
    }

    /// Add a new TODO item
    pub fn add(&mut self, todo: Todo) {
        self.items.push(todo);
        self.last_updated = Utc::now().to_rfc3339();
    }

    /// Update a TODO item's completion status
    pub fn update_completion(&mut self, id: &str, completed: bool) -> bool {
        if let Some(item) = self.items.iter_mut().find(|t| t.id == id) {
            item.completed = completed;
            item.completed_at = if completed {
                Some(Utc::now().to_rfc3339())
            } else {
                None
            };
            self.last_updated = Utc::now().to_rfc3339();
            true
        } else {
            false
        }
    }

    /// Delete a TODO item
    pub fn delete(&mut self, id: &str) -> bool {
        let initial_len = self.items.len();
        self.items.retain(|t| t.id != id);
        if self.items.len() != initial_len {
            self.last_updated = Utc::now().to_rfc3339();
            true
        } else {
            false
        }
    }

    /// Get incomplete items
    pub fn pending(&self) -> Vec<&Todo> {
        self.items.iter().filter(|t| !t.completed).collect()
    }

    /// Get completed items
    pub fn completed(&self) -> Vec<&Todo> {
        self.items.iter().filter(|t| t.completed).collect()
    }
}

// ============================================================================
// TODO LIST TOOL
// ============================================================================

/// Tool for managing a TODO list within agent execution
pub struct TodoTool;

impl TodoTool {
    /// Create a new TODO tool
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Load the TODO list from session state
    /// Note: The backend centralizes TODOs from all agents (orchestrator + subagents)
    fn load_todos(ctx: &Arc<dyn ToolContext>) -> TodoList {
        ctx.get_state(TODO_LIST_KEY)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_else(TodoList::new)
    }

    /// Save the TODO list to session state
    fn save_todos(ctx: &Arc<dyn ToolContext>, todos: &TodoList) {
        if let Ok(value) = serde_json::to_value(todos) {
            ctx.set_state(TODO_LIST_KEY.to_string(), value);
        }
    }

    /// Get agent identity from context
    fn get_agent_identity(ctx: &Arc<dyn ToolContext>) -> (String, String, bool) {
        let agent_id = ctx.get_state("app:agent_id")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "unknown".to_string());

        let root_agent_id = ctx.get_state("app:root_agent_id")
            .and_then(|v| v.as_str().map(String::from));

        // Check if this is the orchestrator (no root_agent_id or same as agent_id)
        let is_orchestrator = root_agent_id.as_ref().map_or(true, |root| root == &agent_id);

        // Extract agent name from agent_id (e.g., "parent.subagent" -> "subagent")
        let agent_name = if is_orchestrator {
            agent_id.clone()
        } else {
            agent_id.rsplit('.').next().unwrap_or(&agent_id).to_string()
        };

        (agent_id, agent_name, is_orchestrator)
    }
}

impl Default for TodoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todos"
    }

    fn description(&self) -> &str {
        "Manage a TODO list for tracking tasks. Supports add, update, delete, and list operations. \
         The TODO list persists across conversation turns within the session."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "update", "delete", "list"],
                    "description": "Action to perform: add (create new), update (mark complete/incomplete), delete (remove), list (show all)"
                },
                "title": {
                    "type": "string",
                    "description": "Title of the TODO item (required for 'add' action)"
                },
                "description": {
                    "type": "string",
                    "description": "Detailed description of the TODO item (optional, for 'add' action)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["low", "medium", "high"],
                    "default": "medium",
                    "description": "Priority level (optional, for 'add' action)"
                },
                "id": {
                    "type": "string",
                    "description": "ID of the TODO item (required for 'update' and 'delete' actions)"
                },
                "completed": {
                    "type": "boolean",
                    "description": "Completion status (for 'update' action)"
                },
                "filter": {
                    "type": "string",
                    "enum": ["all", "pending", "completed"],
                    "default": "all",
                    "description": "Filter for 'list' action"
                }
            },
            "required": ["action"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'action' parameter".to_string()))?;

        match action {
            "add" => {
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ZeroError::Tool("Missing 'title' parameter for add action".to_string()))?;

                let description = args.get("description").and_then(|v| v.as_str()).map(String::from);
                let priority = args
                    .get("priority")
                    .and_then(|v| v.as_str())
                    .unwrap_or("medium")
                    .to_string();

                // Get agent identity for the TODO
                let (agent_id, agent_name, is_orchestrator) = Self::get_agent_identity(&ctx);

                let todo = Todo::new(
                    title.to_string(),
                    description,
                    priority,
                    agent_id,
                    agent_name,
                    is_orchestrator,
                );
                let id = todo.id.clone();

                let mut todos = Self::load_todos(&ctx);
                todos.add(todo);
                Self::save_todos(&ctx, &todos);

                Ok(json!({
                    "__todo_update": true,
                    "todos": todos,
                    "success": true,
                    "action": "add",
                    "id": id,
                    "message": format!("Created TODO: {}", title),
                    "total_items": todos.items.len(),
                    "pending_count": todos.pending().len()
                }))
            }

            "update" => {
                let id = args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ZeroError::Tool("Missing 'id' parameter for update action".to_string()))?;

                let completed = args
                    .get("completed")
                    .and_then(|v| v.as_bool())
                    .ok_or_else(|| ZeroError::Tool("Missing 'completed' parameter for update action".to_string()))?;

                let mut todos = Self::load_todos(&ctx);
                let found = todos.update_completion(id, completed);

                if found {
                    Self::save_todos(&ctx, &todos);
                    Ok(json!({
                        "__todo_update": true,
                        "todos": todos,
                        "success": true,
                        "action": "update",
                        "id": id,
                        "completed": completed,
                        "message": if completed { "Marked as completed" } else { "Marked as incomplete" },
                        "pending_count": todos.pending().len()
                    }))
                } else {
                    Err(ZeroError::Tool(format!("TODO item not found: {}", id)))
                }
            }

            "delete" => {
                let id = args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ZeroError::Tool("Missing 'id' parameter for delete action".to_string()))?;

                let mut todos = Self::load_todos(&ctx);
                let found = todos.delete(id);

                if found {
                    Self::save_todos(&ctx, &todos);
                    Ok(json!({
                        "__todo_update": true,
                        "todos": todos,
                        "success": true,
                        "action": "delete",
                        "id": id,
                        "message": "TODO item deleted",
                        "total_items": todos.items.len()
                    }))
                } else {
                    Err(ZeroError::Tool(format!("TODO item not found: {}", id)))
                }
            }

            "list" => {
                let filter = args
                    .get("filter")
                    .and_then(|v| v.as_str())
                    .unwrap_or("all");

                let todos = Self::load_todos(&ctx);

                let items: Vec<&Todo> = match filter {
                    "pending" => todos.pending(),
                    "completed" => todos.completed(),
                    _ => todos.items.iter().collect(),
                };

                Ok(json!({
                    "__todo_update": true,
                    "todos": todos,
                    "success": true,
                    "action": "list",
                    "filter": filter,
                    "items": items,
                    "total_count": todos.items.len(),
                    "pending_count": todos.pending().len(),
                    "completed_count": todos.completed().len(),
                    "last_updated": todos.last_updated
                }))
            }

            _ => Err(ZeroError::Tool(format!("Unknown action: {}", action))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_todo(title: &str, priority: &str) -> Todo {
        Todo::new(
            title.to_string(),
            None,
            priority.to_string(),
            "test-agent".to_string(),
            "test-agent".to_string(),
            true,
        )
    }

    #[test]
    fn test_todo_new() {
        let todo = make_todo("Test task", "medium");
        assert!(!todo.id.is_empty());
        assert_eq!(todo.title, "Test task");
        assert!(!todo.completed);
        assert_eq!(todo.priority, "medium");
        assert_eq!(todo.agent_id, "test-agent");
        assert!(todo.is_orchestrator);
    }

    #[test]
    fn test_todolist_add_and_delete() {
        let mut list = TodoList::new();
        let todo = make_todo("Task 1", "high");
        let id = todo.id.clone();

        list.add(todo);
        assert_eq!(list.items.len(), 1);

        assert!(list.delete(&id));
        assert_eq!(list.items.len(), 0);

        // Delete non-existent
        assert!(!list.delete("fake-id"));
    }

    #[test]
    fn test_todolist_update_completion() {
        let mut list = TodoList::new();
        let todo = make_todo("Task 1", "medium");
        let id = todo.id.clone();
        list.add(todo);

        assert!(list.update_completion(&id, true));
        assert!(list.items[0].completed);
        assert!(list.items[0].completed_at.is_some());

        assert!(list.update_completion(&id, false));
        assert!(!list.items[0].completed);
        assert!(list.items[0].completed_at.is_none());
    }

    #[test]
    fn test_todolist_filters() {
        let mut list = TodoList::new();

        let t1 = make_todo("Pending 1", "low");
        let t2 = make_todo("Pending 2", "medium");
        let mut t3 = make_todo("Done", "high");
        t3.completed = true;

        list.add(t1);
        list.add(t2);
        list.add(t3);

        assert_eq!(list.pending().len(), 2);
        assert_eq!(list.completed().len(), 1);
    }
}
