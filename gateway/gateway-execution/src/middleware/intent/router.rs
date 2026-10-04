//! Intent routing: trivial bypass + procedure match + agent (which searches).

use super::agent::{run_intent_decision, IntentAgentDeps, INTENT_AGENT_BUDGET};
use super::contract::{
    ExecutionApproach, ExecutionStrategy, IntentAnalysis, PinnedProcedure, WardAction,
    WardRecommendation,
};
use agent_primitives::vault_paths::SharedVaultPaths;
use zbot_stores_traits::ProcedureStore;

/// Greetings and non-task messages bypass the agent entirely — no LLM call.
fn is_trivial(message: &str) -> bool {
    let trimmed = message.trim();
    let word_count = trimmed.split_whitespace().count();
    let trivial = [
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
    trivial
        .iter()
        .any(|p| lower == *p || (lower.starts_with(p) && word_count <= 4))
}

fn trivial_analysis() -> IntentAnalysis {
    IntentAnalysis {
        primary_intent: String::new(),
        hidden_intents: vec![],
        solution_path: vec![],
        recommended_skills: vec![],
        recommended_agents: vec![],
        recommended_procedures: vec![],
        recommended_capabilities: vec![],
        ward_recommendation: WardRecommendation {
            action: WardAction::UseExisting,
            ward_name: "general".to_string(),
            subdirectory: None,
            structure: std::collections::HashMap::new(),
            reason: "Trivial message".to_string(),
        },
        execution_strategy: ExecutionStrategy {
            approach: ExecutionApproach::Simple,
            explanation: String::new(),
        },
        complexity: None,
        explanation: String::new(),
        pinned_procedure: None,
    }
}

/// Deterministic procedure name match — no LLM needed.
async fn match_procedure(
    procedure_store: Option<&dyn ProcedureStore>,
    message: &str,
) -> Option<PinnedProcedure> {
    let store = procedure_store?;
    let names = store
        .list_procedure_names("root", 500)
        .await
        .unwrap_or_default();
    if names.is_empty() {
        return None;
    }
    let haystack = message.to_lowercase();
    names
        .into_iter()
        .find(|(name, _)| {
            let needle = name.to_lowercase();
            needle.len() >= 4
                && haystack
                    .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                    .any(|word| word == needle)
        })
        .map(|(name, ward_id)| PinnedProcedure { name, ward_id })
}

/// An explicitly degraded analysis never recommends creating a ward.
pub fn fallback_analysis(user_message: &str, reason: &str) -> IntentAnalysis {
    let mut analysis = trivial_analysis();
    analysis.primary_intent = super::catalog::advisory_string(user_message.trim(), 60);
    if analysis.primary_intent.is_empty() {
        analysis.primary_intent = "user request".into();
    }
    analysis.ward_recommendation.ward_name = "scratch".into();
    analysis.ward_recommendation.reason =
        "Analysis unavailable; retain the current workspace".into();
    let safe_code = match reason {
        "deadline_exceeded"
        | "missing_json_block"
        | "invalid_json_block"
        | "provider_request_failed"
        | "client_configuration"
        | "unsupported_output_mode"
        | "request_budget_exhausted"
        | "invalid_provider_response"
        | "missing_submission"
        | "invalid_submission"
        | "invalid_tool_arguments"
        | "unexpected_tool_calls"
        | "invalid_json"
        | "invalid_decision_shape"
        | "invalid_primary_intent"
        | "advisory_limit"
        | "invalid_complexity"
        | "unknown_resource"
        | "unknown_capability"
        | "unsafe_ward"
        | "unknown_ward"
        | "unsafe_subdirectory"
        | "reserved_fallback_marker" => reason,
        _ => "unavailable",
    };
    analysis.execution_strategy.explanation = format!("fallback analysis: {safe_code}");
    analysis
}

pub fn is_fallback_analysis(analysis: &IntentAnalysis) -> bool {
    analysis
        .execution_strategy
        .explanation
        .starts_with("fallback analysis:")
}

/// Classify a user request. The agent searches and decides simple vs graph.
pub async fn analyze_intent(deps: &IntentAgentDeps, user_message: &str) -> IntentAnalysis {
    tokio::time::timeout(
        INTENT_AGENT_BUDGET,
        analyze_with_bypasses(deps, user_message),
    )
    .await
    .unwrap_or_else(|_| fallback_analysis(user_message, "deadline_exceeded"))
}

async fn analyze_with_bypasses(deps: &IntentAgentDeps, user_message: &str) -> IntentAnalysis {
    if is_trivial(user_message) {
        return trivial_analysis();
    }

    if let Some(pinned) = match_procedure(deps.procedure_store.as_deref(), user_message).await {
        let mut analysis = trivial_analysis();
        analysis.execution_strategy.explanation =
            "Request names a learned procedure — direct invocation".to_string();
        analysis.pinned_procedure = Some(pinned);
        return analysis;
    }

    match run_intent_decision(deps, user_message).await {
        Ok(analysis) => analysis,
        Err(error) => {
            tracing::warn!(code = error.0, "intent analysis using fallback");
            fallback_analysis(user_message, error.0)
        }
    }
}

/// Load the intent prompt, preferring the vault-local override.
pub fn load_intent_analysis_prompt(paths: &SharedVaultPaths) -> String {
    let override_path = paths.config_dir().join("intent-analysis-prompt.md");
    match std::fs::read_to_string(&override_path) {
        Ok(content) if !content.trim().is_empty() => content,
        _ => super::prompt::INTENT_AGENT_PROMPT.to_string(),
    }
}
