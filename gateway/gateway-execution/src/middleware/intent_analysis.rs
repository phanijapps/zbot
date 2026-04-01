use agent_runtime::{ChatMessage, LlmClient};
use gateway_services::{AgentService, SharedVaultPaths, SkillService};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zero_core::MemoryFactStore;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentAnalysis {
    pub primary_intent: String,
    pub hidden_intents: Vec<String>,
    pub recommended_skills: Vec<String>,
    pub recommended_agents: Vec<String>,
    pub ward_recommendation: WardRecommendation,
    pub execution_strategy: ExecutionStrategy,
    /// Kept for backward compat with existing logs; no longer requested from LLM.
    #[serde(default)]
    pub rewritten_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStrategy {
    pub approach: String,
    pub graph: Option<ExecutionGraph>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// Kept for backward compat with existing logs; no longer requested from LLM.
    /// Will be derived from nodes/edges in code when UI needs it.
    #[serde(default)]
    pub mermaid: Option<String>,
    #[serde(default)]
    pub max_cycles: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub task: String,
    pub agent: String,
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCondition {
    pub when: String,
    pub to: String,
}

// ---------------------------------------------------------------------------
// LLM prompt
// ---------------------------------------------------------------------------

const INTENT_ANALYSIS_PROMPT: &str = r#"You are an intent analyzer. Given a user request and available resources, determine intent, ward, and execution approach.

## Rules
- Hidden intents: actionable instructions the user didn't state but expects. Not labels.
- Skills and agents are DIFFERENT. Skills = load_skill(). Agents = delegate_to_agent(). Never mix them.
- recommended_skills: from the "Relevant Skills" list only.
- recommended_agents: from the "Relevant Agents" list or "root" only. Never put skill names as agents.
- Wards are domain-level workspaces (e.g., "financial-analysis"), not task-specific. Reuse existing wards.
- approach "simple" for greetings, quick questions, single-step tasks.
- approach "graph" when the task needs multiple agents, code, or multi-step orchestration.
- When approach is "graph", ALWAYS include "coding" in recommended_skills — it provides the ward structure and task runner.

## Output Format
Respond with ONLY a JSON object (no markdown fences):
{
  "primary_intent": "string",
  "hidden_intents": ["actionable instruction for each hidden intent"],
  "recommended_skills": ["skill-name"],
  "recommended_agents": ["agent-name"],
  "ward_recommendation": {
    "action": "use_existing | create_new",
    "ward_name": "domain-level name",
    "subdirectory": "task-specific subdir or null",
    "reason": "why"
  },
  "execution_strategy": {
    "approach": "simple | graph",
    "explanation": "one sentence — why this approach"
  }
}"#;

// ---------------------------------------------------------------------------
// format_intent_injection — appended to agent instructions so the agent
// can follow the intent analysis recommendations.
// ---------------------------------------------------------------------------

/// Format an `IntentAnalysis` as a markdown section suitable for appending
/// to the agent's system prompt / instructions.
///
/// `spec_guidance` is optional domain-specific guidance for writing specs
/// (e.g., "Cover data sources and rate limits"). When provided, it is appended
/// after the ward rules section.
pub fn format_intent_injection(analysis: &IntentAnalysis, spec_guidance: Option<&str>) -> String {
    let mut out = String::from("\n\n## Intent Analysis\n\n");

    out.push_str(&format!("**Primary Intent:** {}\n", analysis.primary_intent));

    if !analysis.hidden_intents.is_empty() {
        out.push_str("**Hidden Intents:**\n");
        for h in &analysis.hidden_intents {
            out.push_str(&format!("- {}\n", h));
        }
    }

    // Ward recommendation — the agent MUST use this ward name
    let wr = &analysis.ward_recommendation;
    out.push_str(&format!(
        "\n**Ward:** {} ({}) — {}\n",
        wr.ward_name, wr.action, wr.reason
    ));
    if let Some(ref sub) = wr.subdirectory {
        out.push_str(&format!("  Subdirectory: {}\n", sub));
    }

    if !analysis.recommended_skills.is_empty() {
        out.push_str(&format!(
            "\n**Recommended Skills:** {}\n",
            analysis.recommended_skills.join(", ")
        ));
    }
    if !analysis.recommended_agents.is_empty() {
        out.push_str(&format!(
            "**Recommended Agents:** {}\n",
            analysis.recommended_agents.join(", ")
        ));
    }

    // Execution approach
    let es = &analysis.execution_strategy;
    out.push_str(&format!("\n**Execution Approach:** {}\n", es.approach));
    if !es.explanation.is_empty() {
        out.push_str(&format!("{}\n", es.explanation));
    }

    // SDLC pattern for graph approach — root designs and executes the graph
    if es.approach == "graph" {
        out.push_str(r#"
## Execution Plan — SDLC Pattern

You are the orchestrator. Execute this pipeline:

### Phase 1: Specs (YOU — do not delegate)
Write one spec per module in specs/<subdirectory>/. One apply_patch per spec. Under 3KB each.

### Phase 2: Tasks.json (YOU — do not delegate)
After specs, create `specs/<subdirectory>/tasks.json` — an ordered task list for the code-agent.
Each task has: id, action (create/run/verify), file or command, spec_ref, depends_on, status (pending).
Core module creates come FIRST (no dependencies). Task scripts depend on core modules. Run/verify depend on creates.
Every task MUST have: id, action, description, acceptance criteria, spec_ref, depends_on, status.
Example:
```json
{"tasks":[
  {"id":1,"action":"create","file":"core/options.py","description":"Options chain utilities — IV calc, chain parsing","spec_ref":"03-options.md#core-module-candidates","acceptance":"Exports: calculate_iv(chain,price)->float, parse_chain(raw)->dict. Importable.","depends_on":[],"status":"pending"},
  {"id":2,"action":"create","file":"stocks/amd/collect.py","description":"AMD data collection — imports core.data_fetcher, core.options","spec_ref":"01-data.md","acceptance":"Creates: ohlcv.csv (200+ rows), fundamentals.json, options_chain.json","depends_on":[1],"status":"pending"},
  {"id":3,"action":"run","command":"python3 stocks/amd/collect.py","description":"Execute data collection","acceptance":"Exit 0, all data files created","depends_on":[2],"status":"pending"},
  {"id":4,"action":"verify","command":"ls -la stocks/amd/data/","description":"Verify outputs","acceptance":"ohlcv.csv, fundamentals.json, options_chain.json exist, non-zero","depends_on":[3],"status":"pending"}
]}
```

### Phase 3: Coding (delegate to code-agent)
Delegate with: "Process tasks.json at specs/<subdirectory>/tasks.json using ralph.py"
The code-agent uses `ralph.py` (at ward root) to get next task, execute it, mark complete/fail.

### Phase 4: Review (delegate to code-agent, skills: [code-review])
Review code against specs. Expects RESULT: APPROVED or DEFECTS.

### Phase 5: Validation (delegate to data-analyst/research-agent, skills: [domain-validation])
Run code, validate output. Expects RESULT: APPROVED or DEFECTS.

### Phase 6: Output (delegate or do yourself)
Produce final deliverable.

If review/validation returns DEFECTS, re-delegate to coding with the defect list.

### Delegation
- Phase 3 delegation MUST say: "Process tasks.json at specs/<subdirectory>/tasks.json using ralph.py"
- Do NOT write custom task descriptions — ralph.py + tasks.json IS the task.
- Do NOT set max_iterations — default 1000 is correct. System auto-kills stuck agents.
- Pass skills in the `skills` parameter.
- Subagent has ward CWD, AGENTS.md, and spec content pre-loaded. Do NOT tell it to call ward(use).

### After a crash callback
- Read the TASK RUNNER STATUS in the crash report
- Re-delegate with: "Continue processing specs/<subdirectory>/tasks.json using ralph.py"
- Do NOT code the remaining tasks yourself. Always re-delegate.

### Discipline
- Do NOT call list_skills or list_agents.
- Update plan ONLY at phase transitions.
- Do NOT poll with shell. System sends callback automatically. Stop and wait.
"#);
    }

    // Ward rules — always included regardless of approach
    out.push_str(r#"
**Ward Rule:** ALL code must be written inside a ward. If you need to write code:
1. Enter the recommended ward (or create if new)
2. Read AGENTS.md to understand what exists in core/
3. Check if existing core/ modules already solve your need — reuse, don't recreate
4. If new functionality: write a spec first, then implement
5. After implementing: archive spec to specs/archive/

**Spec Lifecycle:**
- Active specs: `specs/<task-name>/<nn>-<module>.md`
- Archived specs: `specs/archive/<task-name>/`
- Path uses the task subdirectory name, NOT the ward name

**Spec Structure:**
One spec per functional unit. Never one giant file.
Example for a task named "my-analysis" with data collection, processing, and output:
- `specs/my-analysis/01-data-collection.md`
- `specs/my-analysis/02-processing.md`
- `specs/my-analysis/03-output.md`
Each spec under 3KB. If it's growing large, split it.

**Spec Quality — MANDATORY sections (all 8 required):**

1. **Purpose**: One sentence — what and why.
2. **Inputs**: Exact sources with schema, types, expected volume.
3. **Outputs**: Exact file path, format, full schema with types.
4. **Algorithm**: Step-by-step logic with formulas — not just library/function names.
5. **Dependencies**: Which core/ modules to import (with signatures), external packages.
6. **Error handling**: What happens on missing data, API failure, invalid values.
7. **Validation**: How to verify correctness — expected ranges, spot-check methods.
8. **Core module candidates**: What belongs in core/ (reusable) vs task-specific.

Do NOT start implementation until all specs have all 8 sections.
"#);

    if let Some(guidance) = spec_guidance {
        out.push_str(&format!("\n**Domain Spec Guidance:**\n{}\n", guidance));
    }

    out
}

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
// analyze_intent
// ---------------------------------------------------------------------------

/// Analyze user intent: searches semantically for resources, calls LLM.
///
/// Resource indexing must happen before this call (see `index_resources`).
pub async fn analyze_intent(
    llm_client: &dyn LlmClient,
    user_message: &str,
    fact_store: &dyn MemoryFactStore,
) -> Result<IntentAnalysis, String> {
    tracing::info!("Starting intent analysis for root session");

    // Step 1: Semantic search for relevant resources
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
    let analysis: IntentAnalysis = serde_json::from_str(&content)
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
pub async fn index_resources(
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
            }
        }"#;

        let analysis: IntentAnalysis = serde_json::from_str(json).unwrap();
        assert_eq!(analysis.primary_intent, "greeting");
        assert!(analysis.hidden_intents.is_empty());
        assert!(analysis.recommended_skills.is_empty());
        assert!(analysis.recommended_agents.is_empty());
        assert_eq!(analysis.execution_strategy.approach, "simple");
        assert!(analysis.execution_strategy.graph.is_none());
        // rewritten_prompt defaults to empty when not present
        assert!(analysis.rewritten_prompt.is_empty());
    }

    /// Old logs with rewritten_prompt, structure, mermaid still deserialize (backward compat).
    #[test]
    fn test_backward_compat_old_format() {
        let json = r#"{
            "primary_intent": "code_generation",
            "hidden_intents": [],
            "recommended_skills": [],
            "recommended_agents": [],
            "ward_recommendation": {
                "action": "create_new", "ward_name": "test",
                "subdirectory": null, "reason": "test",
                "structure": {"core/": "shared modules"}
            },
            "execution_strategy": {
                "approach": "graph",
                "graph": {
                    "nodes": [{"id": "A", "task": "do it", "agent": "root", "skills": []}],
                    "edges": [{"from": "A", "to": "END"}],
                    "mermaid": "graph TD\nA-->END",
                    "max_cycles": 2
                },
                "explanation": "test"
            },
            "rewritten_prompt": "old rewritten prompt"
        }"#;

        let analysis: IntentAnalysis = serde_json::from_str(json).unwrap();
        assert_eq!(analysis.rewritten_prompt, "old rewritten prompt");
        assert_eq!(analysis.ward_recommendation.structure.len(), 1);
        let graph = analysis.execution_strategy.graph.unwrap();
        assert_eq!(graph.mermaid, Some("graph TD\nA-->END".to_string()));
        assert_eq!(graph.max_cycles, Some(2));
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
                    ]
                },
                "explanation": "Generate, review, fix loop with max 3 cycles"
            }
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
        assert!(graph.mermaid.is_none());
        assert!(graph.max_cycles.is_none());

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
                }
            }"#
            .to_string(),
        };

        let fact_store = MockFactStore;
        let result = analyze_intent(&mock, "Hi", &fact_store).await;
        let analysis = result.expect("should parse simple intent");
        assert_eq!(analysis.primary_intent, "greeting");
        assert_eq!(analysis.execution_strategy.approach, "simple");
        assert!(analysis.execution_strategy.graph.is_none());
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
                        ]
                    },
                    "explanation": "Generate then done"
                }
            }"#
            .to_string(),
        };

        let fact_store = MockFactStore;
        let result = analyze_intent(&mock, "Write code", &fact_store).await;
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
    }

    #[tokio::test]
    async fn test_analyze_intent_malformed_json() {
        let mock = MockLlmClient {
            response: "This is not valid JSON at all.".to_string(),
        };

        let fact_store = MockFactStore;
        let result = analyze_intent(&mock, "Hello", &fact_store).await;
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
    }
}
```"#
            .to_string(),
        };

        let fact_store = MockFactStore;
        let result = analyze_intent(&mock, "Hi", &fact_store).await;
        let analysis = result.expect("should strip fences and parse");
        assert_eq!(analysis.primary_intent, "greeting");
    }

    #[test]
    fn test_format_intent_injection() {
        let analysis = IntentAnalysis {
            primary_intent: "financial_analysis".to_string(),
            hidden_intents: vec!["Save results to output/".to_string()],
            recommended_skills: vec!["coding".to_string(), "web-search".to_string()],
            recommended_agents: vec!["code-agent".to_string()],
            ward_recommendation: WardRecommendation {
                action: "create_new".to_string(),
                ward_name: "financial-analysis".to_string(),
                subdirectory: Some("stocks/spy".to_string()),
                structure: Default::default(),
                reason: "Domain-level ward for all financial work".to_string(),
            },
            execution_strategy: ExecutionStrategy {
                approach: "graph".to_string(),
                graph: None,
                explanation: "Research then analyze".to_string(),
            },
            rewritten_prompt: String::new(),
        };

        let injection = format_intent_injection(&analysis, None);
        assert!(injection.contains("## Intent Analysis"));
        assert!(injection.contains("**Ward:** financial-analysis (create_new)"));
        assert!(injection.contains("Subdirectory: stocks/spy"));
        assert!(injection.contains("coding, web-search"));
        assert!(injection.contains("code-agent"));
        assert!(injection.contains("Research then analyze"));
        assert!(injection.contains("Ward Rule:"));
        assert!(injection.contains("Spec Lifecycle:"));
        assert!(injection.contains("Spec Quality"));
    }

    #[test]
    fn test_format_intent_injection_includes_ward_rules() {
        let analysis = IntentAnalysis {
            primary_intent: "code_generation".to_string(),
            hidden_intents: vec![],
            recommended_skills: vec![],
            recommended_agents: vec![],
            ward_recommendation: WardRecommendation {
                action: "create_new".to_string(),
                ward_name: "test-ward".to_string(),
                subdirectory: None,
                structure: Default::default(),
                reason: "test".to_string(),
            },
            execution_strategy: ExecutionStrategy {
                approach: "graph".to_string(),
                graph: None,
                explanation: "test".to_string(),
            },
            rewritten_prompt: String::new(),
        };

        let injection = format_intent_injection(&analysis, None);
        assert!(injection.contains("Ward Rule:"));
        assert!(injection.contains("Spec Lifecycle:"));
        assert!(injection.contains("Spec Quality"));
    }

    #[test]
    fn test_format_intent_injection_includes_spec_guidance_when_provided() {
        let analysis = IntentAnalysis {
            primary_intent: "code_generation".to_string(),
            hidden_intents: vec![],
            recommended_skills: vec![],
            recommended_agents: vec![],
            ward_recommendation: WardRecommendation {
                action: "create_new".to_string(),
                ward_name: "test-ward".to_string(),
                subdirectory: None,
                structure: Default::default(),
                reason: "test".to_string(),
            },
            execution_strategy: ExecutionStrategy {
                approach: "graph".to_string(),
                graph: None,
                explanation: "test".to_string(),
            },
            rewritten_prompt: String::new(),
        };

        let injection = format_intent_injection(&analysis, Some("Cover data sources and rate limits"));
        assert!(injection.contains("Domain Spec Guidance:"));
        assert!(injection.contains("Cover data sources"));
    }

    #[test]
    fn test_format_intent_injection_omits_spec_guidance_when_none() {
        let analysis = IntentAnalysis {
            primary_intent: "code_generation".to_string(),
            hidden_intents: vec![],
            recommended_skills: vec![],
            recommended_agents: vec![],
            ward_recommendation: WardRecommendation {
                action: "create_new".to_string(),
                ward_name: "test-ward".to_string(),
                subdirectory: None,
                structure: Default::default(),
                reason: "test".to_string(),
            },
            execution_strategy: ExecutionStrategy {
                approach: "graph".to_string(),
                graph: None,
                explanation: "test".to_string(),
            },
            rewritten_prompt: String::new(),
        };

        let injection = format_intent_injection(&analysis, None);
        assert!(!injection.contains("Domain Spec Guidance:"));
    }
}
