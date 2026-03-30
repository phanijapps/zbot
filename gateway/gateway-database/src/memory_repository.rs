// ============================================================================
// MEMORY FACTS REPOSITORY
// CRUD, hybrid search, and embedding cache for the memory evolution system
// ============================================================================

use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::DatabaseManager;

// ============================================================================
// TYPES
// ============================================================================

/// A structured memory fact extracted from session distillation or manual save.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFact {
    pub id: String,
    pub session_id: Option<String>,
    pub agent_id: String,
    pub scope: String,
    pub category: String,
    pub key: String,
    pub content: String,
    pub confidence: f64,
    pub mention_count: i32,
    pub source_summary: Option<String>,
    /// Raw f32 embedding bytes (little-endian). `None` if not yet embedded.
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
    /// Ward (sandbox) this fact belongs to. `"__global__"` means shared across all wards.
    pub ward_id: String,
    /// If set, the key of the newer fact that contradicts this one.
    pub contradicted_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
}

/// A memory fact with a computed relevance score from hybrid search.
#[derive(Debug, Clone, Serialize)]
pub struct ScoredFact {
    pub fact: MemoryFact,
    pub score: f64,
}

// ============================================================================
// MEMORY REPOSITORY
// ============================================================================

/// Repository for memory fact operations.
pub struct MemoryRepository {
    db: Arc<DatabaseManager>,
}

impl MemoryRepository {
    /// Create a new memory repository.
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    // =========================================================================
    // FACT CRUD
    // =========================================================================

    /// Upsert a memory fact.
    ///
    /// On conflict (same agent_id + scope + key), updates content, bumps mention_count,
    /// and refreshes updated_at. Embedding is updated if provided.
    pub fn upsert_memory_fact(&self, fact: &MemoryFact) -> Result<(), String> {
        let embedding_blob = fact.embedding.as_ref().map(|v| f32_vec_to_blob(v));

        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO memory_facts (id, session_id, agent_id, scope, category, key, content, confidence, mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                 ON CONFLICT(agent_id, scope, ward_id, key) DO UPDATE SET
                    content = excluded.content,
                    confidence = MAX(memory_facts.confidence, excluded.confidence),
                    mention_count = memory_facts.mention_count + 1,
                    source_summary = COALESCE(excluded.source_summary, memory_facts.source_summary),
                    embedding = COALESCE(excluded.embedding, memory_facts.embedding),
                    updated_at = excluded.updated_at,
                    session_id = COALESCE(excluded.session_id, memory_facts.session_id)",
                params![
                    fact.id,
                    fact.session_id,
                    fact.agent_id,
                    fact.scope,
                    fact.category,
                    fact.key,
                    fact.content,
                    fact.confidence,
                    fact.mention_count,
                    fact.source_summary,
                    embedding_blob,
                    fact.ward_id,
                    fact.contradicted_by,
                    fact.created_at,
                    fact.updated_at,
                    fact.expires_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Get memory facts for an agent, optionally filtered by scope.
    pub fn get_memory_facts(
        &self,
        agent_id: &str,
        scope: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryFact>, String> {
        self.db.with_connection(|conn| {
            let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(s) = scope {
                (
                    "SELECT id, session_id, agent_id, scope, category, key, content, confidence, mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                     FROM memory_facts
                     WHERE agent_id = ?1 AND scope = ?2
                     ORDER BY updated_at DESC
                     LIMIT ?3".to_string(),
                    vec![
                        Box::new(agent_id.to_string()),
                        Box::new(s.to_string()),
                        Box::new(limit as i64),
                    ],
                )
            } else {
                (
                    "SELECT id, session_id, agent_id, scope, category, key, content, confidence, mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                     FROM memory_facts
                     WHERE agent_id = ?1
                     ORDER BY updated_at DESC
                     LIMIT ?2".to_string(),
                    vec![
                        Box::new(agent_id.to_string()),
                        Box::new(limit as i64),
                    ],
                )
            };

            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| row_to_memory_fact(row))?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Delete a memory fact by ID.
    pub fn delete_memory_fact(&self, id: &str) -> Result<bool, String> {
        self.db.with_connection(|conn| {
            let count = conn.execute(
                "DELETE FROM memory_facts WHERE id = ?1",
                params![id],
            )?;
            Ok(count > 0)
        })
    }

    /// Get a single memory fact by ID.
    pub fn get_memory_fact_by_id(&self, id: &str) -> Result<Option<MemoryFact>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                        mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                 FROM memory_facts
                 WHERE id = ?1"
            )?;

            let result = stmt.query_row(params![id], |row| row_to_memory_fact(row));

            match result {
                Ok(fact) => Ok(Some(fact)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// List memory facts with optional filters (category, scope) and pagination.
    pub fn list_memory_facts(
        &self,
        agent_id: &str,
        category: Option<&str>,
        scope: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MemoryFact>, String> {
        self.db.with_connection(|conn| {
            let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
                match (category, scope) {
                    (Some(cat), Some(scp)) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE agent_id = ?1 AND category = ?2 AND scope = ?3
                         ORDER BY updated_at DESC
                         LIMIT ?4 OFFSET ?5".to_string(),
                        vec![
                            Box::new(agent_id.to_string()),
                            Box::new(cat.to_string()),
                            Box::new(scp.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                    (Some(cat), None) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE agent_id = ?1 AND category = ?2
                         ORDER BY updated_at DESC
                         LIMIT ?3 OFFSET ?4".to_string(),
                        vec![
                            Box::new(agent_id.to_string()),
                            Box::new(cat.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                    (None, Some(scp)) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE agent_id = ?1 AND scope = ?2
                         ORDER BY updated_at DESC
                         LIMIT ?3 OFFSET ?4".to_string(),
                        vec![
                            Box::new(agent_id.to_string()),
                            Box::new(scp.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                    (None, None) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE agent_id = ?1
                         ORDER BY updated_at DESC
                         LIMIT ?2 OFFSET ?3".to_string(),
                        vec![
                            Box::new(agent_id.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                };

            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| row_to_memory_fact(row))?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Decay confidence of stale facts.
    ///
    /// Facts not updated in `older_than_days` have their confidence multiplied
    /// by `decay_factor` (e.g., 0.95). Returns number of facts decayed.
    pub fn decay_stale_facts(&self, older_than_days: u32, decay_factor: f64) -> Result<usize, String> {
        self.db.with_connection(|conn| {
            let count = conn.execute(
                "UPDATE memory_facts SET confidence = confidence * ?1, updated_at = datetime('now')
                 WHERE julianday('now') - julianday(updated_at) > ?2
                 AND confidence > 0.1",
                params![decay_factor, older_than_days],
            )?;
            Ok(count)
        })
    }

    /// Mark a fact as contradicted by a newer fact with the given key.
    ///
    /// Reduces confidence by 0.15 (floor of 0.1) and records which key contradicted it.
    pub fn mark_contradicted(&self, fact_id: &str, contradicted_by_key: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE memory_facts SET contradicted_by = ?1, confidence = MAX(0.1, confidence - 0.15) WHERE id = ?2",
                params![contradicted_by_key, fact_id],
            )?;
            Ok(())
        })
    }

    /// Archive a fact by moving it from `memory_facts` to `memory_facts_archive`.
    ///
    /// Performs an atomic INSERT-then-DELETE within a single connection so the
    /// fact is never lost. Used by the pruning subsystem to keep the active
    /// store lean without discarding data.
    pub fn archive_fact(&self, fact_id: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO memory_facts_archive
                 SELECT id, agent_id, scope, category, key, content, confidence, ward_id,
                        mention_count, source_summary, embedding, contradicted_by,
                        created_at, updated_at, datetime('now')
                 FROM memory_facts WHERE id = ?1",
                params![fact_id],
            )?;
            conn.execute("DELETE FROM memory_facts WHERE id = ?1", params![fact_id])?;
            Ok(())
        })
    }

    /// Search for facts similar to the given embedding, filtered by minimum similarity threshold.
    ///
    /// Returns facts with cosine similarity >= `min_similarity`. Used for contradiction detection.
    pub fn search_similar_facts(
        &self,
        query_embedding: &[f32],
        agent_id: &str,
        min_similarity: f64,
        limit: usize,
        ward_id: Option<&str>,
    ) -> Result<Vec<ScoredFact>, String> {
        let mut results = self.search_memory_facts_vector(query_embedding, agent_id, limit, ward_id)?;
        results.retain(|sf| sf.score >= min_similarity);
        Ok(results)
    }

    // =========================================================================
    // FTS5 KEYWORD SEARCH
    // =========================================================================

    /// Search memory facts using FTS5 BM25 keyword matching.
    ///
    /// When `ward_id` is `Some(w)`, results are filtered to facts belonging to
    /// the `__global__` ward **or** the specified ward. When `None`, all wards
    /// are returned (no ward filtering).
    pub fn search_memory_facts_fts(
        &self,
        query: &str,
        agent_id: &str,
        limit: usize,
        ward_id: Option<&str>,
    ) -> Result<Vec<ScoredFact>, String> {
        self.db.with_connection(|conn| {
            let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(w) = ward_id {
                (
                    "SELECT mf.id, mf.session_id, mf.agent_id, mf.scope, mf.category, mf.key,
                            mf.content, mf.confidence, mf.mention_count, mf.source_summary,
                            mf.embedding, mf.ward_id, mf.contradicted_by, mf.created_at, mf.updated_at, mf.expires_at,
                            rank
                     FROM memory_facts_fts fts
                     JOIN memory_facts mf ON mf.rowid = fts.rowid
                     WHERE memory_facts_fts MATCH ?1 AND mf.agent_id = ?2
                       AND (mf.ward_id = '__global__' OR mf.ward_id = ?3)
                     ORDER BY rank
                     LIMIT ?4".to_string(),
                    vec![
                        Box::new(query.to_string()),
                        Box::new(agent_id.to_string()),
                        Box::new(w.to_string()),
                        Box::new(limit as i64),
                    ],
                )
            } else {
                (
                    "SELECT mf.id, mf.session_id, mf.agent_id, mf.scope, mf.category, mf.key,
                            mf.content, mf.confidence, mf.mention_count, mf.source_summary,
                            mf.embedding, mf.ward_id, mf.contradicted_by, mf.created_at, mf.updated_at, mf.expires_at,
                            rank
                     FROM memory_facts_fts fts
                     JOIN memory_facts mf ON mf.rowid = fts.rowid
                     WHERE memory_facts_fts MATCH ?1 AND mf.agent_id = ?2
                     ORDER BY rank
                     LIMIT ?3".to_string(),
                    vec![
                        Box::new(query.to_string()),
                        Box::new(agent_id.to_string()),
                        Box::new(limit as i64),
                    ],
                )
            };

            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                let fact = row_to_memory_fact(row)?;
                let rank: f64 = row.get(16)?;
                // FTS5 rank is negative (lower = better). Normalize to 0..1 range.
                let bm25_score = (-rank).min(30.0) / 30.0;
                Ok(ScoredFact {
                    fact,
                    score: bm25_score,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    // =========================================================================
    // HYBRID SEARCH (FTS5 + VECTOR)
    // =========================================================================

    /// Hybrid search combining FTS5 keyword matching and vector cosine similarity.
    ///
    /// 1. Run FTS5 search to get keyword matches with BM25 scores
    /// 2. Run vector search by loading embeddings and computing cosine similarity in Rust
    /// 3. Combine scores: `vector_weight * cos_sim + bm25_weight * bm25_score`
    /// 4. Apply confidence, recency, and mention_count modifiers
    pub fn search_memory_facts_hybrid(
        &self,
        query_text: &str,
        query_embedding: Option<&[f32]>,
        agent_id: &str,
        limit: usize,
        vector_weight: f64,
        bm25_weight: f64,
        ward_id: Option<&str>,
    ) -> Result<Vec<ScoredFact>, String> {
        // Step 1: FTS5 keyword results
        let fts_results = self.search_memory_facts_fts(query_text, agent_id, 30, ward_id)
            .unwrap_or_default();

        // Step 2: Vector results (if embedding provided)
        let vec_results = if let Some(qe) = query_embedding {
            self.search_memory_facts_vector(qe, agent_id, 30, ward_id)?
        } else {
            Vec::new()
        };

        // Step 3: Merge results by fact ID
        let mut score_map: std::collections::HashMap<String, (Option<f64>, Option<f64>, MemoryFact)> =
            std::collections::HashMap::new();

        for sf in fts_results {
            score_map.insert(sf.fact.id.clone(), (None, Some(sf.score), sf.fact));
        }

        for sf in vec_results {
            score_map
                .entry(sf.fact.id.clone())
                .and_modify(|(vec_s, _, _)| {
                    *vec_s = Some(sf.score);
                })
                .or_insert((Some(sf.score), None, sf.fact));
        }

        // Step 4: Compute final weighted score
        let now = chrono::Utc::now();
        let mut results: Vec<ScoredFact> = score_map
            .into_values()
            .map(|(vec_score, bm25_score, fact)| {
                let vs = vec_score.unwrap_or(0.0);
                let bs = bm25_score.unwrap_or(0.0);

                let base_score = vector_weight * vs + bm25_weight * bs;

                // Confidence modifier
                let conf = fact.confidence;

                // Recency modifier: decay older facts
                let days_old = chrono::DateTime::parse_from_rfc3339(&fact.updated_at)
                    .map(|dt| (now - dt.with_timezone(&chrono::Utc)).num_days() as f64)
                    .unwrap_or(30.0);
                let recency = 1.0 / (1.0 + days_old * 0.01);

                // Mention count modifier
                let mention_boost = 1.0 + (fact.mention_count as f64 + 1.0).ln() / 10.0_f64.ln();

                let final_score = base_score * conf * recency * mention_boost;

                ScoredFact {
                    fact,
                    score: final_score,
                }
            })
            .collect();

        // Sort by score descending, take top-K
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Search memory facts by vector cosine similarity.
    ///
    /// Loads all embeddings for the agent and computes cosine similarity in Rust.
    /// This is brute-force but fast for <10K facts (~2-5ms).
    ///
    /// When `ward_id` is `Some(w)`, results are filtered to facts belonging to
    /// the `__global__` ward **or** the specified ward. When `None`, all wards
    /// are returned (no ward filtering).
    fn search_memory_facts_vector(
        &self,
        query_embedding: &[f32],
        agent_id: &str,
        limit: usize,
        ward_id: Option<&str>,
    ) -> Result<Vec<ScoredFact>, String> {
        self.db.with_connection(|conn| {
            let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(w) = ward_id {
                (
                    "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                            mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                     FROM memory_facts
                     WHERE agent_id = ?1 AND embedding IS NOT NULL
                       AND (ward_id = '__global__' OR ward_id = ?2)".to_string(),
                    vec![
                        Box::new(agent_id.to_string()),
                        Box::new(w.to_string()),
                    ],
                )
            } else {
                (
                    "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                            mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                     FROM memory_facts
                     WHERE agent_id = ?1 AND embedding IS NOT NULL".to_string(),
                    vec![
                        Box::new(agent_id.to_string()),
                    ],
                )
            };

            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| row_to_memory_fact(row))?;

            let mut scored: Vec<ScoredFact> = rows
                .filter_map(|r| r.ok())
                .filter_map(|fact| {
                    let emb = fact.embedding.as_ref()?;
                    let sim = cosine_similarity(query_embedding, emb);
                    Some(ScoredFact {
                        fact,
                        score: sim,
                    })
                })
                .collect();

            scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit);
            Ok(scored)
        })
    }

    // =========================================================================
    // EMBEDDING CACHE
    // =========================================================================

    /// Look up a cached embedding by content hash and model.
    pub fn get_cached_embedding(
        &self,
        content_hash: &str,
        model: &str,
    ) -> Result<Option<Vec<f32>>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT embedding FROM embedding_cache WHERE content_hash = ?1 AND model = ?2"
            )?;

            let result = stmt.query_row(params![content_hash, model], |row| {
                let blob: Vec<u8> = row.get(0)?;
                Ok(blob_to_f32_vec(&blob))
            });

            match result {
                Ok(vec) => Ok(Some(vec)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Cache an embedding for future reuse.
    pub fn cache_embedding(
        &self,
        content_hash: &str,
        model: &str,
        embedding: &[f32],
    ) -> Result<(), String> {
        let blob = f32_vec_to_blob(embedding);
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO embedding_cache (content_hash, model, embedding, created_at)
                 VALUES (?1, ?2, ?3, datetime('now'))",
                params![content_hash, model, blob],
            )?;
            Ok(())
        })
    }

    /// Update the embedding for an existing fact.
    pub fn update_fact_embedding(
        &self,
        fact_id: &str,
        embedding: &[f32],
    ) -> Result<(), String> {
        let blob = f32_vec_to_blob(embedding);
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE memory_facts SET embedding = ?1 WHERE id = ?2",
                params![blob, fact_id],
            )?;
            Ok(())
        })
    }

    /// Get high-confidence facts that should always be included in recall.
    pub fn get_high_confidence_facts(
        &self,
        agent_id: &str,
        min_confidence: f64,
        limit: usize,
    ) -> Result<Vec<MemoryFact>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                        mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                 FROM memory_facts
                 WHERE agent_id = ?1 AND confidence >= ?2
                 AND (expires_at IS NULL OR expires_at > datetime('now'))
                 ORDER BY confidence DESC, mention_count DESC
                 LIMIT ?3"
            )?;

            let rows = stmt.query_map(
                params![agent_id, min_confidence, limit as i64],
                |row| row_to_memory_fact(row),
            )?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Get top facts for a specific category, ordered by confidence then recency.
    ///
    /// Used to always-inject corrections into recall (regardless of query
    /// similarity) and for capability gap detection (skill/agent categories).
    pub fn get_facts_by_category(
        &self,
        agent_id: &str,
        category: &str,
        limit: usize,
    ) -> Result<Vec<MemoryFact>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                        mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                 FROM memory_facts
                 WHERE agent_id = ?1 AND category = ?2
                   AND (expires_at IS NULL OR expires_at > datetime('now'))
                 ORDER BY confidence DESC, updated_at DESC
                 LIMIT ?3"
            )?;

            let rows = stmt.query_map(
                params![agent_id, category, limit as i64],
                |row| row_to_memory_fact(row),
            )?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Count total memory facts for an agent.
    pub fn count_memory_facts(&self, agent_id: &str) -> Result<usize, String> {
        self.db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM memory_facts WHERE agent_id = ?1",
                params![agent_id],
                |row| row.get(0),
            )?;
            Ok(count as usize)
        })
    }

    /// List all memory facts across all agents with optional filters and pagination.
    pub fn list_all_memory_facts(
        &self,
        agent_id: Option<&str>,
        category: Option<&str>,
        scope: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MemoryFact>, String> {
        self.db.with_connection(|conn| {
            let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
                match (agent_id, category, scope) {
                    (Some(aid), Some(cat), Some(scp)) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE agent_id = ?1 AND category = ?2 AND scope = ?3
                         ORDER BY updated_at DESC
                         LIMIT ?4 OFFSET ?5".to_string(),
                        vec![
                            Box::new(aid.to_string()),
                            Box::new(cat.to_string()),
                            Box::new(scp.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                    (Some(aid), Some(cat), None) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE agent_id = ?1 AND category = ?2
                         ORDER BY updated_at DESC
                         LIMIT ?3 OFFSET ?4".to_string(),
                        vec![
                            Box::new(aid.to_string()),
                            Box::new(cat.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                    (Some(aid), None, Some(scp)) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE agent_id = ?1 AND scope = ?2
                         ORDER BY updated_at DESC
                         LIMIT ?3 OFFSET ?4".to_string(),
                        vec![
                            Box::new(aid.to_string()),
                            Box::new(scp.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                    (Some(aid), None, None) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE agent_id = ?1
                         ORDER BY updated_at DESC
                         LIMIT ?2 OFFSET ?3".to_string(),
                        vec![
                            Box::new(aid.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                    (None, Some(cat), Some(scp)) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE category = ?1 AND scope = ?2
                         ORDER BY updated_at DESC
                         LIMIT ?3 OFFSET ?4".to_string(),
                        vec![
                            Box::new(cat.to_string()),
                            Box::new(scp.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                    (None, Some(cat), None) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE category = ?1
                         ORDER BY updated_at DESC
                         LIMIT ?2 OFFSET ?3".to_string(),
                        vec![
                            Box::new(cat.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                    (None, None, Some(scp)) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         WHERE scope = ?1
                         ORDER BY updated_at DESC
                         LIMIT ?2 OFFSET ?3".to_string(),
                        vec![
                            Box::new(scp.to_string()),
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                    (None, None, None) => (
                        "SELECT id, session_id, agent_id, scope, category, key, content, confidence,
                                mention_count, source_summary, embedding, ward_id, contradicted_by, created_at, updated_at, expires_at
                         FROM memory_facts
                         ORDER BY updated_at DESC
                         LIMIT ?1 OFFSET ?2".to_string(),
                        vec![
                            Box::new(limit as i64),
                            Box::new(offset as i64),
                        ],
                    ),
                };

            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| row_to_memory_fact(row))?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Count all memory facts across all agents.
    pub fn count_all_memory_facts(&self, agent_id: Option<&str>) -> Result<usize, String> {
        self.db.with_connection(|conn| {
            let count: i64 = if let Some(aid) = agent_id {
                conn.query_row(
                    "SELECT COUNT(*) FROM memory_facts WHERE agent_id = ?1",
                    params![aid],
                    |row| row.get(0),
                )?
            } else {
                conn.query_row(
                    "SELECT COUNT(*) FROM memory_facts",
                    [],
                    |row| row.get(0),
                )?
            };
            Ok(count as usize)
        })
    }
}

// ============================================================================
// HELPERS
// ============================================================================

/// Map a database row to a MemoryFact struct.
fn row_to_memory_fact(row: &rusqlite::Row) -> Result<MemoryFact, rusqlite::Error> {
    let embedding_blob: Option<Vec<u8>> = row.get(10)?;
    let embedding = embedding_blob.map(|b| blob_to_f32_vec(&b));

    Ok(MemoryFact {
        id: row.get(0)?,
        session_id: row.get(1)?,
        agent_id: row.get(2)?,
        scope: row.get(3)?,
        category: row.get(4)?,
        key: row.get(5)?,
        content: row.get(6)?,
        confidence: row.get(7)?,
        mention_count: row.get(8)?,
        source_summary: row.get(9)?,
        embedding,
        ward_id: row.get(11)?,
        contradicted_by: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        expires_at: row.get(15)?,
    })
}

/// Convert f32 vector to raw bytes (little-endian) for SQLite BLOB storage.
fn f32_vec_to_blob(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert raw bytes (little-endian) back to f32 vector.
fn blob_to_f32_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;

    for (x, y) in a.iter().zip(b.iter()) {
        let x = *x as f64;
        let y = *y as f64;
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Arc<DatabaseManager> {
        use tempfile::TempDir;
        use gateway_services::VaultPaths;
        
        let temp_dir = TempDir::new().unwrap();
        let paths = Arc::new(VaultPaths::new(temp_dir.path().to_path_buf()));
        let _ = temp_dir.keep();
        let db = DatabaseManager::new(paths).unwrap();
        Arc::new(db)
    }

    fn make_fact(agent_id: &str, key: &str, content: &str, category: &str) -> MemoryFact {
        MemoryFact {
            id: format!("fact-{}", uuid::Uuid::new_v4()),
            session_id: None,
            agent_id: agent_id.to_string(),
            scope: "agent".to_string(),
            category: category.to_string(),
            key: key.to_string(),
            content: content.to_string(),
            confidence: 0.8,
            mention_count: 1,
            source_summary: None,
            embedding: None,
            ward_id: "__global__".to_string(),
            contradicted_by: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            expires_at: None,
        }
    }

    #[test]
    fn test_upsert_and_get_fact() {
        let db = create_test_db();
        let repo = MemoryRepository::new(db);

        let fact = make_fact("agent-1", "user.name", "User's name is Alice", "preference");
        repo.upsert_memory_fact(&fact).unwrap();

        let facts = repo.get_memory_facts("agent-1", None, 10).unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].content, "User's name is Alice");
        assert_eq!(facts[0].key, "user.name");
    }

    #[test]
    fn test_upsert_dedup_bumps_mention_count() {
        let db = create_test_db();
        let repo = MemoryRepository::new(db);

        let fact1 = make_fact("agent-1", "lang.preferred", "Python", "preference");
        repo.upsert_memory_fact(&fact1).unwrap();

        // Same key, different content — should update
        let mut fact2 = make_fact("agent-1", "lang.preferred", "Rust", "preference");
        fact2.confidence = 0.9;
        repo.upsert_memory_fact(&fact2).unwrap();

        let facts = repo.get_memory_facts("agent-1", None, 10).unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].content, "Rust");
        assert_eq!(facts[0].mention_count, 2);
        assert_eq!(facts[0].confidence, 0.9); // MAX of 0.8 and 0.9
    }

    #[test]
    fn test_delete_fact() {
        let db = create_test_db();
        let repo = MemoryRepository::new(db);

        let fact = make_fact("agent-1", "test.delete", "will be deleted", "entity");
        repo.upsert_memory_fact(&fact).unwrap();

        assert!(repo.delete_memory_fact(&fact.id).unwrap());
        assert_eq!(repo.count_memory_facts("agent-1").unwrap(), 0);
    }

    #[test]
    fn test_scope_isolation() {
        let db = create_test_db();
        let repo = MemoryRepository::new(db);

        let mut fact1 = make_fact("agent-1", "shared.key", "shared fact", "entity");
        fact1.scope = "shared".to_string();
        repo.upsert_memory_fact(&fact1).unwrap();

        let fact2 = make_fact("agent-1", "agent.key", "agent fact", "entity");
        repo.upsert_memory_fact(&fact2).unwrap();

        let shared = repo.get_memory_facts("agent-1", Some("shared"), 10).unwrap();
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].key, "shared.key");

        let agent = repo.get_memory_facts("agent-1", Some("agent"), 10).unwrap();
        assert_eq!(agent.len(), 1);
        assert_eq!(agent[0].key, "agent.key");
    }

    #[test]
    fn test_fts5_search() {
        let db = create_test_db();
        let repo = MemoryRepository::new(db);

        repo.upsert_memory_fact(&make_fact("agent-1", "build.tool", "Uses cargo for building Rust projects", "pattern")).unwrap();
        repo.upsert_memory_fact(&make_fact("agent-1", "editor.pref", "Prefers VS Code for editing", "preference")).unwrap();
        repo.upsert_memory_fact(&make_fact("agent-1", "lang.main", "Primary language is Rust", "decision")).unwrap();

        let results = repo.search_memory_facts_fts("Rust", "agent-1", 10, None).unwrap();
        assert!(results.len() >= 2, "Should find 'Rust' in at least 2 facts, got {}", results.len());
    }

    #[test]
    fn test_embedding_storage_and_vector_search() {
        let db = create_test_db();
        let repo = MemoryRepository::new(db);

        // Create facts with embeddings
        let mut fact1 = make_fact("agent-1", "vec.test1", "hello world", "entity");
        fact1.embedding = Some(vec![1.0, 0.0, 0.0]);
        repo.upsert_memory_fact(&fact1).unwrap();

        let mut fact2 = make_fact("agent-1", "vec.test2", "goodbye world", "entity");
        fact2.embedding = Some(vec![0.0, 1.0, 0.0]);
        repo.upsert_memory_fact(&fact2).unwrap();

        // Search with query embedding close to fact1
        let query = vec![0.9, 0.1, 0.0];
        let results = repo.search_memory_facts_hybrid(
            "hello", Some(&query), "agent-1", 10, 0.7, 0.3, None,
        ).unwrap();

        assert!(!results.is_empty(), "Should find at least one result");
    }

    #[test]
    fn test_embedding_cache() {
        let db = create_test_db();
        let repo = MemoryRepository::new(db);

        let hash = "abc123";
        let model = "all-MiniLM-L6-v2";
        let embedding = vec![0.1_f32, 0.2, 0.3, 0.4];

        // Cache miss
        assert!(repo.get_cached_embedding(hash, model).unwrap().is_none());

        // Cache write
        repo.cache_embedding(hash, model, &embedding).unwrap();

        // Cache hit
        let cached = repo.get_cached_embedding(hash, model).unwrap().unwrap();
        assert_eq!(cached.len(), 4);
        assert!((cached[0] - 0.1).abs() < 0.001);
        assert!((cached[3] - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_f32_blob_roundtrip() {
        let original = vec![1.5_f32, -2.5, 0.0, 3.14159];
        let blob = f32_vec_to_blob(&original);
        let recovered = blob_to_f32_vec(&blob);
        assert_eq!(original.len(), recovered.len());
        for (a, b) in original.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < 0.0001);
        }
    }

    #[test]
    fn test_high_confidence_facts() {
        let db = create_test_db();
        let repo = MemoryRepository::new(db);

        let mut high = make_fact("agent-1", "important", "always remember this", "instruction");
        high.confidence = 0.95;
        repo.upsert_memory_fact(&high).unwrap();

        let mut low = make_fact("agent-1", "maybe", "might be useful", "pattern");
        low.confidence = 0.3;
        repo.upsert_memory_fact(&low).unwrap();

        let facts = repo.get_high_confidence_facts("agent-1", 0.9, 10).unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].key, "important");
    }
}
