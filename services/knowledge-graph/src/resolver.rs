//! EntityResolver — merges entity variants (e.g., "Savarkar" ↔ "V.D. Savarkar")
//! on write. Runs a cascade of matchers; the first match wins.
//!
//! Cascade order (cheapest first):
//!   1. Exact normalized match (lowercase + strip honorifics)
//!   2. Fuzzy name match (Levenshtein distance ≤ 3) within same type
//!   3. Embedding similarity (cosine ≥ 0.87) on name+description within same type

use crate::Entity;
use rusqlite::{params, Connection};

/// Outcome of resolving a candidate entity against existing ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveOutcome {
    /// Existing entity found; candidate should be merged into it.
    /// Caller should add candidate.name to the existing entity's aliases.
    Merge {
        existing_id: String,
        reason: MatchReason,
    },
    /// No match; candidate should be created as a new entity.
    Create,
}

/// Why a match was selected — useful for observability and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchReason {
    ExactNormalized,
    FuzzyName,
    EmbeddingSimilarity,
}

/// Resolve a candidate entity against existing entities in the same agent+type scope.
///
/// This is a blocking function that operates on a SQLite Connection — the
/// caller is responsible for holding the connection lock.
pub fn resolve(
    conn: &Connection,
    agent_id: &str,
    candidate: &Entity,
    candidate_embedding: Option<&[f32]>,
) -> Result<ResolveOutcome, String> {
    // 1. Exact normalized match
    if let Some(existing_id) = exact_match(conn, agent_id, candidate)? {
        return Ok(ResolveOutcome::Merge {
            existing_id,
            reason: MatchReason::ExactNormalized,
        });
    }

    // 2. Fuzzy name match
    if let Some(existing_id) = fuzzy_match(conn, agent_id, candidate)? {
        return Ok(ResolveOutcome::Merge {
            existing_id,
            reason: MatchReason::FuzzyName,
        });
    }

    // 3. Embedding similarity (only if embedding provided)
    if let Some(emb) = candidate_embedding {
        if let Some(existing_id) = embedding_match(conn, agent_id, candidate, emb)? {
            return Ok(ResolveOutcome::Merge {
                existing_id,
                reason: MatchReason::EmbeddingSimilarity,
            });
        }
    }

    Ok(ResolveOutcome::Create)
}

/// Normalize a name for exact-match comparison: lowercase, trim, strip
/// common honorifics and punctuation.
pub fn normalize_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let stripped = strip_honorifics(&lower);
    stripped.replace(['.', ','], "")
}

fn strip_honorifics(name: &str) -> String {
    const HONORIFICS: &[&str] = &[
        "dr ", "dr. ", "mr ", "mr. ", "mrs ", "mrs. ", "ms ", "ms. ", "prof ", "prof. ", "sir ",
        "shri ", "smt ",
    ];
    for h in HONORIFICS {
        if let Some(rest) = name.strip_prefix(h) {
            return rest.to_string();
        }
    }
    name.to_string()
}

fn exact_match(
    conn: &Connection,
    agent_id: &str,
    candidate: &Entity,
) -> Result<Option<String>, String> {
    let normalized = normalize_name(&candidate.name);
    let type_str = candidate.entity_type.as_str();

    let mut stmt = conn
        .prepare(
            "SELECT id, name, aliases FROM kg_entities \
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND entity_type = ?2",
        )
        .map_err(|e| format!("prepare failed: {e}"))?;

    let rows = stmt
        .query_map(params![agent_id, type_str], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let aliases: Option<String> = row.get(2)?;
            Ok((id, name, aliases))
        })
        .map_err(|e| format!("query failed: {e}"))?;

    for row in rows {
        let (id, name, aliases) = row.map_err(|e| format!("row read failed: {e}"))?;
        if normalize_name(&name) == normalized {
            return Ok(Some(id));
        }
        if let Some(aliases_json) = aliases {
            if alias_list_contains(&aliases_json, &normalized) {
                return Ok(Some(id));
            }
        }
    }
    Ok(None)
}

fn alias_list_contains(aliases_json: &str, normalized_target: &str) -> bool {
    serde_json::from_str::<Vec<String>>(aliases_json)
        .map(|list| list.iter().any(|a| normalize_name(a) == normalized_target))
        .unwrap_or(false)
}

/// Fuzzy name match: Levenshtein distance ≤ 3 within the same type.
/// Only applied when candidate name is long enough to avoid false matches
/// on short strings (e.g., "A" and "B" have distance 1).
fn fuzzy_match(
    conn: &Connection,
    agent_id: &str,
    candidate: &Entity,
) -> Result<Option<String>, String> {
    let candidate_norm = normalize_name(&candidate.name);
    if candidate_norm.len() < 6 {
        return Ok(None); // too short for reliable fuzzy matching
    }

    let type_str = candidate.entity_type.as_str();
    let mut stmt = conn
        .prepare(
            "SELECT id, name FROM kg_entities \
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND entity_type = ?2 \
             ORDER BY mention_count DESC LIMIT 100",
        )
        .map_err(|e| format!("prepare failed: {e}"))?;

    let rows = stmt
        .query_map(params![agent_id, type_str], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query failed: {e}"))?;

    for row in rows {
        let (id, name) = row.map_err(|e| format!("row read failed: {e}"))?;
        let name_norm = normalize_name(&name);
        if name_norm.len() < 6 {
            continue;
        }
        if levenshtein(&candidate_norm, &name_norm) <= 3 {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// Compute Levenshtein distance between two strings.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let (m, n) = (a_chars.len(), b_chars.len());

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0_usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            let insert = curr[j - 1] + 1;
            let delete = prev[j] + 1;
            let substitute = prev[j - 1] + cost;
            curr[j] = insert.min(delete).min(substitute);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Embedding similarity match — requires candidate_embedding to be provided.
/// Uses cosine similarity ≥ 0.87 within the same type.
fn embedding_match(
    conn: &Connection,
    agent_id: &str,
    candidate: &Entity,
    candidate_emb: &[f32],
) -> Result<Option<String>, String> {
    let type_str = candidate.entity_type.as_str();
    let mut stmt = conn
        .prepare(
            "SELECT id, properties FROM kg_entities \
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND entity_type = ?2 \
             AND properties LIKE '%_name_embedding%' \
             ORDER BY mention_count DESC LIMIT 50",
        )
        .map_err(|e| format!("prepare failed: {e}"))?;

    let rows = stmt
        .query_map(params![agent_id, type_str], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|e| format!("query failed: {e}"))?;

    let mut best: Option<(String, f64)> = None;
    for row in rows {
        let (id, props_json) = row.map_err(|e| format!("row read failed: {e}"))?;
        let Some(props_str) = props_json else {
            continue;
        };
        let Ok(props) = serde_json::from_str::<serde_json::Value>(&props_str) else {
            continue;
        };
        let Some(emb_arr) = props.get("_name_embedding").and_then(|v| v.as_array()) else {
            continue;
        };
        let existing_emb: Vec<f32> = emb_arr
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();
        if existing_emb.len() != candidate_emb.len() {
            continue;
        }
        let sim = cosine_similarity(candidate_emb, &existing_emb);
        if sim >= 0.87 {
            match &best {
                Some((_, prev_sim)) if *prev_sim >= sim => {}
                _ => best = Some((id, sim)),
            }
        }
    }
    Ok(best.map(|(id, _)| id))
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut na = 0.0_f64;
    let mut nb = 0.0_f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let (x, y) = (f64::from(*x), f64::from(*y));
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

/// Add a new alias to an existing entity's alias list (JSON array).
/// Caller handles the actual DB update; this just computes the new JSON.
pub fn merge_alias(existing_aliases_json: Option<&str>, new_alias: &str) -> String {
    let mut list: Vec<String> = existing_aliases_json
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default();

    let norm_new = normalize_name(new_alias);
    if !list.iter().any(|a| normalize_name(a) == norm_new) {
        list.push(new_alias.to_string());
    }
    serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_name_lowercases_and_trims() {
        assert_eq!(normalize_name("  Savarkar  "), "savarkar");
        assert_eq!(normalize_name("V.D. Savarkar"), "vd savarkar");
    }

    #[test]
    fn normalize_name_strips_honorifics() {
        assert_eq!(normalize_name("Dr. Ambedkar"), "ambedkar");
        assert_eq!(normalize_name("Mrs. Gandhi"), "gandhi");
        assert_eq!(normalize_name("Shri Patel"), "patel");
    }

    #[test]
    fn levenshtein_basic() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("abc", "abd"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
    }

    #[test]
    fn levenshtein_handles_savarkar_variants() {
        assert_eq!(levenshtein("savarkar", "savarker"), 1);
    }

    #[test]
    fn cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        assert!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_empty_is_zero() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
        assert_eq!(cosine_similarity(&[1.0], &[]), 0.0);
    }

    #[test]
    fn merge_alias_dedups_normalized() {
        let existing = Some(r#"["V.D. Savarkar"]"#);
        let merged = merge_alias(existing, "v.d. savarkar");
        let list: Vec<String> = serde_json::from_str(&merged).unwrap();
        assert_eq!(list.len(), 1, "Case/punct variants should dedup");
    }

    #[test]
    fn merge_alias_adds_new() {
        let existing = Some(r#"["Savarkar"]"#);
        let merged = merge_alias(existing, "V.D. Savarkar");
        let list: Vec<String> = serde_json::from_str(&merged).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn merge_alias_from_empty() {
        let merged = merge_alias(None, "Savarkar");
        let list: Vec<String> = serde_json::from_str(&merged).unwrap();
        assert_eq!(list, vec!["Savarkar"]);
    }

    #[test]
    fn alias_list_contains_matches_normalized() {
        assert!(alias_list_contains(r#"["V.D. Savarkar"]"#, "vd savarkar"));
        assert!(!alias_list_contains(r#"["V.D. Savarkar"]"#, "gandhi"));
    }
}
