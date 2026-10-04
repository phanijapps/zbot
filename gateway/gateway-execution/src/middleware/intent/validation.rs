//! One semantic boundary for every provider transport.
use super::catalog::{advisory_string, safe_id, Catalog};
use super::contract::{IntentAnalysis, WardAction};
use serde_json::Value;
use std::collections::HashSet;

pub(crate) fn decision_schema() -> Value {
    let mut schema =
        serde_json::to_value(schemars::schema_for!(IntentAnalysis)).expect("static intent schema");
    // Layout is host-owned. Free-form maps also conflict with native strict schemas.
    if let Some(ward) = schema.pointer_mut("/$defs/WardRecommendation/properties/structure") {
        *ward = serde_json::json!({"type":"object","properties":{},"additionalProperties":false});
    }
    if let Some(ward) = schema.pointer_mut("/definitions/WardRecommendation/properties/structure") {
        *ward = serde_json::json!({"type":"object","properties":{},"additionalProperties":false});
    }
    // Ollama tool grammars interpret unresolved object references as strings.
    // Inline this finite generated contract for every transport, keeping one schema.
    let source = schema.clone();
    inline_contract_refs(&mut schema, &source);
    if let Some(root) = schema.as_object_mut() {
        root.remove("$defs");
        root.remove("definitions");
    }
    schema
}

fn inline_contract_refs(node: &mut Value, source: &Value) {
    if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
        let target = source
            .pointer(
                reference
                    .strip_prefix('#')
                    .expect("local generated schema reference"),
            )
            .expect("generated schema reference resolves")
            .clone();
        *node = target;
    }
    match node {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if key != "$defs" && key != "definitions" {
                    inline_contract_refs(value, source);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                inline_contract_refs(item, source);
            }
        }
        _ => {}
    }
}

fn ids(values: &[String], allowed: &HashSet<String>) -> bool {
    values.len() <= 20 && values.iter().all(|id| safe_id(id) && allowed.contains(id))
}
fn clean(text: &mut String, limit: usize) -> bool {
    if text.chars().count() > limit {
        return false;
    }
    *text = advisory_string(text, limit);
    true
}
fn list(values: &mut [String]) -> bool {
    values.len() <= 12 && values.iter_mut().all(|v| clean(v, 512))
}
pub(crate) fn validate(value: Value, catalog: &Catalog) -> Result<IntentAnalysis, &'static str> {
    let mut a: IntentAnalysis =
        serde_json::from_value(value).map_err(|_| "invalid_decision_shape")?;
    if !clean(&mut a.primary_intent, 240) || a.primary_intent.trim().is_empty() {
        return Err("invalid_primary_intent");
    }
    if !list(&mut a.hidden_intents)
        || !list(&mut a.solution_path)
        || !clean(&mut a.explanation, 1024)
        || !clean(&mut a.execution_strategy.explanation, 1024)
        || !clean(&mut a.ward_recommendation.reason, 512)
    {
        return Err("advisory_limit");
    }
    if a.execution_strategy
        .explanation
        .starts_with("fallback analysis:")
    {
        return Err("reserved_fallback_marker");
    }
    if a.complexity
        .as_ref()
        .is_some_and(|c| !matches!(c.as_str(), "S" | "M" | "L" | "XL"))
    {
        return Err("invalid_complexity");
    }
    if !ids(&a.recommended_skills, &catalog.skills)
        || !ids(&a.recommended_agents, &catalog.agents)
        || !ids(&a.recommended_procedures, &catalog.procedures)
    {
        return Err("unknown_resource");
    }
    if a.recommended_capabilities.len() > 20
        || a.recommended_capabilities.iter().any(|c| {
            (c.agent_id != "root" && !catalog.agents.contains(&c.agent_id))
                || !safe_id(&c.agent_id)
                || !ids(&c.skills, &catalog.skills)
                || !ids(&c.mcps, &catalog.mcps)
        })
    {
        return Err("unknown_capability");
    }
    let ward = &a.ward_recommendation;
    if ward.ward_name.is_empty()
        || ward.ward_name.len() > 64
        || !ward
            .ward_name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
        || !ward.structure.is_empty()
    {
        return Err("unsafe_ward");
    }
    if ward.action == WardAction::UseExisting
        && ward.ward_name != "scratch"
        && !catalog.wards.contains(&ward.ward_name)
    {
        return Err("unknown_ward");
    }
    if ward.subdirectory.as_ref().is_some_and(|path| {
        path.len() > 256
            || path.is_empty()
            || path.split('/').any(|part| {
                part.is_empty()
                    || part == "."
                    || part == ".."
                    || !part
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
            })
    }) {
        return Err("unsafe_subdirectory");
    }
    Ok(a)
}
