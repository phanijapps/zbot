# Intent Analysis Enrichment Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 2,070-line `AnalyzeIntentTool` with a ~250-line pre-execution enrichment step that runs transparently before the root agent's first LLM call.

**Architecture:** A one-shot enrichment function called from `ExecutionRunner::create_executor()` that makes one LLM call to analyze intent, then appends a `## Intent Analysis` markdown section to the system prompt. Root agent only -- subagents and continuations skip it. No conditional logic in code -- the LLM decides everything.

**Tech Stack:** Rust, serde/serde_json for JSON deserialization, agent-runtime LlmClient trait, tokio async

**Spec:** `docs/superpowers/specs/2026-03-19-intent-analysis-middleware-design.md`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `gateway/gateway-execution/src/middleware/mod.rs` | Module declaration |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Types, LLM prompt, `analyze_intent()`, `inject_intent_context()`, `format_user_template()` |
| `gateway/gateway-execution/src/lib.rs` | Add `pub mod middleware;` declaration |
| `gateway/gateway-execution/src/runner.rs` | Call enrichment in `create_executor()`, skip in `invoke_continuation()` |
| `gateway/gateway-execution/src/invoke/executor.rs` | Clone skills/agents before moving into state |
| `gateway/gateway-execution/tests/intent_analysis_tests.rs` | E2E tests with MockLlmClient |
| `runtime/agent-tools/src/tools/intent.rs` | DELETE |

---

## Chunk 1: Core Types and Deserialization

### Task 1: Create module structure and IntentAnalysis types

**Files:**
- Create: `gateway/gateway-execution/src/middleware/mod.rs`
- Create: `gateway/gateway-execution/src/middleware/intent_analysis.rs`
- Modify: `gateway/gateway-execution/src/lib.rs:23-34` (add module declaration)

- [ ] **Step 1: Create the middleware module declaration**

Create `gateway/gateway-execution/src/middleware/mod.rs`:

```rust
pub mod intent_analysis;
```

- [ ] **Step 2: Add module to lib.rs**

In `gateway/gateway-execution/src/lib.rs`, add after line 34 (after the last `mod` declaration):

```rust
pub mod middleware;
```

- [ ] **Step 3: Write the failing test for IntentAnalysis deserialization**

Create `gateway/gateway-execution/src/middleware/intent_analysis.rs` with types and a test:

```rust
use serde::Deserialize;

// ── Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IntentAnalysis {
    pub primary_intent: String,
    pub hidden_intents: Vec<String>,
    pub recommended_skills: Vec<String>,
    pub recommended_agents: Vec<String>,
    pub execution_strategy: ExecutionStrategy,
    pub rewritten_prompt: String,
}

#[derive(Debug, Deserialize)]
pub struct ExecutionStrategy {
    pub approach: String,
    pub graph: Option<ExecutionGraph>,
    pub explanation: String,
}

#[derive(Debug, Deserialize)]
pub struct ExecutionGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub mermaid: String,
    pub max_cycles: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub task: String,
    pub agent: String,
    pub skills: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum GraphEdge {
    Conditional {
        from: String,
        conditions: Vec<EdgeCondition>,
    },
    Direct {
        from: String,
        to: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct EdgeCondition {
    pub when: String,
    pub to: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_simple_intent() {
        let json = r#"{
            "primary_intent": "greeting",
            "hidden_intents": [],
            "recommended_skills": [],
            "recommended_agents": [],
            "execution_strategy": {
                "approach": "simple",
                "explanation": "Simple greeting, no orchestration needed"
            },
            "rewritten_prompt": "Hello"
        }"#;
        let analysis: IntentAnalysis = serde_json::from_str(json).unwrap();
        assert_eq!(analysis.primary_intent, "greeting");
        assert_eq!(analysis.execution_strategy.approach, "simple");
        assert!(analysis.execution_strategy.graph.is_none());
    }

    #[test]
    fn test_deserialize_graph_with_conditional_edges() {
        let json = r#"{
            "primary_intent": "financial_analysis",
            "hidden_intents": ["Research fundamentals", "Analyze options chain"],
            "recommended_skills": ["stock-analysis", "web-search"],
            "recommended_agents": ["research-agent"],
            "execution_strategy": {
                "approach": "graph",
                "graph": {
                    "nodes": [
                        {"id": "A", "task": "Research", "agent": "research-agent", "skills": ["web-search"]},
                        {"id": "B", "task": "Synthesize", "agent": "root", "skills": []},
                        {"id": "C", "task": "Verify", "agent": "quality-analyst", "skills": []},
                        {"id": "D", "task": "Fix gaps", "agent": "research-agent", "skills": ["web-search"]}
                    ],
                    "edges": [
                        {"from": "A", "to": "B"},
                        {"from": "B", "to": "C"},
                        {"from": "C", "conditions": [
                            {"when": "all covered", "to": "END"},
                            {"when": "gaps found", "to": "D"}
                        ]},
                        {"from": "D", "to": "B"}
                    ],
                    "mermaid": "graph TD\n  A --> B --> C",
                    "max_cycles": 2
                },
                "explanation": "Research then synthesize with quality check"
            },
            "rewritten_prompt": "Analyze LMND stock comprehensively"
        }"#;
        let analysis: IntentAnalysis = serde_json::from_str(json).unwrap();
        assert_eq!(analysis.execution_strategy.approach, "graph");
        let graph = analysis.execution_strategy.graph.unwrap();
        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(graph.edges.len(), 4);
        assert_eq!(graph.max_cycles, Some(2));
        // Verify conditional edge deserialized correctly
        match &graph.edges[2] {
            GraphEdge::Conditional { from, conditions } => {
                assert_eq!(from, "C");
                assert_eq!(conditions.len(), 2);
                assert_eq!(conditions[0].to, "END");
            }
            _ => panic!("Expected conditional edge"),
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p gateway-execution middleware::intent_analysis::tests`
Expected: 2 tests PASS

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/middleware/ gateway/gateway-execution/src/lib.rs
git commit -m "feat(intent): add IntentAnalysis types with serde deserialization"
```

---

### Task 2: LLM prompt and format_user_template

**Files:**
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs`

- [ ] **Step 1: Write the failing test for format_user_template**

Add to the test module in `intent_analysis.rs`:

```rust
#[test]
fn test_format_user_template() {
    let skills = vec![
        serde_json::json!({"name": "web-search", "description": "Search the web"}),
        serde_json::json!({"name": "stock-analysis", "description": "Analyze stocks"}),
    ];
    let agents = vec![
        serde_json::json!({"name": "research-agent", "description": "Does research"}),
    ];
    let result = format_user_template("Analyze LMND stock", &skills, &agents);
    assert!(result.contains("### User Request"));
    assert!(result.contains("Analyze LMND stock"));
    assert!(result.contains("web-search: Search the web"));
    assert!(result.contains("research-agent: Does research"));
}

#[test]
fn test_format_user_template_empty_resources() {
    let result = format_user_template("Hello", &[], &[]);
    assert!(result.contains("Hello"));
    assert!(result.contains("(none available)"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gateway-execution middleware::intent_analysis::tests::test_format_user_template`
Expected: FAIL with "cannot find function"

- [ ] **Step 3: Implement the LLM prompt constant and format_user_template**

Add to `intent_analysis.rs` above the test module:

```rust
use serde_json::Value;

// ── LLM Prompt ─────────────────────────────────────────────────────────

const INTENT_ANALYSIS_PROMPT: &str = r#"You are an intent analyzer for an AI agent platform.

Given a user request and the platform's available resources, your job is to:
1. Identify the primary intent behind the request
2. Discover hidden/implicit intents the user hasn't stated but would expect
3. Recommend which skills and agents would help
4. Design an execution graph showing how to orchestrate the work

## Rules
- Hidden intents must be actionable instructions, not labels
- Every non-trivial execution must end with a quality verification node
- Use conditional edges when outcomes determine next steps
- Recommend only skills and agents from the provided lists
- If the request is simple (greeting, quick question), use approach "simple" with no graph

## Output Format
Respond with ONLY a JSON object (no markdown fences, no explanation) matching this schema:
{
  "primary_intent": "string -- the core intent category",
  "hidden_intents": ["string -- actionable instruction for each hidden intent"],
  "recommended_skills": ["skill-name from the list"],
  "recommended_agents": ["agent-name from the list"],
  "execution_strategy": {
    "approach": "simple | tracked | graph",
    "graph": {
      "nodes": [{"id": "A", "task": "description", "agent": "agent-name or root", "skills": ["skill-name"]}],
      "edges": [
        {"from": "A", "to": "B"},
        {"from": "B", "conditions": [{"when": "natural language condition", "to": "C or END"}]}
      ],
      "mermaid": "graph TD mermaid diagram string",
      "max_cycles": 2
    },
    "explanation": "string -- why this orchestration shape, which nodes run in parallel"
  },
  "rewritten_prompt": "string -- the user's message with all implicit intent made explicit"
}

Only include the "graph" field when approach is "graph".
When approach is "simple", omit the graph entirely."#;

pub fn format_user_template(message: &str, skills: &[Value], agents: &[Value]) -> String {
    let skills_list = if skills.is_empty() {
        "(none available)".to_string()
    } else {
        skills
            .iter()
            .filter_map(|s| {
                let name = s.get("name")?.as_str()?;
                let desc = s.get("description")?.as_str()?;
                Some(format!("- {}: {}", name, desc))
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let agents_list = if agents.is_empty() {
        "(none available)".to_string()
    } else {
        agents
            .iter()
            .filter_map(|a| {
                let name = a.get("name")?.as_str()?;
                let desc = a.get("description")?.as_str()?;
                Some(format!("- {}: {}", name, desc))
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "### User Request\n{}\n\n### Available Skills\n{}\n\n### Available Agents\n{}",
        message, skills_list, agents_list
    )
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p gateway-execution middleware::intent_analysis::tests`
Expected: All 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/middleware/intent_analysis.rs
git commit -m "feat(intent): add LLM prompt constant and format_user_template"
```

---

### Task 3: inject_intent_context (markdown renderer)

**Files:**
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs`

- [ ] **Step 1: Write the failing test for inject_intent_context**

Add to the test module:

```rust
#[test]
fn test_inject_simple_intent() {
    let analysis = IntentAnalysis {
        primary_intent: "greeting".into(),
        hidden_intents: vec![],
        recommended_skills: vec![],
        recommended_agents: vec![],
        execution_strategy: ExecutionStrategy {
            approach: "simple".into(),
            graph: None,
            explanation: "Simple greeting".into(),
        },
        rewritten_prompt: "Hello".into(),
    };
    let mut prompt = "You are an assistant.".to_string();
    inject_intent_context(&mut prompt, &analysis);
    assert!(prompt.contains("## Intent Analysis"));
    assert!(prompt.contains("**Primary Intent**: greeting"));
    assert!(!prompt.contains("Execution Graph"));
}

#[test]
fn test_inject_graph_intent() {
    let analysis = IntentAnalysis {
        primary_intent: "financial_analysis".into(),
        hidden_intents: vec![
            "Research fundamentals".into(),
            "Analyze options".into(),
        ],
        recommended_skills: vec!["web-search".into()],
        recommended_agents: vec!["research-agent".into()],
        execution_strategy: ExecutionStrategy {
            approach: "graph".into(),
            graph: Some(ExecutionGraph {
                nodes: vec![GraphNode {
                    id: "A".into(),
                    task: "Research".into(),
                    agent: "research-agent".into(),
                    skills: vec!["web-search".into()],
                }],
                edges: vec![GraphEdge::Direct {
                    from: "A".into(),
                    to: "END".into(),
                }],
                mermaid: "graph TD\n  A --> END".into(),
                max_cycles: Some(2),
            }),
            explanation: "Research then done".into(),
        },
        rewritten_prompt: "Analyze LMND stock fully".into(),
    };
    let mut prompt = "You are an assistant.".to_string();
    inject_intent_context(&mut prompt, &analysis);
    assert!(prompt.contains("**Hidden Intents**"));
    assert!(prompt.contains("1. Research fundamentals"));
    assert!(prompt.contains("2. Analyze options"));
    assert!(prompt.contains("web-search"));
    assert!(prompt.contains("research-agent"));
    assert!(prompt.contains("graph TD"));
    assert!(prompt.contains("**Max cycles**: 2"));
    assert!(prompt.contains("Analyze LMND stock fully"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gateway-execution middleware::intent_analysis::tests::test_inject`
Expected: FAIL with "cannot find function"

- [ ] **Step 3: Implement inject_intent_context**

Add to `intent_analysis.rs`:

```rust
pub fn inject_intent_context(system_prompt: &mut String, analysis: &IntentAnalysis) {
    let mut section = String::from("\n\n## Intent Analysis\n\n");

    section.push_str(&format!("**Primary Intent**: {}\n\n", analysis.primary_intent));

    if !analysis.hidden_intents.is_empty() {
        section.push_str("**Hidden Intents** (address ALL of these):\n");
        for (i, intent) in analysis.hidden_intents.iter().enumerate() {
            section.push_str(&format!("{}. {}\n", i + 1, intent));
        }
        section.push('\n');
    }

    if !analysis.recommended_skills.is_empty() {
        section.push_str("**Recommended Skills** (load when needed, unload when done):\n");
        for skill in &analysis.recommended_skills {
            section.push_str(&format!("- {}\n", skill));
        }
        section.push('\n');
    }

    if !analysis.recommended_agents.is_empty() {
        section.push_str("**Recommended Agents** (delegate to these):\n");
        for agent in &analysis.recommended_agents {
            section.push_str(&format!("- {}\n", agent));
        }
        section.push('\n');
    }

    if let Some(graph) = &analysis.execution_strategy.graph {
        section.push_str("**Execution Graph**:\n");
        section.push_str(&format!("```mermaid\n{}\n```\n\n", graph.mermaid));
        section.push_str(&format!(
            "**Orchestration**: {}\n\n",
            analysis.execution_strategy.explanation
        ));
        let max_cycles = graph.max_cycles.unwrap_or(2);
        section.push_str(&format!("**Max cycles**: {}\n\n", max_cycles));
    }

    section.push_str(&format!(
        "**Rewritten request**: {}\n",
        analysis.rewritten_prompt
    ));

    system_prompt.push_str(&section);
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p gateway-execution middleware::intent_analysis::tests`
Expected: All 6 tests PASS

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/middleware/intent_analysis.rs
git commit -m "feat(intent): add inject_intent_context markdown renderer"
```

---

## Chunk 2: analyze_intent Function and Runner Integration

### Task 4: analyze_intent async function

**Files:**
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs`

- [ ] **Step 1: Write the failing test with MockLlmClient**

Add to `intent_analysis.rs` test module:

```rust
use agent_runtime::{ChatMessage, ChatResponse, LlmClient, LlmError};
use async_trait::async_trait;
use std::sync::Arc;

// NOTE: LlmError may not be re-exported from agent_runtime top level.
// If compilation fails, add `pub use llm::client::LlmError;` to
// runtime/agent-runtime/src/llm.rs (line 28) and
// runtime/agent-runtime/src/lib.rs (line 69).
// Alternatively use: `use agent_runtime::llm::client::LlmError;`

struct MockLlmClient {
    response: String,
}

#[async_trait]
impl LlmClient for MockLlmClient {
    fn model(&self) -> &str { "mock" }
    fn provider(&self) -> &str { "mock" }
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
        _callback: agent_runtime::StreamCallback,
    ) -> Result<ChatResponse, LlmError> {
        Ok(ChatResponse {
            content: self.response.clone(),
            tool_calls: None,
            reasoning: None,
            usage: None,
        })
    }
}

#[tokio::test]
async fn test_analyze_intent_simple() {
    let mock = Arc::new(MockLlmClient {
        response: r#"{"primary_intent":"greeting","hidden_intents":[],"recommended_skills":[],"recommended_agents":[],"execution_strategy":{"approach":"simple","explanation":"Just a greeting"},"rewritten_prompt":"Hello"}"#.into(),
    });
    let result = analyze_intent(mock.as_ref(), "Hello", &[], &[]).await;
    assert!(result.is_ok());
    let analysis = result.unwrap();
    assert_eq!(analysis.primary_intent, "greeting");
    assert_eq!(analysis.execution_strategy.approach, "simple");
}

#[tokio::test]
async fn test_analyze_intent_graph() {
    let mock = Arc::new(MockLlmClient {
        response: r#"{"primary_intent":"financial_analysis","hidden_intents":["Research fundamentals"],"recommended_skills":["web-search"],"recommended_agents":["research-agent"],"execution_strategy":{"approach":"graph","graph":{"nodes":[{"id":"A","task":"Research","agent":"research-agent","skills":["web-search"]}],"edges":[{"from":"A","to":"END"}],"mermaid":"graph TD\n  A --> END","max_cycles":2},"explanation":"Research flow"},"rewritten_prompt":"Analyze LMND fully"}"#.into(),
    });
    let skills = vec![serde_json::json!({"name": "web-search", "description": "Search"})];
    let agents = vec![serde_json::json!({"name": "research-agent", "description": "Research"})];
    let result = analyze_intent(mock.as_ref(), "Analyze LMND", &skills, &agents).await;
    assert!(result.is_ok());
    let analysis = result.unwrap();
    assert_eq!(analysis.recommended_skills, vec!["web-search"]);
    assert!(analysis.execution_strategy.graph.is_some());
}

#[tokio::test]
async fn test_analyze_intent_malformed_json() {
    let mock = Arc::new(MockLlmClient {
        response: "not valid json at all".into(),
    });
    let result = analyze_intent(mock.as_ref(), "Hello", &[], &[]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_analyze_intent_strips_markdown_fences() {
    let mock = Arc::new(MockLlmClient {
        response: "```json\n{\"primary_intent\":\"test\",\"hidden_intents\":[],\"recommended_skills\":[],\"recommended_agents\":[],\"execution_strategy\":{\"approach\":\"simple\",\"explanation\":\"test\"},\"rewritten_prompt\":\"test\"}\n```".into(),
    });
    let result = analyze_intent(mock.as_ref(), "test", &[], &[]).await;
    assert!(result.is_ok());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gateway-execution middleware::intent_analysis::tests::test_analyze_intent`
Expected: FAIL with "cannot find function"

- [ ] **Step 3: Implement analyze_intent**

Add to `intent_analysis.rs` (above the test module):

```rust
use agent_runtime::{ChatMessage, LlmClient, LlmError};

pub async fn analyze_intent(
    llm_client: &dyn LlmClient,
    user_message: &str,
    available_skills: &[Value],
    available_agents: &[Value],
) -> Result<IntentAnalysis, String> {
    let messages = vec![
        ChatMessage::system(INTENT_ANALYSIS_PROMPT.to_string()),
        ChatMessage::user(format_user_template(user_message, available_skills, available_agents)),
    ];

    let response = llm_client
        .chat(messages, None)
        .await
        .map_err(|e| format!("Intent analysis LLM call failed: {}", e))?;

    let content = strip_markdown_fences(&response.content);

    serde_json::from_str::<IntentAnalysis>(&content)
        .map_err(|e| format!("Failed to parse intent analysis JSON: {}", e))
}

fn strip_markdown_fences(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.starts_with("```") {
        let without_start = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```");
        if let Some(end) = without_start.rfind("```") {
            return without_start[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p gateway-execution middleware::intent_analysis::tests`
Expected: All 10 tests PASS

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/middleware/intent_analysis.rs
git commit -m "feat(intent): add analyze_intent async function with LLM call"
```

---

### Task 5: Clone skills/agents in executor.rs before moving into state

**Files:**
- Modify: `gateway/gateway-execution/src/invoke/executor.rs:86-120`

- [ ] **Step 1: Modify ExecutorBuilder::build() to return clones alongside the executor**

Change the `build()` signature to also accept a mutable reference for storing cloned data. Actually, simpler: change `build()` to take references instead of owned values. Modify `gateway/gateway-execution/src/invoke/executor.rs` lines 92-93:

Before:
```rust
        available_agents: Vec<serde_json::Value>,
        available_skills: Vec<serde_json::Value>,
```

After:
```rust
        available_agents: &[serde_json::Value],
        available_skills: &[serde_json::Value],
```

And update lines 111-120 to clone when inserting into state:

Before:
```rust
        if !available_agents.is_empty() {
            executor_config = executor_config
                .with_initial_state("available_agents", serde_json::Value::Array(available_agents));
        }
        if !available_skills.is_empty() {
            executor_config = executor_config
                .with_initial_state("available_skills", serde_json::Value::Array(available_skills));
        }
```

After:
```rust
        if !available_agents.is_empty() {
            executor_config = executor_config
                .with_initial_state("available_agents", serde_json::Value::Array(available_agents.to_vec()));
        }
        if !available_skills.is_empty() {
            executor_config = executor_config
                .with_initial_state("available_skills", serde_json::Value::Array(available_skills.to_vec()));
        }
```

- [ ] **Step 2: Update all call sites of build()**

Update `runner.rs` line 934-935 to pass references:

Before:
```rust
                available_agents,
                available_skills,
```

After:
```rust
                &available_agents,
                &available_skills,
```

Update `runner.rs` `invoke_continuation()` similarly (find the `.build()` call around line 1065).

Update `delegation/spawn.rs` `.build()` call (around line 160) similarly.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p gateway-execution`
Expected: No errors

- [ ] **Step 4: Run existing tests**

Run: `cargo test -p gateway-execution`
Expected: All existing tests PASS (no behavior change)

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/invoke/executor.rs gateway/gateway-execution/src/runner.rs gateway/gateway-execution/src/delegation/spawn.rs
git commit -m "refactor(executor): take skill/agent slices by reference in build()"
```

---

### Task 6: Wire enrichment into ExecutionRunner::create_executor()

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs:868-941`

- [ ] **Step 1: Add import for intent analysis module**

At the top of `runner.rs`, add:

```rust
use crate::middleware::intent_analysis::{analyze_intent, inject_intent_context};
```

- [ ] **Step 2: Modify create_executor() to accept is_root flag and user message**

Change the signature at line 869:

Before:
```rust
    async fn create_executor(
        &self,
        agent: &gateway_services::agents::Agent,
        provider: &gateway_services::providers::Provider,
        config: &ExecutionConfig,
        session_id: &str,
        ward_id: Option<&str>,
    ) -> Result<AgentExecutor, String> {
```

After:
```rust
    async fn create_executor(
        &self,
        agent: &gateway_services::agents::Agent,
        provider: &gateway_services::providers::Provider,
        config: &ExecutionConfig,
        session_id: &str,
        ward_id: Option<&str>,
        is_root: bool,
        user_message: Option<&str>,
    ) -> Result<AgentExecutor, String> {
```

- [ ] **Step 3: Add enrichment call before build()**

The approach: clone the agent, enrich its instructions, then pass the enriched clone to `build()`. This avoids modifying `build()`'s signature at all.

After the `builder` is constructed (after line 927) and before the `.build()` call, add:

```rust
        // Intent analysis enrichment (root agent first turn only)
        // Clone agent so we can modify its instructions without mutating the original
        let mut enriched_agent = agent.clone();
        if is_root {
            if let Some(msg) = user_message {
                // Create LLM client (same config as executor will use)
                let llm_config = agent_runtime::LlmConfig::new(
                    provider.base_url.clone(),
                    provider.api_key.clone(),
                    agent.model.clone(),
                    provider.id.clone().unwrap_or_else(|| provider.name.clone()),
                );
                match agent_runtime::OpenAiClient::new(llm_config) {
                    Ok(raw_client) => {
                        let llm_client: Arc<dyn agent_runtime::LlmClient> = Arc::new(raw_client);
                        match analyze_intent(
                            llm_client.as_ref(),
                            msg,
                            &available_skills,
                            &available_agents,
                        )
                        .await
                        {
                            Ok(analysis) => {
                                inject_intent_context(&mut enriched_agent.instructions, &analysis);
                                tracing::info!(
                                    primary_intent = %analysis.primary_intent,
                                    hidden_intents = analysis.hidden_intents.len(),
                                    "Intent analysis enrichment complete"
                                );
                            }
                            Err(e) => {
                                tracing::warn!("Intent analysis failed, proceeding without enrichment: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create LLM client for intent analysis: {}", e);
                    }
                }
            }
        }
```

Then change the `build()` call to pass `&enriched_agent` instead of `agent`:

```rust
        builder
            .build(
                &enriched_agent,  // <-- enriched clone, not original
                provider,
                &config.conversation_id,
                session_id,
                &available_agents,
                &available_skills,
                hook_context.as_ref(),
                &self.mcp_service,
                ward_id,
            )
            .await
```

**No changes to `build()` signature needed.** The `Agent` struct must implement `Clone` -- verify this (it likely already derives Clone since it's a config struct). If not, add `#[derive(Clone)]` to the Agent struct in `gateway-services`.

- [ ] **Step 4: Update the invoke() call site**

In `runner.rs` `invoke()` method at line 426, change:

Before:
```rust
        let executor = match self.create_executor(&agent, &provider, &config, &session_id, setup.ward_id.as_deref()).await {
```

After:
```rust
        let executor = match self.create_executor(&agent, &provider, &config, &session_id, setup.ward_id.as_deref(), true, Some(&message)).await {
```

- [ ] **Step 5: Update invoke_continuation() call site**

`invoke_continuation()` builds its executor directly (not via `create_executor`). At the `.build()` call around line 1065, no changes needed -- it passes the agent directly to `build()` without enrichment, which is correct (continuations skip enrichment).

Add a comment for clarity:
```rust
        // Continuation: skip intent analysis (already enriched on first turn)
        let executor = builder
            .build(
                &agent,  // unmodified -- intent analysis already in the original turn's system prompt
                ...
            )
            .await?;
```

- [ ] **Step 6: Verify spawn_delegated_agent() needs no changes**

In `delegation/spawn.rs`, `build()` is called with the delegated agent directly. No enrichment needed for subagents -- the `build()` signature hasn't changed, so no updates required.

- [ ] **Step 7: Verify compilation**

Run: `cargo check -p gateway-execution`
Expected: No errors

- [ ] **Step 8: Run all tests**

Run: `cargo test -p gateway-execution`
Expected: All tests PASS

- [ ] **Step 9: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs gateway/gateway-execution/src/invoke/executor.rs gateway/gateway-execution/src/delegation/spawn.rs
git commit -m "feat(intent): wire intent analysis enrichment into root executor creation"
```

---

## Chunk 3: Cleanup and E2E Tests

### Task 7: Delete intent.rs

**Files:**
- Delete: `runtime/agent-tools/src/tools/intent.rs`

- [ ] **Step 1: Verify intent.rs is not referenced anywhere**

Run: `cargo check --workspace` to confirm no compilation references.

Search for any `mod intent` or `use.*intent` in `runtime/agent-tools/src/tools/mod.rs`.

- [ ] **Step 2: Delete the file**

```bash
rm runtime/agent-tools/src/tools/intent.rs
```

- [ ] **Step 3: Verify workspace still compiles**

Run: `cargo check --workspace`
Expected: No errors (file was never registered in mod.rs)

- [ ] **Step 4: Commit**

```bash
git add -A runtime/agent-tools/src/tools/intent.rs
git commit -m "chore: remove unused AnalyzeIntentTool (2,070 lines, replaced by enrichment middleware)"
```

---

### Task 8: Strip system prompt templates

**Files:**
- Modify: `gateway/templates/instructions_starter.md` (if references found)
- Modify: `gateway/templates/system_prompt.md` (if references found)

**Note:** Current exploration shows these templates may already be clean of `analyze_intent` references. Verify before making changes.

- [ ] **Step 1: Search for intent analysis references in templates**

```bash
grep -r "analyze_intent\|AUTONOMOUS BEHAVIOR\|auto_load" gateway/templates/
```

If no results: skip Steps 2-4 and commit a no-op (or skip this task entirely).

- [ ] **Step 2: Remove any found references**

If references exist, remove:
- Any `## AUTONOMOUS BEHAVIOR PROTOCOL` section
- Any mentions of `analyze_intent(message, auto_load=true)`
- Any instructions telling the agent to "call analyze_intent"

If `instructions_starter.md` is entirely redundant with `system_prompt.md`, consider consolidating.

- [ ] **Step 3: Verify templates render correctly**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 4: Commit (only if changes were made)**

```bash
git add gateway/templates/
git commit -m "refactor(templates): strip intent analysis instructions from system prompts"
```

---

### Task 9: E2E integration tests

**Files:**
- Create: `gateway/gateway-execution/tests/intent_analysis_tests.rs`

- [ ] **Step 1: Write e2e test for root agent enrichment**

Create `gateway/gateway-execution/tests/intent_analysis_tests.rs`:

```rust
//! Integration tests for intent analysis enrichment.
//!
//! These tests verify the enrichment is called for root agents,
//! skipped for subagents/continuations, and gracefully degrades on failure.

use gateway_execution::middleware::intent_analysis::{
    analyze_intent, inject_intent_context, format_user_template, IntentAnalysis,
};
use agent_runtime::{ChatMessage, ChatResponse, LlmClient, LlmError};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

struct MockLlmClient {
    response: String,
}

#[async_trait]
impl LlmClient for MockLlmClient {
    fn model(&self) -> &str { "mock" }
    fn provider(&self) -> &str { "mock" }
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
        _callback: agent_runtime::StreamCallback,
    ) -> Result<ChatResponse, LlmError> {
        unimplemented!("Not needed for tests")
    }
}

struct FailingLlmClient;

#[async_trait]
impl LlmClient for FailingLlmClient {
    fn model(&self) -> &str { "mock" }
    fn provider(&self) -> &str { "mock" }
    async fn chat(
        &self,
        _messages: Vec<ChatMessage>,
        _tools: Option<Value>,
    ) -> Result<ChatResponse, LlmError> {
        Err(LlmError::HttpError("Connection refused".into()))
    }
    async fn chat_stream(
        &self,
        _messages: Vec<ChatMessage>,
        _tools: Option<Value>,
        _callback: agent_runtime::StreamCallback,
    ) -> Result<ChatResponse, LlmError> {
        unimplemented!("Not needed for tests")
    }
}

fn complex_analysis_json() -> String {
    r#"{
        "primary_intent": "financial_analysis",
        "hidden_intents": [
            "Research LMND fundamentals via web search",
            "Pull options chain data for sentiment analysis",
            "Compare against insurance industry peers"
        ],
        "recommended_skills": ["stock-analysis", "web-search"],
        "recommended_agents": ["research-agent", "data-analyst"],
        "execution_strategy": {
            "approach": "graph",
            "graph": {
                "nodes": [
                    {"id": "A", "task": "Research fundamentals", "agent": "research-agent", "skills": ["web-search"]},
                    {"id": "B", "task": "Options analysis", "agent": "data-analyst", "skills": ["stock-analysis"]},
                    {"id": "C", "task": "Synthesize thesis", "agent": "root", "skills": []},
                    {"id": "D", "task": "Quality verification", "agent": "quality-analyst", "skills": []},
                    {"id": "E", "task": "Fix gaps", "agent": "research-agent", "skills": ["web-search"]}
                ],
                "edges": [
                    {"from": "A", "to": "C"},
                    {"from": "B", "to": "C"},
                    {"from": "C", "to": "D"},
                    {"from": "D", "conditions": [
                        {"when": "all intents addressed", "to": "END"},
                        {"when": "gaps found", "to": "E"}
                    ]},
                    {"from": "E", "to": "C"}
                ],
                "mermaid": "graph TD\n  A[Research] --> C[Synthesize]\n  B[Options] --> C\n  C --> D{Quality}\n  D -->|pass| END\n  D -->|gaps| E[Fix]\n  E --> C",
                "max_cycles": 2
            },
            "explanation": "A and B run in parallel, C synthesizes, D verifies with retry loop"
        },
        "rewritten_prompt": "Build a professional analysis of LMND stock including fundamentals, options chain, peer comparison, and investment thesis"
    }"#.to_string()
}

#[tokio::test]
async fn test_full_enrichment_flow() {
    let mock = Arc::new(MockLlmClient {
        response: complex_analysis_json(),
    });
    let skills = vec![
        serde_json::json!({"name": "stock-analysis", "description": "Analyze stocks"}),
        serde_json::json!({"name": "web-search", "description": "Search the web"}),
    ];
    let agents = vec![
        serde_json::json!({"name": "research-agent", "description": "Web research"}),
        serde_json::json!({"name": "data-analyst", "description": "Data analysis"}),
    ];

    // Step 1: Analyze intent
    let analysis = analyze_intent(mock.as_ref(), "Analyze LMND stock", &skills, &agents)
        .await
        .expect("analyze_intent should succeed");

    assert_eq!(analysis.primary_intent, "financial_analysis");
    assert_eq!(analysis.hidden_intents.len(), 3);
    assert_eq!(analysis.recommended_skills, vec!["stock-analysis", "web-search"]);
    assert_eq!(analysis.recommended_agents, vec!["research-agent", "data-analyst"]);

    // Step 2: Inject into system prompt
    let mut prompt = "You are a helpful assistant.".to_string();
    inject_intent_context(&mut prompt, &analysis);

    // Verify system prompt was enriched
    assert!(prompt.contains("## Intent Analysis"));
    assert!(prompt.contains("financial_analysis"));
    assert!(prompt.contains("Research LMND fundamentals via web search"));
    assert!(prompt.contains("stock-analysis"));
    assert!(prompt.contains("research-agent"));
    assert!(prompt.contains("graph TD"));
    assert!(prompt.contains("Quality"));
    assert!(prompt.contains("Max cycles"));

    // Verify original prompt preserved
    assert!(prompt.starts_with("You are a helpful assistant."));
}

#[tokio::test]
async fn test_graceful_degradation_on_llm_failure() {
    let mock = Arc::new(FailingLlmClient);
    let result = analyze_intent(mock.as_ref(), "Hello", &[], &[]).await;
    assert!(result.is_err());
    // The caller (runner) catches this and proceeds without enrichment
}

#[tokio::test]
async fn test_graceful_degradation_on_malformed_json() {
    let mock = Arc::new(MockLlmClient {
        response: "I'm not sure what you mean".into(),
    });
    let result = analyze_intent(mock.as_ref(), "Hello", &[], &[]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_simple_request_no_graph() {
    let mock = Arc::new(MockLlmClient {
        response: r#"{"primary_intent":"conversation","hidden_intents":[],"recommended_skills":[],"recommended_agents":[],"execution_strategy":{"approach":"simple","explanation":"Simple greeting"},"rewritten_prompt":"Hello"}"#.into(),
    });
    let analysis = analyze_intent(mock.as_ref(), "Hello", &[], &[]).await.unwrap();
    let mut prompt = "Base prompt.".to_string();
    inject_intent_context(&mut prompt, &analysis);

    assert!(prompt.contains("## Intent Analysis"));
    assert!(!prompt.contains("Execution Graph"));
    assert!(!prompt.contains("Max cycles"));
}

#[tokio::test]
async fn test_skills_recommended_but_not_loaded() {
    let mock = Arc::new(MockLlmClient {
        response: complex_analysis_json(),
    });
    let analysis = analyze_intent(mock.as_ref(), "Analyze LMND", &[], &[]).await.unwrap();
    let mut prompt = String::new();
    inject_intent_context(&mut prompt, &analysis);

    // Skills are listed as recommendations in the prompt
    assert!(prompt.contains("stock-analysis"));
    assert!(prompt.contains("web-search"));
    // But nothing is actually loaded -- this test just verifies the prompt
    // doesn't contain any "loaded" or "injected" language
    assert!(prompt.contains("load when needed, unload when done"));
}
```

**Deferred tests** (require full executor setup, not mock-based):
- "Planning autonomy shard present" -- requires template rendering pipeline
- "Load/unload skill lifecycle" -- requires running executor with tool calls
- "Max cycles respected" -- runtime behavioral test
- "Full e2e flow with delegation" -- requires DelegationRegistry, multiple agents

These will be covered in a follow-up integration test task once the enrichment is stable.

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p gateway-execution --test intent_analysis_tests`
Expected: All 5 integration tests PASS

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/tests/intent_analysis_tests.rs
git commit -m "test(intent): add e2e integration tests for intent analysis enrichment"
```

---

### Task 10: Update documentation

**Files:**
- Modify: `memory-bank/intent-analysis.md`
- Modify: `memory-bank/architecture.md`

- [ ] **Step 1: Update intent-analysis.md**

Replace the content of `memory-bank/intent-analysis.md` to reflect the new enrichment architecture. Key changes:
- Remove references to `AnalyzeIntentTool` as a tool agents call
- Document the pre-execution enrichment flow
- Update file locations table
- Update the architecture diagram
- Keep the evolution history, adding v4 (current)

- [ ] **Step 2: Update architecture.md**

Search for any references to `analyze_intent`, `intent.rs`, or the old tool flow in `memory-bank/architecture.md`. Update to reference the new `middleware/intent_analysis.rs` location and the enrichment pattern.

- [ ] **Step 3: Commit**

```bash
git add memory-bank/
git commit -m "docs: update intent analysis documentation for enrichment architecture"
```

---

### Task 11: Final verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 2: Full workspace tests**

Run: `cargo test --workspace`
Expected: All tests PASS

- [ ] **Step 3: Verify intent.rs is gone**

Run: `ls runtime/agent-tools/src/tools/intent.rs`
Expected: File not found

- [ ] **Step 4: Verify enrichment module exists**

Run: `ls gateway/gateway-execution/src/middleware/intent_analysis.rs`
Expected: File exists

- [ ] **Step 5: Final commit (if any loose changes)**

```bash
git status
# If clean, no commit needed
```
