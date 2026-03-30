use agent_runtime::{ChatMessage, LlmClient};
use gateway_services::{AgentService, SharedVaultPaths, SkillService};
use serde::Deserialize;
use serde_json::Value;
use zero_core::MemoryFactStore;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct IntentAnalysis {
    pub primary_intent: String,
    pub hidden_intents: Vec<String>,
    pub recommended_skills: Vec<String>,
    pub recommended_agents: Vec<String>,
    pub ward_recommendation: WardRecommendation,
    pub execution_strategy: ExecutionStrategy,
    pub rewritten_prompt: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WardRecommendation {
    /// "use_existing" or "create_new"
    pub action: String,
    /// Ward name — domain-level reusable name (e.g., "financial-analysis", "math-tutor")
    pub ward_name: String,
    /// Suggested subdirectory for this specific task (e.g., "stocks/lmnd", "trinomials")
    pub subdirectory: Option<String>,
    /// Directory layout: key = directory path, value = purpose
    /// e.g., {"core/": "Shared Python modules", "output/": "Reports, charts, HTML"}
    #[serde(default)]
    pub structure: std::collections::HashMap<String, String>,
    /// Why this ward
    pub reason: String,
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
4. Recommend the right ward (project workspace) — reuse existing or create new
5. Design an execution graph showing how to orchestrate the work

## Ward Philosophy
Wards are reusable project workspaces organized by DOMAIN, not by task. Think of them as permanent libraries:
- "financial-analysis" for ALL stock/options/market work
- "math-tutor" for ALL math work
- "research-hub" for general research projects
If an existing ward matches the domain, REUSE it. Only create new for genuinely new domains.

Every ward MUST follow this directory convention:
- core/         — Shared reusable Python modules (data fetching, indicators, formatters)
- {task-subdir}/ — Task-specific scripts and intermediate data (e.g., stocks/spy/, trinomials/)
- output/       — All final deliverables: reports, charts, HTML, PDF, CSV exports
- AGENTS.md     — Living documentation of the ward's purpose, structure, and reusable components

Include a "structure" map in your ward_recommendation showing directories and their purpose.

## Rules
- Hidden intents must be actionable instructions, not labels
- Every non-trivial execution must end with a quality verification node
- Use conditional edges when outcomes determine next steps
- CRITICAL: Skills and agents are DIFFERENT things. Skills are loaded with load_skill(). Agents are delegated to with delegate_to_agent().
- In "recommended_skills": use skill names from the "Relevant Skills" list. These are SKILLS, not agents.
- In "recommended_agents" and graph node "agent" fields: use ONLY agent names from the "Relevant Agents" list or "root". NEVER put a skill name (like "coding" or "ml-pipeline-builder") as an agent. Any invalid agent name will crash.
- If the request is simple (greeting, quick question), use approach "simple" with no graph
- Ward names must be domain-level (not task-specific): "financial-analysis" not "lmnd-report"
- Any graph node that creates or modifies files MUST include "coding" in its skills list. The coding skill teaches agents how to write clean, modular, reusable code in the ward directory structure.

## Output Format
Respond with ONLY a JSON object (no markdown fences, no explanation) matching this schema:
{
  "primary_intent": "string -- the core intent category",
  "hidden_intents": ["string -- actionable instruction for each hidden intent"],
  "recommended_skills": ["skill-name from the list"],
  "recommended_agents": ["agent-name from the list"],
  "ward_recommendation": {
    "action": "use_existing | create_new",
    "ward_name": "domain-level name like financial-analysis, math-tutor, research-hub",
    "subdirectory": "task-specific subdir like stocks/spy, trinomials -- null for simple tasks",
    "structure": {"core/": "purpose", "stocks/spy/": "purpose", "output/": "purpose"},
    "reason": "why this ward"
  },
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

pub fn format_user_template(
    message: &str,
    skills: &[Value],
    agents: &[Value],
    wards: &[String],
) -> String {
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

    let wards_list = if wards.is_empty() {
        "(none — all new)".to_string()
    } else {
        wards.iter().map(|w| format!("- {}", w)).collect::<Vec<_>>().join("\n")
    };

    format!(
        "### User Request\n{}\n\n### Available Skills\n{}\n\n### Available Agents\n{}\n\n### Existing Wards\n{}",
        message, skills_list, agents_list, wards_list
    )
}

// ---------------------------------------------------------------------------
// inject_intent_context
// ---------------------------------------------------------------------------

pub fn inject_intent_context(system_prompt: &mut String, analysis: &IntentAnalysis) {
    let mut section = String::from("\n\n## Intent Analysis\n\n");
    let is_graph = analysis.execution_strategy.graph.is_some();

    section.push_str(&format!("**Primary Intent**: {}\n\n", analysis.primary_intent));

    if !analysis.hidden_intents.is_empty() {
        section.push_str("**Hidden Intents** (address ALL of these):\n");
        for (i, intent) in analysis.hidden_intents.iter().enumerate() {
            section.push_str(&format!("{}. {}\n", i + 1, intent));
        }
        section.push('\n');
    }

    // Ward
    let ward = &analysis.ward_recommendation;
    section.push_str(&format!("**Ward**: `{}` ({}) — {}\n\n", ward.ward_name, ward.action, ward.reason));

    if is_graph {
        // GRAPH TASKS: slim injection — specs are in the ward, not here
        section.push_str(&format!(
            "**Approach**: graph ({} nodes). Placeholder specs are in `specs/` in the ward.\n\
             **Action**: Delegate to a planning subagent to fill the specs, then delegate execution.\n\
             Do NOT load skills, create your own plan, or write code directly. Read the specs.\n\n",
            analysis.execution_strategy.graph.as_ref().map(|g| g.nodes.len()).unwrap_or(0)
        ));
    } else {
        // SIMPLE/TRACKED TASKS: full injection — skills, agents, everything
        if !analysis.recommended_skills.is_empty() {
            section.push_str("**Recommended Skills** (load when needed):\n");
            for skill in &analysis.recommended_skills {
                section.push_str(&format!("- {}\n", skill));
            }
            section.push('\n');
        }

        if !analysis.recommended_agents.is_empty() {
            section.push_str("**Recommended Agents**:\n");
            for agent in &analysis.recommended_agents {
                section.push_str(&format!("- {}\n", agent));
            }
            section.push('\n');
        }
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

/// Autonomous intent analysis: indexes resources, searches semantically, calls LLM.
pub async fn analyze_intent(
    llm_client: &dyn LlmClient,
    user_message: &str,
    fact_store: &dyn MemoryFactStore,
    skill_service: &SkillService,
    agent_service: &AgentService,
    vault_paths: &SharedVaultPaths,
) -> Result<IntentAnalysis, String> {
    tracing::info!("Starting intent analysis for root session");

    // Step 1: Index resources into memory (idempotent upsert)
    index_resources(fact_store, skill_service, agent_service, vault_paths).await;

    // Step 2: Semantic search for relevant resources
    let results = search_resources(fact_store, user_message).await;

    tracing::info!(
        skills_matched = results.skills.len(),
        agents_matched = results.agents.len(),
        wards_matched = results.wards.len(),
        "Semantic search complete"
    );

    // Step 3: Build LLM prompt with only relevant resources
    let messages = vec![
        ChatMessage::system(INTENT_ANALYSIS_PROMPT.to_string()),
        ChatMessage::user(format_user_template(
            user_message,
            &results.skills,
            &results.agents,
            &results.wards,
        )),
    ];

    tracing::info!(
        skills = results.skills.len(),
        agents = results.agents.len(),
        wards = results.wards.len(),
        "LLM call — sending relevant resources"
    );

    // Step 4: Call LLM
    let response = llm_client
        .chat(messages, None)
        .await
        .map_err(|e| format!("Intent analysis LLM call failed: {}", e))?;

    tracing::debug!(raw_response = %response.content, "LLM raw response");

    let content = strip_markdown_fences(&response.content);

    // Step 5: Parse response
    let analysis = serde_json::from_str::<IntentAnalysis>(&content)
        .map_err(|e| format!("Failed to parse intent analysis JSON: {}", e))?;

    tracing::info!(
        primary_intent = %analysis.primary_intent,
        hidden_intents = analysis.hidden_intents.len(),
        ward = %analysis.ward_recommendation.ward_name,
        approach = %analysis.execution_strategy.approach,
        "Intent analysis complete"
    );

    Ok(analysis)
}

/// Index skills, agents, and wards into memory_facts for semantic search.
/// Uses upsert (save_fact) so this is idempotent — safe to call every session.
async fn index_resources(
    fact_store: &dyn MemoryFactStore,
    skill_service: &SkillService,
    agent_service: &AgentService,
    vault_paths: &SharedVaultPaths,
) {
    // Index skills
    match skill_service.list().await {
        Ok(skills) => {
            tracing::info!(count = skills.len(), "Indexing skills into memory");
            for skill in &skills {
                let key = format!("skill:{}", skill.name);
                let content = format!("{} | {} | category: {}", skill.name, skill.description, skill.category);
                if let Err(e) = fact_store.save_fact("root", "skill", &key, &content, 1.0, None).await {
                    tracing::debug!("Failed to index skill {}: {}", skill.name, e);
                }
            }
        }
        Err(e) => tracing::warn!("Failed to list skills for indexing: {}", e),
    }

    // Index agents
    match agent_service.list().await {
        Ok(agents) => {
            tracing::info!(count = agents.len(), "Indexing agents into memory");
            for agent in &agents {
                let key = format!("agent:{}", agent.id);
                let content = format!("{} | {}", agent.id, agent.description);
                if let Err(e) = fact_store.save_fact("root", "agent", &key, &content, 1.0, None).await {
                    tracing::debug!("Failed to index agent {}: {}", agent.id, e);
                }
            }
        }
        Err(e) => tracing::warn!("Failed to list agents for indexing: {}", e),
    }

    // Index wards
    let wards_dir = vault_paths.wards_dir();
    match std::fs::read_dir(&wards_dir) {
        Ok(entries) => {
            let ward_dirs: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .collect();
            tracing::info!(count = ward_dirs.len(), "Indexing wards into memory");
            for entry in &ward_dirs {
                let name = entry.file_name().to_string_lossy().to_string();
                let agents_md_path = entry.path().join("AGENTS.md");
                let purpose = if agents_md_path.exists() {
                    std::fs::read_to_string(&agents_md_path)
                        .ok()
                        .and_then(|content| {
                            content
                                .lines()
                                .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
                                .map(|l| l.trim().to_string())
                        })
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                let key = format!("ward:{}", name);
                let content = if purpose.is_empty() {
                    name.clone()
                } else {
                    format!("{} | {}", name, purpose)
                };
                if let Err(e) = fact_store.save_fact("root", "ward", &key, &content, 1.0, None).await {
                    tracing::debug!("Failed to index ward {}: {}", name, e);
                }
            }
        }
        Err(e) => tracing::warn!("Failed to read wards directory: {}", e),
    }
}

/// Semantic search result grouped by resource type.
struct SearchResults {
    skills: Vec<Value>,
    agents: Vec<Value>,
    wards: Vec<String>,
}

/// Minimum relevance score to include a result (filters noise).
const MIN_RELEVANCE_SCORE: f64 = 0.15;
/// Maximum skills to send to the LLM.
const MAX_SKILLS: usize = 8;
/// Maximum agents to send to the LLM.
const MAX_AGENTS: usize = 5;
/// Maximum wards to send to the LLM.
const MAX_WARDS: usize = 5;

/// Search memory_facts for resources semantically relevant to the user message.
async fn search_resources(fact_store: &dyn MemoryFactStore, user_message: &str) -> SearchResults {
    let mut skills = Vec::new();
    let mut agents = Vec::new();
    let mut wards = Vec::new();

    // Recall with generous fetch limit, then filter by score and cap per category
    match fact_store.recall_facts("root", user_message, 50).await {
        Ok(result) => {
            if let Some(items) = result.get("results").and_then(|r| r.as_array()) {
                for item in items {
                    let score = item.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
                    if score < MIN_RELEVANCE_SCORE {
                        continue;
                    }

                    let category = item.get("category").and_then(|c| c.as_str()).unwrap_or("");
                    let content = item.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    let key = item.get("key").and_then(|k| k.as_str()).unwrap_or("");

                    match category {
                        "skill" if skills.len() < MAX_SKILLS => {
                            let name = key.strip_prefix("skill:").unwrap_or(key);
                            let parts: Vec<&str> = content.splitn(3, " | ").collect();
                            let desc = parts.get(1).copied().unwrap_or("");
                            skills.push(serde_json::json!({
                                "name": name,
                                "description": desc,
                            }));
                        }
                        "agent" if agents.len() < MAX_AGENTS => {
                            let name = key.strip_prefix("agent:").unwrap_or(key);
                            let parts: Vec<&str> = content.splitn(2, " | ").collect();
                            let desc = parts.get(1).copied().unwrap_or("");
                            agents.push(serde_json::json!({
                                "name": name,
                                "description": desc,
                            }));
                        }
                        "ward" if wards.len() < MAX_WARDS => {
                            wards.push(content.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
        Err(e) => tracing::warn!("Semantic search failed: {}", e),
    }

    tracing::info!(
        skills_above_threshold = skills.len(),
        agents_above_threshold = agents.len(),
        wards_above_threshold = wards.len(),
        min_score = MIN_RELEVANCE_SCORE,
        "Filtered by relevance score"
    );

    SearchResults { skills, agents, wards }
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
            "ward_recommendation": {"action": "use_existing", "ward_name": "scratch", "subdirectory": null, "reason": "test"},
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
            "ward_recommendation": {"action": "use_existing", "ward_name": "scratch", "subdirectory": null, "reason": "test"},
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

        let result = format_user_template("Build a REST API", &skills, &agents, &[]);

        assert!(result.contains("### User Request\nBuild a REST API"));
        assert!(result.contains("- code-gen: Generates code from specs"));
        assert!(result.contains("- testing: Runs unit tests"));
        assert!(result.contains("- coder: Writes production code"));
        assert!(result.contains("### Existing Wards\n(none — all new)"));
    }

    #[test]
    fn test_format_user_template_empty_resources() {
        let result = format_user_template("Hello", &[], &[], &[]);

        assert!(result.contains("### User Request\nHello"));
        assert!(result.contains("### Available Skills\n(none available)"));
        assert!(result.contains("### Available Agents\n(none available)"));
        assert!(result.contains("### Existing Wards\n(none — all new)"));
    }

    #[test]
    fn test_inject_simple_intent() {
        let analysis = IntentAnalysis {
            primary_intent: "greeting".to_string(),
            hidden_intents: vec![],
            recommended_skills: vec![],
            recommended_agents: vec![],
            ward_recommendation: WardRecommendation {
                action: "use_existing".into(),
                ward_name: "scratch".into(),
                subdirectory: None,
                structure: Default::default(),
                reason: "test".into(),
            },
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
            ward_recommendation: WardRecommendation {
                action: "use_existing".into(),
                ward_name: "scratch".into(),
                subdirectory: None,
                structure: Default::default(),
                reason: "test".into(),
            },
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
        assert!(prompt.contains("**Ward**: `scratch`"));
        // Graph tasks get slim injection — no skills, agents, or mermaid
        assert!(prompt.contains("**Approach**: graph (1 nodes)"));
        assert!(prompt.contains("Placeholder specs"));
        assert!(!prompt.contains("```mermaid")); // No graph in slim injection
        assert!(!prompt.contains("Node A")); // No skill-node mapping
        assert!(prompt.contains("**Rewritten request**: Generate code with tests and error handling"));
    }

    // -----------------------------------------------------------------------
    // MockLlmClient, MockFactStore & async tests for analyze_intent
    // -----------------------------------------------------------------------

    use agent_runtime::{ChatResponse, LlmError, StreamCallback};
    use async_trait::async_trait;
    use std::sync::Arc;

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

    /// Create test fixtures: fact store, skill service, agent service, vault paths.
    fn test_fixtures() -> (MockFactStore, SkillService, AgentService, SharedVaultPaths) {
        let tmp = std::env::temp_dir().join("intent_analysis_test");
        let _ = std::fs::create_dir_all(tmp.join("skills"));
        let _ = std::fs::create_dir_all(tmp.join("agents"));
        let _ = std::fs::create_dir_all(tmp.join("wards"));

        let fact_store = MockFactStore;
        let skill_service = SkillService::new(tmp.join("skills"));
        let agent_service = AgentService::new(tmp.join("agents"));
        let vault_paths: SharedVaultPaths = Arc::new(gateway_services::VaultPaths::new(tmp));

        (fact_store, skill_service, agent_service, vault_paths)
    }

    #[tokio::test]
    async fn test_analyze_intent_simple() {
        let mock = MockLlmClient {
            response: r#"{
                "primary_intent": "greeting",
                "hidden_intents": [],
                "recommended_skills": [],
                "recommended_agents": [],
                "ward_recommendation": {"action": "create_new", "ward_name": "test-ward", "subdirectory": null, "reason": "test"},
                "execution_strategy": {
                    "approach": "simple",
                    "explanation": "Simple greeting"
                },
                "rewritten_prompt": "Hello!"
            }"#
            .to_string(),
        };

        let (fact_store, skill_svc, agent_svc, paths) = test_fixtures();
        let result = analyze_intent(&mock, "Hi", &fact_store, &skill_svc, &agent_svc, &paths).await;
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
                "ward_recommendation": {"action": "create_new", "ward_name": "test-ward", "subdirectory": null, "reason": "test"},
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

        let (fact_store, skill_svc, agent_svc, paths) = test_fixtures();
        let result = analyze_intent(&mock, "Write code", &fact_store, &skill_svc, &agent_svc, &paths).await;
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

        let (fact_store, skill_svc, agent_svc, paths) = test_fixtures();
        let result = analyze_intent(&mock, "Hello", &fact_store, &skill_svc, &agent_svc, &paths).await;
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
    "ward_recommendation": {"action": "create_new", "ward_name": "test-ward", "subdirectory": null, "reason": "test"},
    "execution_strategy": {
        "approach": "simple",
        "explanation": "Simple greeting"
    },
    "rewritten_prompt": "Hello!"
}
```"#
            .to_string(),
        };

        let (fact_store, skill_svc, agent_svc, paths) = test_fixtures();
        let result = analyze_intent(&mock, "Hi", &fact_store, &skill_svc, &agent_svc, &paths).await;
        let analysis = result.expect("should strip fences and parse");
        assert_eq!(analysis.primary_intent, "greeting");
        assert_eq!(analysis.rewritten_prompt, "Hello!");
    }
}
