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
- ward_name MUST be a reusable domain category, NEVER task-specific or ticker-specific.
  GOOD: "financial-analysis", "stock-analysis", "market-research", "personal-life", "homework"
  BAD: "amd-stock-analysis", "spy-options-trade", "math-homework-ch5"
  The ward is reused across many tasks in the same domain. Use subdirectory for task-specific paths.
- If an existing ward matches the domain, use action "use_existing" with that ward name.
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
    "ward_name": "domain-level reusable name (e.g. financial-analysis, NOT amd-analysis)",
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
pub fn format_intent_injection(
    analysis: &IntentAnalysis,
    _spec_guidance: Option<&str>,
    original_message: Option<&str>,
) -> String {
    let mut out = String::from("\n\n## Task Analysis\n\n");

    // Original user request — verbatim, unmodified
    if let Some(msg) = original_message {
        out.push_str(&format!("**Original Request:** {}\n", msg));
    }

    // Analyzed goal
    out.push_str(&format!("**Goal:** {}\n", analysis.primary_intent));

    // Hidden requirements — things the user expects but didn't say
    if !analysis.hidden_intents.is_empty() {
        out.push_str("\n**Requirements (implicit):**\n");
        for h in &analysis.hidden_intents {
            out.push_str(&format!("- {}\n", h));
        }
    }

    // Ward
    let wr = &analysis.ward_recommendation;
    out.push_str(&format!(
        "\n**Workspace:** ward `{}` ({}) — {}\n",
        wr.ward_name, wr.action, wr.reason
    ));
    if let Some(ref sub) = wr.subdirectory {
        out.push_str(&format!("  Subdirectory: `{}`\n", sub));
    }

    // Available resources
    if !analysis.recommended_skills.is_empty() || !analysis.recommended_agents.is_empty() {
        out.push_str("\n**Available Resources:**\n");
        for skill in &analysis.recommended_skills {
            out.push_str(&format!("- skill: `{}` (load with load_skill)\n", skill));
        }
        for agent in &analysis.recommended_agents {
            out.push_str(&format!(
                "- agent: `{}` (delegate with delegate_to_agent)\n",
                agent
            ));
        }
    }

    // Execution approach
    let es = &analysis.execution_strategy;
    if es.approach == "graph" {
        out.push_str(&format!(
            "\n**Approach:** Complex task requiring multi-step execution.\n\
             \n**First step:** Delegate to `planner-agent`:\n\
             ```\n\
             delegate_to_agent(agent_id=\"planner-agent\", task=\"Plan this goal: {}. Ward: {}.\")\n\
             ```\n\
             The planner will read the ward, check existing code, and return a structured execution plan.\n\
             Then execute each step from the plan by delegating to the assigned agent.\n",
            analysis.primary_intent, wr.ward_name
        ));
    } else if !es.explanation.is_empty() {
        out.push_str(&format!("\n**Approach:** {}\n", es.explanation));
    }

    // Lightweight ward reminder
    out.push_str(r#"
**Ward Rule:** All file-producing work happens inside the ward. Enter it before delegating. Read AGENTS.md to know what exists — reuse before creating.
"#);

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
        wards
            .iter()
            .map(|w| format!("- {}", w))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "### User Request\n{}\n\n### Available Skills\n{}\n\n### Available Agents\n{}\n\n### Existing Wards\n{}",
        message, skills_list, agents_list, wards_list
    )
}

// ---------------------------------------------------------------------------
// analyze_intent
// ---------------------------------------------------------------------------

/// Check if a message is trivially simple (greeting, short question, etc.)
/// and doesn't need a full LLM intent analysis call.
fn is_simple_message(message: &str) -> bool {
    let trimmed = message.trim();
    let word_count = trimmed.split_whitespace().count();

    // Common greetings and simple phrases — must match exactly or start with
    let simple_patterns = [
        "hello",
        "hi",
        "hey",
        "good morning",
        "good afternoon",
        "good evening",
        "thanks",
        "thank you",
        "bye",
        "goodbye",
        "what's up",
        "how are you",
        "help",
        "what can you do",
        "who are you",
    ];
    let lower = trimmed.to_lowercase();
    for pattern in &simple_patterns {
        if lower == *pattern || (lower.starts_with(pattern) && word_count <= 4) {
            return true;
        }
    }

    false
}

/// Build a default "simple" intent analysis for trivial messages.
fn simple_analysis(message: &str) -> IntentAnalysis {
    IntentAnalysis {
        primary_intent: message.chars().take(100).collect(),
        hidden_intents: vec![],
        recommended_skills: vec![],
        recommended_agents: vec![],
        ward_recommendation: WardRecommendation {
            action: "use_existing".to_string(),
            ward_name: "general".to_string(),
            subdirectory: None,
            structure: std::collections::HashMap::new(),
            reason: "Simple request — no ward needed".to_string(),
        },
        execution_strategy: ExecutionStrategy {
            approach: "simple".to_string(),
            graph: None,
            explanation: "Short/simple message — skipped LLM analysis".to_string(),
        },
        rewritten_prompt: String::new(),
    }
}

/// Analyze user intent: searches semantically for resources, calls LLM.
///
/// Resource indexing must happen before this call (see `index_resources`).
///
/// Short/trivial messages (greetings, 1-3 word phrases) skip the LLM call
/// entirely and return a default "simple" analysis to avoid 5-30s latency.
pub async fn analyze_intent(
    llm_client: &dyn LlmClient,
    user_message: &str,
    fact_store: &dyn MemoryFactStore,
    memory_recall: Option<&crate::recall::MemoryRecall>,
) -> Result<IntentAnalysis, String> {
    // Fast path: skip LLM for trivial messages
    if is_simple_message(user_message) {
        tracing::info!(
            message = user_message,
            "Skipping intent analysis — trivial message"
        );
        return Ok(simple_analysis(user_message));
    }

    tracing::info!("Starting intent analysis for root session");

    // Step 0: Query memory for relevant past context via the unified recall pool.
    // Intent analysis runs at root level before a specific agent is selected, so
    // we use "root" as the agent_id. No ward is available at this site yet.
    let memory_context = if let Some(recall) = memory_recall {
        match recall
            .recall_unified("root", user_message, None, &[], 10)
            .await
        {
            Ok(items) if !items.is_empty() => {
                let formatted = crate::recall::format_scored_items(&items);
                tracing::info!(
                    count = items.len(),
                    "Recalled unified context for intent analysis"
                );
                formatted
            }
            Ok(_) => String::new(),
            Err(e) => {
                tracing::warn!("Intent-analysis unified recall failed: {}", e);
                String::new()
            }
        }
    } else {
        String::new()
    };

    // Step 0b: Recall proven procedures that match the user's request
    let mut procedure_context = String::new();
    if let Some(recall) = memory_recall {
        if let Ok(procedures) = recall
            .recall_procedures(user_message, "root", None, 3)
            .await
        {
            for (proc, score) in &procedures {
                if *score > 0.7 && proc.success_count >= 2 {
                    let total = (proc.success_count + proc.failure_count).max(1) as f64;
                    let success_rate = proc.success_count as f64 / total;
                    procedure_context = format!(
                        "\n## Proven Procedure Available: {}\n{}\nSteps: {}\nSuccess rate: {:.0}% ({} uses)\n",
                        proc.name, proc.description, proc.steps,
                        success_rate * 100.0, proc.success_count,
                    );
                    break; // Only use the top match
                }
            }
        }
    }

    // Combine memory context with procedure context
    let mut memory_context = memory_context;
    if !procedure_context.is_empty() {
        memory_context = format!("{}\n{}", memory_context, procedure_context);
    }

    if !memory_context.is_empty() {
        tracing::info!(
            memory_context_len = memory_context.len(),
            "Memory context retrieved for intent analysis"
        );
    } else {
        tracing::debug!(
            "No memory context for intent analysis (recall returned empty or unavailable)"
        );
    }

    // Step 1: Semantic search for relevant resources
    let results = search_resources(fact_store, user_message).await;

    tracing::info!(
        skills_matched = results.skills.len(),
        agents_matched = results.agents.len(),
        wards_matched = results.wards.len(),
        "Semantic search complete"
    );

    // Step 2: Build LLM prompt with only relevant resources
    let user_template = format_user_template(
        user_message,
        &results.skills,
        &results.agents,
        &results.wards,
    );

    // Prepend memory context to user message if available
    let user_content = if memory_context.is_empty() {
        user_template
    } else {
        format!("{}\n\n{}", memory_context, user_template)
    };

    let messages = vec![
        ChatMessage::system(INTENT_ANALYSIS_PROMPT.to_string()),
        ChatMessage::user(user_content),
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

/// Count filesystem resources (skills + agents + wards) to check staleness.
async fn count_filesystem_resources(
    skill_service: &SkillService,
    agent_service: &AgentService,
    vault_paths: &SharedVaultPaths,
) -> usize {
    let skill_count = skill_service.list().await.map(|s| s.len()).unwrap_or(0);
    let agent_count = agent_service.list().await.map(|a| a.len()).unwrap_or(0);
    let ward_count = std::fs::read_dir(vault_paths.wards_dir())
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count()
        })
        .unwrap_or(0);
    skill_count + agent_count + ward_count
}

/// Index skills, agents, and wards into memory_facts for semantic search.
/// Uses upsert (save_fact) so this is idempotent — safe to call every session.
///
/// Skips re-indexing if the DB fact count matches the filesystem resource count
/// (skills + agents + wards). This avoids N embedding+upsert calls per session
/// when nothing has changed.
pub async fn index_resources(
    fact_store: &dyn MemoryFactStore,
    skill_service: &SkillService,
    agent_service: &AgentService,
    vault_paths: &SharedVaultPaths,
) {
    // Quick staleness check: compare filesystem count vs last indexed count
    let fs_count = count_filesystem_resources(skill_service, agent_service, vault_paths).await;

    let temp_dir = vault_paths.vault_dir().join("temp");
    let index_marker = temp_dir.join(".resource_index_count");
    let last_count: usize = std::fs::read_to_string(&index_marker)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    if last_count == fs_count && fs_count > 0 {
        tracing::info!(
            fs_count = fs_count,
            last_indexed = last_count,
            "Resource index up-to-date, skipping re-index"
        );
        return;
    }

    tracing::info!(
        fs_count = fs_count,
        last_indexed = last_count,
        "Resource index stale, re-indexing"
    );

    // Index skills
    match skill_service.list().await {
        Ok(skills) => {
            tracing::info!(count = skills.len(), "Indexing skills into memory");
            for skill in &skills {
                let key = format!("skill:{}", skill.name);
                let content = format!(
                    "{} | {} | category: {}",
                    skill.name, skill.description, skill.category
                );
                if let Err(e) = fact_store
                    .save_fact("root", "skill", &key, &content, 1.0, None)
                    .await
                {
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
                if let Err(e) = fact_store
                    .save_fact("root", "agent", &key, &content, 1.0, None)
                    .await
                {
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
                if let Err(e) = fact_store
                    .save_fact("root", "ward", &key, &content, 1.0, None)
                    .await
                {
                    tracing::debug!("Failed to index ward {}: {}", name, e);
                }
            }
        }
        Err(e) => tracing::warn!("Failed to read wards directory: {}", e),
    }

    // Write index marker so next session can skip if unchanged
    let _ = std::fs::create_dir_all(&temp_dir);
    let _ = std::fs::write(&index_marker, fs_count.to_string());
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

    SearchResults {
        skills,
        agents,
        wards,
    }
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
        let agents = vec![json!({"name": "coder", "description": "Writes production code"})];

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
            Ok(serde_json::json!({"results": [], "count": 0}))
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
        let result = analyze_intent(
            &mock,
            "Tell me about the weather forecast for tomorrow",
            &fact_store,
            None,
        )
        .await;
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
        let result = analyze_intent(&mock, "Write code", &fact_store, None).await;
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
        let result = analyze_intent(
            &mock,
            "Build me a web scraper for news articles",
            &fact_store,
            None,
        )
        .await;
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
        let result = analyze_intent(
            &mock,
            "Analyze this dataset and create visualizations",
            &fact_store,
            None,
        )
        .await;
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

        let injection = format_intent_injection(&analysis, None, None);
        assert!(injection.contains("## Task Analysis"));
        assert!(injection.contains("financial-analysis"));
        assert!(injection.contains("stocks/spy"));
        assert!(injection.contains("coding"));
        assert!(injection.contains("code-agent"));
        assert!(injection.contains("Ward Rule:"));
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

        let injection = format_intent_injection(&analysis, None, None);
        assert!(injection.contains("Ward Rule:"));
        assert!(injection.contains("test-ward"));
    }

    #[test]
    fn test_format_intent_injection_spec_guidance_ignored() {
        // spec_guidance is no longer injected — root decides its own approach
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

        let injection =
            format_intent_injection(&analysis, Some("Cover data sources and rate limits"), None);
        // Should still produce valid injection without spec guidance section
        assert!(injection.contains("## Task Analysis"));
        assert!(injection.contains("test-ward"));
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

        let injection = format_intent_injection(&analysis, None, None);
        assert!(!injection.contains("Domain Spec Guidance:"));
    }
}
