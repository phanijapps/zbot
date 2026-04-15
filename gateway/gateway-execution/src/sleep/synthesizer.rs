//! Synthesizer — extracts cross-session strategy facts.
//!
//! Runs during sleep-time maintenance. For each entity that appears across
//! at least 2 distinct sessions in the last 30 days, asks an LLM whether the
//! pattern warrants a `category='strategy'` memory fact. Conservative:
//! any LLM/DB/parse error skips the candidate — the whole cycle never
//! fails hard.
//!
//! Not yet wired into `SleepTimeWorker` — that happens in T5.

use std::sync::Arc;

use agent_runtime::llm::embedding::EmbeddingClient;
use agent_runtime::llm::{ChatMessage, LlmClient, LlmConfig};
use async_trait::async_trait;
use gateway_database::{CompactionRepository, KnowledgeDatabase, MemoryFact, MemoryRepository};
use gateway_services::ProviderService;
use rusqlite::params;
use serde::Deserialize;

use crate::ingest::json_shape::parse_llm_json;

/// Maximum candidates fetched from the DB per cycle.
const CANDIDATE_LIMIT: usize = 20;
/// Maximum LLM calls per cycle (budget).
const MAX_LLM_CALLS_PER_CYCLE: usize = 10;
/// Minimum confidence required to insert a synthesis fact.
const MIN_CONFIDENCE: f64 = 0.7;
/// Cosine threshold for dedup against existing strategy facts.
const DEDUP_COSINE_THRESHOLD: f64 = 0.88;
/// Default `scope` for memory_facts written by the Synthesizer.
const FACT_SCOPE: &str = "agent";
/// Default `ward_id` — global across wards.
const FACT_WARD: &str = "__global__";
/// Fact category used by cross-session strategy syntheses.
const STRATEGY_CATEGORY: &str = "strategy";

/// Stats returned from one synthesis cycle.
#[derive(Debug, Default, Clone)]
pub struct SynthesisStats {
    pub candidates_considered: u64,
    pub llm_calls_made: u64,
    pub facts_inserted: u64,
    pub facts_bumped: u64,
    pub skipped_low_confidence: u64,
    pub skipped_llm_or_parse_error: u64,
}

/// Parsed LLM response shape.
#[derive(Debug, Clone, Deserialize)]
pub struct SynthesisResponse {
    pub strategy: String,
    pub confidence: f64,
    pub key_fact: String,
    pub decision: String, // "synthesize" | "skip"
}

/// Neighborhood context sent to the LLM for a single candidate.
#[derive(Debug, Clone)]
pub struct SynthesisInput {
    pub entity_name: String,
    pub entity_type: String,
    pub session_count: u64,
    pub task_summaries: Vec<String>,
    pub relationship_summaries: Vec<String>,
}

/// Abstraction so tests can inject a mock LLM without touching the network.
/// Production impl wraps an OpenAI-compatible client.
#[async_trait]
pub trait SynthesisLlm: Send + Sync {
    async fn synthesize(&self, input: &SynthesisInput) -> Result<SynthesisResponse, String>;
}

/// Internal representation of a single candidate row.
struct Candidate {
    entity_id: String,
    agent_id: String,
    name: String,
    entity_type: String,
    n_sessions: u64,
}

/// Cross-session strategy synthesizer.
pub struct Synthesizer {
    db: Arc<KnowledgeDatabase>,
    memory_repo: Arc<MemoryRepository>,
    compaction_repo: Arc<CompactionRepository>,
    llm: Arc<dyn SynthesisLlm>,
    /// Optional embedding client used for cosine dedup. When absent,
    /// dedup falls back to the unique `(agent_id, scope, ward_id, key)`
    /// constraint on `memory_facts` (via upsert).
    embedder: Option<Arc<dyn EmbeddingClient>>,
}

impl Synthesizer {
    pub fn new(
        db: Arc<KnowledgeDatabase>,
        memory_repo: Arc<MemoryRepository>,
        compaction_repo: Arc<CompactionRepository>,
        llm: Arc<dyn SynthesisLlm>,
        embedder: Option<Arc<dyn EmbeddingClient>>,
    ) -> Self {
        Self {
            db,
            memory_repo,
            compaction_repo,
            llm,
            embedder,
        }
    }

    /// Run one synthesis cycle. Returns aggregate stats. Any per-candidate
    /// error is logged and skipped — the cycle never fails hard.
    pub async fn run_cycle(&self, run_id: &str) -> Result<SynthesisStats, String> {
        let mut stats = SynthesisStats::default();
        let candidates = self.load_candidates()?;
        for cand in candidates.into_iter().take(MAX_LLM_CALLS_PER_CYCLE) {
            stats.candidates_considered += 1;
            self.process_candidate(run_id, &cand, &mut stats).await;
        }
        Ok(stats)
    }

    fn load_candidates(&self) -> Result<Vec<Candidate>, String> {
        let limit = CANDIDATE_LIMIT as i64;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT e.id, e.agent_id, e.name, e.entity_type, COUNT(DISTINCT ep.session_id) AS n_sessions
                 FROM kg_entities e
                 JOIN kg_relationships r ON r.source_entity_id = e.id OR r.target_entity_id = e.id
                 JOIN kg_episodes ep    ON instr(COALESCE(r.source_episode_ids, ''), ep.id) > 0
                 WHERE e.epistemic_class != 'archival'
                   AND e.compressed_into IS NULL
                   AND ep.created_at > datetime('now', '-30 days')
                   AND ep.session_id IS NOT NULL
                 GROUP BY e.id
                 HAVING n_sessions >= 2
                 ORDER BY n_sessions DESC, e.mention_count DESC
                 LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit], |row| {
                    Ok(Candidate {
                        entity_id: row.get(0)?,
                        agent_id: row.get(1)?,
                        name: row.get(2)?,
                        entity_type: row.get(3)?,
                        n_sessions: row.get::<_, i64>(4)? as u64,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    async fn process_candidate(&self, run_id: &str, cand: &Candidate, stats: &mut SynthesisStats) {
        let input = match self.build_input(cand) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!(entity = %cand.entity_id, error = %e, "synth: build_input failed");
                stats.skipped_llm_or_parse_error += 1;
                return;
            }
        };
        let episode_ids = match self.episode_ids_for(cand) {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!(entity = %cand.entity_id, error = %e, "synth: episode_ids failed");
                stats.skipped_llm_or_parse_error += 1;
                return;
            }
        };

        stats.llm_calls_made += 1;
        let resp = match self.llm.synthesize(&input).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(entity = %cand.entity_id, error = %e, "synth: LLM failed");
                stats.skipped_llm_or_parse_error += 1;
                return;
            }
        };

        if resp.decision != "synthesize" || resp.confidence < MIN_CONFIDENCE {
            stats.skipped_low_confidence += 1;
            return;
        }

        self.commit_synthesis(run_id, cand, &resp, &episode_ids, stats)
            .await;
    }

    async fn commit_synthesis(
        &self,
        run_id: &str,
        cand: &Candidate,
        resp: &SynthesisResponse,
        episode_ids: &[String],
        stats: &mut SynthesisStats,
    ) {
        // Dedup step (optional — requires embedder)
        let embedding = self.embed_content(&resp.key_fact).await;
        if let Some(ref emb) = embedding {
            match self.try_bump_existing(&cand.agent_id, emb, episode_ids) {
                Ok(Some(fact_id)) => {
                    stats.facts_bumped += 1;
                    self.audit(run_id, &fact_id, resp, "bumped existing");
                    return;
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "synth: dedup lookup failed; will upsert");
                }
            }
        }

        let fact_id = match self.insert_new(cand, resp, episode_ids, embedding) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(entity = %cand.entity_id, error = %e, "synth: insert failed");
                stats.skipped_llm_or_parse_error += 1;
                return;
            }
        };
        stats.facts_inserted += 1;
        self.audit(run_id, &fact_id, resp, "new synthesis");
    }

    fn audit(&self, run_id: &str, fact_id: &str, resp: &SynthesisResponse, note: &str) {
        let reason = format!(
            "{note}: strategy={} confidence={:.2}",
            resp.strategy, resp.confidence
        );
        if let Err(e) = self
            .compaction_repo
            .record_synthesis(run_id, fact_id, &reason)
        {
            tracing::warn!(fact_id = %fact_id, error = %e, "synth: record_synthesis failed");
        }
    }

    fn try_bump_existing(
        &self,
        agent_id: &str,
        key_fact_emb: &[f32],
        episode_ids: &[String],
    ) -> Result<Option<String>, String> {
        // Pull candidate strategy facts and compare embeddings pulled from
        // the vec0 index. Limit of 50 is plenty for the strategy category.
        let candidates = self
            .memory_repo
            .get_facts_by_category(agent_id, STRATEGY_CATEGORY, 50)
            .unwrap_or_default();
        for fact in candidates {
            let stored = match self.memory_repo.get_fact_embedding(&fact.id)? {
                Some(v) => v,
                None => continue,
            };
            let sim = cosine_similarity(key_fact_emb, &stored);
            if sim >= DEDUP_COSINE_THRESHOLD {
                self.bump_fact(&fact, episode_ids)?;
                return Ok(Some(fact.id));
            }
        }
        Ok(None)
    }

    fn bump_fact(&self, fact: &MemoryFact, episode_ids: &[String]) -> Result<(), String> {
        let merged = merge_episode_ids(fact.source_episode_id.as_deref(), episode_ids);
        let now = chrono::Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE memory_facts
                 SET mention_count = mention_count + 1,
                     updated_at = ?1,
                     source_episode_id = ?2
                 WHERE id = ?3",
                params![now, merged, fact.id],
            )?;
            Ok(())
        })
    }

    fn insert_new(
        &self,
        cand: &Candidate,
        resp: &SynthesisResponse,
        episode_ids: &[String],
        embedding: Option<Vec<f32>>,
    ) -> Result<String, String> {
        let now = chrono::Utc::now().to_rfc3339();
        let hash8 = short_hash(&resp.key_fact);
        let slug = slugify(&cand.name);
        let key = format!("strategy.synthesis.{slug}.{hash8}");
        let id = format!("fact-{}", uuid::Uuid::new_v4());
        let source_episode_id = Some(encode_episode_ids(episode_ids));

        let fact = MemoryFact {
            id: id.clone(),
            session_id: None,
            agent_id: cand.agent_id.clone(),
            scope: FACT_SCOPE.to_string(),
            category: STRATEGY_CATEGORY.to_string(),
            key,
            content: resp.key_fact.clone(),
            confidence: resp.confidence,
            mention_count: 1,
            source_summary: Some(format!(
                "cross-session synthesis over {} sessions (entity: {})",
                cand.n_sessions, cand.name
            )),
            embedding,
            ward_id: FACT_WARD.to_string(),
            contradicted_by: None,
            created_at: now.clone(),
            updated_at: now,
            expires_at: None,
            valid_from: None,
            valid_until: None,
            superseded_by: None,
            pinned: false,
            epistemic_class: Some("convention".to_string()),
            source_episode_id,
            source_ref: None,
        };

        self.memory_repo.upsert_memory_fact(&fact)?;
        Ok(id)
    }

    async fn embed_content(&self, text: &str) -> Option<Vec<f32>> {
        let client = self.embedder.as_ref()?;
        match client.embed(&[text]).await {
            Ok(mut v) if !v.is_empty() => Some(v.remove(0)),
            Ok(_) => None,
            Err(e) => {
                tracing::warn!(error = %e, "synth: embed failed");
                None
            }
        }
    }

    fn build_input(&self, cand: &Candidate) -> Result<SynthesisInput, String> {
        let (rel_summaries, session_ids) = self.fetch_relationships(cand)?;
        let task_summaries = self.fetch_task_summaries(&session_ids)?;
        Ok(SynthesisInput {
            entity_name: cand.name.clone(),
            entity_type: cand.entity_type.clone(),
            session_count: cand.n_sessions,
            task_summaries,
            relationship_summaries: rel_summaries,
        })
    }

    /// Returns (relationship_summaries, distinct_session_ids) for a candidate.
    #[allow(clippy::type_complexity)]
    fn fetch_relationships(&self, cand: &Candidate) -> Result<(Vec<String>, Vec<String>), String> {
        let entity_id = cand.entity_id.clone();
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT r.relationship_type, r.source_entity_id, r.target_entity_id,
                        r.source_episode_ids
                 FROM kg_relationships r
                 WHERE r.source_entity_id = ?1 OR r.target_entity_id = ?1
                 LIMIT 50",
            )?;
            let rows: Vec<(String, String, String, Option<String>)> = stmt
                .query_map(params![entity_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let summaries: Vec<String> = rows
                .iter()
                .map(|(ty, src, tgt, _)| format!("{src} --[{ty}]--> {tgt}"))
                .collect();

            let episode_id_blob: String = rows
                .iter()
                .filter_map(|(_, _, _, eids)| eids.clone())
                .collect::<Vec<_>>()
                .join(",");

            let mut session_ids: Vec<String> = Vec::new();
            if !episode_id_blob.is_empty() {
                let mut q = conn.prepare(
                    "SELECT DISTINCT session_id FROM kg_episodes
                     WHERE session_id IS NOT NULL
                       AND instr(?1, id) > 0
                       AND created_at > datetime('now', '-30 days')",
                )?;
                session_ids = q
                    .query_map(params![episode_id_blob], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
            }
            Ok((summaries, session_ids))
        })
    }

    fn fetch_task_summaries(&self, session_ids: &[String]) -> Result<Vec<String>, String> {
        if session_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; session_ids.len()].join(",");
        let sql = format!(
            "SELECT task_summary FROM session_episodes
             WHERE session_id IN ({placeholders}) AND task_summary IS NOT NULL"
        );
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let params_vec: Vec<&dyn rusqlite::types::ToSql> = session_ids
                .iter()
                .map(|s| s as &dyn rusqlite::types::ToSql)
                .collect();
            let rows = stmt
                .query_map(params_vec.as_slice(), |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// Contributing episode ids for the candidate (distinct).
    fn episode_ids_for(&self, cand: &Candidate) -> Result<Vec<String>, String> {
        let entity_id = cand.entity_id.clone();
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT r.source_episode_ids FROM kg_relationships r
                 WHERE (r.source_entity_id = ?1 OR r.target_entity_id = ?1)
                   AND r.source_episode_ids IS NOT NULL",
            )?;
            let blobs = stmt
                .query_map(params![entity_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            let mut ids: Vec<String> = blobs
                .iter()
                .flat_map(|b| b.split(',').map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect();
            ids.sort();
            ids.dedup();
            // Filter to those that exist AND are within 30 days AND have a session.
            if ids.is_empty() {
                return Ok(ids);
            }
            let placeholders = vec!["?"; ids.len()].join(",");
            let sql = format!(
                "SELECT id FROM kg_episodes
                 WHERE id IN ({placeholders})
                   AND session_id IS NOT NULL
                   AND created_at > datetime('now', '-30 days')"
            );
            let mut q = conn.prepare(&sql)?;
            let params_vec: Vec<&dyn rusqlite::types::ToSql> = ids
                .iter()
                .map(|s| s as &dyn rusqlite::types::ToSql)
                .collect();
            let filtered: Vec<String> = q
                .query_map(params_vec.as_slice(), |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(filtered)
        })
    }
}

/// LLM-backed `SynthesisLlm` wired to the default configured provider.
/// Conservative on failure — propagates `Err` so `run_cycle` can log+skip.
pub struct LlmSynthesizer {
    provider_service: Arc<ProviderService>,
}

impl LlmSynthesizer {
    pub fn new(provider_service: Arc<ProviderService>) -> Self {
        Self { provider_service }
    }

    fn build_client(&self) -> Result<Arc<dyn LlmClient>, String> {
        let providers = self
            .provider_service
            .list()
            .map_err(|e| format!("list providers: {e}"))?;
        let provider = providers
            .iter()
            .find(|p| p.is_default)
            .or_else(|| providers.first())
            .ok_or_else(|| "no providers configured".to_string())?;
        let model = provider.default_model().to_string();
        let provider_id = provider.id.clone().unwrap_or_else(|| "default".to_string());
        let config = LlmConfig::new(
            provider.base_url.clone(),
            provider.api_key.clone(),
            model,
            provider_id,
        )
        .with_temperature(0.0)
        .with_max_tokens(512);
        let client = agent_runtime::llm::openai::OpenAiClient::new(config)
            .map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn LlmClient>)
    }
}

#[async_trait]
impl SynthesisLlm for LlmSynthesizer {
    async fn synthesize(&self, input: &SynthesisInput) -> Result<SynthesisResponse, String> {
        let client = self.build_client()?;
        let prompt = format!(
            "You identify reusable cross-session strategies from an agent's knowledge graph.\n\
             The entity below has appeared across {n} distinct sessions within the last 30 days.\n\
             Decide whether the repeated co-occurrence reveals a *strategy* worth memorising \
             as a stable rule (e.g. \"when X times out, retry with backoff\").\n\n\
             Return ONLY JSON: {{\"strategy\": string, \"confidence\": 0.0-1.0, \
             \"key_fact\": string, \"decision\": \"synthesize\" | \"skip\"}}.\n\n\
             Entity: name={name:?} type={etype}\n\
             Recent task summaries:\n{tasks}\n\n\
             Relationships:\n{rels}",
            n = input.session_count,
            name = input.entity_name,
            etype = input.entity_type,
            tasks = input
                .task_summaries
                .iter()
                .map(|t| format!("- {t}"))
                .collect::<Vec<_>>()
                .join("\n"),
            rels = input
                .relationship_summaries
                .iter()
                .map(|r| format!("- {r}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let messages = vec![
            ChatMessage::system("You return only valid JSON.".to_string()),
            ChatMessage::user(prompt),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("LLM call: {e}"))?;
        parse_llm_json::<SynthesisResponse>(&response.content)
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn encode_episode_ids(ids: &[String]) -> String {
    // Comma-joined; decoded by merge_episode_ids. We avoid JSON to match the
    // convention used elsewhere in the schema (`kg_relationships.source_episode_ids`).
    ids.join(",")
}

fn merge_episode_ids(existing: Option<&str>, incoming: &[String]) -> String {
    let mut set: Vec<String> = existing
        .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
        .unwrap_or_default();
    set.retain(|s| !s.is_empty());
    for id in incoming {
        if !set.iter().any(|s| s == id) {
            set.push(id.clone());
        }
    }
    set.join(",")
}

fn short_hash(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:08x}", (h.finish() & 0xFFFF_FFFF) as u32)
}

fn slugify(s: &str) -> String {
    let out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "entity".to_string()
    } else {
        trimmed
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0f64;
    let mut na = 0f64;
    let mut nb = 0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let x = *x as f64;
        let y = *y as f64;
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_database::vector_index::{SqliteVecIndex, VectorIndex};
    use gateway_services::VaultPaths;
    use std::sync::Mutex;

    struct MockLlm {
        response: Mutex<SynthesisResponse>,
        calls: Mutex<u64>,
    }

    impl MockLlm {
        fn new(resp: SynthesisResponse) -> Self {
            Self {
                response: Mutex::new(resp),
                calls: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl SynthesisLlm for MockLlm {
        async fn synthesize(&self, _input: &SynthesisInput) -> Result<SynthesisResponse, String> {
            *self.calls.lock().unwrap() += 1;
            Ok(self.response.lock().unwrap().clone())
        }
    }

    struct Harness {
        _tmp: tempfile::TempDir,
        db: Arc<KnowledgeDatabase>,
        memory_repo: Arc<MemoryRepository>,
        compaction_repo: Arc<CompactionRepository>,
    }

    fn setup() -> Harness {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        let vec_index: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(db.clone(), "memory_facts_index", "fact_id")
                .expect("vec index init"),
        );
        let memory_repo = Arc::new(MemoryRepository::new(db.clone(), vec_index));
        let compaction_repo = Arc::new(CompactionRepository::new(db.clone()));
        Harness {
            _tmp: tmp,
            db,
            memory_repo,
            compaction_repo,
        }
    }

    /// Seed kg_entities, kg_relationships, kg_episodes such that an entity
    /// "postgres-timeout" spans 3 distinct sessions within the last 30 days.
    fn seed_cross_session(db: &KnowledgeDatabase, agent_id: &str) -> Vec<String> {
        let now = chrono::Utc::now().to_rfc3339();
        // Entity
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO kg_entities
                    (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                     epistemic_class, confidence, mention_count, access_count,
                     first_seen_at, last_seen_at)
                 VALUES ('ent-pg', ?1, 'Concept', 'postgres-timeout', 'postgres-timeout',
                         'hash-pg', 'current', 0.9, 3, 0, ?2, ?2)",
                params![agent_id, now],
            )?;
            // 3 episodes, one per session.
            for (i, sid) in ["sess-1", "sess-2", "sess-3"].iter().enumerate() {
                conn.execute(
                    "INSERT INTO kg_episodes
                        (id, source_type, source_ref, content_hash, session_id, agent_id,
                         status, retry_count, created_at, completed_at)
                     VALUES (?1, 'session', ?2, ?3, ?4, ?5, 'done', 0, ?6, ?6)",
                    params![
                        format!("ep-{i}"),
                        format!("ref-{i}"),
                        format!("hash-{i}"),
                        sid,
                        agent_id,
                        now,
                    ],
                )?;
                // Also seed a session_episodes row with a task_summary.
                conn.execute(
                    "INSERT INTO session_episodes
                        (id, session_id, agent_id, task_summary, outcome, created_at)
                     VALUES (?1, ?2, ?3, ?4, 'success', ?5)",
                    params![
                        format!("se-{i}"),
                        sid,
                        agent_id,
                        format!("Investigated postgres timeout issue #{i}"),
                        now,
                    ],
                )?;
            }
            // A self-related relationship whose source_episode_ids references
            // all three episodes (comma-separated, matching production convention).
            conn.execute(
                "INSERT INTO kg_entities
                    (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                     epistemic_class, confidence, mention_count, access_count,
                     first_seen_at, last_seen_at)
                 VALUES ('ent-svc', ?1, 'Concept', 'service-layer', 'service-layer',
                         'hash-svc', 'current', 0.9, 1, 0, ?2, ?2)",
                params![agent_id, now],
            )?;
            conn.execute(
                "INSERT INTO kg_relationships
                    (id, agent_id, source_entity_id, target_entity_id, relationship_type,
                     epistemic_class, confidence, mention_count, access_count,
                     first_seen_at, last_seen_at, source_episode_ids)
                 VALUES ('rel-1', ?1, 'ent-pg', 'ent-svc', 'affects',
                         'current', 0.9, 3, 0, ?2, ?2, 'ep-0,ep-1,ep-2')",
                params![agent_id, now],
            )?;
            Ok(())
        })
        .expect("seed");
        vec!["ep-0".to_string(), "ep-1".to_string(), "ep-2".to_string()]
    }

    #[tokio::test]
    async fn synthesizes_strategy_across_sessions() {
        let h = setup();
        let agent_id = "agent-synth";
        let episode_ids = seed_cross_session(&h.db, agent_id);

        let mock = Arc::new(MockLlm::new(SynthesisResponse {
            strategy: "retry postgres with backoff".into(),
            confidence: 0.85,
            key_fact: "When postgres-timeout recurs, prefer jittered exponential backoff".into(),
            decision: "synthesize".into(),
        }));

        let synth = Synthesizer::new(
            h.db.clone(),
            h.memory_repo.clone(),
            h.compaction_repo.clone(),
            mock.clone(),
            None, // no embedder -> key-based dedup path
        );

        let run_id = "run-synth-test";
        let stats = synth.run_cycle(run_id).await.expect("run_cycle");

        // Both endpoints of the relationship ("postgres-timeout" and
        // "service-layer") legitimately span 3 sessions via the single
        // 'affects' edge, so we expect 2 candidates, one fact each.
        assert_eq!(stats.candidates_considered, 2, "candidates: {stats:?}");
        assert_eq!(stats.llm_calls_made, 2);
        assert_eq!(stats.facts_inserted, 2);
        assert_eq!(*mock.calls.lock().unwrap(), 2);

        // Memory fact rows.
        let facts = h
            .memory_repo
            .get_facts_by_category(agent_id, "strategy", 10)
            .expect("facts");
        assert_eq!(facts.len(), 2);
        let pg_fact = facts
            .iter()
            .find(|f| f.key.contains("postgres-timeout"))
            .expect("postgres-timeout fact");
        assert_eq!(pg_fact.category, "strategy");
        let sei = pg_fact.source_episode_id.clone().unwrap_or_default();
        for ep in &episode_ids {
            assert!(sei.contains(ep), "episode {ep} missing from {sei}");
        }
        assert!(pg_fact.key.starts_with("strategy.synthesis."));
        assert!(pg_fact.content.contains("postgres-timeout"));

        // Compaction audit rows: one per inserted fact.
        let rows = h.compaction_repo.list_run(run_id).expect("list_run");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.operation == "synthesize"));
        let fact_ids: std::collections::HashSet<_> = facts.iter().map(|f| f.id.clone()).collect();
        for row in &rows {
            assert!(
                fact_ids.contains(row.entity_id.as_deref().unwrap_or("")),
                "audit row must reference an inserted fact id"
            );
        }
    }

    #[tokio::test]
    async fn skips_when_decision_is_skip() {
        let h = setup();
        seed_cross_session(&h.db, "agent-skip");
        let mock = Arc::new(MockLlm::new(SynthesisResponse {
            strategy: "".into(),
            confidence: 0.99,
            key_fact: "".into(),
            decision: "skip".into(),
        }));
        let synth = Synthesizer::new(
            h.db.clone(),
            h.memory_repo.clone(),
            h.compaction_repo.clone(),
            mock,
            None,
        );
        let stats = synth.run_cycle("run-skip").await.expect("run_cycle");
        assert_eq!(stats.skipped_low_confidence, 2);
        assert_eq!(stats.facts_inserted, 0);
        let rows = h.compaction_repo.list_run("run-skip").expect("rows");
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn skips_when_confidence_below_threshold() {
        let h = setup();
        seed_cross_session(&h.db, "agent-lowconf");
        let mock = Arc::new(MockLlm::new(SynthesisResponse {
            strategy: "maybe".into(),
            confidence: 0.5,
            key_fact: "some fact".into(),
            decision: "synthesize".into(),
        }));
        let synth = Synthesizer::new(
            h.db.clone(),
            h.memory_repo.clone(),
            h.compaction_repo.clone(),
            mock,
            None,
        );
        let stats = synth.run_cycle("run-lowconf").await.expect("run_cycle");
        assert_eq!(stats.skipped_low_confidence, 2);
        assert_eq!(stats.facts_inserted, 0);
    }

    #[test]
    fn merge_episode_ids_preserves_order_and_dedups() {
        let merged = merge_episode_ids(Some("ep-a,ep-b"), &["ep-b".into(), "ep-c".into()]);
        assert_eq!(merged, "ep-a,ep-b,ep-c");
    }

    #[test]
    fn slugify_replaces_nonalnum() {
        assert_eq!(slugify("Postgres Timeout!"), "postgres-timeout");
        assert_eq!(slugify("   "), "entity");
    }

    #[test]
    fn cosine_similarity_basics() {
        let a = vec![1.0f32, 0.0];
        let b = vec![1.0f32, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-9);
        let c = vec![0.0f32, 1.0];
        assert!(cosine_similarity(&a, &c).abs() < 1e-9);
    }
}
