//! # Execution Trace
//!
//! Tracing and observability for orchestrator execution.
//!
//! ## Overview
//!
//! ExecutionTrace captures the complete execution history of an orchestration,
//! including:
//! - Decisions made by the orchestrator
//! - Task assignments and results
//! - Timing information
//! - Errors and retries
//!
//! ## Example
//!
//! ```rust
//! use zero_agent::orchestrator::trace::{ExecutionTrace, TraceEvent, TraceEventKind};
//!
//! let mut trace = ExecutionTrace::new("execution-123");
//!
//! trace.record(TraceEvent::new(
//!     TraceEventKind::PlanCreated,
//!     "Created execution plan with 3 tasks",
//! ));
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// TRACE EVENT KIND
// ============================================================================

/// Kind of trace event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceEventKind {
    // Lifecycle events
    ExecutionStarted,
    ExecutionCompleted,
    ExecutionFailed,
    ExecutionCancelled,

    // Planning events
    PlanCreated,
    PlanUpdated,
    TaskAdded,
    DependencyAdded,

    // Routing events
    CapabilityQueried,
    AgentSelected,
    AgentUnavailable,
    RoutingFailed,

    // Execution events
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    TaskRetrying,
    TaskSkipped,
    TaskCancelled,

    // Communication events
    AgentInvoked,
    AgentResponded,
    ToolCalled,
    ToolReturned,

    // Error events
    ErrorOccurred,
    ErrorRecovered,

    // User events
    UserInteraction,
    ApprovalRequested,
    ApprovalReceived,

    // Custom events
    Custom,
}

impl TraceEventKind {
    /// Check if this is an error event.
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            TraceEventKind::ExecutionFailed
                | TraceEventKind::TaskFailed
                | TraceEventKind::RoutingFailed
                | TraceEventKind::ErrorOccurred
        )
    }

    /// Check if this is a lifecycle event.
    pub fn is_lifecycle(&self) -> bool {
        matches!(
            self,
            TraceEventKind::ExecutionStarted
                | TraceEventKind::ExecutionCompleted
                | TraceEventKind::ExecutionFailed
                | TraceEventKind::ExecutionCancelled
        )
    }
}

// ============================================================================
// TRACE EVENT
// ============================================================================

/// A single event in the execution trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// Unique event ID
    pub id: String,

    /// Event kind
    pub kind: TraceEventKind,

    /// Human-readable message
    pub message: String,

    /// When this event occurred
    pub timestamp: DateTime<Utc>,

    /// Associated task ID (if applicable)
    #[serde(default)]
    pub task_id: Option<String>,

    /// Associated agent ID (if applicable)
    #[serde(default)]
    pub agent_id: Option<String>,

    /// Duration in milliseconds (for timed events)
    #[serde(default)]
    pub duration_ms: Option<i64>,

    /// Additional data
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,

    /// Parent event ID (for nested events)
    #[serde(default)]
    pub parent_id: Option<String>,

    /// Span ID for distributed tracing
    #[serde(default)]
    pub span_id: Option<String>,
}

impl TraceEvent {
    /// Create a new trace event.
    pub fn new(kind: TraceEventKind, message: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            message: message.into(),
            timestamp: Utc::now(),
            task_id: None,
            agent_id: None,
            duration_ms: None,
            data: HashMap::new(),
            parent_id: None,
            span_id: None,
        }
    }

    /// Set the task ID.
    pub fn with_task(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    /// Set the agent ID.
    pub fn with_agent(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Set the duration.
    pub fn with_duration(mut self, duration_ms: i64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Add data field.
    pub fn with_data(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    /// Set parent event ID.
    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    /// Set span ID.
    pub fn with_span(mut self, span_id: impl Into<String>) -> Self {
        self.span_id = Some(span_id.into());
        self
    }
}

// ============================================================================
// EXECUTION TRACE
// ============================================================================

/// Complete trace of an orchestration execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Trace identifier (usually matches execution ID)
    pub id: String,

    /// All events in chronological order
    pub events: Vec<TraceEvent>,

    /// When the trace started
    pub started_at: DateTime<Utc>,

    /// When the trace ended
    #[serde(default)]
    pub ended_at: Option<DateTime<Utc>>,

    /// Overall outcome
    #[serde(default)]
    pub outcome: TraceOutcome,

    /// Summary metrics
    #[serde(default)]
    pub metrics: TraceMetrics,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExecutionTrace {
    /// Create a new execution trace.
    pub fn new(id: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            events: Vec::new(),
            started_at: now,
            ended_at: None,
            outcome: TraceOutcome::InProgress,
            metrics: TraceMetrics::default(),
            metadata: HashMap::new(),
        }
    }

    /// Record an event.
    pub fn record(&mut self, event: TraceEvent) {
        // Update metrics based on event kind
        self.metrics.total_events += 1;

        match event.kind {
            TraceEventKind::TaskStarted => self.metrics.tasks_started += 1,
            TraceEventKind::TaskCompleted => self.metrics.tasks_completed += 1,
            TraceEventKind::TaskFailed => self.metrics.tasks_failed += 1,
            TraceEventKind::TaskRetrying => self.metrics.retries += 1,
            TraceEventKind::AgentInvoked => self.metrics.agent_invocations += 1,
            TraceEventKind::ToolCalled => self.metrics.tool_calls += 1,
            TraceEventKind::ErrorOccurred => self.metrics.errors += 1,
            _ => {}
        }

        // Track duration
        if let Some(duration) = event.duration_ms {
            self.metrics.total_duration_ms += duration;
        }

        self.events.push(event);
    }

    /// Record execution start.
    pub fn start(&mut self) {
        self.record(TraceEvent::new(
            TraceEventKind::ExecutionStarted,
            "Execution started",
        ));
    }

    /// Record successful completion.
    pub fn complete(&mut self, message: impl Into<String>) {
        self.ended_at = Some(Utc::now());
        self.outcome = TraceOutcome::Success;
        self.metrics.total_duration_ms =
            (self.ended_at.unwrap() - self.started_at).num_milliseconds();
        self.record(TraceEvent::new(TraceEventKind::ExecutionCompleted, message));
    }

    /// Record failure.
    pub fn fail(&mut self, message: impl Into<String>) {
        self.ended_at = Some(Utc::now());
        self.outcome = TraceOutcome::Failure;
        self.metrics.total_duration_ms =
            (self.ended_at.unwrap() - self.started_at).num_milliseconds();
        self.record(TraceEvent::new(TraceEventKind::ExecutionFailed, message));
    }

    /// Record cancellation.
    pub fn cancel(&mut self, message: impl Into<String>) {
        self.ended_at = Some(Utc::now());
        self.outcome = TraceOutcome::Cancelled;
        self.metrics.total_duration_ms =
            (self.ended_at.unwrap() - self.started_at).num_milliseconds();
        self.record(TraceEvent::new(TraceEventKind::ExecutionCancelled, message));
    }

    /// Get events for a specific task.
    pub fn events_for_task(&self, task_id: &str) -> Vec<&TraceEvent> {
        self.events
            .iter()
            .filter(|e| e.task_id.as_deref() == Some(task_id))
            .collect()
    }

    /// Get events for a specific agent.
    pub fn events_for_agent(&self, agent_id: &str) -> Vec<&TraceEvent> {
        self.events
            .iter()
            .filter(|e| e.agent_id.as_deref() == Some(agent_id))
            .collect()
    }

    /// Get error events.
    pub fn errors(&self) -> Vec<&TraceEvent> {
        self.events.iter().filter(|e| e.kind.is_error()).collect()
    }

    /// Get timeline as (timestamp, event) pairs.
    pub fn timeline(&self) -> Vec<(DateTime<Utc>, &TraceEvent)> {
        self.events.iter().map(|e| (e.timestamp, e)).collect()
    }

    /// Check if execution is still in progress.
    pub fn is_in_progress(&self) -> bool {
        self.outcome == TraceOutcome::InProgress
    }

    /// Duration in milliseconds.
    pub fn duration_ms(&self) -> i64 {
        match self.ended_at {
            Some(end) => (end - self.started_at).num_milliseconds(),
            None => (Utc::now() - self.started_at).num_milliseconds(),
        }
    }
}

// ============================================================================
// TRACE OUTCOME
// ============================================================================

/// Overall outcome of the execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TraceOutcome {
    /// Execution is still in progress
    #[default]
    InProgress,

    /// Execution completed successfully
    Success,

    /// Execution failed
    Failure,

    /// Execution was cancelled
    Cancelled,

    /// Execution completed with partial success
    PartialSuccess,
}

// ============================================================================
// TRACE METRICS
// ============================================================================

/// Summary metrics for an execution trace.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceMetrics {
    /// Total number of events
    pub total_events: usize,

    /// Number of tasks started
    pub tasks_started: usize,

    /// Number of tasks completed successfully
    pub tasks_completed: usize,

    /// Number of tasks that failed
    pub tasks_failed: usize,

    /// Number of agent invocations
    pub agent_invocations: usize,

    /// Number of tool calls
    pub tool_calls: usize,

    /// Number of retries
    pub retries: usize,

    /// Number of errors encountered
    pub errors: usize,

    /// Total duration in milliseconds
    pub total_duration_ms: i64,
}

impl TraceMetrics {
    /// Success rate (0.0 - 1.0).
    pub fn success_rate(&self) -> f64 {
        let total = self.tasks_completed + self.tasks_failed;
        if total == 0 {
            1.0
        } else {
            self.tasks_completed as f64 / total as f64
        }
    }
}

// ============================================================================
// TRACE BUILDER
// ============================================================================

/// Builder for creating trace events with a fluent API.
pub struct TraceBuilder {
    trace: ExecutionTrace,
    current_span: Option<String>,
}

impl TraceBuilder {
    /// Create a new trace builder.
    pub fn new(id: impl Into<String>) -> Self {
        let mut trace = ExecutionTrace::new(id);
        trace.start();
        Self {
            trace,
            current_span: None,
        }
    }

    /// Start a new span for grouping events.
    pub fn begin_span(&mut self, name: impl Into<String>) -> String {
        let span_id = format!("span-{}", uuid::Uuid::new_v4());
        self.current_span = Some(span_id.clone());
        self.trace.record(
            TraceEvent::new(TraceEventKind::Custom, format!("Begin: {}", name.into()))
                .with_span(span_id.clone()),
        );
        span_id
    }

    /// End the current span.
    pub fn end_span(&mut self, name: impl Into<String>) {
        if let Some(span_id) = self.current_span.take() {
            self.trace.record(
                TraceEvent::new(TraceEventKind::Custom, format!("End: {}", name.into()))
                    .with_span(span_id),
            );
        }
    }

    /// Record a task start.
    pub fn task_started(&mut self, task_id: impl Into<String>, message: impl Into<String>) {
        let mut event = TraceEvent::new(TraceEventKind::TaskStarted, message).with_task(task_id);
        if let Some(ref span) = self.current_span {
            event = event.with_span(span.clone());
        }
        self.trace.record(event);
    }

    /// Record a task completion.
    pub fn task_completed(
        &mut self,
        task_id: impl Into<String>,
        message: impl Into<String>,
        duration_ms: i64,
    ) {
        let mut event = TraceEvent::new(TraceEventKind::TaskCompleted, message)
            .with_task(task_id)
            .with_duration(duration_ms);
        if let Some(ref span) = self.current_span {
            event = event.with_span(span.clone());
        }
        self.trace.record(event);
    }

    /// Record a task failure.
    pub fn task_failed(&mut self, task_id: impl Into<String>, message: impl Into<String>) {
        let mut event = TraceEvent::new(TraceEventKind::TaskFailed, message).with_task(task_id);
        if let Some(ref span) = self.current_span {
            event = event.with_span(span.clone());
        }
        self.trace.record(event);
    }

    /// Record an agent selection.
    pub fn agent_selected(&mut self, agent_id: impl Into<String>, reason: impl Into<String>) {
        let mut event = TraceEvent::new(TraceEventKind::AgentSelected, reason).with_agent(agent_id);
        if let Some(ref span) = self.current_span {
            event = event.with_span(span.clone());
        }
        self.trace.record(event);
    }

    /// Record an error.
    pub fn error(&mut self, message: impl Into<String>) {
        let mut event = TraceEvent::new(TraceEventKind::ErrorOccurred, message);
        if let Some(ref span) = self.current_span {
            event = event.with_span(span.clone());
        }
        self.trace.record(event);
    }

    /// Complete the trace successfully.
    pub fn complete(mut self, message: impl Into<String>) -> ExecutionTrace {
        self.trace.complete(message);
        self.trace
    }

    /// Complete the trace with failure.
    pub fn fail(mut self, message: impl Into<String>) -> ExecutionTrace {
        self.trace.fail(message);
        self.trace
    }

    /// Get the trace (for inspection without consuming).
    pub fn trace(&self) -> &ExecutionTrace {
        &self.trace
    }

    /// Get mutable trace reference.
    pub fn trace_mut(&mut self) -> &mut ExecutionTrace {
        &mut self.trace
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_event_creation() {
        let event = TraceEvent::new(TraceEventKind::TaskStarted, "Starting task")
            .with_task("task-1")
            .with_agent("agent-1")
            .with_data("input", serde_json::json!({"query": "test"}));

        assert_eq!(event.kind, TraceEventKind::TaskStarted);
        assert_eq!(event.task_id, Some("task-1".to_string()));
        assert!(event.data.contains_key("input"));
    }

    #[test]
    fn test_execution_trace() {
        let mut trace = ExecutionTrace::new("exec-1");
        trace.start();

        trace
            .record(TraceEvent::new(TraceEventKind::TaskStarted, "Task 1 started").with_task("t1"));
        trace.record(TraceEvent::new(TraceEventKind::TaskCompleted, "Task 1 done").with_task("t1"));

        assert_eq!(trace.events.len(), 3); // start + 2 task events
        assert_eq!(trace.metrics.tasks_started, 1);
        assert_eq!(trace.metrics.tasks_completed, 1);

        trace.complete("All done");
        assert_eq!(trace.outcome, TraceOutcome::Success);
    }

    #[test]
    fn test_trace_builder() {
        let mut builder = TraceBuilder::new("exec-1");

        builder.begin_span("phase1");
        builder.task_started("t1", "Starting task 1");
        builder.task_completed("t1", "Task 1 done", 100);
        builder.end_span("phase1");

        let trace = builder.complete("Execution complete");

        assert_eq!(trace.outcome, TraceOutcome::Success);
        assert!(trace.metrics.tasks_completed > 0);
    }

    #[test]
    fn test_trace_filtering() {
        let mut trace = ExecutionTrace::new("exec-1");
        trace.start();

        trace.record(TraceEvent::new(TraceEventKind::TaskStarted, "T1").with_task("t1"));
        trace.record(TraceEvent::new(TraceEventKind::TaskStarted, "T2").with_task("t2"));
        trace.record(TraceEvent::new(TraceEventKind::TaskCompleted, "T1 done").with_task("t1"));
        trace.record(TraceEvent::new(TraceEventKind::TaskFailed, "T2 failed").with_task("t2"));

        let t1_events = trace.events_for_task("t1");
        assert_eq!(t1_events.len(), 2);

        let errors = trace.errors();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_trace_metrics() {
        let mut trace = ExecutionTrace::new("exec-1");
        trace.start();

        for i in 0..5 {
            trace.record(TraceEvent::new(
                TraceEventKind::TaskStarted,
                format!("Task {}", i),
            ));
            if i < 4 {
                trace.record(TraceEvent::new(
                    TraceEventKind::TaskCompleted,
                    format!("Task {} done", i),
                ));
            } else {
                trace.record(TraceEvent::new(
                    TraceEventKind::TaskFailed,
                    format!("Task {} failed", i),
                ));
            }
        }

        assert_eq!(trace.metrics.tasks_started, 5);
        assert_eq!(trace.metrics.tasks_completed, 4);
        assert_eq!(trace.metrics.tasks_failed, 1);
        assert!((trace.metrics.success_rate() - 0.8).abs() < 0.01);
    }

    // ============================================================================
    // ADDITIONAL TESTS
    // ============================================================================

    #[test]
    fn test_trace_event_kind_predicates() {
        assert!(TraceEventKind::TaskFailed.is_error());
        assert!(TraceEventKind::ExecutionFailed.is_error());
        assert!(TraceEventKind::RoutingFailed.is_error());
        assert!(TraceEventKind::ErrorOccurred.is_error());
        assert!(!TraceEventKind::TaskStarted.is_error());

        assert!(TraceEventKind::ExecutionStarted.is_lifecycle());
        assert!(TraceEventKind::ExecutionCompleted.is_lifecycle());
        assert!(TraceEventKind::ExecutionFailed.is_lifecycle());
        assert!(TraceEventKind::ExecutionCancelled.is_lifecycle());
        assert!(!TraceEventKind::TaskStarted.is_lifecycle());
    }

    #[test]
    fn test_trace_event_with_helpers() {
        let event = TraceEvent::new(TraceEventKind::AgentInvoked, "msg")
            .with_task("t-1")
            .with_agent("a-1")
            .with_duration(123)
            .with_data("k", serde_json::json!("v"))
            .with_parent("p-1")
            .with_span("s-1");
        assert_eq!(event.task_id.as_deref(), Some("t-1"));
        assert_eq!(event.agent_id.as_deref(), Some("a-1"));
        assert_eq!(event.duration_ms, Some(123));
        assert_eq!(event.parent_id.as_deref(), Some("p-1"));
        assert_eq!(event.span_id.as_deref(), Some("s-1"));
    }

    #[test]
    fn test_execution_trace_cancel() {
        let mut trace = ExecutionTrace::new("exec");
        trace.start();
        trace.cancel("user cancelled");
        assert_eq!(trace.outcome, TraceOutcome::Cancelled);
        assert!(trace.ended_at.is_some());
    }

    #[test]
    fn test_execution_trace_fail() {
        let mut trace = ExecutionTrace::new("exec");
        trace.start();
        trace.fail("oops");
        assert_eq!(trace.outcome, TraceOutcome::Failure);
    }

    #[test]
    fn test_execution_trace_events_for_agent_and_timeline() {
        let mut trace = ExecutionTrace::new("exec");
        trace.start();
        trace.record(TraceEvent::new(TraceEventKind::AgentInvoked, "invoke").with_agent("a-1"));
        trace.record(TraceEvent::new(TraceEventKind::TaskStarted, "task"));

        let agent_events = trace.events_for_agent("a-1");
        assert_eq!(agent_events.len(), 1);

        let timeline = trace.timeline();
        assert!(timeline.len() >= 2);

        // is_in_progress is true until complete/fail/cancel is called
        assert!(trace.is_in_progress());

        // duration_ms before ending uses now - start
        assert!(trace.duration_ms() >= 0);

        trace.complete("done");
        assert!(!trace.is_in_progress());
        // After ending, duration is end-start
        assert!(trace.duration_ms() >= 0);
    }

    #[test]
    fn test_trace_metrics_tracks_more_kinds() {
        let mut trace = ExecutionTrace::new("exec");
        trace.start();
        trace.record(TraceEvent::new(TraceEventKind::AgentInvoked, "i"));
        trace.record(TraceEvent::new(TraceEventKind::ToolCalled, "t"));
        trace.record(TraceEvent::new(TraceEventKind::TaskRetrying, "r"));
        trace.record(TraceEvent::new(TraceEventKind::ErrorOccurred, "e"));
        trace.record(TraceEvent::new(TraceEventKind::AgentInvoked, "i2").with_duration(99));
        assert_eq!(trace.metrics.agent_invocations, 2);
        assert_eq!(trace.metrics.tool_calls, 1);
        assert_eq!(trace.metrics.retries, 1);
        assert_eq!(trace.metrics.errors, 1);
        // Duration should accumulate from the .with_duration(99) event
        assert!(trace.metrics.total_duration_ms >= 99);
    }

    #[test]
    fn test_trace_metrics_success_rate_no_tasks() {
        let metrics = TraceMetrics::default();
        // No completed and no failed tasks → returns 1.0.
        assert_eq!(metrics.success_rate(), 1.0);
    }

    #[test]
    fn test_trace_builder_task_failed_and_agent_selected() {
        let mut builder = TraceBuilder::new("exec");
        builder.begin_span("span1");
        builder.task_failed("t-1", "fail msg");
        builder.agent_selected("a-1", "selected");
        builder.error("oops");
        builder.end_span("span1");
        let trace = builder.fail("done with failures");
        assert_eq!(trace.outcome, TraceOutcome::Failure);
    }

    #[test]
    fn test_trace_builder_methods_outside_span() {
        let mut builder = TraceBuilder::new("exec");
        // No span begun — exercises the `if let Some(ref span)` None branch.
        builder.task_started("t", "msg");
        builder.task_completed("t", "done", 10);
        builder.task_failed("t2", "fail");
        builder.agent_selected("a", "why");
        builder.error("err");
        builder.end_span("s"); // end_span when no span started — should be a no-op
        let _trace_ref = builder.trace();
        let trace = builder.complete("done");
        assert_eq!(trace.outcome, TraceOutcome::Success);
    }
}
