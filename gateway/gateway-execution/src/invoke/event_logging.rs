//! # Event Logging
//!
//! Execution log helpers for stream event processing.
//!
//! Each public function builds an [`ExecutionLog`] entry and routes it through
//! the batch writer if one is present, otherwise writes directly via the log service.

use api_logs::{ExecutionLog, LogCategory, LogLevel};

use super::stream_context::StreamContext;

// ============================================================================
// PRIVATE ROUTING HELPER
// ============================================================================

/// Route a log entry through the batch writer if available, else write directly.
fn log_entry(ctx: &StreamContext, entry: ExecutionLog) {
    if let Some(writer) = &ctx.batch_writer {
        writer.log(entry);
    } else {
        let _ = ctx.log_service.log(entry);
    }
}

// ============================================================================
// PUBLIC LOG FUNCTIONS
// ============================================================================

/// Log a delegation event.
pub fn log_delegation(ctx: &StreamContext, child_agent: &str, task: &str) {
    let entry = ExecutionLog::new(
        &ctx.execution_id,
        &ctx.session_id,
        &ctx.agent_id,
        LogLevel::Info,
        LogCategory::Delegation,
        format!("Delegating to {}", child_agent),
    )
    .with_metadata(serde_json::json!({
        "child_agent": child_agent,
        "task": task,
    }));
    log_entry(ctx, entry);
}

/// Log a tool call start event.
pub fn log_tool_call(
    ctx: &StreamContext,
    tool_id: &str,
    tool_name: &str,
    args: &serde_json::Value,
) {
    let entry = ExecutionLog::new(
        &ctx.execution_id,
        &ctx.session_id,
        &ctx.agent_id,
        LogLevel::Info,
        LogCategory::ToolCall,
        format!("Calling tool: {}", tool_name),
    )
    .with_metadata(serde_json::json!({
        "tool_id": tool_id,
        "tool_name": tool_name,
        "args": args,
    }));
    log_entry(ctx, entry);
}

/// Log a tool result event.
pub fn log_tool_result(ctx: &StreamContext, tool_id: &str, result: &str, error: &Option<String>) {
    // Tool failures are expected behavior, use Warn not Error
    let level = if error.is_some() {
        LogLevel::Warn
    } else {
        LogLevel::Info
    };

    // Truncate result for logging
    let truncated = if result.len() > 500 {
        format!("{}...", zero_core::truncate_str(result, 500))
    } else {
        result.to_string()
    };

    let entry = ExecutionLog::new(
        &ctx.execution_id,
        &ctx.session_id,
        &ctx.agent_id,
        level,
        LogCategory::ToolResult,
        if error.is_some() {
            "Tool returned error"
        } else {
            "Tool completed"
        },
    )
    .with_metadata(serde_json::json!({
        "tool_id": tool_id,
        "result": truncated,
        "error": error,
    }));
    log_entry(ctx, entry);
}

/// Log an error event.
pub fn log_error(ctx: &StreamContext, error: &str) {
    let entry = ExecutionLog::new(
        &ctx.execution_id,
        &ctx.session_id,
        &ctx.agent_id,
        LogLevel::Error,
        LogCategory::Error,
        error,
    );
    log_entry(ctx, entry);
}
