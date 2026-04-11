//! # Task Graph
//!
//! Directed acyclic graph (DAG) for planning multi-step agent execution.
//!
//! ## Overview
//!
//! The TaskGraph represents a plan of tasks with dependencies:
//! - Tasks can depend on other tasks
//! - Execution respects dependency order
//! - Parallel execution where dependencies allow
//!
//! ## Example
//!
//! ```rust
//! use zero_agent::orchestrator::task_graph::{TaskGraph, TaskNode, TaskStatus};
//!
//! let mut graph = TaskGraph::new("analyze-codebase");
//!
//! // Add tasks
//! let scan = graph.add_task(TaskNode::new("scan", "Scan source files"));
//! let analyze = graph.add_task(TaskNode::new("analyze", "Analyze patterns"));
//! let report = graph.add_task(TaskNode::new("report", "Generate report"));
//!
//! // Add dependencies: analyze depends on scan, report depends on analyze
//! graph.add_dependency(&analyze, &scan);
//! graph.add_dependency(&report, &analyze);
//!
//! // Get execution order
//! let order = graph.execution_order().unwrap();
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

// ============================================================================
// TASK STATUS
// ============================================================================

/// Status of a task in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is waiting to be executed
    #[default]
    Pending,

    /// Task is ready to execute (all dependencies met)
    Ready,

    /// Task is currently executing
    Running,

    /// Task completed successfully
    Completed,

    /// Task failed
    Failed,

    /// Task was skipped (e.g., due to upstream failure)
    Skipped,

    /// Task was cancelled
    Cancelled,
}

impl TaskStatus {
    /// Check if this status represents a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed
                | TaskStatus::Failed
                | TaskStatus::Skipped
                | TaskStatus::Cancelled
        )
    }

    /// Check if this status allows dependent tasks to proceed.
    pub fn allows_dependents(&self) -> bool {
        matches!(self, TaskStatus::Completed)
    }
}

// ============================================================================
// TASK NODE
// ============================================================================

/// A task in the execution graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    /// Unique task identifier
    pub id: String,

    /// Human-readable description
    pub description: String,

    /// Required capability for this task
    #[serde(default)]
    pub required_capability: Option<String>,

    /// Agent assigned to this task
    #[serde(default)]
    pub assigned_agent: Option<String>,

    /// Current status
    #[serde(default)]
    pub status: TaskStatus,

    /// Input data for this task
    #[serde(default)]
    pub input: Option<serde_json::Value>,

    /// Output data from this task
    #[serde(default)]
    pub output: Option<serde_json::Value>,

    /// Error message if failed
    #[serde(default)]
    pub error: Option<String>,

    /// When the task started
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,

    /// When the task completed
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,

    /// Retry count
    #[serde(default)]
    pub retry_count: u32,

    /// Maximum retries allowed
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Priority (higher = execute first among ready tasks)
    #[serde(default)]
    pub priority: i32,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

fn default_max_retries() -> u32 {
    3
}

impl TaskNode {
    /// Create a new task node.
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            required_capability: None,
            assigned_agent: None,
            status: TaskStatus::Pending,
            input: None,
            output: None,
            error: None,
            started_at: None,
            completed_at: None,
            retry_count: 0,
            max_retries: 3,
            priority: 0,
            metadata: HashMap::new(),
        }
    }

    /// Set the required capability.
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.required_capability = Some(capability.into());
        self
    }

    /// Set the input data.
    pub fn with_input(mut self, input: serde_json::Value) -> Self {
        self.input = Some(input);
        self
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set max retries.
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    /// Check if the task can be retried.
    pub fn can_retry(&self) -> bool {
        self.status == TaskStatus::Failed && self.retry_count < self.max_retries
    }

    /// Mark as started.
    pub fn start(&mut self) {
        self.status = TaskStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Mark as completed with output.
    pub fn complete(&mut self, output: serde_json::Value) {
        self.status = TaskStatus::Completed;
        self.output = Some(output);
        self.completed_at = Some(Utc::now());
    }

    /// Mark as failed with error.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = TaskStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(Utc::now());
    }

    /// Reset for retry.
    pub fn reset_for_retry(&mut self) {
        self.status = TaskStatus::Pending;
        self.error = None;
        self.started_at = None;
        self.completed_at = None;
        self.retry_count += 1;
    }

    /// Execution duration in milliseconds.
    pub fn duration_ms(&self) -> Option<i64> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some((end - start).num_milliseconds()),
            _ => None,
        }
    }
}

// ============================================================================
// TASK GRAPH
// ============================================================================

/// Directed acyclic graph of tasks for execution planning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    /// Graph identifier
    pub id: String,

    /// Human-readable name
    #[serde(default)]
    pub name: String,

    /// Tasks indexed by ID
    pub tasks: HashMap<String, TaskNode>,

    /// Dependencies: task_id -> set of dependency task_ids
    pub dependencies: HashMap<String, HashSet<String>>,

    /// Reverse dependencies: task_id -> set of dependent task_ids
    #[serde(default)]
    pub dependents: HashMap<String, HashSet<String>>,

    /// When the graph was created
    pub created_at: DateTime<Utc>,

    /// When the graph was last updated
    pub updated_at: DateTime<Utc>,
}

impl TaskGraph {
    /// Create a new empty task graph.
    pub fn new(id: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: String::new(),
            tasks: HashMap::new(),
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Set the graph name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add a task to the graph.
    ///
    /// Returns the task ID for use in dependency creation.
    pub fn add_task(&mut self, task: TaskNode) -> String {
        let id = task.id.clone();
        self.tasks.insert(id.clone(), task);
        self.dependencies.entry(id.clone()).or_default();
        self.dependents.entry(id.clone()).or_default();
        self.updated_at = Utc::now();
        id
    }

    /// Add a dependency: `dependent` depends on `dependency`.
    ///
    /// Returns an error if this would create a cycle.
    pub fn add_dependency(
        &mut self,
        dependent: impl AsRef<str>,
        dependency: impl AsRef<str>,
    ) -> Result<(), TaskGraphError> {
        let dependent = dependent.as_ref().to_string();
        let dependency = dependency.as_ref().to_string();

        // Verify both tasks exist
        if !self.tasks.contains_key(&dependent) {
            return Err(TaskGraphError::TaskNotFound(dependent));
        }
        if !self.tasks.contains_key(&dependency) {
            return Err(TaskGraphError::TaskNotFound(dependency));
        }

        // Check for cycle
        if self.would_create_cycle(&dependent, &dependency) {
            return Err(TaskGraphError::CycleDetected {
                from: dependent,
                to: dependency,
            });
        }

        // Add dependency
        self.dependencies
            .entry(dependent.clone())
            .or_default()
            .insert(dependency.clone());

        // Add reverse dependency
        self.dependents
            .entry(dependency)
            .or_default()
            .insert(dependent);

        self.updated_at = Utc::now();
        Ok(())
    }

    /// Check if adding a dependency would create a cycle.
    fn would_create_cycle(&self, dependent: &str, dependency: &str) -> bool {
        // If dependency depends on dependent (directly or transitively), adding this edge creates a cycle
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(dependency.to_string());

        while let Some(current) = queue.pop_front() {
            if current == dependent {
                return true;
            }
            if visited.insert(current.clone()) {
                if let Some(deps) = self.dependencies.get(&current) {
                    for dep in deps {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }

        false
    }

    /// Get a task by ID.
    pub fn get_task(&self, id: &str) -> Option<&TaskNode> {
        self.tasks.get(id)
    }

    /// Get a mutable task by ID.
    pub fn get_task_mut(&mut self, id: &str) -> Option<&mut TaskNode> {
        self.tasks.get_mut(id)
    }

    /// Get dependencies of a task.
    pub fn get_dependencies(&self, task_id: &str) -> Vec<&str> {
        self.dependencies
            .get(task_id)
            .map(|deps| deps.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get tasks that depend on a task.
    pub fn get_dependents(&self, task_id: &str) -> Vec<&str> {
        self.dependents
            .get(task_id)
            .map(|deps| deps.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get tasks that are ready to execute (all dependencies completed).
    pub fn ready_tasks(&self) -> Vec<&TaskNode> {
        self.tasks
            .values()
            .filter(|task| {
                if task.status != TaskStatus::Pending {
                    return false;
                }

                // Check all dependencies are completed
                self.dependencies
                    .get(&task.id)
                    .map(|deps| {
                        deps.iter().all(|dep_id| {
                            self.tasks
                                .get(dep_id)
                                .map(|t| t.status.allows_dependents())
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(true)
            })
            .collect()
    }

    /// Get execution order via topological sort.
    ///
    /// Returns an error if the graph has cycles (should not happen if add_dependency is used).
    pub fn execution_order(&self) -> Result<Vec<&TaskNode>, TaskGraphError> {
        let mut result = Vec::new();
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut queue = VecDeque::new();

        // Calculate in-degrees
        for task_id in self.tasks.keys() {
            let degree = self.dependencies.get(task_id).map(|d| d.len()).unwrap_or(0);
            in_degree.insert(task_id.as_str(), degree);
            if degree == 0 {
                queue.push_back(task_id.as_str());
            }
        }

        // Process tasks with no dependencies first
        while let Some(task_id) = queue.pop_front() {
            if let Some(task) = self.tasks.get(task_id) {
                result.push(task);
            }

            // Reduce in-degree of dependents
            if let Some(dependents) = self.dependents.get(task_id) {
                for dependent in dependents {
                    if let Some(degree) = in_degree.get_mut(dependent.as_str()) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent.as_str());
                        }
                    }
                }
            }
        }

        if result.len() != self.tasks.len() {
            return Err(TaskGraphError::CycleDetected {
                from: "unknown".into(),
                to: "unknown".into(),
            });
        }

        Ok(result)
    }

    /// Get parallel execution groups.
    ///
    /// Returns groups of tasks that can be executed in parallel.
    /// Each group's tasks have all their dependencies in previous groups.
    pub fn parallel_groups(&self) -> Result<Vec<Vec<&TaskNode>>, TaskGraphError> {
        let mut groups = Vec::new();
        let mut completed: HashSet<&str> = HashSet::new();

        while completed.len() < self.tasks.len() {
            let mut group = Vec::new();

            for (task_id, task) in &self.tasks {
                if completed.contains(task_id.as_str()) {
                    continue;
                }

                // Check if all dependencies are in completed set
                let deps_met = self
                    .dependencies
                    .get(task_id)
                    .map(|deps| deps.iter().all(|d| completed.contains(d.as_str())))
                    .unwrap_or(true);

                if deps_met {
                    group.push(task);
                }
            }

            if group.is_empty() {
                // Cycle detected - some tasks have unmet dependencies but none are ready
                return Err(TaskGraphError::CycleDetected {
                    from: "unknown".into(),
                    to: "unknown".into(),
                });
            }

            // Add group's tasks to completed set
            for task in &group {
                completed.insert(&task.id);
            }

            groups.push(group);
        }

        Ok(groups)
    }

    /// Check if the graph is complete (all tasks terminal).
    pub fn is_complete(&self) -> bool {
        self.tasks.values().all(|t| t.status.is_terminal())
    }

    /// Check if any task has failed.
    pub fn has_failures(&self) -> bool {
        self.tasks.values().any(|t| t.status == TaskStatus::Failed)
    }

    /// Get summary of task statuses.
    pub fn status_summary(&self) -> HashMap<TaskStatus, usize> {
        let mut summary = HashMap::new();
        for task in self.tasks.values() {
            *summary.entry(task.status).or_insert(0) += 1;
        }
        summary
    }

    /// Number of tasks in the graph.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Check if the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

// ============================================================================
// ERRORS
// ============================================================================

/// Errors that can occur with task graphs.
#[derive(Debug, Clone, thiserror::Error)]
pub enum TaskGraphError {
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Cycle detected: {from} -> {to}")]
    CycleDetected { from: String, to: String },

    #[error("Invalid state transition: {from:?} -> {to:?}")]
    InvalidStateTransition { from: TaskStatus, to: TaskStatus },
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_node_lifecycle() {
        let mut task = TaskNode::new("test", "Test task");
        assert_eq!(task.status, TaskStatus::Pending);

        task.start();
        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());

        task.complete(serde_json::json!({"result": "success"}));
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.completed_at.is_some());
        assert!(task.duration_ms().is_some());
    }

    #[test]
    fn test_task_retry() {
        let mut task = TaskNode::new("test", "Test task").with_max_retries(2);

        task.start();
        task.fail("Error 1");
        assert!(task.can_retry());

        task.reset_for_retry();
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.retry_count, 1);

        task.start();
        task.fail("Error 2");
        assert!(task.can_retry());

        task.reset_for_retry();
        task.start();
        task.fail("Error 3");
        assert!(!task.can_retry()); // Max retries reached
    }

    #[test]
    fn test_graph_creation() {
        let mut graph = TaskGraph::new("test-graph");

        let t1 = graph.add_task(TaskNode::new("t1", "Task 1"));
        let t2 = graph.add_task(TaskNode::new("t2", "Task 2"));
        let t3 = graph.add_task(TaskNode::new("t3", "Task 3"));

        assert_eq!(graph.len(), 3);

        // t2 depends on t1
        graph.add_dependency(&t2, &t1).unwrap();
        // t3 depends on t2
        graph.add_dependency(&t3, &t2).unwrap();

        assert_eq!(graph.get_dependencies(&t2), vec!["t1"]);
        assert_eq!(graph.get_dependents(&t1), vec!["t2"]);
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = TaskGraph::new("test");

        let t1 = graph.add_task(TaskNode::new("t1", "Task 1"));
        let t2 = graph.add_task(TaskNode::new("t2", "Task 2"));
        let t3 = graph.add_task(TaskNode::new("t3", "Task 3"));

        graph.add_dependency(&t2, &t1).unwrap();
        graph.add_dependency(&t3, &t2).unwrap();

        // This would create a cycle: t1 -> t2 -> t3 -> t1
        let result = graph.add_dependency(&t1, &t3);
        assert!(result.is_err());
    }

    #[test]
    fn test_execution_order() {
        let mut graph = TaskGraph::new("test");

        let t1 = graph.add_task(TaskNode::new("t1", "Task 1"));
        let t2 = graph.add_task(TaskNode::new("t2", "Task 2"));
        let t3 = graph.add_task(TaskNode::new("t3", "Task 3"));

        graph.add_dependency(&t2, &t1).unwrap();
        graph.add_dependency(&t3, &t2).unwrap();

        let order = graph.execution_order().unwrap();
        let ids: Vec<_> = order.iter().map(|t| t.id.as_str()).collect();

        // t1 must come before t2, t2 must come before t3
        let pos1 = ids.iter().position(|&id| id == "t1").unwrap();
        let pos2 = ids.iter().position(|&id| id == "t2").unwrap();
        let pos3 = ids.iter().position(|&id| id == "t3").unwrap();

        assert!(pos1 < pos2);
        assert!(pos2 < pos3);
    }

    #[test]
    fn test_parallel_groups() {
        let mut graph = TaskGraph::new("test");

        // Create a diamond pattern: t1 -> (t2, t3) -> t4
        let t1 = graph.add_task(TaskNode::new("t1", "Task 1"));
        let t2 = graph.add_task(TaskNode::new("t2", "Task 2"));
        let t3 = graph.add_task(TaskNode::new("t3", "Task 3"));
        let t4 = graph.add_task(TaskNode::new("t4", "Task 4"));

        graph.add_dependency(&t2, &t1).unwrap();
        graph.add_dependency(&t3, &t1).unwrap();
        graph.add_dependency(&t4, &t2).unwrap();
        graph.add_dependency(&t4, &t3).unwrap();

        let groups = graph.parallel_groups().unwrap();

        // Group 0: t1
        // Group 1: t2, t3 (can run in parallel)
        // Group 2: t4
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].len(), 1);
        assert_eq!(groups[1].len(), 2);
        assert_eq!(groups[2].len(), 1);
    }

    #[test]
    fn test_ready_tasks() {
        let mut graph = TaskGraph::new("test");

        let t1 = graph.add_task(TaskNode::new("t1", "Task 1"));
        let t2 = graph.add_task(TaskNode::new("t2", "Task 2"));

        graph.add_dependency(&t2, &t1).unwrap();

        // Initially only t1 is ready
        let ready = graph.ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "t1");

        // Complete t1
        graph
            .get_task_mut(&t1)
            .unwrap()
            .complete(serde_json::json!({}));

        // Now t2 is ready
        let ready = graph.ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "t2");
    }
}
