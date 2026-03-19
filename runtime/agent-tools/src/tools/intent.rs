// ============================================================================
// INTENT ANALYSIS TOOL
// Analyzes user requests to expand intent and discover relevant resources
// Uses LLM-powered analysis for intelligent intent detection
// ============================================================================

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use agent_runtime::llm::client::LlmClient;
use agent_runtime::types::ChatMessage;
use zero_core::{FileSystemContext, KnowledgeGraphStore, MemoryFactStore, Result, Tool, ToolContext, ToolPermissions, ZeroError};

use crate::tools::indexer::skill::scan_skills_dir;
use crate::tools::indexer::agent::scan_agents_dir;

pub struct AnalyzeIntentTool {
    fs: Arc<dyn FileSystemContext>,
    fact_store: Option<Arc<dyn MemoryFactStore>>,
    graph_store: Option<Arc<dyn KnowledgeGraphStore>>,
    llm_client: Option<Arc<dyn LlmClient>>,
}

impl AnalyzeIntentTool {
    pub fn new(fs: Arc<dyn FileSystemContext>) -> Self {
        Self { fs, fact_store: None, graph_store: None, llm_client: None }
    }

    /// Create with optional memory fact store for semantic search
    pub fn with_fact_store(mut self, fact_store: Arc<dyn MemoryFactStore>) -> Self {
        self.fact_store = Some(fact_store);
        self
    }

    /// Create with optional knowledge graph store for index validation
    pub fn with_graph_store(mut self, graph_store: Arc<dyn KnowledgeGraphStore>) -> Self {
        self.graph_store = Some(graph_store);
        self
    }

    /// Create with optional LLM client for intelligent intent analysis
    pub fn with_llm_client(mut self, llm_client: Arc<dyn LlmClient>) -> Self {
        self.llm_client = Some(llm_client);
        self
    }
}

#[async_trait]
impl Tool for AnalyzeIntentTool {
    fn name(&self) -> &str {
        "analyze_intent"
    }

    fn description(&self) -> &str {
        "Analyze user request to discover hidden intents, find relevant resources (skills, agents, wards), \
         and recommend execution strategy. Call this FIRST for non-trivial requests. \
         Returns recommendations - the agent then decides whether to load skills or delegate."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The user message to analyze"
                }
            },
            "required": ["message"]
        }))
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions::safe()
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let message = args.get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("message required".to_string()))?;

        // Auto-index if needed (lazy initialization)
        if self.needs_reindex(&ctx).await {
            tracing::info!("Auto-indexing resources (index out of sync or empty)");
            self.trigger_reindex(&ctx).await;
        }

        // Get agent's configured skills from state (set by executor for subagents)
        let configured_skills: HashSet<String> = ctx.get_state("agent_configured_skills")
            .and_then(|v| {
                let arr = v.as_array()?;
                Some(arr.iter().filter_map(|s| s.as_str().map(|s| s.to_string())).collect())
            })
            .unwrap_or_default();

        // Get ALL available resources (not pre-filtered)
        let all_skills = self.discover_skills_cached(&ctx)?;
        let all_agents = self.discover_agents_cached(&ctx)?;
        let all_wards = self.discover_wards()?;

        // ============================================================================
        // LLM IS PRIMARY - Use it if available
        // ============================================================================
        if let Some(ref llm_client) = self.llm_client {
            // Convert to Value arrays for LLM analysis
            let skills_for_llm: Vec<Value> = all_skills.iter()
                .map(|s| json!({
                    "name": s.name,
                    "description": s.description,
                    "trigger_keywords": s.trigger_keywords,
                    "domain_hints": s.domain_hints,
                    "configured": configured_skills.contains(&s.name)
                }))
                .collect();

            let agents_for_llm: Vec<Value> = all_agents.iter()
                .map(|a| json!({
                    "agent_id": a.agent_id,
                    "description": a.description
                }))
                .collect();

            match self.llm_analyze_full(llm_client, message, &skills_for_llm, &agents_for_llm).await {
                Ok(llm_plan) => {
                    tracing::info!(
                        primary_intent = %llm_plan.primary_intent,
                        skills = ?llm_plan.recommended_skills,
                        agents = ?llm_plan.recommended_agents,
                        "LLM intent analysis complete - using LLM plan"
                    );

                    // Build execution steps from LLM recommendations
                    let execution_steps = self.build_execution_steps_from_llm(&llm_plan, &all_skills, &all_agents);

                    // Build required first action from LLM recommendations
                    let required_action = self.build_required_action_from_llm(&llm_plan, &execution_steps);

                    // Convert ward recommendation
                    let ward_rec = if let Some(ref ward_name) = llm_plan.suggested_ward {
                        json!({
                            "action": "create_generic",
                            "ward_name": ward_name,
                            "reason": "LLM-recommended reusable ward for this domain",
                            "source": "llm"
                        })
                    } else {
                        json!({
                            "action": "use_scratch",
                            "ward_name": "scratch",
                            "reason": "No specific ward recommended"
                        })
                    };

                    let result = json!({
                        "primary_intent": llm_plan.primary_intent,
                        "domain": llm_plan.domain,
                        "explicit_goals": llm_plan.explicit_goals,
                        "implicit_goals": llm_plan.implicit_goals,
                        "hidden_intents": llm_plan.hidden_intents,
                        "rewritten_prompt": llm_plan.rewritten_prompt,
                        "discovered_resources": {
                            "skills": self.skills_to_values(&llm_plan.recommended_skills, &all_skills, &configured_skills),
                            "agents": self.agents_to_values(&llm_plan.recommended_agents, &all_agents),
                            "wards": self.wards_to_values(&all_wards)
                        },
                        "ward_recommendation": ward_rec,
                        "execution_plan": {
                            "strategy": llm_plan.execution_strategy,
                            "use_execution_graph": llm_plan.use_execution_graph,
                            "required_first_action": required_action,
                            "execution_steps": execution_steps,
                            "complexity": if llm_plan.use_execution_graph { "high" } else { &llm_plan.execution_strategy }
                        },
                        "llm_analysis": {
                            "entities": llm_plan.entities,
                            "reasoning": llm_plan.reasoning
                        },
                        "analysis_source": "llm"
                    });

                    return Ok(result);
                }
                Err(e) => {
                    tracing::warn!("LLM analysis failed, falling back to heuristics: {}", e);
                }
            }
        }

        // ============================================================================
        // FALLBACK: Heuristic analysis (only when no LLM or LLM failed)
        // ============================================================================
        tracing::info!("Using heuristic intent analysis (LLM not available or failed)");
        self.heuristic_analysis(message, &all_skills, &all_agents, &all_wards, &configured_skills).await
    }
}

// ============================================================================
// HELPER METHODS FOR ANALYZE INTENT TOOL
// ============================================================================

impl AnalyzeIntentTool {
    /// Full LLM analysis - receives ALL available resources, returns complete plan
    async fn llm_analyze_full(
        &self,
        llm_client: &Arc<dyn LlmClient>,
        message: &str,
        all_skills: &[Value],
        all_agents: &[Value],
    ) -> Result<LlmIntentAnalysis> {
        // Build context about ALL available resources
        let skills_info: Vec<String> = all_skills
            .iter()
            .filter_map(|s| {
                let name = s.get("name").and_then(|n| n.as_str())?;
                let desc = s.get("description").and_then(|d| d.as_str()).unwrap_or("");
                let configured = s.get("configured").and_then(|c| c.as_bool()).unwrap_or(false);
                Some(if configured {
                    format!("- {} [CONFIGURED]: {}", name, desc)
                } else {
                    format!("- {}: {}", name, desc)
                })
            })
            .collect();

        let agents_info: Vec<String> = all_agents
            .iter()
            .filter_map(|a| {
                let id = a.get("agent_id").and_then(|i| i.as_str())?;
                let desc = a.get("description").and_then(|d| d.as_str()).unwrap_or("");
                Some(format!("- {}: {}", id, desc))
            })
            .collect();

        let user_prompt = format!(
            r#"## User Request
{}

## ALL Available Skills
{}

## ALL Available Agents
{}

Analyze the user's request and return JSON with your recommendations.
- Review ALL skills and recommend the most relevant ones
- Review ALL agents and recommend which should handle parts of this task
- Identify hidden intents the user might not have explicitly stated
- Suggest a generic, reusable ward name if this task would benefit from persistent state"#,
            message,
            if skills_info.is_empty() { "No skills available".to_string() } else { skills_info.join("\n") },
            if agents_info.is_empty() { "No agents available".to_string() } else { agents_info.join("\n") }
        );

        let messages = vec![
            ChatMessage::system(Self::INTENT_ANALYSIS_PROMPT.to_string()),
            ChatMessage::user(user_prompt),
        ];

        match llm_client.chat(messages, None).await {
            Ok(response) => {
                let content = response.content.trim();
                let json_str = extract_json_from_content(content);

                match serde_json::from_str::<LlmIntentAnalysis>(&json_str) {
                    Ok(analysis) => Ok(analysis),
                    Err(e) => {
                        tracing::debug!("LLM response was: {}", json_str);
                        Err(ZeroError::Tool(format!("Failed to parse LLM intent analysis: {}", e)))
                    }
                }
            }
            Err(e) => Err(ZeroError::Tool(format!("LLM call failed: {}", e)))
        }
    }

    /// Build execution steps from LLM recommendations
    fn build_execution_steps_from_llm(
        &self,
        llm_plan: &LlmIntentAnalysis,
        all_skills: &[DiscoveredSkill],
        all_agents: &[DiscoveredAgent],
    ) -> Vec<Value> {
        let mut steps = Vec::new();

        // Step 1: Load recommended skills
        if !llm_plan.recommended_skills.is_empty() {
            steps.push(json!({
                "step": 1,
                "action": "load_skills",
                "skills": llm_plan.recommended_skills,
                "reason": "Skills recommended by intent analysis"
            }));
        }

        // Step 2: Delegate to recommended agents
        for (idx, agent_id) in llm_plan.recommended_agents.iter().enumerate() {
            // Find the agent description
            let agent_desc = all_agents.iter()
                .find(|a| &a.agent_id == agent_id)
                .map(|a| a.description.as_str())
                .unwrap_or("Complete assigned task");

            steps.push(json!({
                "step": 2 + idx,
                "action": "delegate",
                "agent_id": agent_id,
                "task": format!("Handle aspect of: {}", llm_plan.rewritten_prompt),
                "description": agent_desc,
                "wait_for_result": true,
                "output_file": format!("outputs/{}_output.md", agent_id)
            }));
        }

        // Step 3: Execute primary task if not delegated
        if llm_plan.recommended_agents.is_empty() {
            steps.push(json!({
                "step": steps.len() + 1,
                "action": "execute",
                "task": llm_plan.rewritten_prompt,
                "reason": "No delegation recommended - handle directly"
            }));
        }

        steps
    }

    /// Build required first action from LLM recommendations
    fn build_required_action_from_llm(&self, llm_plan: &LlmIntentAnalysis, execution_steps: &[Value]) -> Value {
        // If LLM recommends agents, delegation is MANDATORY
        if !llm_plan.recommended_agents.is_empty() {
            let first_agent = &llm_plan.recommended_agents[0];
            let delegate_step = execution_steps.iter()
                .find(|s| s.get("action").and_then(|a| a.as_str()) == Some("delegate"));

            return json!({
                "action": "delegate",
                "reason": format!("LLM intent analysis recommends specialist agent: {}", first_agent),
                "command": format!(
                    "delegate_to_agent(agent_id=\"{}\", task=\"{}\", wait_for_result=true)",
                    first_agent,
                    llm_plan.rewritten_prompt.replace("\"", "\\\"")
                ),
                "agent_id": first_agent,
                "task": llm_plan.rewritten_prompt,
                "MANDATORY": "YOU MUST delegate to this agent before doing any other work. This agent has the expertise for this task."
            });
        }

        // If LLM recommends skills, loading is expected
        if !llm_plan.recommended_skills.is_empty() {
            return json!({
                "action": "load_skills",
                "reason": "LLM intent analysis recommends these skills for the task",
                "skills": llm_plan.recommended_skills,
                "MANDATORY": "Skills have been auto-loaded. Review their instructions in context before proceeding."
            });
        }

        // Default: proceed with execution
        json!({
            "action": "proceed",
            "reason": "No specialist delegation needed - handle directly",
            "strategy": llm_plan.execution_strategy
        })
    }

    /// Convert skill names to Values with relevance
    fn skills_to_values(&self, skill_names: &[String], all_skills: &[DiscoveredSkill], configured_skills: &HashSet<String>) -> Vec<Value> {
        skill_names.iter()
            .filter_map(|name| {
                all_skills.iter()
                    .find(|s| s.name == *name)
                    .map(|s| json!({
                        "name": s.name,
                        "description": s.description,
                        "relevance": if configured_skills.contains(&s.name) { 1.0 } else { 0.8 },
                        "domain_hints": s.domain_hints,
                        "configured": configured_skills.contains(&s.name),
                        "source": "llm_recommendation"
                    }))
                    .or_else(|| {
                        // Skill might not be in discovered list but LLM recommended it
                        Some(json!({
                            "name": name,
                            "relevance": 0.8,
                            "source": "llm_recommendation"
                        }))
                    })
            })
            .collect()
    }

    /// Convert agent IDs to Values with relevance
    fn agents_to_values(&self, agent_ids: &[String], all_agents: &[DiscoveredAgent]) -> Vec<Value> {
        agent_ids.iter()
            .filter_map(|id| {
                all_agents.iter()
                    .find(|a| a.agent_id == *id)
                    .map(|a| json!({
                        "agent_id": a.agent_id,
                        "description": a.description,
                        "relevance": 0.8,
                        "source": "llm_recommendation"
                    }))
                    .or_else(|| {
                        Some(json!({
                            "agent_id": id,
                            "relevance": 0.8,
                            "source": "llm_recommendation"
                        }))
                    })
            })
            .collect()
    }

    /// Convert wards to Values
    fn wards_to_values(&self, wards: &[DiscoveredWard]) -> Vec<Value> {
        wards.iter()
            .map(|w| json!({
                "name": w.name,
                "purpose": w.purpose,
                "domain": w.domain,
                "modules": w.modules
            }))
            .collect()
    }

    /// Heuristic fallback analysis (when LLM not available)
    async fn heuristic_analysis(
        &self,
        message: &str,
        all_skills: &[DiscoveredSkill],
        all_agents: &[DiscoveredAgent],
        all_wards: &[DiscoveredWard],
        configured_skills: &HashSet<String>,
    ) -> Result<Value> {
        // Try semantic search first if fact_store is available
        let semantic_skills = self.search_skills_semantic(message).await;
        let semantic_agents = self.search_agents_semantic(message).await;

        // Use semantic results if available, otherwise fall back to keyword matching
        let matched_skills = if let Some(semantic) = semantic_skills {
            let mut keyword_matched = self.match_skills(all_skills, message, configured_skills);
            for sem_skill in semantic {
                let name = sem_skill.get("name").and_then(|n: &Value| n.as_str()).unwrap_or("");
                if !keyword_matched.iter().any(|k| k.get("name").and_then(|n| n.as_str()) == Some(name)) {
                    keyword_matched.push(sem_skill);
                }
            }
            keyword_matched
        } else {
            self.match_skills(all_skills, message, configured_skills)
        };

        let matched_agents = if let Some(semantic) = semantic_agents {
            let mut keyword_matched = self.match_agents(all_agents, message);
            for sem_agent in semantic {
                let agent_id = sem_agent.get("agent_id").and_then(|n: &Value| n.as_str()).unwrap_or("");
                if !keyword_matched.iter().any(|k| k.get("agent_id").and_then(|n| n.as_str()) == Some(agent_id)) {
                    keyword_matched.push(sem_agent);
                }
            }
            keyword_matched
        } else {
            self.match_agents(all_agents, message)
        };

        let matched_wards = self.match_wards(all_wards, message);

        // Heuristic intent detection
        let intent = self.detect_intent_pattern(message);
        let complexity = self.score_complexity(&matched_skills, &intent, message);
        let ward_rec = self.recommend_ward(&matched_wards, &intent, message);

        let execution_plan = self.build_execution_plan(&matched_skills, &matched_agents, &intent, &complexity, message);

        // Return pure analysis - agent decides what to load
        let result = json!({
            "primary_intent": intent.pattern_type,
            "domain": intent.domain,
            "explicit_goals": intent.explicit_goals,
            "implicit_goals": intent.implicit_goals,
            "discovered_resources": {
                "skills": matched_skills,
                "agents": matched_agents,
                "wards": matched_wards
            },
            "ward_recommendation": ward_rec,
            "execution_plan": execution_plan,
            "complexity": complexity,
            "analysis_source": "heuristic"
        });

        Ok(result)
    }
}

// ============================================================================
// LLM-POWERED INTENT ANALYSIS
// ============================================================================

/// LLM-extracted intent analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmIntentAnalysis {
    /// Primary intent type (coding, research, analysis, delegation, etc.)
    primary_intent: String,
    /// Domain of the request (software, finance, data, etc.)
    domain: String,
    /// Explicitly stated goals
    explicit_goals: Vec<String>,
    /// Implicitly inferred goals
    implicit_goals: Vec<String>,
    /// Rewritten/clarified prompt for better understanding
    rewritten_prompt: String,
    /// Hidden or secondary intents the user might have
    hidden_intents: Vec<String>,
    /// Recommended skills to use (matched by name)
    recommended_skills: Vec<String>,
    /// Recommended agents to delegate to
    recommended_agents: Vec<String>,
    /// Whether to use execution graph
    use_execution_graph: bool,
    /// Suggested ward name (generic, reusable)
    suggested_ward: Option<String>,
    /// Execution strategy: "direct", "tracked", "graph"
    execution_strategy: String,
    /// Key entities mentioned (for knowledge graph)
    entities: Vec<String>,
    /// Reasoning for the recommendations
    reasoning: String,
}

impl AnalyzeIntentTool {
    /// System prompt for LLM-based intent analysis
    const INTENT_ANALYSIS_PROMPT: &'static str = r#"You are an expert intent analyzer for an AI agent system. Analyze the user's request and return a structured JSON response.

Your job is to:
1. Understand the user's true intent (what they really want)
2. Identify hidden or secondary intents they might have
3. Rewrite the prompt for clarity if needed
4. Match the request to available skills and agents
5. Recommend an execution strategy

Return JSON in this exact format:
{
  "primary_intent": "coding|research|analysis|delegation|workflow|conversation|planning",
  "domain": "software|finance|data|research|education|writing|general",
  "explicit_goals": ["goal 1", "goal 2"],
  "implicit_goals": ["inferred goal"],
  "rewritten_prompt": "Clearer version of the user's request",
  "hidden_intents": ["secondary intent user might have"],
  "recommended_skills": ["skill_name"],
  "recommended_agents": ["agent_id"],
  "use_execution_graph": false,
  "suggested_ward": "generic-ward-name",
  "execution_strategy": "direct|tracked|graph",
  "entities": ["important things mentioned"],
  "reasoning": "Why you made these recommendations"
}

Guidelines:
- primary_intent: What type of task is this?
  - coding: Writing/modifying code
  - research: Gathering information
  - analysis: Processing data, calculations
  - delegation: Should be handled by a specialist agent
  - workflow: Multi-step process
  - planning: Creating plans or strategies
  - conversation: General chat, questions

- domain: What domain does this belong to?

- recommended_skills: Match to available skills by name (partial match OK)
- recommended_agents: Match to available agents by ID (partial match OK)

- use_execution_graph: true if task has 3+ dependent steps
- execution_strategy:
  - direct: Simple task, handle directly
  - tracked: Medium complexity, track progress
  - graph: Complex, use execution graph with subagents

- suggested_ward: GENERIC reusable name (e.g., "stock-analyst" not "lmnd-analysis")
  Only suggest if task would benefit from persistent workspace

- hidden_intents: What else might the user want? (e.g., "wants to learn", "needs visualization")"#;
}

/// Extract JSON from content that might have markdown or extra text
fn extract_json_from_content(content: &str) -> String {
    let trimmed = content.trim();

    // Try to find JSON object
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return trimmed[start..=end].to_string();
        }
    }

    // Return as-is if no JSON structure found
    trimmed.to_string()
}

struct DiscoveredSkill {
    name: String,
    description: String,
    trigger_keywords: Vec<String>,
    domain_hints: Vec<String>,
}

struct DiscoveredAgent {
    agent_id: String,
    description: String,
}

struct DiscoveredWard {
    name: String,
    purpose: String,
    domain: String,
    modules: Vec<String>,
}

struct IntentPattern {
    pattern_type: String,
    domain: String,
    explicit_goals: Vec<String>,
    implicit_goals: Vec<String>,
}

impl AnalyzeIntentTool {
    /// Discover skills - uses cached index from context state if available, falls back to disk scan
    fn discover_skills_cached(&self, ctx: &Arc<dyn ToolContext>) -> Result<Vec<DiscoveredSkill>> {
        // Try to read cached skill list from context state first
        if let Some(cached_skills) = ctx.get_state("index:skills") {
            if let Some(skills_array) = cached_skills.as_array() {
                let skills: Vec<DiscoveredSkill> = skills_array
                    .iter()
                    .filter_map(|s| {
                        let name = s.get("name").and_then(|n| n.as_str())?.to_string();
                        let description = s.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                        let trigger_keywords = s.get("trigger_keywords")
                            .and_then(|k| k.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                            .unwrap_or_default();
                        let domain_hints = s.get("domain_hints")
                            .and_then(|d| d.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                            .unwrap_or_default();
                        Some(DiscoveredSkill {
                            name,
                            description,
                            trigger_keywords,
                            domain_hints,
                        })
                    })
                    .collect();

                if !skills.is_empty() {
                    tracing::debug!("Using cached skills index ({} skills)", skills.len());
                    return Ok(skills);
                }
            }
        }

        // Fall back to disk scan
        self.discover_skills()
    }

    /// Discover agents - uses cached index from context state if available, falls back to disk scan
    fn discover_agents_cached(&self, ctx: &Arc<dyn ToolContext>) -> Result<Vec<DiscoveredAgent>> {
        // Try to read cached agent list from context state first
        if let Some(cached_agents) = ctx.get_state("index:agents") {
            if let Some(agents_array) = cached_agents.as_array() {
                let agents: Vec<DiscoveredAgent> = agents_array
                    .iter()
                    .filter_map(|a| {
                        let agent_id = a.get("name").and_then(|n| n.as_str())?.to_string();
                        let description = a.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                        Some(DiscoveredAgent {
                            agent_id,
                            description,
                        })
                    })
                    .collect();

                if !agents.is_empty() {
                    tracing::debug!("Using cached agents index ({} agents)", agents.len());
                    return Ok(agents);
                }
            }
        }

        // Fall back to disk scan
        self.discover_agents()
    }

    /// Search indexed skills semantically using memory fact store.
    ///
    /// Queries the fact store for skills matching the message, returning
    /// relevant skills based on semantic similarity.
    async fn search_skills_semantic(&self, message: &str) -> Option<Vec<Value>> {
        let fact_store = self.fact_store.as_ref()?;

        // Query for skill facts
        let facts_value = fact_store
            .recall_facts("default", message, 10)
            .await
            .ok()?;

        // Convert to array
        let facts = facts_value.as_array()?;

        // Filter for skill category and convert to Value
        let skills: Vec<Value> = facts
            .iter()
            .filter(|f| {
                f.get("category").and_then(|c| c.as_str()) == Some("skill")
            })
            .filter_map(|f| {
                let key = f.get("key").and_then(|k| k.as_str())?;
                let name = key.strip_prefix("skill:").unwrap_or(key);
                let content = f.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let score = f.get("score").and_then(|s| s.as_f64()).unwrap_or(0.5);

                Some(json!({
                    "name": name,
                    "description": content,
                    "relevance": (score * 100.0).round() / 100.0,
                    "source": "semantic_index"
                }))
            })
            .collect();

        if !skills.is_empty() {
            tracing::debug!("Found {} skills via semantic search", skills.len());
            Some(skills)
        } else {
            None
        }
    }

    /// Search indexed agents semantically using memory fact store.
    async fn search_agents_semantic(&self, message: &str) -> Option<Vec<Value>> {
        let fact_store = self.fact_store.as_ref()?;

        // Query for agent facts
        let facts_value = fact_store
            .recall_facts("default", message, 10)
            .await
            .ok()?;

        // Convert to array
        let facts = facts_value.as_array()?;

        // Filter for agent category and convert to Value
        let agents: Vec<Value> = facts
            .iter()
            .filter(|f| {
                f.get("category").and_then(|c| c.as_str()) == Some("agent")
            })
            .filter_map(|f| {
                let key = f.get("key").and_then(|k| k.as_str())?;
                let name = key.strip_prefix("agent:").unwrap_or(key);
                let content = f.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let score = f.get("score").and_then(|s| s.as_f64()).unwrap_or(0.5);

                Some(json!({
                    "agent_id": name,
                    "description": content,
                    "relevance": (score * 100.0).round() / 100.0,
                    "source": "semantic_index"
                }))
            })
            .collect();

        if !agents.is_empty() {
            tracing::debug!("Found {} agents via semantic search", agents.len());
            Some(agents)
        } else {
            None
        }
    }

    /// Discover skills - scans disk for skills (used when cache is empty)
    fn discover_skills(&self) -> Result<Vec<DiscoveredSkill>> {
        // Try to use the indexer module for consistent metadata
        let skills_dir = self.fs.skills_dir()
            .ok_or_else(|| ZeroError::Tool("Skills directory not found".to_string()))?;

        if !skills_dir.exists() {
            return Ok(Vec::new());
        }

        // Use the indexer's scan_skills_dir for consistent parsing
        let indexed_skills = scan_skills_dir(&skills_dir)?;

        // Convert SkillMetadata to DiscoveredSkill
        let skills: Vec<DiscoveredSkill> = indexed_skills.into_iter()
            .map(|s| DiscoveredSkill {
                name: s.name,
                description: s.description,
                trigger_keywords: s.trigger_keywords,
                domain_hints: s.domain_hints,
            })
            .collect();

        Ok(skills)
    }

    /// Discover agents - uses index if available, falls back to disk scan
    fn discover_agents(&self) -> Result<Vec<DiscoveredAgent>> {
        // Try to use the indexer module for consistent metadata
        let agents_dir = self.fs.agents_dir()
            .ok_or_else(|| ZeroError::Tool("Agents directory not found".to_string()))?;

        if !agents_dir.exists() {
            return Ok(Vec::new());
        }

        // Use the indexer's scan_agents_dir for consistent parsing
        let indexed_agents = scan_agents_dir(&agents_dir)?;

        // Convert AgentMetadata to DiscoveredAgent
        let agents: Vec<DiscoveredAgent> = indexed_agents.into_iter()
            .map(|a| DiscoveredAgent {
                agent_id: a.name,
                description: a.description,
            })
            .collect();

        Ok(agents)
    }

    fn discover_wards(&self) -> Result<Vec<DiscoveredWard>> {
        let wards_dir = self.fs.wards_root_dir()
            .ok_or_else(|| ZeroError::Tool("Wards directory not found".to_string()))?;

        if !wards_dir.exists() {
            return Ok(Vec::new());
        }

        let mut wards = Vec::new();
        for entry in std::fs::read_dir(&wards_dir)? {
            let entry = entry?;
            let ward_path = entry.path();
            if ward_path.is_dir() {
                let name = ward_path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                
                if name.starts_with('.') {
                    continue;
                }

                let agents_md = ward_path.join("AGENTS.md");
                if agents_md.exists() {
                    if let Some(ward) = self.parse_ward_agents_md(&name, &agents_md)? {
                        wards.push(ward);
                    }
                } else {
                    wards.push(DiscoveredWard {
                        name,
                        purpose: String::new(),
                        domain: String::new(),
                        modules: self.list_ward_modules(&ward_path),
                    });
                }
            }
        }
        Ok(wards)
    }

    fn parse_ward_agents_md(&self, name: &str, path: &PathBuf) -> Result<Option<DiscoveredWard>> {
        let content = std::fs::read_to_string(path)?;
        let purpose = self.extract_section(&content, "Purpose").unwrap_or_default();

        let domain = if purpose.to_lowercase().contains("stock") || purpose.to_lowercase().contains("financial") {
            "financial".to_string()
        } else if purpose.to_lowercase().contains("api") || purpose.to_lowercase().contains("web") {
            "web".to_string()
        } else {
            String::new()
        };

        Ok(Some(DiscoveredWard {
            name: name.to_string(),
            purpose,
            domain,
            modules: self.extract_modules_from_content(&content),
        }))
    }

    fn list_ward_modules(&self, ward_path: &PathBuf) -> Vec<String> {
        let mut modules = Vec::new();
        let core_dir = ward_path.join("core");
        if core_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&core_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".py") || name.ends_with(".rs") || name.ends_with(".js") {
                        modules.push(name);
                    }
                }
            }
        }
        modules
    }

    fn extract_section(&self, content: &str, section_name: &str) -> Option<String> {
        let section_header = format!("## {}", section_name);
        let mut in_section = false;
        let mut result = String::new();

        for line in content.lines() {
            if line.starts_with("## ") {
                if in_section {
                    break;
                }
                if line == section_header {
                    in_section = true;
                    continue;
                }
            }
            if in_section && !line.trim().is_empty() && !line.starts_with('|') && !line.starts_with("```") {
                result.push_str(line);
                result.push(' ');
            }
        }

        if result.is_empty() { None } else { Some(result.trim().to_string()) }
    }

    fn extract_modules_from_content(&self, content: &str) -> Vec<String> {
        let mut modules = Vec::new();
        let re = Regex::new(r"\|\s*([\w.-]+\.(?:py|rs|js|ts))\s*\|").unwrap();
        for cap in re.captures_iter(content) {
            if let Some(m) = cap.get(1) {
                modules.push(m.as_str().to_string());
            }
        }
        modules
    }

    fn match_skills(&self, skills: &[DiscoveredSkill], message: &str, configured_skills: &HashSet<String>) -> Vec<Value> {
        let msg_lower = message.to_lowercase();
        let msg_words: HashSet<&str> = msg_lower.split_whitespace().collect();

        let mut scored: Vec<(f64, &DiscoveredSkill)> = skills.iter()
            .map(|s| {
                let mut score = self.skill_relevance(s, &msg_words, &msg_lower);
                // Boost configured skills significantly
                if configured_skills.contains(&s.name) {
                    score = (score + 0.5).min(1.0); // Add 0.5 boost, cap at 1.0
                }
                (score, s)
            })
            .filter(|(score, _)| *score > 0.05) // Lower threshold from 0.1 to 0.05
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().take(5).map(|(score, skill)| {
            json!({
                "name": skill.name,
                "description": skill.description,
                "relevance": (score * 100.0).round() / 100.0,
                "domain_hints": skill.domain_hints,
                "configured": configured_skills.contains(&skill.name)
            })
        }).collect()
    }

    fn skill_relevance(&self, skill: &DiscoveredSkill, msg_words: &HashSet<&str>, msg_lower: &str) -> f64 {
        // 1. Keyword matching (from trigger_keywords)
        let keyword_score = skill.trigger_keywords.iter()
            .filter(|k| msg_words.contains(k.as_str()) || msg_lower.contains(&k.to_lowercase()))
            .count() as f64 / (skill.trigger_keywords.len().max(1) as f64);

        // 2. Domain matching
        let domain_score = skill.domain_hints.iter()
            .filter(|d| msg_lower.contains(&d.to_lowercase()))
            .count() as f64 * 0.3;

        // 3. Description matching - Match message words against skill description
        let desc_lower = skill.description.to_lowercase();
        let desc_words: HashSet<&str> = desc_lower.split_whitespace()
            .filter(|w| w.len() > 3) // Skip short words
            .collect();
        let desc_overlap = desc_words.intersection(&msg_words).count() as f64;
        let desc_score = (desc_overlap / 10.0).min(0.5); // Cap at 0.5

        // 4. Skill name matching - does message relate to skill name?
        let name_lower = skill.name.to_lowercase();
        let name_parts: Vec<&str> = name_lower.split(['-', '_']).collect();
        let name_score = name_parts.iter()
            .filter(|p| msg_words.contains(*p) || msg_lower.contains(*p))
            .count() as f64 * 0.2;

        // 5. DOMAIN-SPECIFIC BOOSTERS - Critical for matching skills to task domains
        let mut domain_boost = 0.0;

        // Stock/financial analysis tasks need: search, fundamentals, data
        if msg_lower.contains("stock") || msg_lower.contains("analysis") || msg_lower.contains("financial") || msg_lower.contains("market") {
            if name_lower.contains("search") || name_lower.contains("duckduckgo") {
                domain_boost += 0.4; // Research requires web search
            }
            if name_lower.contains("yf") || name_lower.contains("yahoo") || name_lower.contains("fundamental") {
                domain_boost += 0.5; // Financial data skills
            }
            if name_lower.contains("data") || name_lower.contains("analysis") {
                domain_boost += 0.3;
            }
        }

        // Research tasks need search skills
        if msg_lower.contains("research") || msg_lower.contains("find") || msg_lower.contains("look up") || msg_lower.contains("search") {
            if name_lower.contains("search") || name_lower.contains("web") || name_lower.contains("duckduckgo") {
                domain_boost += 0.5;
            }
        }

        // Code tasks need development skills
        if msg_lower.contains("code") || msg_lower.contains("build") || msg_lower.contains("implement") || msg_lower.contains("refactor") {
            if name_lower.contains("rust") || name_lower.contains("python") || name_lower.contains("code") {
                domain_boost += 0.4;
            }
        }

        // 6. Description semantic matching - does skill description mention task-relevant concepts?
        if desc_lower.contains("search") && (msg_lower.contains("research") || msg_lower.contains("analysis") || msg_lower.contains("find")) {
            domain_boost += 0.3;
        }
        if desc_lower.contains("stock") || desc_lower.contains("financial") || desc_lower.contains("market") {
            if msg_lower.contains("stock") || msg_lower.contains("financial") || msg_lower.contains("analysis") {
                domain_boost += 0.4;
            }
        }

        // Combine scores with weights
        let base_score = keyword_score * 0.2 + domain_score * 0.15 + desc_score * 0.25 + name_score * 0.2;
        (base_score + domain_boost).min(1.0)
    }

    fn match_agents(&self, agents: &[DiscoveredAgent], message: &str) -> Vec<Value> {
        let msg_lower = message.to_lowercase();
        let msg_words: HashSet<&str> = msg_lower.split_whitespace().collect();

        // Score agents based on description/keywords matching the task
        let mut scored: Vec<(f64, &DiscoveredAgent)> = agents.iter()
            .map(|a| (self.agent_relevance(a, &msg_words, &msg_lower), a))
            .filter(|(score, _)| *score > 0.1)  // Lower threshold for agents
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().take(3).map(|(score, agent)| {
            json!({
                "agent_id": agent.agent_id,
                "description": agent.description,
                "relevance": (score * 100.0).round() / 100.0
            })
        }).collect()
    }

    fn agent_relevance(&self, agent: &DiscoveredAgent, msg_words: &HashSet<&str>, msg_lower: &str) -> f64 {
        let name_score = agent.agent_id.split(['-', '_'])
            .filter(|p| msg_words.contains(*p) || msg_lower.contains(&p.to_lowercase()))
            .count() as f64 * 0.3;

        let desc_score = agent.description.to_lowercase().split_whitespace()
            .filter(|w| {
                let w_lower = w.to_lowercase();
                // Match if word is in message or message contains word
                msg_words.contains(&w_lower.as_str()) || msg_lower.contains(&w_lower)
            })
            .count() as f64 * 0.05;

        // Boost for common patterns
        let pattern_boost = if msg_lower.contains("analy") && agent.description.to_lowercase().contains("analy") {
            0.4
        } else if msg_lower.contains("research") && agent.description.to_lowercase().contains("research") {
            0.4
        } else if msg_lower.contains("code") && agent.description.to_lowercase().contains("code") {
            0.3
        } else if msg_lower.contains("data") && agent.description.to_lowercase().contains("data") {
            0.3
        } else if (msg_lower.contains("stock") || msg_lower.contains("financ")) && agent.description.to_lowercase().contains("analy") {
            0.35
        } else {
            0.0
        };

        (name_score + desc_score + pattern_boost).min(1.0)
    }

    fn match_wards(&self, wards: &[DiscoveredWard], message: &str) -> Vec<Value> {
        let msg_lower = message.to_lowercase();
        let msg_words: HashSet<&str> = msg_lower.split_whitespace().collect();

        let mut scored: Vec<(f64, &DiscoveredWard)> = wards.iter()
            .map(|w| (self.ward_relevance(w, &msg_words, &msg_lower), w))
            .filter(|(score, _)| *score > 0.05)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().take(3).map(|(score, ward)| {
            json!({
                "name": ward.name,
                "purpose": ward.purpose,
                "domain": ward.domain,
                "modules": ward.modules,
                "relevance": (score * 100.0).round() / 100.0
            })
        }).collect()
    }

    fn ward_relevance(&self, ward: &DiscoveredWard, msg_words: &HashSet<&str>, msg_lower: &str) -> f64 {
        let name_score = ward.name.split(['-', '_'])
            .filter(|p| msg_words.contains(*p) || msg_lower.contains(&p.to_lowercase()))
            .count() as f64 * 0.5;

        let purpose_score = ward.purpose.to_lowercase().split_whitespace()
            .filter(|w| msg_words.contains(*w) || msg_lower.contains(&w.to_lowercase()))
            .count() as f64 * 0.1;

        (name_score + purpose_score).min(1.0)
    }

    fn detect_intent_pattern(&self, message: &str) -> IntentPattern {
        let msg_lower = message.to_lowercase();

        let patterns = [
            ("research", vec!["research", "find out", "investigate", "look into", "learn about"]),
            ("build", vec!["build", "create", "implement", "develop", "make"]),
            ("analyze", vec!["analyze", "examine", "assess", "review", "evaluate"]),
            ("fix", vec!["fix", "debug", "resolve", "troubleshoot"]),
            ("automate", vec!["automate", "pipeline", "workflow", "schedule"]),
            ("learn", vec!["explain", "teach", "how does", "tutorial"]),
        ];

        let mut best_pattern = "general";
        let mut best_score = 0;

        for (name, keywords) in &patterns {
            let score = keywords.iter().filter(|k| msg_lower.contains(*k)).count();
            if score > best_score {
                best_score = score;
                best_pattern = name;
            }
        }

        IntentPattern {
            pattern_type: best_pattern.to_string(),
            domain: self.detect_domain(message),
            explicit_goals: self.extract_explicit_goals(message),
            implicit_goals: self.extract_implicit_goals(best_pattern),
        }
    }

    fn detect_domain(&self, message: &str) -> String {
        let msg_lower = message.to_lowercase();
        let domains = [
            ("financial", vec!["stock", "price", "market", "trading", "investment", "yfinance"]),
            ("web", vec!["website", "url", "http", "api", "rest", "frontend"]),
            ("data", vec!["data", "dataset", "csv", "json", "pipeline"]),
            ("code", vec!["code", "function", "module", "class", "library"]),
        ];

        for (name, keywords) in domains {
            if keywords.iter().any(|k| msg_lower.contains(k)) {
                return name.to_string();
            }
        }
        "general".to_string()
    }

    fn extract_explicit_goals(&self, message: &str) -> Vec<String> {
        vec![format!("Complete: {}", message.split_whitespace().take(10).collect::<Vec<_>>().join(" "))]
    }

    fn extract_implicit_goals(&self, pattern: &str) -> Vec<String> {
        match pattern {
            "analyze" => vec!["Gather relevant data".into(), "Identify patterns".into(), "Generate insights".into()],
            "research" => vec!["Search for information".into(), "Synthesize findings".into()],
            "build" => vec!["Design architecture".into(), "Implement core functionality".into()],
            "fix" => vec!["Diagnose root cause".into(), "Verify fix".into()],
            _ => vec![],
        }
    }

    fn score_complexity(&self, skills: &[Value], intent: &IntentPattern, message: &str) -> String {
        let base = skills.len() as i32
            + if intent.pattern_type == "build" || intent.pattern_type == "automate" { 2 } else { 0 }
            + if message.len() > 100 { 1 } else { 0 };

        match base {
            0..=1 => "simple".to_string(),
            2..=3 => "medium".to_string(),
            _ => "high".to_string(),
        }
    }

    fn recommend_ward(&self, wards: &[Value], intent: &IntentPattern, message: &str) -> Value {
        // Check for existing ward match first
        if let Some(best) = wards.first() {
            let rel = best.get("relevance").and_then(|v| v.as_f64()).unwrap_or(0.0);
            if rel > 0.5 {
                return json!({
                    "action": "use_existing",
                    "ward_name": best.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "reason": format!("Existing ward matches ({:.0}%)", rel * 100.0)
                });
            }
        }

        // Use scratch for one-off debugging
        if intent.pattern_type == "fix" {
            return json!({
                "action": "use_scratch",
                "ward_name": "scratch",
                "reason": "One-off debugging task"
            });
        }

        // Generate GENERIC ward name based on domain (NOT task-specific!)
        let generic_name = self.suggest_generic_ward_name(&intent.domain, message);

        json!({
            "action": "create_generic",
            "ward_name": generic_name,
            "reason": "Generic reusable ward for this domain",
            "subdirectories": self.suggest_subdirectories(&intent.domain, message)
        })
    }

    /// Suggest a GENERIC ward name based on domain (reusable across tasks)
    fn suggest_generic_ward_name(&self, domain: &str, message: &str) -> String {
        let msg_lower = message.to_lowercase();

        // Domain-specific generic names (NOT task-specific!)
        let generic_name: String = match domain {
            "software" if msg_lower.contains("test") => "test-suite".to_string(),
            "software" if msg_lower.contains("api") || msg_lower.contains("rest") => "api-builder".to_string(),
            "software" if msg_lower.contains("web") || msg_lower.contains("ui") || msg_lower.contains("frontend") => "web-builder".to_string(),
            "software" => "code-builder".to_string(),
            "finance" | "financial" | "stock" | "trading" | "investment" => "stock-analyst".to_string(),
            "data" | "analysis" | "analytics" | "dataset" | "csv" => "data-analyst".to_string(),
            "research" | "information" | "search" | "investigate" => "research-hub".to_string(),
            "education" | "learn" | "teach" | "tutor" | "homework" | "study" => "math-tutor".to_string(),
            "writing" | "document" | "report" | "article" | "content" => "content-writer".to_string(),
            "notes" | "journal" | "diary" | "log" => "daily-journal".to_string(),
            _ => {
                // Extract a generic noun from the message
                let words: Vec<&str> = message.split_whitespace()
                    .filter(|w| w.len() > 3 && !["the", "for", "get", "please", "help", "need", "want", "make", "create", "build", "analyze"].contains(w))
                    .take(2)
                    .collect();

                if words.is_empty() {
                    return "new-project".to_string();
                }

                // Create generic name like "project-builder" or "task-manager"
                format!("{}-hub", words[0].to_lowercase())
            }
        };

        // Sanitize
        Regex::new(r"[^a-zA-Z0-9-]").unwrap().replace_all(&generic_name.to_lowercase(), "-").to_string()
    }

    /// Suggest subdirectories for the specific task instance
    fn suggest_subdirectories(&self, domain: &str, message: &str) -> Vec<String> {
        let msg_lower = message.to_lowercase();

        match domain {
            "finance" | "financial" | "stock" | "trading" | "investment" => {
                // Extract ticker symbols
                let tickers: Vec<String> = Regex::new(r"\b[A-Z]{2,5}\b")
                    .unwrap()
                    .find_iter(message)
                    .filter_map(|m| {
                        let sym = m.as_str();
                        if !["THE", "AND", "FOR", "WITH", "FROM", "THIS", "THAT"].contains(&sym) {
                            Some(format!("tickers/{}", sym))
                        } else {
                            None
                        }
                    })
                    .take(3)
                    .collect();

                if !tickers.is_empty() {
                    return tickers;
                }
                vec!["tickers/".to_string()]
            }
            "data" | "analysis" => {
                vec!["datasets/".to_string(), "outputs/".to_string()]
            }
            "software" => {
                vec!["src/".to_string(), "tests/".to_string()]
            }
            _ => vec!["outputs/".to_string()]
        }
    }

    /// Build comprehensive execution plan with delegation strategy
    fn build_execution_plan(&self, skills: &[Value], agents: &[Value], intent: &IntentPattern, complexity: &str, message: &str) -> Value {
        let msg_lower = message.to_lowercase();

        // Determine execution strategy
        let (strategy, use_graph) = match complexity {
            "simple" => ("direct", false),
            "medium" => ("tracked", false),
            "high" => ("graph", true),
            _ => ("direct", false),
        };

        // Build delegation plan based on available agents and task type
        let delegations = self.build_delegation_plan(agents, intent, message);

        // Build research plan
        let research_plan = self.build_research_plan(skills, intent, message);

        // Build coding plan
        let coding_plan = self.build_coding_plan(intent, message);

        // Get recommended skills
        let skill_recommendations: Vec<Value> = skills.iter()
            .filter(|s| s.get("relevance").and_then(|v| v.as_f64()).unwrap_or(0.0) > 0.2)
            .map(|s| json!({
                "name": s.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                "relevance": s.get("relevance").and_then(|v| v.as_f64()).unwrap_or(0.0),
                "auto_load": s.get("relevance").and_then(|v| v.as_f64()).unwrap_or(0.0) > 0.3
            }))
            .take(5)
            .collect();

        // Build REQUIRED FIRST ACTION - the agent MUST follow this
        let required_first_action = self.build_required_action(&delegations, &skill_recommendations, strategy);

        json!({
            "strategy": strategy,
            "use_execution_graph": use_graph,
            "complexity": complexity,
            "required_first_action": required_first_action,
            "delegation_plan": delegations,
            "research_plan": research_plan,
            "coding_plan": coding_plan,
            "recommended_skills": skill_recommendations
        })
    }

    /// Build delegation plan with specific tasks for each relevant agent
    fn build_delegation_plan(&self, agents: &[Value], intent: &IntentPattern, message: &str) -> Value {
        let msg_lower = message.to_lowercase();
        let mut delegations: Vec<Value> = Vec::new();
        let mut priority = 1;

        // High complexity = SHOULD delegate
        let should_delegate = intent.explicit_goals.len() > 2 ||
            msg_lower.contains("comprehensive") ||
            msg_lower.contains("complete") ||
            msg_lower.contains("full") ||
            msg_lower.contains("detailed");

        if !should_delegate {
            return json!({
                "strategy": "self_execute",
                "reason": "Task is simple enough for single agent",
                "delegations": []
            });
        }

        // Research delegation
        if msg_lower.contains("sentiment") || msg_lower.contains("news") ||
           msg_lower.contains("research") || msg_lower.contains("search") ||
           msg_lower.contains("analysis") || msg_lower.contains("analyze") {

            let research_agent = agents.iter()
                .find(|a| {
                    let id = a.get("agent_id").and_then(|n| n.as_str()).unwrap_or("");
                    id.contains("research")
                });

            if let Some(agent) = research_agent {
                let research_task = self.build_research_delegation_task(message);
                delegations.push(json!({
                    "id": "research",
                    "agent_id": agent.get("agent_id").and_then(|n| n.as_str()).unwrap_or("research-agent"),
                    "task": research_task,
                    "output_file": "outputs/research_summary.md",
                    "priority": priority,
                    "depends_on": []
                }));
                priority += 1;
            }
        }

        // Data analysis delegation
        if msg_lower.contains("technical") || msg_lower.contains("data") ||
           msg_lower.contains("analysis") || msg_lower.contains("prediction") ||
           msg_lower.contains("statistics") || msg_lower.contains("chart") {

            let data_agent = agents.iter()
                .find(|a| {
                    let id = a.get("agent_id").and_then(|n| n.as_str()).unwrap_or("");
                    id.contains("data") || id.contains("analyst")
                });

            if let Some(agent) = data_agent {
                let analysis_task = self.build_analysis_delegation_task(message);
                let depends_on: Vec<&str> = if delegations.iter().any(|d| d.get("id").and_then(|i| i.as_str()) == Some("research")) {
                    vec!["research"]
                } else {
                    vec![]
                };

                delegations.push(json!({
                    "id": "analysis",
                    "agent_id": agent.get("agent_id").and_then(|n| n.as_str()).unwrap_or("data-analyst"),
                    "task": analysis_task,
                    "output_file": "outputs/analysis_report.md",
                    "priority": priority,
                    "depends_on": depends_on
                }));
                priority += 1;
            }
        }

        // Writing delegation for reports
        if msg_lower.contains("report") || msg_lower.contains("document") ||
           msg_lower.contains("write") || msg_lower.contains("summary") {

            let writer_agent = agents.iter()
                .find(|a| {
                    let id = a.get("agent_id").and_then(|n| n.as_str()).unwrap_or("");
                    id.contains("writer") || id.contains("summarizer")
                });

            if let Some(agent) = writer_agent {
                delegations.push(json!({
                    "id": "synthesis",
                    "agent_id": agent.get("agent_id").and_then(|n| n.as_str()).unwrap_or("summarizer"),
                    "task": "Synthesize all research and analysis into a professional report with clear sections and executive summary",
                    "output_file": "outputs/final_report.md",
                    "priority": priority,
                    "depends_on": delegations.iter().map(|d| d.get("id").and_then(|i| i.as_str()).unwrap_or("")).collect::<Vec<_>>()
                }));
            }
        }

        let delegation_strategy = if delegations.is_empty() {
            "self_execute"
        } else if delegations.len() == 1 {
            "single_delegation"
        } else {
            "parallel_delegation"
        };

        json!({
            "strategy": delegation_strategy,
            "reason": if should_delegate { "Complex task benefits from specialized agents" } else { "" },
            "delegations": delegations
        })
    }

    /// Build REQUIRED FIRST ACTION - explicit command that agent MUST follow
    fn build_required_action(&self, delegation_plan: &Value, skills: &[Value], strategy: &str) -> Value {
        let delegations = delegation_plan.get("delegations")
            .and_then(|d| d.as_array())
            .filter(|d| !d.is_empty());

        // If there are delegations, agent MUST delegate first
        if let Some(delegs) = delegations {
            if let Some(first_delegation) = delegs.first() {
                let agent_id = first_delegation.get("agent_id")
                    .and_then(|a| a.as_str())
                    .unwrap_or("data-analyst");
                let task = first_delegation.get("task")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Complete the assigned task");

                return json!({
                    "action": "delegate",
                    "reason": "analyze_intent detected a specialist agent for this task",
                    "command": format!("delegate_to_agent(agent_id=\"{}\", task=\"{}\", wait_for_result=true)", agent_id, task),
                    "agent_id": agent_id,
                    "task": task,
                    "MANDATORY": "You MUST execute this delegation before doing any other work"
                });
            }
        }

        // If there are high-relevance skills, agent MUST load them
        let high_relevance_skills: Vec<&Value> = skills.iter()
            .filter(|s| s.get("relevance").and_then(|r| r.as_f64()).unwrap_or(0.0) > 0.5)
            .collect();

        if !high_relevance_skills.is_empty() {
            let skill_names: Vec<&str> = high_relevance_skills.iter()
                .filter_map(|s| s.get("name").and_then(|n| n.as_str()))
                .take(3)
                .collect();

            return json!({
                "action": "load_skills",
                "reason": "High-relevance skills detected for this task",
                "skills": skill_names,
                "MANDATORY": "Skills have been auto-loaded. Review their instructions before proceeding."
            });
        }

        // Default: proceed with direct execution
        json!({
            "action": "proceed",
            "reason": "No specialist agents or high-relevance skills detected",
            "strategy": strategy
        })
    }

    fn build_research_delegation_task(&self, message: &str) -> String {
        let msg_lower = message.to_lowercase();

        // Extract topic/subject from message
        let topic = if msg_lower.contains("stock") || msg_lower.contains("lmnd") {
            "stock news, analyst ratings, earnings reports, and market sentiment"
        } else if msg_lower.contains("crypto") {
            "cryptocurrency news, market trends, and regulatory updates"
        } else {
            "relevant news, expert opinions, and recent developments"
        };

        format!(
            "Research and compile {}. Use search tools to find current information. \
            Save your findings to outputs/research_summary.md with key points and sources.",
            topic
        )
    }

    fn build_analysis_delegation_task(&self, message: &str) -> String {
        let msg_lower = message.to_lowercase();

        if msg_lower.contains("stock") || msg_lower.contains("trading") {
            "Perform technical analysis including RSI, MACD, Bollinger Bands, and moving averages. \
             Generate charts and save to outputs/ directory. Include validation checks for data availability."
        } else if msg_lower.contains("data") {
            "Analyze the dataset with statistical methods. Generate visualizations. \
             Check for data quality issues before analysis."
        } else {
            "Perform detailed analysis with appropriate methods. \
             Generate visualizations and save results to outputs/."
        }.to_string()
    }

    /// Build research plan
    fn build_research_plan(&self, skills: &[Value], intent: &IntentPattern, message: &str) -> Value {
        let msg_lower = message.to_lowercase();

        let needs_research = msg_lower.contains("sentiment") ||
            msg_lower.contains("news") ||
            msg_lower.contains("research") ||
            msg_lower.contains("search") ||
            msg_lower.contains("find") ||
            msg_lower.contains("current") ||
            msg_lower.contains("recent") ||
            msg_lower.contains("latest");

        if !needs_research {
            return json!({
                "needed": false,
                "reason": "Task does not require external research"
            });
        }

        // Find search skill
        let search_skill = skills.iter()
            .find(|s| {
                let name = s.get("name").and_then(|n| n.as_str()).unwrap_or("");
                name.contains("search") || name.contains("duckduckgo")
            });

        json!({
            "needed": true,
            "approach": "Search for relevant information using available tools",
            "skill_to_use": search_skill.map(|s| s.get("name").and_then(|n| n.as_str()).unwrap_or("duckduckgo-search")).unwrap_or("web-search"),
            "search_queries": self.suggest_search_queries(message),
            "output_file": "outputs/research_findings.md"
        })
    }

    fn suggest_search_queries(&self, message: &str) -> Vec<String> {
        let msg_lower = message.to_lowercase();
        let mut queries = Vec::new();

        // Stock analysis
        if msg_lower.contains("stock") {
            let tickers: Vec<&str> = Regex::new(r"\b[A-Z]{2,5}\b")
                .unwrap()
                .find_iter(message)
                .filter_map(|m| {
                    let sym = m.as_str();
                    if !["THE", "AND", "FOR", "WITH"].contains(&sym) { Some(sym) } else { None }
                })
                .collect();

            for ticker in tickers.iter().take(2) {
                queries.push(format!("{} stock news analyst sentiment", ticker));
                queries.push(format!("{} earnings report latest", ticker));
            }
        }

        if queries.is_empty() {
            queries.push(format!("{} latest news", message.split_whitespace().take(5).collect::<Vec<_>>().join(" ")));
        }

        queries
    }

    /// Build coding plan with language, libraries, and validation checks
    fn build_coding_plan(&self, intent: &IntentPattern, message: &str) -> Value {
        let msg_lower = message.to_lowercase();

        // Determine language
        let language = if msg_lower.contains("python") || msg_lower.contains("pandas") ||
                         msg_lower.contains("data analysis") || msg_lower.contains("stock") {
            "python"
        } else if msg_lower.contains("javascript") || msg_lower.contains("node") ||
                  msg_lower.contains("react") || msg_lower.contains("web") {
            "javascript"
        } else if msg_lower.contains("rust") {
            "rust"
        } else {
            "python"  // Default
        };

        // Determine libraries based on task
        let libraries = if msg_lower.contains("stock") || msg_lower.contains("financial") {
            vec!["yfinance", "pandas", "numpy", "matplotlib", "pandas-ta"]
        } else if msg_lower.contains("data") || msg_lower.contains("analysis") {
            vec!["pandas", "numpy", "matplotlib", "seaborn"]
        } else if msg_lower.contains("web") || msg_lower.contains("scrape") {
            vec!["requests", "beautifulsoup4", "selenium"]
        } else {
            vec!["pandas", "numpy"]
        };

        // Validation checks based on common patterns
        let validation_checks = self.suggest_validation_checks(intent, message);

        // File structure
        let file_structure = self.suggest_file_structure(intent, message);

        json!({
            "needed": intent.pattern_type != "research",
            "language": language,
            "libraries": libraries,
            "approach": self.suggest_coding_approach(intent, message),
            "file_structure": file_structure,
            "validation_checks": validation_checks
        })
    }

    fn suggest_coding_approach(&self, intent: &IntentPattern, message: &str) -> String {
        let msg_lower = message.to_lowercase();

        if msg_lower.contains("predict") || msg_lower.contains("forecast") {
            "Use scikit-learn for prediction models. Split data into train/test sets. Validate with cross-validation."
        } else if msg_lower.contains("analysis") {
            "Use file-first approach: write analysis script to file, then execute. Include data validation before processing."
        } else if msg_lower.contains("chart") || msg_lower.contains("visuali") {
            "Use matplotlib/seaborn for static charts. Save to outputs/ directory with descriptive names."
        } else {
            "Write code to file first using apply_patch, then execute. Handle errors gracefully."
        }.to_string()
    }

    fn suggest_file_structure(&self, intent: &IntentPattern, message: &str) -> Value {
        let msg_lower = message.to_lowercase();

        if msg_lower.contains("stock") || msg_lower.contains("financial") {
            json!({
                "core/fetch_data.py": "Reusable data fetching utility",
                "core/indicators.py": "Technical analysis library",
                "core/report.py": "Report generation templates",
                "outputs/": "Generated charts and reports"
            })
        } else if msg_lower.contains("data") {
            json!({
                "core/load_data.py": "Data loading utilities",
                "core/analysis.py": "Analysis functions",
                "outputs/": "Results and visualizations"
            })
        } else {
            json!({
                "core/utils.py": "Shared utilities",
                "main.py": "Main execution script",
                "outputs/": "Generated outputs"
            })
        }
    }

    fn suggest_validation_checks(&self, intent: &IntentPattern, message: &str) -> Vec<String> {
        let msg_lower = message.to_lowercase();
        let mut checks = Vec::new();

        // Data validation
        if msg_lower.contains("data") || msg_lower.contains("analysis") || msg_lower.contains("stock") {
            checks.push("Check if DataFrame is empty before processing".to_string());
            checks.push("Verify required columns exist (e.g., 'Close', 'Date')".to_string());
            checks.push("Handle None/NaN values from API responses".to_string());
        }

        // Timezone handling
        if msg_lower.contains("date") || msg_lower.contains("time") || msg_lower.contains("stock") {
            checks.push("Use UTC timezone for all datetime operations".to_string());
            checks.push("Convert strings to datetime before date arithmetic".to_string());
        }

        // File handling
        if msg_lower.contains("file") || msg_lower.contains("read") || msg_lower.contains("load") {
            checks.push("Check file exists before reading".to_string());
            checks.push("Handle file not found gracefully".to_string());
        }

        // API handling
        if msg_lower.contains("api") || msg_lower.contains("fetch") || msg_lower.contains("stock") {
            checks.push("Check API response for None values".to_string());
            checks.push("Handle rate limiting and retries".to_string());
        }

        if checks.is_empty() {
            checks.push("Validate inputs before processing".to_string());
        }

        checks
    }

    /// Load skill content from disk for auto-injection
    fn load_skill_content(&self, skill_name: &str) -> Result<String> {
        let skills_dir = self.fs.skills_dir()
            .ok_or_else(|| ZeroError::Tool("Skills directory not configured".to_string()))?;

        let skill_file = skills_dir.join(skill_name).join("SKILL.md");

        if !skill_file.exists() {
            return Err(ZeroError::Tool(format!("Skill file not found: {}", skill_name)));
        }

        std::fs::read_to_string(&skill_file)
            .map_err(|e| ZeroError::Tool(format!("Failed to read skill {}: {}", skill_name, e)))
    }

    /// Check if resources need reindexing.
    ///
    /// Returns true if:
    /// 1. No index exists (empty knowledge graph)
    /// 2. Disk counts don't match indexed counts (files added/removed)
    async fn needs_reindex(&self, ctx: &Arc<dyn ToolContext>) -> bool {
        // Check if we've already indexed in this session
        if ctx.get_state("index:initialized").is_some() {
            return false;
        }

        // Count resources on disk
        let disk_skill_count = self.count_disk_skills();
        let disk_agent_count = self.count_disk_agents();

        // Count resources in knowledge graph
        let (indexed_skill_count, indexed_agent_count) = self.count_indexed_resources().await;

        // Check if counts match
        let counts_match = disk_skill_count == indexed_skill_count
            && disk_agent_count == indexed_agent_count;

        // Also check if index is completely empty
        let index_empty = indexed_skill_count == 0 && indexed_agent_count == 0;

        if !counts_match || index_empty {
            tracing::debug!(
                disk_skills = disk_skill_count,
                disk_agents = disk_agent_count,
                indexed_skills = indexed_skill_count,
                indexed_agents = indexed_agent_count,
                "Index needs reindexing"
            );
            return true;
        }

        false
    }

    /// Count skills on disk
    fn count_disk_skills(&self) -> usize {
        self.fs.skills_dir()
            .and_then(|dir| {
                if !dir.exists() { return None; }
                std::fs::read_dir(&dir).ok().map(|entries| {
                    entries.filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                        .count()
                })
            })
            .unwrap_or(0)
    }

    /// Count agents on disk
    fn count_disk_agents(&self) -> usize {
        self.fs.agents_dir()
            .and_then(|dir| {
                if !dir.exists() { return None; }
                scan_agents_dir(&dir).ok().map(|agents| agents.len())
            })
            .unwrap_or(0)
    }

    /// Count indexed resources from knowledge graph
    async fn count_indexed_resources(&self) -> (usize, usize) {
        if let Some(ref graph_store) = self.graph_store {
            match graph_store.count_entities_by_type("indexer").await {
                Ok(counts) => {
                    let skills = counts.get("skill").copied().unwrap_or(0);
                    let agents = counts.get("agent").copied().unwrap_or(0);
                    (skills, agents)
                }
                Err(e) => {
                    tracing::warn!("Failed to count indexed resources: {}", e);
                    (0, 0)
                }
            }
        } else {
            (0, 0)
        }
    }

    /// Trigger reindexing by performing the indexing directly
    ///
    /// This performs the actual indexing of skills and agents into the
    /// knowledge graph and memory fact store.
    async fn trigger_reindex(&self, ctx: &Arc<dyn ToolContext>) {
        tracing::info!("Auto-indexing resources (lazy initialization)");

        // Index skills
        if let Some(skills_dir) = self.fs.skills_dir() {
            if skills_dir.exists() {
                if let Ok(skills) = scan_skills_dir(&skills_dir) {
                    let mut indexed_skills = Vec::new();
                    let mut stored_mtimes: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

                    for skill in &skills {
                        let mtime_secs = skill.mtime
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);

                        indexed_skills.push(json!({
                            "name": skill.name,
                            "description": skill.description,
                            "trigger_keywords": skill.trigger_keywords,
                            "domain_hints": skill.domain_hints,
                            "metadata": {
                                "file_path": skill.file_path.to_string_lossy().to_string(),
                                "mtime": mtime_secs
                            }
                        }));

                        stored_mtimes.insert(skill.name.clone(), mtime_secs);

                        // Store to memory facts for semantic search
                        if let Some(ref fact_store) = self.fact_store {
                            let content = format!(
                                "{} {} {}",
                                skill.name,
                                skill.description,
                                skill.trigger_keywords.join(" ")
                            );
                            let _ = fact_store.save_fact(
                                "indexer",
                                "skill",
                                &format!("skill:{}", skill.name),
                                &content,
                                1.0,
                                None,
                            ).await;
                        }

                        // Store to knowledge graph
                        if let Some(ref graph_store) = self.graph_store {
                            let _ = graph_store.store_entity(
                                "indexer",
                                "skill",
                                &skill.name,
                                json!({
                                    "description": skill.description,
                                    "trigger_keywords": skill.trigger_keywords,
                                    "domain_hints": skill.domain_hints,
                                    "file_path": skill.file_path.to_string_lossy().to_string()
                                }),
                            ).await;
                        }
                    }

                    ctx.set_state("index:skills".to_string(), json!(indexed_skills));
                    ctx.set_state("index:skills_mtimes".to_string(), json!(stored_mtimes));

                    tracing::debug!("Indexed {} skills", skills.len());
                }
            }
        }

        // Index agents
        if let Some(agents_dir) = self.fs.agents_dir() {
            if agents_dir.exists() {
                if let Ok(agents) = scan_agents_dir(&agents_dir) {
                    let mut indexed_agents = Vec::new();
                    let mut stored_mtimes: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

                    for agent in &agents {
                        let mtime_secs = agent.mtime
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);

                        indexed_agents.push(json!({
                            "name": agent.name,
                            "display_name": agent.display_name,
                            "description": agent.description,
                            "model": agent.model,
                            "provider_id": agent.provider_id,
                            "tools": agent.tools,
                            "skills": agent.skills,
                            "mcps": agent.mcps,
                            "metadata": {
                                "file_path": agent.file_path.to_string_lossy().to_string(),
                                "mtime": mtime_secs
                            }
                        }));

                        stored_mtimes.insert(agent.name.clone(), mtime_secs);

                        // Store to memory facts for semantic search
                        if let Some(ref fact_store) = self.fact_store {
                            let content = format!(
                                "{} {} {} {} {}",
                                agent.name,
                                agent.display_name,
                                agent.description,
                                agent.skills.join(" "),
                                agent.tools.join(" ")
                            );
                            let _ = fact_store.save_fact(
                                "indexer",
                                "agent",
                                &format!("agent:{}", agent.name),
                                &content,
                                1.0,
                                None,
                            ).await;
                        }

                        // Store to knowledge graph
                        if let Some(ref graph_store) = self.graph_store {
                            let _ = graph_store.store_entity(
                                "indexer",
                                "agent",
                                &agent.name,
                                json!({
                                    "display_name": agent.display_name,
                                    "description": agent.description,
                                    "model": agent.model,
                                    "provider_id": agent.provider_id,
                                    "tools": agent.tools,
                                    "skills": agent.skills,
                                    "mcps": agent.mcps,
                                    "file_path": agent.file_path.to_string_lossy().to_string()
                                }),
                            ).await;
                        }
                    }

                    ctx.set_state("index:agents".to_string(), json!(indexed_agents));
                    ctx.set_state("index:agents_mtimes".to_string(), json!(stored_mtimes));

                    tracing::debug!("Indexed {} agents", agents.len());
                }
            }
        }

        // Mark initialization complete
        ctx.set_state("index:initialized".to_string(), json!(true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    struct TestFs {
        base: PathBuf,
    }

    impl FileSystemContext for TestFs {
        fn conversation_dir(&self, _id: &str) -> Option<PathBuf> { None }
        fn outputs_dir(&self) -> Option<PathBuf> { None }
        fn skills_dir(&self) -> Option<PathBuf> { Some(self.base.join("skills")) }
        fn agents_dir(&self) -> Option<PathBuf> { Some(self.base.join("agents")) }
        fn agent_data_dir(&self, _id: &str) -> Option<PathBuf> { None }
        fn python_executable(&self) -> Option<PathBuf> { None }
        fn vault_path(&self) -> Option<PathBuf> { Some(self.base.clone()) }
        fn wards_root_dir(&self) -> Option<PathBuf> { Some(self.base.join("wards")) }
    }

    fn create_tool() -> AnalyzeIntentTool {
        let dir = TempDir::new().unwrap();
        let fs = Arc::new(TestFs { base: dir.path().to_path_buf() });
        AnalyzeIntentTool::new(fs)
    }

    #[test]
    fn test_detect_intent_research() {
        let tool = create_tool();
        let result = tool.detect_intent_pattern("research transformer architectures");
        assert_eq!(result.pattern_type, "research");
    }

    #[test]
    fn test_detect_intent_analyze() {
        let tool = create_tool();
        let result = tool.detect_intent_pattern("analyze the performance");
        assert_eq!(result.pattern_type, "analyze");
    }

    #[test]
    fn test_detect_domain_financial() {
        let tool = create_tool();
        let result = tool.detect_domain("get stock price for AAPL");
        assert_eq!(result, "financial");
    }

    #[test]
    fn test_detect_domain_web() {
        let tool = create_tool();
        let result = tool.detect_domain("create a REST API");
        assert_eq!(result, "web");
    }

    #[test]
    fn test_tool_name() {
        let tool = create_tool();
        assert_eq!(tool.name(), "analyze_intent");
    }

    #[test]
    fn test_suggest_ward_name() {
        let tool = create_tool();
        let domain = tool.detect_domain("get me a report on LMND stock");
        let result = tool.suggest_generic_ward_name(&domain, "get me a report on LMND stock");
        // Should suggest a generic name like "stock-analyst" for financial domain
        assert!(result.contains("stock") || result.contains("analyst") || result.contains("financial"));
    }
}
