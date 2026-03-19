//! E2E integration tests for the intent analysis enrichment pipeline.
//!
//! These tests verify the full flow:
//!   analyze_intent -> inject_intent_context -> system prompt enriched

use agent_runtime::{ChatMessage, ChatResponse, LlmClient, LlmError, StreamCallback};
use async_trait::async_trait;
use gateway_execution::middleware::intent_analysis::{analyze_intent, inject_intent_context};
use serde_json::{json, Value};

// ===========================================================================
// Mock LLM clients
// ===========================================================================

struct MockLlmClient {
    response: String,
}

#[async_trait]
impl LlmClient for MockLlmClient {
    fn model(&self) -> &str {
        "mock"
    }
    fn provider(&self) -> &str {
        "mock"
    }
    async fn chat(
        &self,
        _messages: Vec<ChatMessage>,
        _tools: Option<Value>,
    ) -> Result<ChatResponse, LlmError> {
        Ok(ChatResponse {
            content: self.response.clone(),
            tool_calls: None,
            reasoning: None,
            usage: None,
        })
    }
    async fn chat_stream(
        &self,
        _messages: Vec<ChatMessage>,
        _tools: Option<Value>,
        _callback: StreamCallback,
    ) -> Result<ChatResponse, LlmError> {
        Ok(ChatResponse {
            content: self.response.clone(),
            tool_calls: None,
            reasoning: None,
            usage: None,
        })
    }
}

struct FailingLlmClient;

#[async_trait]
impl LlmClient for FailingLlmClient {
    fn model(&self) -> &str {
        "failing-mock"
    }
    fn provider(&self) -> &str {
        "mock"
    }
    async fn chat(
        &self,
        _messages: Vec<ChatMessage>,
        _tools: Option<Value>,
    ) -> Result<ChatResponse, LlmError> {
        Err(LlmError::ApiError("Service unavailable".into()))
    }
    async fn chat_stream(
        &self,
        _messages: Vec<ChatMessage>,
        _tools: Option<Value>,
        _callback: StreamCallback,
    ) -> Result<ChatResponse, LlmError> {
        Err(LlmError::ApiError("Service unavailable".into()))
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

fn complex_analysis_json() -> String {
    serde_json::json!({
        "primary_intent": "financial_analysis",
        "hidden_intents": [
            "Compare historical performance across asset classes",
            "Identify tax-loss harvesting opportunities",
            "Generate risk-adjusted return projections"
        ],
        "recommended_skills": ["web-search", "code-exec", "file-write"],
        "recommended_agents": ["researcher", "analyst"],
        "execution_strategy": {
            "approach": "graph",
            "graph": {
                "nodes": [
                    {"id": "A", "task": "Research current financial data and historical trends", "agent": "researcher", "skills": ["web-search"]},
                    {"id": "B", "task": "Evaluate investment options and model scenarios", "agent": "analyst", "skills": ["code-exec"]},
                    {"id": "C", "task": "Synthesize findings into actionable recommendations", "agent": "analyst", "skills": ["file-write"]},
                    {"id": "D", "task": "Quality verification -- check completeness and accuracy", "agent": "root", "skills": []},
                    {"id": "E", "task": "Fix gaps or inaccuracies found during verification", "agent": "analyst", "skills": ["web-search", "code-exec"]}
                ],
                "edges": [
                    {"from": "A", "to": "B"},
                    {"from": "B", "to": "C"},
                    {"from": "C", "to": "D"},
                    {"from": "D", "conditions": [
                        {"when": "all checks pass", "to": "END"},
                        {"when": "gaps or errors found", "to": "E"}
                    ]},
                    {"from": "E", "to": "D"}
                ],
                "mermaid": "graph TD\n  A[Research] --> B[Options]\n  B --> C[Synthesize]\n  C --> D{Quality}\n  D -->|pass| END\n  D -->|fail| E[Fix gaps]\n  E --> D",
                "max_cycles": 2
            },
            "explanation": "Research feeds into analysis, then synthesis. A quality gate loops back to fix issues, capped at 2 cycles to avoid infinite loops."
        },
        "rewritten_prompt": "Analyze my investment portfolio: research current market data and historical performance across asset classes, evaluate options with risk-adjusted return projections, identify tax-loss harvesting opportunities, and produce a written recommendations report with quality verification."
    })
    .to_string()
}

fn sample_skills() -> Vec<Value> {
    vec![
        json!({"name": "web-search", "description": "Search the web for real-time information"}),
        json!({"name": "code-exec", "description": "Execute code in a sandboxed environment"}),
        json!({"name": "file-write", "description": "Write content to files"}),
    ]
}

fn sample_agents() -> Vec<Value> {
    vec![
        json!({"name": "researcher", "description": "Specializes in information gathering and research"}),
        json!({"name": "analyst", "description": "Performs quantitative and qualitative analysis"}),
    ]
}

// ===========================================================================
// Tests
// ===========================================================================

/// Full happy-path: analyze_intent -> inject_intent_context -> enriched prompt.
#[tokio::test]
async fn test_full_enrichment_flow() {
    let mock = MockLlmClient {
        response: complex_analysis_json(),
    };

    // Step 1: analyze_intent
    let analysis = analyze_intent(&mock, "Analyze my investment portfolio", &sample_skills(), &sample_agents())
        .await
        .expect("analyze_intent should succeed with valid JSON");

    // Verify analysis fields
    assert_eq!(analysis.primary_intent, "financial_analysis");
    assert_eq!(analysis.hidden_intents.len(), 3);
    assert!(analysis.hidden_intents[0].contains("historical performance"));
    assert!(analysis.hidden_intents[1].contains("tax-loss harvesting"));
    assert!(analysis.hidden_intents[2].contains("risk-adjusted"));
    assert_eq!(analysis.recommended_skills, vec!["web-search", "code-exec", "file-write"]);
    assert_eq!(analysis.recommended_agents, vec!["researcher", "analyst"]);
    assert_eq!(analysis.execution_strategy.approach, "graph");

    let graph = analysis.execution_strategy.graph.as_ref().expect("graph should be present");
    assert_eq!(graph.nodes.len(), 5);
    assert_eq!(graph.max_cycles, Some(2));

    // Step 2: inject_intent_context on a base prompt
    let mut prompt = String::from("You are a helpful financial assistant.");
    inject_intent_context(&mut prompt, &analysis);

    // Original prompt preserved at start
    assert!(prompt.starts_with("You are a helpful financial assistant."));

    // Intent Analysis section header
    assert!(prompt.contains("## Intent Analysis"));

    // Primary intent
    assert!(prompt.contains("**Primary Intent**: financial_analysis"));

    // Hidden intents
    assert!(prompt.contains("**Hidden Intents**"));
    assert!(prompt.contains("1. Compare historical performance across asset classes"));
    assert!(prompt.contains("2. Identify tax-loss harvesting opportunities"));
    assert!(prompt.contains("3. Generate risk-adjusted return projections"));

    // Recommended skills
    assert!(prompt.contains("**Recommended Skills**"));
    assert!(prompt.contains("- web-search"));
    assert!(prompt.contains("- code-exec"));
    assert!(prompt.contains("- file-write"));

    // Recommended agents
    assert!(prompt.contains("**Recommended Agents**"));
    assert!(prompt.contains("- researcher"));
    assert!(prompt.contains("- analyst"));

    // Mermaid graph
    assert!(prompt.contains("```mermaid"));
    assert!(prompt.contains("graph TD"));

    // Max cycles
    assert!(prompt.contains("**Max cycles**: 2"));
}

/// LLM call failure should propagate as Err.
#[tokio::test]
async fn test_graceful_degradation_on_llm_failure() {
    let client = FailingLlmClient;

    let result = analyze_intent(&client, "Hello", &[], &[]).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("Intent analysis LLM call failed"),
        "unexpected error message: {}",
        err
    );
}

/// Malformed (non-JSON) LLM output should return a parse error.
#[tokio::test]
async fn test_graceful_degradation_on_malformed_json() {
    let mock = MockLlmClient {
        response: "I'm not sure what you mean.".to_string(),
    };

    let result = analyze_intent(&mock, "Do something", &[], &[]).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("Failed to parse intent analysis JSON"),
        "unexpected error message: {}",
        err
    );
}

/// Simple strategy without a graph should produce Intent Analysis section but
/// no Execution Graph or Max cycles.
#[tokio::test]
async fn test_simple_request_no_graph() {
    let simple_json = serde_json::json!({
        "primary_intent": "greeting",
        "hidden_intents": [],
        "recommended_skills": [],
        "recommended_agents": [],
        "execution_strategy": {
            "approach": "simple",
            "explanation": "Simple greeting, no orchestration needed"
        },
        "rewritten_prompt": "Hello, how are you?"
    })
    .to_string();

    let mock = MockLlmClient {
        response: simple_json,
    };

    let analysis = analyze_intent(&mock, "Hi there", &[], &[])
        .await
        .expect("should parse simple intent");

    let mut prompt = String::from("You are a friendly assistant.");
    inject_intent_context(&mut prompt, &analysis);

    // Intent Analysis section present
    assert!(prompt.contains("## Intent Analysis"));
    assert!(prompt.contains("**Primary Intent**: greeting"));

    // No graph-related content
    assert!(!prompt.contains("Execution Graph"), "simple strategy should not have Execution Graph");
    assert!(!prompt.contains("Max cycles"), "simple strategy should not have Max cycles");
}

/// When skills are recommended but not already loaded, the enriched prompt
/// should instruct the agent to load them on demand.
#[tokio::test]
async fn test_skills_recommended_but_not_loaded() {
    let mock = MockLlmClient {
        response: complex_analysis_json(),
    };

    let analysis = analyze_intent(&mock, "Analyze my portfolio", &sample_skills(), &sample_agents())
        .await
        .expect("should succeed");

    let mut prompt = String::from("You are a helpful assistant.");
    inject_intent_context(&mut prompt, &analysis);

    // The injected section must tell the agent skills are lazy-loaded
    assert!(
        prompt.contains("load when needed, unload when done"),
        "prompt should instruct lazy skill loading; got:\n{}",
        prompt
    );
}
