//! Host metadata is the authority; indexed memory only ranks catalog entries.
use super::agent::IntentAgentDeps;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub(crate) struct Catalog {
    pub skills: HashSet<String>,
    pub agents: HashSet<String>,
    pub mcps: HashSet<String>,
    pub wards: HashSet<String>,
    pub procedures: HashSet<String>,
    pub context: Value,
}

pub(crate) fn advisory_string(text: &str, limit: usize) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(limit)
        .collect()
}

pub(crate) fn encoded_data(data: &Value) -> String {
    // Prevent data from closing the advisory delimiter or introducing a fence.
    data.to_string()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('`', "\\u0060")
        .replace('*', "\\u002a")
        .replace('#', "\\u0023")
}

pub(crate) async fn retrieve(deps: &IntentAgentDeps, message: &str) -> Catalog {
    let memory = deps
        .fact_store
        .recall_facts("root", message, 20)
        .await
        .unwrap_or(Value::Null);
    let rank: HashMap<String, usize> = memory["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|fact| fact["key"].as_str())
        .take(20)
        .enumerate()
        .map(|(rank, key)| (key.to_owned(), rank))
        .collect();
    let mut context = json!({});
    let mut sets = HashMap::new();
    for (kind, prefix, limit) in [
        ("skills", "skill", 20),
        ("agents", "agent", 20),
        ("mcps", "mcp", 20),
        ("wards", "ward", 64),
    ] {
        let mut entries: Vec<Value> = deps.resources[kind].as_array().into_iter().flatten().filter_map(|entry| {
            let id = entry.as_str().or_else(|| entry["id"].as_str())?;
            if !safe_id(id) { return None; }
            Some(json!({"id":id,"name":advisory_string(entry["name"].as_str().unwrap_or(id),128),"description":advisory_string(entry["description"].as_str().unwrap_or_default(),512)}))
        }).collect();
        sets.insert(
            kind,
            entries
                .iter()
                .filter_map(|v| v["id"].as_str().map(str::to_owned))
                .collect::<HashSet<_>>(),
        );
        entries.sort_by(|a, b| {
            let a = a["id"].as_str().unwrap_or_default();
            let b = b["id"].as_str().unwrap_or_default();
            rank.get(&format!("{prefix}:{a}"))
                .unwrap_or(&usize::MAX)
                .cmp(rank.get(&format!("{prefix}:{b}")).unwrap_or(&usize::MAX))
                .then_with(|| a.cmp(b))
        });
        entries.truncate(limit);
        context[kind] = json!(entries);
    }
    let names = match &deps.procedure_store {
        Some(store) => store
            .list_procedure_names("root", 20)
            .await
            .unwrap_or_default(),
        None => vec![],
    };
    let procedures: HashSet<String> = names
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| safe_id(name))
        .collect();
    let mut ordered: Vec<_> = procedures.iter().cloned().collect();
    ordered.sort();
    context["procedures"] = json!(ordered);
    Catalog {
        skills: sets.remove("skills").unwrap_or_default(),
        agents: sets.remove("agents").unwrap_or_default(),
        mcps: sets.remove("mcps").unwrap_or_default(),
        wards: sets.remove("wards").unwrap_or_default(),
        procedures,
        context,
    }
}

pub(crate) fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
}
