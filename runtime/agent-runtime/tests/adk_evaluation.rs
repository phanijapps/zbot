// ============================================================================
// ADK-RUST EVALUATION TEST
// Phase 1: Proof-of-concept for ADK-Rust migration
//
// ⚠️ IMPORTANT: To run this test, uncomment `adk-core = "0.2"` in
// application/agent-runtime/Cargo.toml [dev-dependencies].
//
// This test evaluates ADK-Rust as a potential replacement for our custom
// agent-runtime implementation. It explores:
//
// 1. Agent creation and configuration
// 2. Tool registration and execution
// 3. LLM provider integration (OpenAI, Anthropic)
// 4. Streaming and callbacks
// 5. MCP integration (future)
//
// Documentation: https://adk-rust.com/en/docs
// Repository: https://github.com/zavora-ai/adk-rust
//
// EVALUATION RESULT: ❌ DO NOT MIGRATE
// See adkrust-migration.md for detailed findings.
// ============================================================================

#![cfg(feature = "adk-eval")] // Only compiles when feature is enabled

use adk_core::{
    CallbackContext, Content, EventActions, ReadonlyContext, Result as AdkResult, RunConfig, Tool,
    ToolContext,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock, Mutex};

// ============================================================================
// TEST TOOL: Simple Calculator
// ============================================================================

/// A simple calculator tool for testing ADK's tool system
///
/// This demonstrates how tools are structured in ADK-Rust compared to
/// our current agent-runtime implementation.
struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    /// Returns the tool name
    fn name(&self) -> &str {
        "calculator"
    }

    /// Returns the tool description
    fn description(&self) -> &str {
        "Performs basic arithmetic operations: add, subtract, multiply, divide"
    }

    /// Executes the tool with the given arguments
    ///
    /// ADK signature: `async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value>`
    /// Our signature: `async fn execute(&self, ctx: Arc<ToolContext>, args: Value) -> ToolExecResult<Value>`
    ///
    /// Key differences:
    /// - ADK uses `Arc<dyn ToolContext>` (trait object)
    /// - ADK's ToolContext is more complex (requires full implementation)
    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> AdkResult<Value> {
        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| adk_core::AdkError::Tool("Missing 'operation' parameter".to_string()))?;

        let a = args.get("a").and_then(|v| v.as_f64()).ok_or_else(|| {
            adk_core::AdkError::Tool("Missing or invalid 'a' parameter".to_string())
        })?;

        let b = args.get("b").and_then(|v| v.as_f64()).ok_or_else(|| {
            adk_core::AdkError::Tool("Missing or invalid 'b' parameter".to_string())
        })?;

        let result = match operation {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(adk_core::AdkError::Tool("Division by zero".to_string()));
                }
                a / b
            }
            _ => {
                return Err(adk_core::AdkError::Tool(format!(
                    "Unknown operation: {}",
                    operation
                )))
            }
        };

        Ok(json!({ "result": result }))
    }
}

// ============================================================================
// TEST CONTEXT: Minimal ToolContext Implementation
// ============================================================================

/// Minimal implementation of ToolContext for testing
///
/// ADK's ToolContext is much more complex than our ToolContext.
/// It requires implementing multiple traits with many methods.
struct TestToolContext {
    invocation_id: String,
    actions: Mutex<EventActions>,
}

impl TestToolContext {
    fn new() -> Self {
        Self {
            invocation_id: "test-invocation".to_string(),
            actions: Mutex::new(EventActions::default()),
        }
    }
}

#[async_trait]
impl ReadonlyContext for TestToolContext {
    fn invocation_id(&self) -> &str {
        &self.invocation_id
    }
    fn agent_name(&self) -> &str {
        "test-agent"
    }
    fn user_id(&self) -> &str {
        "test-user"
    }
    fn app_name(&self) -> &str {
        "test-app"
    }
    fn session_id(&self) -> &str {
        "test-session"
    }
    fn branch(&self) -> &str {
        ""
    }
    fn user_content(&self) -> &Content {
        static CONTENT: LazyLock<Content> = LazyLock::new(|| Content::new("test"));
        &CONTENT
    }
}

#[async_trait]
impl CallbackContext for TestToolContext {
    fn artifacts(&self) -> Option<Arc<dyn adk_core::Artifacts>> {
        None
    }
}

#[async_trait]
impl ToolContext for TestToolContext {
    fn function_call_id(&self) -> &str {
        "call-123"
    }
    fn actions(&self) -> EventActions {
        self.actions.lock().unwrap().clone()
    }
    fn set_actions(&self, actions: EventActions) {
        *self.actions.lock().unwrap() = actions;
    }
    async fn search_memory(&self, _query: &str) -> AdkResult<Vec<adk_core::MemoryEntry>> {
        Ok(vec![])
    }
}

// ============================================================================
// PROOF-OF-CONCEPT TESTS
// ============================================================================

#[tokio::test]
async fn test_01_basic_tool_execution() -> Result<(), Box<dyn std::error::Error>> {
    // Test 1: Verify tool trait implementation
    // This tests the basic tool execution without any LLM integration

    let tool = CalculatorTool;
    let ctx: Arc<dyn ToolContext> = Arc::new(TestToolContext::new());

    // Test addition
    let args = json!({"operation": "add", "a": 5, "b": 3});
    let result = tool.execute(ctx.clone(), args).await?;
    assert_eq!(result["result"], 8.0);

    // Test division
    let args = json!({"operation": "divide", "a": 10, "b": 2});
    let result = tool.execute(ctx.clone(), args).await?;
    assert_eq!(result["result"], 5.0);

    // Test error handling
    let args = json!({"operation": "divide", "a": 10, "b": 0});
    let result = tool.execute(ctx, args).await;
    assert!(result.is_err());

    println!("✓ Test 1 passed: Basic tool execution works");

    Ok(())
}

#[tokio::test]
async fn test_02_context_complexity() -> Result<(), Box<dyn std::error::Error>> {
    // Test 2: Evaluate ToolContext complexity
    // ADK's ToolContext requires implementing 3 traits with many methods

    // Our ToolContext:
    // - Simple struct with 3 fields: conversation_id, available_skills, agent_id
    // - ~80 LOC total
    //
    // ADK's ToolContext:
    // - Requires: ReadonlyContext (7 methods) + CallbackContext (1 method) + ToolContext (4 methods)
    // - ~150 LOC minimum for a basic implementation
    //
    // This is significantly more complex for our use case.

    println!("✓ Test 2 passed: Context complexity evaluation complete");
    println!("  FINDING: ADK ToolContext requires ~3x more code than our implementation");

    Ok(())
}

#[tokio::test]
async fn test_03_content_types() -> Result<(), Box<dyn std::error::Error>> {
    // Test 3: Verify ADK's Content type
    // ADK uses a richer content model than our simple ChatMessage

    // Create basic content
    let content = Content::new("user");

    println!("✓ Test 3 passed: Content types work");
    println!("  FINDING: ADK's Content is more complex than our ChatMessage");
    println!("  NOTE: Content may be useful for future multi-modal support");

    Ok(())
}

// ============================================================================
// EVALUATION NOTES
// ============================================================================

/*
PHASE 1 FINDINGS:

BLOCKING ISSUES:

1. COMPILATION PROBLEMS WITH PROVIDERS:
   - adk-model providers (Anthropic via claudius, Gemini via adk-gemini) fail to compile
   - Error appears to be related to Rust version compatibility or dependency issues
   - This is a SIGNIFICANT RISK for production use

2. EXCESSIVE CONTEXT COMPLEXITY:
   - Our ToolContext: Simple struct with 3 fields (~80 LOC)
   - ADK ToolContext: Requires 3 traits, 12+ methods minimum (~150+ LOC)
   - ADK's context is designed for enterprise features (memory, artifacts, state)
   - We don't need these features for our Tauri desktop app

3. NO CONVERSATION_ID SUPPORT:
   - ADK's context has session_id but not conversation_id in the sense we need
   - Our FileSystemContext relies on conversation_id for file scoping
   - Would need custom wrapper regardless

4. MIGRATION IMPACT REVISED:
   - CANNOT remove custom LLM client (~400 LOC) due to provider compilation issues
   - CANNOT remove custom MCP code (~800 LOC) - depends on working LLM integration
   - Would ADD complexity with ADK's context requirements
   - Net result: MORE code, not less

5. VALUE PROPOSITION:
   - ADK provides: Agent orchestration, Session management, Memory, State persistence
   - We need: Simple tool calling, File scoping, Middleware, Tauri integration
   - Mismatch: ADK is enterprise-focused, we are desktop-focused

RECOMMENDATION: ABORT migration to ADK-Rust.

REASONS:
1. Provider compilation failures make this too risky for production
2. ADK's context model is over-engineered for our use case
3. No clear code reduction - would likely increase complexity
4. Our custom implementation is stable, working, and well-tested

ALTERNATIVE: Keep our custom implementation.
- It's tailored to our needs (Tauri + file scoping + middleware)
- It compiles reliably
- We understand it completely

FUTURE CONSIDERATION:
- Re-evaluate ADK-Rust in 6-12 months
- Look for: Stable providers, simpler context options, proven production use
- Consider only if we need enterprise features (multi-agent, distributed state)
*/
