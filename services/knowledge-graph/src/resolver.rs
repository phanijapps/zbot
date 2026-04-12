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

    // 2. Embedding similarity (only if embedding provided)
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
    // vec0 KNN queries require a bare `k = ?` or `LIMIT ?` on the virtual table
    // itself — JOINs and extra WHERE predicates are not accepted at prepare time.
    // So we do a two-step: pull the top-K nearest ids from the index, then filter
    // by agent / entity_type against `kg_entities`.
    const K: i64 = 10;
    let mut stmt = conn
        .prepare(
            "SELECT entity_id, distance \
             FROM kg_name_index \
             WHERE name_embedding MATCH ?1 \
             ORDER BY distance \
             LIMIT ?2",
        )
        .map_err(|e| format!("prepare failed: {e}"))?;

    let rows = stmt
        .query_map(params![embedding_json, K], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
        })
        .map_err(|e| format!("query failed: {e}"))?;

    let mut filter_stmt = conn
        .prepare(
            "SELECT 1 FROM kg_entities \
             WHERE id = ?1 \
               AND entity_type = ?2 \
               AND (agent_id = ?3 OR agent_id = '__global__') \
             LIMIT 1",
        )
        .map_err(|e| format!("prepare filter failed: {e}"))?;

    for row in rows {
        let (id, dist) = row.map_err(|e| format!("row read failed: {e}"))?;
        if dist > L2_SQ_THRESHOLD {
            // Rows are ordered by distance asc; once we exceed threshold, stop.
            break;
        }
        let matches: Option<i64> = filter_stmt
            .query_row(params![id, type_str, agent_id], |r| r.get(0))
            .ok();
        if matches.is_some() {
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
}
