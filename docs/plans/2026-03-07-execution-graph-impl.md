# ExecutionGraphTool Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a runtime DAG workflow engine that lets the orchestrator agent dynamically create and execute multi-step workflows with conditional branching, result routing, and retry support.

**Architecture:** Single Rust tool (`execution_graph`) with 4 actions (create/execute_next/status/add_node) stored in session state. Works with existing delegation system — tool determines what's ready, agent dispatches via `delegate_to_agent`.

**Tech Stack:** Rust, serde_json, async-trait, zero-core (Tool trait, ToolContext)

---

### Task 1: Data Model — Graph types and condition evaluator

**Files:**
- Create: `runtime/agent-tools/src/tools/execution/graph.rs`

**Step 1: Define all data types**

Write the core structs: `ExecutionGraph`, `GraphNode`, `Condition`, `InputRef`, `RetryPolicy`, and all enums (`GraphStatus`, `NodeStatus`, `DependMode`, `ConditionOp`, `TimeoutAction`). All types derive `Serialize, Deserialize, Clone, Debug`.

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionGraph {
    pub id: String,
    pub nodes: HashMap<String, GraphNode>,
    pub status: GraphStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GraphStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub agent: String,
    pub task: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_depend_mode")]
    pub depend_mode: DependMode,
    #[serde(default)]
    pub when: Option<Condition>,
    #[serde(default)]
    pub inputs: HashMap<String, InputRef>,
    #[serde(default)]
    pub retry: Option<RetryPolicy>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default = "default_timeout_action")]
    pub on_timeout: TimeoutAction,
    #[serde(default)]
    pub status: NodeStatus,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    #[default]
    Pending,
    Ready,
    Running,
    Completed,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DependMode {
    #[default]
    All,
    AnyCompleted,
    AnyOne,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    #[serde(rename = "ref")]
    pub ref_node: String,
    #[serde(default = "default_field")]
    pub field: String,
    pub operator: ConditionOp,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOp {
    Contains,
    NotContains,
    Equals,
    Gt,
    Lt,
    Regex,
    LlmEval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRef {
    pub from: String,
    pub field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    #[serde(default = "default_max_retries")]
    pub max: u32,
    pub on_fail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutAction {
    #[default]
    Skip,
    Fail,
}

fn default_depend_mode() -> DependMode { DependMode::All }
fn default_timeout_action() -> TimeoutAction { TimeoutAction::Skip }
fn default_field() -> String { "result".to_string() }
fn default_max_retries() -> u32 { 1 }
```

**Step 2: Implement condition evaluator**

Add an `evaluate_condition` function that checks a condition against a node's result.

```rust
impl Condition {
    /// Evaluate this condition against completed node results.
    /// Returns Ok(true) if condition passes, Ok(false) if it fails.
    /// Returns Err with prompt string for llm_eval (agent must evaluate).
    pub fn evaluate(&self, nodes: &HashMap<String, GraphNode>) -> Result<bool, String> {
        let node = nodes.get(&self.ref_node)
            .ok_or_else(|| format!("Referenced node '{}' not found", self.ref_node))?;

        let field_value = match self.field.as_str() {
            "result" => node.result.clone().unwrap_or_default(),
            "status" => serde_json::to_string(&node.status).unwrap_or_default(),
            "error" => node.error.clone().unwrap_or_default(),
            _ => return Err(format!("Unknown field: {}", self.field)),
        };

        match self.operator {
            ConditionOp::Contains => Ok(field_value.contains(&self.value)),
            ConditionOp::NotContains => Ok(!field_value.contains(&self.value)),
            ConditionOp::Equals => Ok(field_value == self.value),
            ConditionOp::Gt => {
                let a: f64 = field_value.parse().unwrap_or(0.0);
                let b: f64 = self.value.parse().unwrap_or(0.0);
                Ok(a > b)
            }
            ConditionOp::Lt => {
                let a: f64 = field_value.parse().unwrap_or(0.0);
                let b: f64 = self.value.parse().unwrap_or(0.0);
                Ok(a < b)
            }
            ConditionOp::Regex => {
                let re = regex::Regex::new(&self.value)
                    .map_err(|e| format!("Invalid regex: {}", e))?;
                Ok(re.is_match(&field_value))
            }
            ConditionOp::LlmEval => {
                // Return the prompt for the agent to evaluate
                Err(format!(
                    "LLM_EVAL_REQUIRED: Evaluate this condition and call execute_next with the result. Question: {} Value from node '{}': {}",
                    self.value, self.ref_node, field_value
                ))
            }
        }
    }
}
```

**Step 3: Implement graph helper methods**

```rust
impl ExecutionGraph {
    pub fn new(id: String, nodes: Vec<GraphNode>) -> Self {
        let mut node_map = HashMap::new();
        for node in nodes {
            node_map.insert(node.id.clone(), node);
        }
        Self {
            id,
            nodes: node_map,
            status: GraphStatus::Pending,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Find nodes that are ready to execute (all dependencies met, conditions passed).
    pub fn find_ready_nodes(&mut self) -> Vec<ReadyNode> {
        let mut ready = Vec::new();
        let node_ids: Vec<String> = self.nodes.keys().cloned().collect();

        for node_id in &node_ids {
            let node = self.nodes.get(node_id).unwrap().clone();
            if node.status != NodeStatus::Pending {
                continue;
            }

            // Check dependencies
            let deps_met = match node.depend_mode {
                DependMode::All => node.depends_on.iter().all(|dep| {
                    self.nodes.get(dep).map_or(false, |n| {
                        n.status == NodeStatus::Completed || n.status == NodeStatus::Skipped
                    })
                }),
                DependMode::AnyCompleted => node.depends_on.is_empty() || node.depends_on.iter().any(|dep| {
                    self.nodes.get(dep).map_or(false, |n| n.status == NodeStatus::Completed)
                }),
                DependMode::AnyOne => node.depends_on.is_empty() || node.depends_on.iter().any(|dep| {
                    self.nodes.get(dep).map_or(false, |n| {
                        n.status == NodeStatus::Completed || n.status == NodeStatus::Skipped || n.status == NodeStatus::Failed
                    })
                }),
            };

            if !deps_met {
                continue;
            }

            // Check condition
            if let Some(condition) = &node.when {
                match condition.evaluate(&self.nodes) {
                    Ok(true) => {}      // Condition passed
                    Ok(false) => {       // Condition failed — skip node
                        if let Some(n) = self.nodes.get_mut(node_id) {
                            n.status = NodeStatus::Skipped;
                        }
                        continue;
                    }
                    Err(prompt) => {
                        // LLM eval needed — include in ready with the prompt
                        ready.push(ReadyNode {
                            id: node_id.clone(),
                            agent: node.agent.clone(),
                            task: self.resolve_task(&node),
                            llm_eval_prompt: Some(prompt),
                        });
                        continue;
                    }
                }
            }

            // Node is ready
            if let Some(n) = self.nodes.get_mut(node_id) {
                n.status = NodeStatus::Ready;
            }
            ready.push(ReadyNode {
                id: node_id.clone(),
                agent: node.agent.clone(),
                task: self.resolve_task(&node),
                llm_eval_prompt: None,
            });
        }

        if !ready.is_empty() && self.status == GraphStatus::Pending {
            self.status = GraphStatus::Running;
        }

        // Check if graph is complete
        if ready.is_empty() {
            let all_done = self.nodes.values().all(|n| {
                matches!(n.status, NodeStatus::Completed | NodeStatus::Skipped | NodeStatus::Failed)
            });
            if all_done {
                let any_failed = self.nodes.values().any(|n| n.status == NodeStatus::Failed);
                self.status = if any_failed { GraphStatus::Failed } else { GraphStatus::Completed };
            }
        }

        ready
    }

    /// Resolve input references in task description.
    fn resolve_task(&self, node: &GraphNode) -> String {
        let mut task = node.task.clone();
        for (param, input_ref) in &node.inputs {
            if let Some(upstream) = self.nodes.get(&input_ref.from) {
                let value = match input_ref.field.as_str() {
                    "result" => upstream.result.clone().unwrap_or_default(),
                    _ => String::new(),
                };
                task = task.replace(&format!("{{{}}}", param), &value);
            }
        }
        task
    }

    /// Mark a node as completed with result.
    pub fn complete_node(&mut self, node_id: &str, result: String) -> bool {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.status = NodeStatus::Completed;
            node.result = Some(result);
            node.attempts += 1;
            true
        } else {
            false
        }
    }

    /// Mark a node as failed.
    pub fn fail_node(&mut self, node_id: &str, error: String) -> bool {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.attempts += 1;
            // Check retry
            if let Some(retry) = &node.retry {
                if node.attempts < retry.max {
                    node.status = NodeStatus::Pending; // will be re-evaluated
                    node.error = Some(error);
                    return true;
                }
                // If retry has fallback, skip this node and add fallback info
                if retry.on_fail.is_some() {
                    node.status = NodeStatus::Failed;
                    node.error = Some(error);
                    return true;
                }
            }
            node.status = NodeStatus::Failed;
            node.error = Some(error);
            true
        } else {
            false
        }
    }

    /// Add a node to the graph dynamically.
    pub fn add_node(&mut self, node: GraphNode) -> bool {
        if self.nodes.contains_key(&node.id) {
            return false;
        }
        self.nodes.insert(node.id.clone(), node);
        true
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadyNode {
    pub id: String,
    pub agent: String,
    pub task: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_eval_prompt: Option<String>,
}
```

**Step 4: Write unit tests for data model**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_condition_contains() {
        let mut nodes = HashMap::new();
        nodes.insert("a".to_string(), GraphNode {
            id: "a".to_string(),
            agent: "test".to_string(),
            task: "test".to_string(),
            status: NodeStatus::Completed,
            result: Some("The analysis shows positive results".to_string()),
            ..Default::default()
        });

        let cond = Condition {
            ref_node: "a".to_string(),
            field: "result".to_string(),
            operator: ConditionOp::Contains,
            value: "positive".to_string(),
        };

        assert!(cond.evaluate(&nodes).unwrap());
    }

    #[test]
    fn test_find_ready_nodes_simple() {
        let nodes = vec![
            GraphNode { id: "a".to_string(), agent: "research".to_string(), task: "research X".to_string(), ..Default::default() },
            GraphNode { id: "b".to_string(), agent: "research".to_string(), task: "research Y".to_string(), ..Default::default() },
            GraphNode { id: "c".to_string(), agent: "writer".to_string(), task: "write report".to_string(), depends_on: vec!["a".to_string(), "b".to_string()], ..Default::default() },
        ];
        let mut graph = ExecutionGraph::new("test".to_string(), nodes);

        let ready = graph.find_ready_nodes();
        assert_eq!(ready.len(), 2);
        assert!(ready.iter().any(|n| n.id == "a"));
        assert!(ready.iter().any(|n| n.id == "b"));

        // Complete A and B
        graph.complete_node("a", "result A".to_string());
        graph.complete_node("b", "result B".to_string());

        let ready = graph.find_ready_nodes();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "c");
    }

    #[test]
    fn test_conditional_skip() {
        let nodes = vec![
            GraphNode {
                id: "a".to_string(), agent: "analyst".to_string(), task: "analyze".to_string(),
                status: NodeStatus::Completed, result: Some("negative outlook".to_string()),
                ..Default::default()
            },
            GraphNode {
                id: "b".to_string(), agent: "writer".to_string(), task: "write positive report".to_string(),
                depends_on: vec!["a".to_string()],
                when: Some(Condition { ref_node: "a".to_string(), field: "result".to_string(), operator: ConditionOp::Contains, value: "positive".to_string() }),
                ..Default::default()
            },
        ];
        let mut graph = ExecutionGraph::new("test".to_string(), nodes);

        let ready = graph.find_ready_nodes();
        assert!(ready.is_empty());
        assert_eq!(graph.nodes.get("b").unwrap().status, NodeStatus::Skipped);
    }

    #[test]
    fn test_input_resolution() {
        let nodes = vec![
            GraphNode {
                id: "a".to_string(), agent: "research".to_string(), task: "research".to_string(),
                status: NodeStatus::Completed, result: Some("important findings".to_string()),
                ..Default::default()
            },
            GraphNode {
                id: "b".to_string(), agent: "writer".to_string(),
                task: "Write a report based on: {research_data}".to_string(),
                depends_on: vec!["a".to_string()],
                inputs: HashMap::from([("research_data".to_string(), InputRef { from: "a".to_string(), field: "result".to_string() })]),
                ..Default::default()
            },
        ];
        let mut graph = ExecutionGraph::new("test".to_string(), nodes);
        let ready = graph.find_ready_nodes();

        assert_eq!(ready.len(), 1);
        assert!(ready[0].task.contains("important findings"));
    }
}
```

**Step 5: Verify it compiles**

Run: `cargo check -p agent-tools`

**Step 6: Commit**

```bash
git add runtime/agent-tools/src/tools/execution/graph.rs
git commit -m "feat(tools): add ExecutionGraph data model and condition evaluator"
```

---

### Task 2: Tool Implementation — ExecutionGraphTool with 4 actions

**Files:**
- Modify: `runtime/agent-tools/src/tools/execution/graph.rs` (append tool impl)

**Step 1: Implement the Tool trait**

Add `ExecutionGraphTool` struct and implement the `Tool` trait with the JSON schema for all 4 actions.

```rust
use std::sync::Arc;
use async_trait::async_trait;
use zero_core::{Tool, ToolContext, Result, ZeroError};

pub struct ExecutionGraphTool;

impl ExecutionGraphTool {
    pub fn new() -> Self { Self }
}

impl Default for ExecutionGraphTool {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl Tool for ExecutionGraphTool {
    fn name(&self) -> &str { "execution_graph" }

    fn description(&self) -> &str {
        "Build and execute workflow DAGs for complex multi-step tasks. Create a graph of nodes (agent tasks) with dependencies, conditions, and result routing. Use with delegate_to_agent to dispatch ready nodes."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "execute_next", "status", "add_node"],
                    "description": "Action to perform on the execution graph"
                },
                "graph_id": {
                    "type": "string",
                    "description": "Graph ID (required for execute_next, status, add_node)"
                },
                "nodes": {
                    "type": "array",
                    "description": "Node definitions (for create action)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "agent": { "type": "string", "description": "Agent ID to delegate to" },
                            "task": { "type": "string", "description": "Task for the agent" },
                            "depends_on": { "type": "array", "items": { "type": "string" } },
                            "depend_mode": { "type": "string", "enum": ["all", "any_completed", "any_one"] },
                            "when": {
                                "type": "object",
                                "properties": {
                                    "ref": { "type": "string" },
                                    "operator": { "type": "string", "enum": ["contains", "not_contains", "equals", "gt", "lt", "regex", "llm_eval"] },
                                    "value": { "type": "string" },
                                    "field": { "type": "string", "default": "result" }
                                }
                            },
                            "inputs": { "type": "object" },
                            "retry": {
                                "type": "object",
                                "properties": {
                                    "max": { "type": "integer" },
                                    "on_fail": { "type": "string" }
                                }
                            },
                            "timeout_seconds": { "type": "integer" },
                            "on_timeout": { "type": "string", "enum": ["skip", "fail"] }
                        },
                        "required": ["id", "agent", "task"]
                    }
                },
                "completed": {
                    "type": "array",
                    "description": "Completed node results (for execute_next)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "result": { "type": "string" },
                            "error": { "type": "string" }
                        },
                        "required": ["id"]
                    }
                },
                "node": {
                    "type": "object",
                    "description": "Single node to add (for add_node action)"
                }
            },
            "required": ["action"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let action = args.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'action' parameter".to_string()))?;

        match action {
            "create" => self.handle_create(ctx, &args).await,
            "execute_next" => self.handle_execute_next(ctx, &args).await,
            "status" => self.handle_status(ctx, &args).await,
            "add_node" => self.handle_add_node(ctx, &args).await,
            _ => Err(ZeroError::Tool(format!("Unknown action: {}", action))),
        }
    }
}
```

**Step 2: Implement action handlers**

```rust
impl ExecutionGraphTool {
    async fn handle_create(&self, ctx: Arc<dyn ToolContext>, args: &Value) -> Result<Value> {
        let nodes_val = args.get("nodes")
            .ok_or_else(|| ZeroError::Tool("Missing 'nodes' array for create action".to_string()))?;

        let nodes: Vec<GraphNode> = serde_json::from_value(nodes_val.clone())
            .map_err(|e| ZeroError::Tool(format!("Invalid node definitions: {}", e)))?;

        if nodes.is_empty() {
            return Err(ZeroError::Tool("Nodes array cannot be empty".to_string()));
        }

        // Validate: no duplicate IDs
        let mut seen = std::collections::HashSet::new();
        for node in &nodes {
            if !seen.insert(&node.id) {
                return Err(ZeroError::Tool(format!("Duplicate node ID: {}", node.id)));
            }
        }

        // Validate: all depends_on references exist
        let node_ids: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        for node in &nodes {
            for dep in &node.depends_on {
                if !node_ids.contains(dep.as_str()) {
                    return Err(ZeroError::Tool(format!("Node '{}' depends on unknown node '{}'", node.id, dep)));
                }
            }
        }

        let graph_id = format!("graph-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0"));
        let mut graph = ExecutionGraph::new(graph_id.clone(), nodes);

        let ready = graph.find_ready_nodes();

        // Store in session state
        let graph_json = serde_json::to_value(&graph)
            .map_err(|e| ZeroError::Tool(format!("Failed to serialize graph: {}", e)))?;
        ctx.set_state(format!("app:graph:{}", graph_id), graph_json);

        Ok(json!({
            "graph_id": graph_id,
            "total_nodes": graph.nodes.len(),
            "ready_nodes": ready,
            "message": format!("{} node(s) ready to dispatch via delegate_to_agent", ready.len())
        }))
    }

    async fn handle_execute_next(&self, ctx: Arc<dyn ToolContext>, args: &Value) -> Result<Value> {
        let graph_id = args.get("graph_id").and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'graph_id' for execute_next".to_string()))?;

        let mut graph = self.load_graph(&ctx, graph_id)?;

        // Process completed nodes
        if let Some(completed) = args.get("completed").and_then(|v| v.as_array()) {
            for entry in completed {
                let id = entry.get("id").and_then(|v| v.as_str())
                    .ok_or_else(|| ZeroError::Tool("Completed entry missing 'id'".to_string()))?;

                if let Some(error) = entry.get("error").and_then(|v| v.as_str()) {
                    graph.fail_node(id, error.to_string());
                } else {
                    let result = entry.get("result").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    graph.complete_node(id, result);
                }
            }
        }

        let ready = graph.find_ready_nodes();

        // Save updated graph
        self.save_graph(&ctx, graph_id, &graph)?;

        if graph.status == GraphStatus::Completed || graph.status == GraphStatus::Failed {
            // Collect final results
            let results: HashMap<String, Option<String>> = graph.nodes.iter()
                .filter(|(_, n)| n.status == NodeStatus::Completed)
                .map(|(id, n)| (id.clone(), n.result.clone()))
                .collect();

            Ok(json!({
                "graph_id": graph_id,
                "status": graph.status,
                "ready_nodes": [],
                "results": results,
                "message": format!("Graph {}", if graph.status == GraphStatus::Completed { "completed" } else { "failed" })
            }))
        } else {
            Ok(json!({
                "graph_id": graph_id,
                "status": graph.status,
                "ready_nodes": ready,
                "message": format!("{} node(s) ready to dispatch", ready.len())
            }))
        }
    }

    async fn handle_status(&self, ctx: Arc<dyn ToolContext>, args: &Value) -> Result<Value> {
        let graph_id = args.get("graph_id").and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'graph_id' for status".to_string()))?;

        let graph = self.load_graph(&ctx, graph_id)?;

        let summary: Vec<Value> = graph.nodes.values().map(|n| {
            json!({ "id": n.id, "agent": n.agent, "status": n.status, "result_preview": n.result.as_ref().map(|r| &r[..r.len().min(100)]) })
        }).collect();

        let completed = graph.nodes.values().filter(|n| n.status == NodeStatus::Completed).count();
        let total = graph.nodes.len();

        Ok(json!({
            "graph_id": graph_id,
            "status": graph.status,
            "progress": format!("{}/{}", completed, total),
            "nodes": summary
        }))
    }

    async fn handle_add_node(&self, ctx: Arc<dyn ToolContext>, args: &Value) -> Result<Value> {
        let graph_id = args.get("graph_id").and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'graph_id' for add_node".to_string()))?;

        let node_val = args.get("node")
            .ok_or_else(|| ZeroError::Tool("Missing 'node' for add_node action".to_string()))?;

        let node: GraphNode = serde_json::from_value(node_val.clone())
            .map_err(|e| ZeroError::Tool(format!("Invalid node definition: {}", e)))?;

        let mut graph = self.load_graph(&ctx, graph_id)?;

        if !graph.add_node(node.clone()) {
            return Err(ZeroError::Tool(format!("Node '{}' already exists", node.id)));
        }

        self.save_graph(&ctx, graph_id, &graph)?;

        Ok(json!({
            "graph_id": graph_id,
            "added": node.id,
            "total_nodes": graph.nodes.len(),
            "message": format!("Node '{}' added to graph", node.id)
        }))
    }

    fn load_graph(&self, ctx: &Arc<dyn ToolContext>, graph_id: &str) -> Result<ExecutionGraph> {
        let graph_val = ctx.get_state(&format!("app:graph:{}", graph_id))
            .ok_or_else(|| ZeroError::Tool(format!("Graph '{}' not found", graph_id)))?;
        serde_json::from_value(graph_val)
            .map_err(|e| ZeroError::Tool(format!("Failed to deserialize graph: {}", e)))
    }

    fn save_graph(&self, ctx: &Arc<dyn ToolContext>, graph_id: &str, graph: &ExecutionGraph) -> Result<()> {
        let graph_json = serde_json::to_value(graph)
            .map_err(|e| ZeroError::Tool(format!("Failed to serialize graph: {}", e)))?;
        ctx.set_state(format!("app:graph:{}", graph_id), graph_json);
        Ok(())
    }
}
```

**Step 3: Add tests for tool actions**

```rust
// In #[cfg(test)] mod tests, add:
#[test]
fn test_execution_graph_tool_schema() {
    let tool = ExecutionGraphTool::new();
    assert_eq!(tool.name(), "execution_graph");
    let schema = tool.parameters_schema().unwrap();
    let props = schema.get("properties").unwrap();
    assert!(props.get("action").is_some());
    assert!(props.get("nodes").is_some());
}
```

**Step 4: Verify it compiles**

Run: `cargo check -p agent-tools`

**Step 5: Commit**

```bash
git add runtime/agent-tools/src/tools/execution/graph.rs
git commit -m "feat(tools): implement ExecutionGraphTool with create/execute_next/status/add_node"
```

---

### Task 3: Register the tool in core tools

**Files:**
- Modify: `runtime/agent-tools/src/tools/execution/mod.rs`
- Modify: `runtime/agent-tools/src/tools/mod.rs`

**Step 1: Add module declaration and re-export in execution/mod.rs**

Add `pub mod graph;` and `pub use graph::ExecutionGraphTool;` to `execution/mod.rs`.

**Step 2: Add re-export and registration in tools/mod.rs**

Add `pub use execution::ExecutionGraphTool;` to the re-exports.
Add `Arc::new(ExecutionGraphTool::new())` to the `core_tools()` function, after the `UpdatePlanTool`.

**Step 3: Verify it compiles**

Run: `cargo check -p agent-tools`

**Step 4: Run all tests**

Run: `cargo test -p agent-tools`

**Step 5: Commit**

```bash
git add runtime/agent-tools/src/tools/execution/mod.rs runtime/agent-tools/src/tools/mod.rs
git commit -m "feat(tools): register ExecutionGraphTool as core tool"
```

---

### Task 4: Add regex dependency if needed

**Files:**
- Check: `runtime/agent-tools/Cargo.toml`

**Step 1: Check if regex is already a dependency**

The condition evaluator uses `regex::Regex`. Check if `regex` is in agent-tools' Cargo.toml. If not, add it.

Run: `grep regex runtime/agent-tools/Cargo.toml`

**Step 2: Add if missing**

Add `regex = "1"` to `[dependencies]` in `runtime/agent-tools/Cargo.toml`.

**Step 3: Verify**

Run: `cargo check -p agent-tools`

**Step 4: Commit (if changed)**

```bash
git add runtime/agent-tools/Cargo.toml
git commit -m "chore: add regex dependency to agent-tools"
```

---

### Task 5: Update system prompt shard

**Files:**
- Modify: `gateway/templates/shards/tooling_skills.md`

**Step 1: Read the current shard**

Read `gateway/templates/shards/tooling_skills.md` to understand the existing format.

**Step 2: Add execution_graph documentation**

Add a section documenting the `execution_graph` tool after the `update_plan` section. Include:
- When to use it (complex multi-step tasks needing conditional branching)
- Actions: create, execute_next, status, add_node
- Workflow pattern: create graph → delegate ready nodes → on continuation, call execute_next with results → repeat
- Example graph for deep research

**Step 3: Commit**

```bash
git add gateway/templates/shards/tooling_skills.md
git commit -m "docs: add execution_graph tool documentation to system prompt shard"
```

---

### Task 6: Full integration test — cargo check + cargo test

**Step 1: Full workspace check**

Run: `cargo check --workspace`

**Step 2: Run agent-tools tests**

Run: `cargo test -p agent-tools -- --nocapture`

**Step 3: Fix any issues**

**Step 4: Final commit if fixes needed**
