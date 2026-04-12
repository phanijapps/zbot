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

    // Stage 1: query kg_aliases by normalized_form, then verify entity
    // type + agent scope on the join target. Uses idx_aliases_normalized.
    let mut stmt = conn
        .prepare(
            "SELECT a.entity_id FROM kg_aliases a \
             INNER JOIN kg_entities e ON e.id = a.entity_id \
             WHERE a.normalized_form = ?1 \
               AND e.entity_type = ?2 \
               AND (e.agent_id = ?3 OR e.agent_id = '__global__') \
             LIMIT 1",
        )
        .map_err(|e| format!("prepare failed: {e}"))?;

    let row: Option<String> = stmt
        .query_row(params![normalized, type_str, agent_id], |r| r.get(0))
        .ok();
    Ok(row)
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

/// Embedding similarity match — queries `kg_name_index` (sqlite-vec virtual table)
/// for nearest neighbours, then filters by agent and entity type.
///
/// For L2-normalised embeddings, cosine ≥ 0.87 ⇔ L2_sq ≤ 0.26.
fn embedding_match(
    conn: &Connection,
    agent_id: &str,
    candidate: &Entity,
    candidate_emb: &[f32],
) -> Result<Option<String>, String> {
    if candidate_emb.is_empty() {
        return Ok(None);
    }
    let embedding_json =
        serde_json::to_string(candidate_emb).map_err(|e| format!("serialize embedding: {e}"))?;
    let type_str = candidate.entity_type.as_str();

    // Cosine ≥ 0.87 on L2-normalised embeddings ⇒ L2_sq ≤ 0.26.
    const L2_SQ_THRESHOLD: f32 = 0.26;
    let mut stmt = conn
        .prepare(
            "SELECT i.entity_id, i.distance \
             FROM kg_name_index i \
             INNER JOIN kg_entities e ON e.id = i.entity_id \
             WHERE i.name_embedding MATCH ?1 \
               AND (e.agent_id = ?2 OR e.agent_id = '__global__') \
               AND e.entity_type = ?3 \
             ORDER BY i.distance \
             LIMIT 5",
        )
        .map_err(|e| format!("prepare failed: {e}"))?;

    let rows = stmt
        .query_map(params![embedding_json, agent_id, type_str], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
        })
        .map_err(|e| format!("query failed: {e}"))?;

    for row in rows {
        let (id, dist) = row.map_err(|e| format!("row read failed: {e}"))?;
        if dist <= L2_SQ_THRESHOLD {
            return Ok(Some(id));
        }
    }
    Ok(None)
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
}
