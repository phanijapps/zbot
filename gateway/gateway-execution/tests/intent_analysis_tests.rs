//! E2E integration tests for the intent analysis enrichment pipeline.
//!
//! These tests verify the full flow:
//!   analyze_intent -> inject_intent_context -> system prompt enriched

use agent_runtime::{ChatMessage, ChatResponse, LlmClient, LlmError, StreamCallback};
use async_trait::async_trait;
use gateway_execution::middleware::intent_analysis::{analyze_intent, inject_intent_context};
use gateway_services::{AgentService, SharedVaultPaths, SkillService, VaultPaths};
use serde_json::Value;
use std::sync::Arc;
use zero_core::MemoryFactStore;

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
// Mock fact store
// ===========================================================================

/// Minimal mock fact store that accepts writes and returns empty results.
struct MockFactStore;

#[async_trait]
impl MemoryFactStore for MockFactStore {
    async fn save_fact(
        &self,
        _agent_id: &str,
        _category: &str,
        _key: &str,
        _content: &str,
        _confidence: f64,
        _session_id: Option<&str>,
    ) -> Result<Value, String> {
        Ok(serde_json::json!({"status": "ok"}))
    }

    async fn recall_facts(
        &self,
        _agent_id: &str,
        _query: &str,
        _limit: usize,
    ) -> Result<Value, String> {
        Ok(serde_json::json!({"results": []}))
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Create test fixtures: fact store, skill service, agent service, vault paths.
fn test_fixtures() -> (MockFactStore, SkillService, AgentService, SharedVaultPaths) {
    let tmp = std::env::temp_dir().join("intent_analysis_integration_test");
    let _ = std::fs::create_dir_all(tmp.join("skills"));
    let _ = std::fs::create_dir_all(tmp.join("agents"));
    let _ = std::fs::create_dir_all(tmp.join("wards"));

    let fact_store = MockFactStore;
    let skill_service = SkillService::new(tmp.join("skills"));
    let agent_service = AgentService::new(tmp.join("agents"));
    let vault_paths: SharedVaultPaths = Arc::new(VaultPaths::new(tmp));

    (fact_store, skill_service, agent_service, vault_paths)
}

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
        "ward_recommendation": {
            "action": "create_new",
            "ward_name": "financial-analysis",
            "subdirectory": "portfolio-review",
            "reason": "New domain for financial work"
        },
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

// ===========================================================================
// Tests
// ===========================================================================

/// Full happy-path: analyze_intent -> inject_intent_context -> enriched prompt.
#[tokio::test]
async fn test_full_enrichment_flow() {
    let mock = MockLlmClient {
        response: complex_analysis_json(),
    };

    let (fact_store, skill_svc, agent_svc, paths) = test_fixtures();

    // Step 1: analyze_intent
    let analysis = analyze_intent(&mock, "Analyze my investment portfolio", &fact_store, &skill_svc, &agent_svc, &paths)
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
    let (fact_store, skill_svc, agent_svc, paths) = test_fixtures();

    let result = analyze_intent(&client, "Hello", &fact_store, &skill_svc, &agent_svc, &paths).await;

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
    let (fact_store, skill_svc, agent_svc, paths) = test_fixtures();

    let result = analyze_intent(&mock, "Do something", &fact_store, &skill_svc, &agent_svc, &paths).await;

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
        "ward_recommendation": {
            "action": "use_existing",
            "ward_name": "scratch",
            "subdirectory": null,
            "reason": "Simple greeting needs no dedicated ward"
        },
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
    let (fact_store, skill_svc, agent_svc, paths) = test_fixtures();

    let analysis = analyze_intent(&mock, "Hi there", &fact_store, &skill_svc, &agent_svc, &paths)
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
    let (fact_store, skill_svc, agent_svc, paths) = test_fixtures();

    let analysis = analyze_intent(&mock, "Analyze my portfolio", &fact_store, &skill_svc, &agent_svc, &paths)
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
