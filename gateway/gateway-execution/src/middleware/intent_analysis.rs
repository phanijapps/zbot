use agent_runtime::{ChatMessage, LlmClient};
use serde::Deserialize;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct IntentAnalysis {
    pub primary_intent: String,
    pub hidden_intents: Vec<String>,
    pub recommended_skills: Vec<String>,
    pub recommended_agents: Vec<String>,
    pub execution_strategy: ExecutionStrategy,
    pub rewritten_prompt: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionStrategy {
    pub approach: String,
    pub graph: Option<ExecutionGraph>,
    pub explanation: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub mermaid: String,
    pub max_cycles: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub task: String,
    pub agent: String,
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct EdgeCondition {
    pub when: String,
    pub to: String,
}

// ---------------------------------------------------------------------------
// LLM prompt
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// format_user_template
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// inject_intent_context
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// analyze_intent
// ---------------------------------------------------------------------------

/// Call the LLM to produce an `IntentAnalysis` for a user message.
pub async fn analyze_intent(
    llm_client: &dyn LlmClient,
    user_message: &str,
    available_skills: &[Value],
    available_agents: &[Value],
) -> Result<IntentAnalysis, String> {
    let messages = vec![
        ChatMessage::system(INTENT_ANALYSIS_PROMPT.to_string()),
        ChatMessage::user(format_user_template(
            user_message,
            available_skills,
            available_agents,
        )),
    ];

    let response = llm_client
        .chat(messages, None)
        .await
        .map_err(|e| format!("Intent analysis LLM call failed: {}", e))?;

    let content = strip_markdown_fences(&response.content);

    serde_json::from_str::<IntentAnalysis>(&content)
        .map_err(|e| format!("Failed to parse intent analysis JSON: {}", e))
}

/// Strip optional markdown code-fences that LLMs sometimes wrap around JSON.
fn strip_markdown_fences(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.starts_with("```") {
        let without_start = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```JSON")
            .trim_start_matches("```");
        if let Some(end) = without_start.rfind("```") {
            return without_start[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
            "rewritten_prompt": "Hello, how are you?"
        }"#;

        let analysis: IntentAnalysis = serde_json::from_str(json).unwrap();
        assert_eq!(analysis.primary_intent, "greeting");
        assert!(analysis.hidden_intents.is_empty());
        assert!(analysis.recommended_skills.is_empty());
        assert!(analysis.recommended_agents.is_empty());
        assert_eq!(analysis.execution_strategy.approach, "simple");
        assert!(analysis.execution_strategy.graph.is_none());
        assert_eq!(analysis.rewritten_prompt, "Hello, how are you?");
    }

    #[test]
    fn test_deserialize_graph_with_conditional_edges() {
        let json = r#"{
            "primary_intent": "code_generation",
            "hidden_intents": ["Write unit tests", "Add error handling"],
            "recommended_skills": ["code-gen", "testing"],
            "recommended_agents": ["coder", "reviewer"],
            "execution_strategy": {
                "approach": "graph",
                "graph": {
                    "nodes": [
                        {"id": "A", "task": "Generate code", "agent": "coder", "skills": ["code-gen"]},
                        {"id": "B", "task": "Review code", "agent": "reviewer", "skills": []},
                        {"id": "C", "task": "Fix issues", "agent": "coder", "skills": ["code-gen"]}
                    ],
                    "edges": [
                        {"from": "A", "to": "B"},
                        {"from": "B", "conditions": [
                            {"when": "review passes", "to": "END"},
                            {"when": "review fails", "to": "C"}
                        ]},
                        {"from": "C", "to": "B"}
                    ],
                    "mermaid": "graph TD\nA-->B\nB-->|pass|END\nB-->|fail|C\nC-->B",
                    "max_cycles": 3
                },
                "explanation": "Generate, review, fix loop with max 3 cycles"
            },
            "rewritten_prompt": "Generate code with unit tests and error handling, then review"
        }"#;

        let analysis: IntentAnalysis = serde_json::from_str(json).unwrap();
        assert_eq!(analysis.primary_intent, "code_generation");
        assert_eq!(analysis.hidden_intents.len(), 2);
        assert_eq!(analysis.recommended_skills, vec!["code-gen", "testing"]);
        assert_eq!(analysis.recommended_agents, vec!["coder", "reviewer"]);
        assert_eq!(analysis.execution_strategy.approach, "graph");

        let graph = analysis.execution_strategy.graph.as_ref().unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.nodes[0].id, "A");
        assert_eq!(graph.nodes[0].agent, "coder");
        assert_eq!(graph.max_cycles, Some(3));

        // Check edges: first is Direct, second is Conditional, third is Direct
        assert_eq!(graph.edges.len(), 3);
        match &graph.edges[0] {
            GraphEdge::Direct { from, to } => {
                assert_eq!(from, "A");
                assert_eq!(to, "B");
            }
            _ => panic!("Expected Direct edge"),
        }
        match &graph.edges[1] {
            GraphEdge::Conditional { from, conditions } => {
                assert_eq!(from, "B");
                assert_eq!(conditions.len(), 2);
                assert_eq!(conditions[0].when, "review passes");
                assert_eq!(conditions[0].to, "END");
            }
            _ => panic!("Expected Conditional edge"),
        }
        match &graph.edges[2] {
            GraphEdge::Direct { from, to } => {
                assert_eq!(from, "C");
                assert_eq!(to, "B");
            }
            _ => panic!("Expected Direct edge"),
        }
    }

    #[test]
    fn test_format_user_template() {
        let skills = vec![
            json!({"name": "code-gen", "description": "Generates code from specs"}),
            json!({"name": "testing", "description": "Runs unit tests"}),
        ];
        let agents = vec![
            json!({"name": "coder", "description": "Writes production code"}),
        ];

        let result = format_user_template("Build a REST API", &skills, &agents);

        assert!(result.contains("### User Request\nBuild a REST API"));
        assert!(result.contains("- code-gen: Generates code from specs"));
        assert!(result.contains("- testing: Runs unit tests"));
        assert!(result.contains("- coder: Writes production code"));
    }

    #[test]
    fn test_format_user_template_empty_resources() {
        let result = format_user_template("Hello", &[], &[]);

        assert!(result.contains("### User Request\nHello"));
        assert!(result.contains("### Available Skills\n(none available)"));
        assert!(result.contains("### Available Agents\n(none available)"));
    }

    #[test]
    fn test_inject_simple_intent() {
        let analysis = IntentAnalysis {
            primary_intent: "greeting".to_string(),
            hidden_intents: vec![],
            recommended_skills: vec![],
            recommended_agents: vec![],
            execution_strategy: ExecutionStrategy {
                approach: "simple".to_string(),
                graph: None,
                explanation: "Simple greeting".to_string(),
            },
            rewritten_prompt: "Hello!".to_string(),
        };

        let mut prompt = String::from("You are a helpful assistant.");
        inject_intent_context(&mut prompt, &analysis);

        assert!(prompt.contains("**Primary Intent**: greeting"));
        assert!(prompt.contains("**Rewritten request**: Hello!"));
        // No graph section
        assert!(!prompt.contains("```mermaid"));
        assert!(!prompt.contains("**Hidden Intents**"));
        assert!(!prompt.contains("**Recommended Skills**"));
        assert!(!prompt.contains("**Recommended Agents**"));
    }

    #[test]
    fn test_inject_graph_intent() {
        let analysis = IntentAnalysis {
            primary_intent: "code_generation".to_string(),
            hidden_intents: vec![
                "Write unit tests".to_string(),
                "Add error handling".to_string(),
            ],
            recommended_skills: vec!["code-gen".to_string(), "testing".to_string()],
            recommended_agents: vec!["coder".to_string(), "reviewer".to_string()],
            execution_strategy: ExecutionStrategy {
                approach: "graph".to_string(),
                graph: Some(ExecutionGraph {
                    nodes: vec![GraphNode {
                        id: "A".to_string(),
                        task: "Generate code".to_string(),
                        agent: "coder".to_string(),
                        skills: vec!["code-gen".to_string()],
                    }],
                    edges: vec![GraphEdge::Direct {
                        from: "A".to_string(),
                        to: "END".to_string(),
                    }],
                    mermaid: "graph TD\nA-->END".to_string(),
                    max_cycles: Some(5),
                }),
                explanation: "Generate then done".to_string(),
            },
            rewritten_prompt: "Generate code with tests and error handling".to_string(),
        };

        let mut prompt = String::from("You are a helpful assistant.");
        inject_intent_context(&mut prompt, &analysis);

        assert!(prompt.contains("**Primary Intent**: code_generation"));
        assert!(prompt.contains("1. Write unit tests"));
        assert!(prompt.contains("2. Add error handling"));
        assert!(prompt.contains("- code-gen"));
        assert!(prompt.contains("- testing"));
        assert!(prompt.contains("- coder"));
        assert!(prompt.contains("- reviewer"));
        assert!(prompt.contains("```mermaid\ngraph TD\nA-->END\n```"));
        assert!(prompt.contains("**Orchestration**: Generate then done"));
        assert!(prompt.contains("**Max cycles**: 5"));
        assert!(prompt.contains("**Rewritten request**: Generate code with tests and error handling"));
    }

    // -----------------------------------------------------------------------
    // MockLlmClient & async tests for analyze_intent
    // -----------------------------------------------------------------------

    use agent_runtime::{ChatResponse, LlmError, StreamCallback};
    use async_trait::async_trait;

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

    #[tokio::test]
    async fn test_analyze_intent_simple() {
        let mock = MockLlmClient {
            response: r#"{
                "primary_intent": "greeting",
                "hidden_intents": [],
                "recommended_skills": [],
                "recommended_agents": [],
                "execution_strategy": {
                    "approach": "simple",
                    "explanation": "Simple greeting"
                },
                "rewritten_prompt": "Hello!"
            }"#
            .to_string(),
        };

        let result = analyze_intent(&mock, "Hi", &[], &[]).await;
        let analysis = result.expect("should parse simple intent");
        assert_eq!(analysis.primary_intent, "greeting");
        assert_eq!(analysis.execution_strategy.approach, "simple");
        assert!(analysis.execution_strategy.graph.is_none());
        assert_eq!(analysis.rewritten_prompt, "Hello!");
    }

    #[tokio::test]
    async fn test_analyze_intent_graph() {
        let mock = MockLlmClient {
            response: r#"{
                "primary_intent": "code_generation",
                "hidden_intents": ["Write unit tests"],
                "recommended_skills": ["code-gen"],
                "recommended_agents": ["coder"],
                "execution_strategy": {
                    "approach": "graph",
                    "graph": {
                        "nodes": [
                            {"id": "A", "task": "Generate code", "agent": "coder", "skills": ["code-gen"]}
                        ],
                        "edges": [
                            {"from": "A", "to": "END"}
                        ],
                        "mermaid": "graph TD\nA-->END",
                        "max_cycles": 2
                    },
                    "explanation": "Generate then done"
                },
                "rewritten_prompt": "Generate code with tests"
            }"#
            .to_string(),
        };

        let skills = vec![json!({"name": "code-gen", "description": "Generates code"})];
        let agents = vec![json!({"name": "coder", "description": "Writes code"})];

        let result = analyze_intent(&mock, "Write code", &skills, &agents).await;
        let analysis = result.expect("should parse graph intent");
        assert_eq!(analysis.primary_intent, "code_generation");
        assert_eq!(analysis.recommended_skills, vec!["code-gen"]);
        assert_eq!(analysis.recommended_agents, vec!["coder"]);
        let graph = analysis
            .execution_strategy
            .graph
            .expect("graph should be present");
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].id, "A");
        assert_eq!(graph.max_cycles, Some(2));
    }

    #[tokio::test]
    async fn test_analyze_intent_malformed_json() {
        let mock = MockLlmClient {
            response: "This is not valid JSON at all.".to_string(),
        };

        let result = analyze_intent(&mock, "Hello", &[], &[]).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Failed to parse intent analysis JSON"),
            "unexpected error: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_analyze_intent_strips_markdown_fences() {
        let mock = MockLlmClient {
            response: r#"```json
{
    "primary_intent": "greeting",
    "hidden_intents": [],
    "recommended_skills": [],
    "recommended_agents": [],
    "execution_strategy": {
        "approach": "simple",
        "explanation": "Simple greeting"
    },
    "rewritten_prompt": "Hello!"
}
```"#
            .to_string(),
        };

        let result = analyze_intent(&mock, "Hi", &[], &[]).await;
        let analysis = result.expect("should strip fences and parse");
        assert_eq!(analysis.primary_intent, "greeting");
        assert_eq!(analysis.rewritten_prompt, "Hello!");
    }
}
