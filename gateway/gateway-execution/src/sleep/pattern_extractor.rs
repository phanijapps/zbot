//! PatternExtractor — procedural pattern extraction sleep-time op.
//!
//! Scans recent successful session_episodes, finds pairs whose task_summary
//! embeddings are semantically similar AND whose tool-call sequences share a
//! structural prefix of 3+ tools in the same order, and asks an LLM to
//! generalize each match into a `procedures` row. Conservative: any
//! per-candidate error is logged and skipped — the cycle never fails hard.
//!
//! Not yet wired into `SleepTimeWorker` — that happens in T5.

use std::sync::Arc;

use agent_runtime::llm::{ChatMessage, LlmClient, LlmConfig};
use async_trait::async_trait;
use zero_stores_sqlite::{
    CompactionRepository, DatabaseManager, KnowledgeDatabase, Procedure, ProcedureRepository,
};
use gateway_services::ProviderService;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::ingest::json_shape::parse_llm_json;

/// Maximum successful session_episodes loaded per cycle.
const CANDIDATE_LIMIT: usize = 50;
/// Maximum LLM calls per cycle.
const MAX_LLM_CALLS_PER_CYCLE: usize = 5;
/// Cosine similarity threshold between task_summary embeddings for a pair
/// to be considered semantically related.
const PAIR_COSINE_THRESHOLD: f64 = 0.82;
/// Minimum number of matching tool names (in order) to call a pair a pattern.
const MIN_PATTERN_LENGTH: usize = 3;
/// Existing procedure with `success_count` at or above this is considered
/// locked-in and will not be overwritten by a new synthesis with the same name.
const DEDUP_SUCCESS_FLOOR: i32 = 2;

/// Default ward id for synthesized procedures.
const PROC_WARD: &str = "__global__";

/// Stats returned from one extraction cycle.
#[derive(Debug, Default, Clone)]
pub struct PatternStats {
    pub episodes_considered: u64,
    pub pairs_evaluated: u64,
    pub pairs_matched: u64,
    pub llm_calls_made: u64,
    pub procedures_inserted: u64,
    pub skipped_existing: u64,
    pub skipped_llm_or_parse_error: u64,
}

/// Parsed LLM response shape.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatternResponse {
    pub name: String,
    pub description: String,
    pub trigger_pattern: String,
    #[serde(default)]
    pub parameters: Vec<String>,
    pub steps: Vec<PatternStep>,
}

/// Single step of a generalized pattern.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatternStep {
    pub action: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub task_template: Option<String>,
}

/// Context passed to the LLM for generalization.
#[derive(Debug, Clone)]
pub struct PatternInput {
    pub task_summary_a: String,
    pub task_summary_b: String,
    pub tool_sequence_a: Vec<String>,
    pub tool_sequence_b: Vec<String>,
    pub matched_prefix: Vec<String>,
}

/// Abstraction so tests can inject a mock LLM without touching the network.
#[async_trait]
pub trait PatternExtractLlm: Send + Sync {
    async fn generalize(&self, input: &PatternInput) -> Result<PatternResponse, String>;
}

/// Internal representation of a single loaded successful episode.
struct EpisodeRow {
    session_id: String,
    agent_id: String,
    task_summary: String,
    embedding: Option<Vec<f32>>,
}

/// Procedural pattern extractor.
pub struct PatternExtractor {
    knowledge_db: Arc<KnowledgeDatabase>,
    conversations_db: Arc<DatabaseManager>,
    procedure_repo: Arc<ProcedureRepository>,
    compaction_repo: Arc<CompactionRepository>,
    llm: Arc<dyn PatternExtractLlm>,
}

impl PatternExtractor {
    pub fn new(
        knowledge_db: Arc<KnowledgeDatabase>,
        conversations_db: Arc<DatabaseManager>,
        procedure_repo: Arc<ProcedureRepository>,
        compaction_repo: Arc<CompactionRepository>,
        llm: Arc<dyn PatternExtractLlm>,
    ) -> Self {
        Self {
            knowledge_db,
            conversations_db,
            procedure_repo,
            compaction_repo,
            llm,
        }
    }

    /// Run one extraction cycle. Returns aggregate stats. Any per-candidate
    /// error is logged and skipped — the cycle never fails hard.
    pub async fn run_cycle(&self, run_id: &str) -> Result<PatternStats, String> {
        let mut stats = PatternStats::default();
        let episodes = self.load_successful_episodes()?;
        stats.episodes_considered = episodes.len() as u64;
        if episodes.len() < 2 {
            return Ok(stats);
        }

        let pairs = build_matching_pairs(&episodes, &mut stats);

        for pair in pairs.into_iter().take(MAX_LLM_CALLS_PER_CYCLE) {
            self.process_pair(run_id, &episodes, pair, &mut stats).await;
        }
        Ok(stats)
    }

    fn load_successful_episodes(&self) -> Result<Vec<EpisodeRow>, String> {
        let limit = CANDIDATE_LIMIT as i64;
        let rows: Vec<(String, String, String)> = self.knowledge_db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, task_summary
                 FROM session_episodes
                 WHERE outcome = 'success'
                   AND task_summary IS NOT NULL
                   AND created_at > datetime('now', '-30 days')
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )?;
            let r = stmt
                .query_map(params![limit], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            // Fetch task_summary separately to keep tuple small; easier to re-read
            // but we already pulled it — do it in one go:
            Ok(r)
        })?;

        let mut out: Vec<EpisodeRow> = Vec::with_capacity(rows.len());
        for (id, session_id, agent_id) in rows {
            let task_summary = self.fetch_task_summary(&id)?;
            let embedding = self.fetch_episode_embedding(&id).unwrap_or(None);
            out.push(EpisodeRow {
                session_id,
                agent_id,
                task_summary,
                embedding,
            });
        }
        Ok(out)
    }

    fn fetch_task_summary(&self, episode_id: &str) -> Result<String, String> {
        self.knowledge_db.with_connection(|conn| {
            conn.query_row(
                "SELECT COALESCE(task_summary, '') FROM session_episodes WHERE id = ?1",
                params![episode_id],
                |row| row.get::<_, String>(0),
            )
        })
    }

    fn fetch_episode_embedding(&self, episode_id: &str) -> Result<Option<Vec<f32>>, String> {
        self.knowledge_db.with_connection(|conn| {
            let r = conn.query_row(
                "SELECT embedding FROM session_episodes_index WHERE episode_id = ?1",
                params![episode_id],
                |row| row.get::<_, Vec<u8>>(0),
            );
            match r {
                Ok(blob) => Ok(Some(blob_to_f32_vec(&blob))),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    async fn process_pair(
        &self,
        run_id: &str,
        episodes: &[EpisodeRow],
        pair: MatchedPair,
        stats: &mut PatternStats,
    ) {
        let ep_a = &episodes[pair.idx_a];
        let ep_b = &episodes[pair.idx_b];

        let tools_a = match self.load_tool_sequence(&ep_a.session_id) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(session = %ep_a.session_id, error = %e, "pattern: tools_a failed");
                stats.skipped_llm_or_parse_error += 1;
                return;
            }
        };
        let tools_b = match self.load_tool_sequence(&ep_b.session_id) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(session = %ep_b.session_id, error = %e, "pattern: tools_b failed");
                stats.skipped_llm_or_parse_error += 1;
                return;
            }
        };

        let matched = longest_common_prefix(&tools_a, &tools_b);
        if matched.len() < MIN_PATTERN_LENGTH {
            // Should not happen — pair only got here after prefix check — but guard.
            return;
        }

        let input = PatternInput {
            task_summary_a: ep_a.task_summary.clone(),
            task_summary_b: ep_b.task_summary.clone(),
            tool_sequence_a: tools_a,
            tool_sequence_b: tools_b,
            matched_prefix: matched,
        };

        stats.llm_calls_made += 1;
        let resp = match self.llm.generalize(&input).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "pattern: LLM failed");
                stats.skipped_llm_or_parse_error += 1;
                return;
            }
        };

        self.commit_pattern(run_id, &ep_a.agent_id, &resp, stats);
    }

    fn commit_pattern(
        &self,
        run_id: &str,
        agent_id: &str,
        resp: &PatternResponse,
        stats: &mut PatternStats,
    ) {
        let name = sanitize_name(&resp.name);
        if name.is_empty() {
            stats.skipped_llm_or_parse_error += 1;
            return;
        }
        match self.existing_procedure(agent_id, &name) {
            Ok(Some(existing)) if existing.success_count >= DEDUP_SUCCESS_FLOOR => {
                stats.skipped_existing += 1;
                return;
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "pattern: existing lookup failed");
                stats.skipped_llm_or_parse_error += 1;
                return;
            }
        }

        let procedure = match build_procedure(agent_id, &name, resp) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "pattern: build procedure failed");
                stats.skipped_llm_or_parse_error += 1;
                return;
            }
        };
        let proc_id = procedure.id.clone();

        if let Err(e) = self.procedure_repo.upsert_procedure(&procedure) {
            tracing::warn!(error = %e, "pattern: upsert_procedure failed");
            stats.skipped_llm_or_parse_error += 1;
            return;
        }
        stats.procedures_inserted += 1;

        let reason = format!("pattern '{}' across 2 sessions", name);
        if let Err(e) = self
            .compaction_repo
            .record_pattern(run_id, &proc_id, &reason)
        {
            tracing::warn!(error = %e, "pattern: record_pattern failed");
        }
    }

    fn existing_procedure(&self, agent_id: &str, name: &str) -> Result<Option<Procedure>, String> {
        self.knowledge_db.with_connection(|conn| {
            let r = conn.query_row(
                "SELECT id, agent_id, ward_id, name, description, trigger_pattern, steps, \
                 parameters, success_count, failure_count, avg_duration_ms, avg_token_cost, \
                 last_used, created_at, updated_at \
                 FROM procedures WHERE agent_id = ?1 AND name = ?2 LIMIT 1",
                params![agent_id, name],
                |row| {
                    Ok(Procedure {
                        id: row.get::<_, String>(0)?,
                        agent_id: row.get::<_, String>(1)?,
                        ward_id: row.get::<_, Option<String>>(2)?,
                        name: row.get::<_, String>(3)?,
                        description: row.get::<_, String>(4)?,
                        trigger_pattern: row.get::<_, Option<String>>(5)?,
                        steps: row.get::<_, String>(6)?,
                        parameters: row.get::<_, Option<String>>(7)?,
                        success_count: row.get::<_, i32>(8)?,
                        failure_count: row.get::<_, i32>(9)?,
                        avg_duration_ms: row.get::<_, Option<i64>>(10)?,
                        avg_token_cost: row.get::<_, Option<i64>>(11)?,
                        last_used: row.get::<_, Option<String>>(12)?,
                        embedding: None,
                        created_at: row.get::<_, String>(13)?,
                        updated_at: row.get::<_, String>(14)?,
                    })
                },
            );
            match r {
                Ok(p) => Ok(Some(p)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    fn load_tool_sequence(&self, session_id: &str) -> Result<Vec<String>, String> {
        self.conversations_db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT tool_calls FROM messages
                 WHERE session_id = ?1
                   AND role = 'assistant'
                   AND tool_calls IS NOT NULL
                 ORDER BY created_at ASC",
                )?;
                let rows = stmt
                    .query_map(params![session_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .map(|blobs| {
                let mut seq: Vec<String> = Vec::new();
                for blob in blobs {
                    extend_tool_names(&blob, &mut seq);
                }
                seq
            })
    }
}

/// Pair of episode indices whose task_summaries are semantically similar AND
/// whose tool-call sequences share a structural prefix.
#[derive(Debug, Clone, Copy)]
struct MatchedPair {
    idx_a: usize,
    idx_b: usize,
}

/// Build candidate pairs by cosine similarity of embeddings. The structural
/// match check happens later, inside `process_pair`, once tool sequences are
/// actually loaded (to keep this function cheap).
fn build_matching_pairs(episodes: &[EpisodeRow], stats: &mut PatternStats) -> Vec<MatchedPair> {
    let mut pairs = Vec::new();
    for i in 0..episodes.len() {
        for j in (i + 1)..episodes.len() {
            stats.pairs_evaluated += 1;
            let (a, b) = (&episodes[i], &episodes[j]);
            let (ea, eb) = match (&a.embedding, &b.embedding) {
                (Some(x), Some(y)) => (x, y),
                _ => continue,
            };
            if a.agent_id != b.agent_id {
                // Keep patterns within a single agent for now.
                continue;
            }
            if cosine_similarity(ea, eb) >= PAIR_COSINE_THRESHOLD {
                stats.pairs_matched += 1;
                pairs.push(MatchedPair { idx_a: i, idx_b: j });
            }
        }
    }
    pairs
}

/// Pulls assistant tool_calls JSON blobs out of the stored format
/// `[{"tool_name": "...", ...}, ...]` and appends tool names in order.
fn extend_tool_names(blob: &str, out: &mut Vec<String>) {
    let parsed: serde_json::Value = match serde_json::from_str(blob) {
        Ok(v) => v,
        Err(_) => return,
    };
    let arr = match parsed.as_array() {
        Some(a) => a,
        None => return,
    };
    for entry in arr {
        if let Some(name) = entry
            .get("tool_name")
            .and_then(|v| v.as_str())
            .or_else(|| entry.get("name").and_then(|v| v.as_str()))
        {
            out.push(name.to_string());
        }
    }
}

/// Returns the longest prefix (in order) common to both sequences.
fn longest_common_prefix<T: PartialEq + Clone>(a: &[T], b: &[T]) -> Vec<T> {
    let n = a.len().min(b.len());
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if a[i] == b[i] {
            out.push(a[i].clone());
        } else {
            break;
        }
    }
    out
}

fn build_procedure(
    agent_id: &str,
    name: &str,
    resp: &PatternResponse,
) -> Result<Procedure, String> {
    let steps_json =
        serde_json::to_string(&resp.steps).map_err(|e| format!("steps serialize: {e}"))?;
    let params_json = if resp.parameters.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&resp.parameters)
                .map_err(|e| format!("parameters serialize: {e}"))?,
        )
    };
    let now = chrono::Utc::now().to_rfc3339();
    Ok(Procedure {
        id: format!("proc-{}", uuid::Uuid::new_v4()),
        agent_id: agent_id.to_string(),
        ward_id: Some(PROC_WARD.to_string()),
        name: name.to_string(),
        description: resp.description.clone(),
        trigger_pattern: Some(resp.trigger_pattern.clone()),
        steps: steps_json,
        parameters: params_json,
        success_count: 1,
        failure_count: 0,
        avg_duration_ms: None,
        avg_token_cost: None,
        last_used: None,
        embedding: None,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// LLM-backed `PatternExtractLlm`. Conservative on failure — propagates `Err`
/// so `run_cycle` can log+skip.
pub struct LlmPatternExtractor {
    provider_service: Arc<ProviderService>,
}

impl LlmPatternExtractor {
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
        .with_max_tokens(1024);
        let client = agent_runtime::llm::openai::OpenAiClient::new(config)
            .map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn LlmClient>)
    }
}

#[async_trait]
impl PatternExtractLlm for LlmPatternExtractor {
    async fn generalize(&self, input: &PatternInput) -> Result<PatternResponse, String> {
        let client = self.build_client()?;
        let prompt = format!(
            "Two recent successful agent sessions shared a recurring tool-call \
             sequence. Generalize it into a reusable procedure.\n\n\
             Return ONLY JSON: {{\"name\": snake_case_string, \"description\": string, \
             \"trigger_pattern\": string, \"parameters\": [string], \
             \"steps\": [{{\"action\": string, \"agent\": string|null, \
             \"note\": string|null, \"task_template\": string|null}}]}}.\n\n\
             Session A task: {sa}\n\
             Session A tool sequence: {ta:?}\n\n\
             Session B task: {sb}\n\
             Session B tool sequence: {tb:?}\n\n\
             Matched prefix: {mp:?}",
            sa = input.task_summary_a,
            sb = input.task_summary_b,
            ta = input.tool_sequence_a,
            tb = input.tool_sequence_b,
            mp = input.matched_prefix,
        );
        let messages = vec![
            ChatMessage::system("You return only valid JSON.".to_string()),
            ChatMessage::user(prompt),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("LLM call: {e}"))?;
        parse_llm_json::<PatternResponse>(&response.content)
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn sanitize_name(raw: &str) -> String {
    let out: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed: String = out
        .trim_matches('_')
        .chars()
        .scan(' ', |prev, c| {
            let emit = !(c == '_' && *prev == '_');
            *prev = c;
            Some(if emit { Some(c) } else { None })
        })
        .flatten()
        .collect();
    trimmed
}

fn blob_to_f32_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
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
    use zero_stores_sqlite::vector_index::{SqliteVecIndex, VectorIndex};
    use gateway_services::VaultPaths;
    use std::sync::Mutex;

    struct MockLlm {
        response: Mutex<PatternResponse>,
        calls: Mutex<u64>,
    }

    impl MockLlm {
        fn new(resp: PatternResponse) -> Self {
            Self {
                response: Mutex::new(resp),
                calls: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl PatternExtractLlm for MockLlm {
        async fn generalize(&self, _input: &PatternInput) -> Result<PatternResponse, String> {
            *self.calls.lock().unwrap() += 1;
            Ok(self.response.lock().unwrap().clone())
        }
    }

    struct Harness {
        _tmp: tempfile::TempDir,
        knowledge_db: Arc<KnowledgeDatabase>,
        conversations_db: Arc<DatabaseManager>,
        procedure_repo: Arc<ProcedureRepository>,
        compaction_repo: Arc<CompactionRepository>,
    }

    fn setup() -> Harness {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
        let knowledge_db = Arc::new(KnowledgeDatabase::new(paths.clone()).expect("knowledge db"));
        let conversations_db = Arc::new(DatabaseManager::new(paths).expect("convo db"));
        let vec_index: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(knowledge_db.clone(), "procedures_index", "procedure_id")
                .expect("vec index init"),
        );
        let procedure_repo = Arc::new(ProcedureRepository::new(knowledge_db.clone(), vec_index));
        let compaction_repo = Arc::new(CompactionRepository::new(knowledge_db.clone()));
        Harness {
            _tmp: tmp,
            knowledge_db,
            conversations_db,
            procedure_repo,
            compaction_repo,
        }
    }

    fn normalize(v: Vec<f32>) -> Vec<f32> {
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if n < 1e-9 {
            v
        } else {
            v.into_iter().map(|x| x / n).collect()
        }
    }

    fn f32_to_blob(v: &[f32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(v.len() * 4);
        for f in v {
            out.extend_from_slice(&f.to_le_bytes());
        }
        out
    }

    /// Seed two successful session_episodes whose embeddings are identical
    /// (cosine = 1.0) and corresponding messages rows with identical 4-step
    /// tool-call sequences.
    fn seed_pair(h: &Harness, agent_id: &str) -> (String, String) {
        let now = chrono::Utc::now().to_rfc3339();
        let emb: Vec<f32> = normalize((0..384).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect());
        let blob = f32_to_blob(&emb);

        for (ep_id, sess_id, summary) in [
            (
                "ep-A",
                "sess-A",
                "Investigate and fix postgres connection timeout",
            ),
            (
                "ep-B",
                "sess-B",
                "Investigate and fix postgres pool exhaustion",
            ),
        ] {
            h.knowledge_db
                .with_connection(|conn| {
                    conn.execute(
                        "INSERT INTO session_episodes
                            (id, session_id, agent_id, ward_id, task_summary, outcome, created_at)
                         VALUES (?1, ?2, ?3, '__global__', ?4, 'success', ?5)",
                        params![ep_id, sess_id, agent_id, summary, now],
                    )?;
                    conn.execute(
                        "INSERT INTO session_episodes_index (episode_id, embedding) VALUES (?1, ?2)",
                        params![ep_id, blob],
                    )?;
                    Ok(())
                })
                .expect("seed episode");
        }

        // Identical 4-step tool-call sequences for both sessions.
        let tool_seq = serde_json::json!([
            {"tool_id": "t1", "tool_name": "search_docs", "args": {}},
            {"tool_id": "t2", "tool_name": "read_file", "args": {}},
            {"tool_id": "t3", "tool_name": "run_query", "args": {}},
            {"tool_id": "t4", "tool_name": "summarize", "args": {}}
        ])
        .to_string();

        for sess_id in ["sess-A", "sess-B"] {
            h.conversations_db
                .with_connection(|conn| {
                    conn.execute(
                        "INSERT INTO sessions (id, status, source, root_agent_id, created_at)
                         VALUES (?1, 'completed', 'web', ?2, ?3)",
                        params![sess_id, agent_id, now],
                    )?;
                    conn.execute(
                        "INSERT INTO messages (id, session_id, role, content, created_at, tool_calls)
                         VALUES (?1, ?2, 'assistant', '', ?3, ?4)",
                        params![format!("msg-{sess_id}"), sess_id, now, tool_seq],
                    )?;
                    Ok(())
                })
                .expect("seed message");
        }

        ("ep-A".to_string(), "ep-B".to_string())
    }

    fn ok_response(name: &str) -> PatternResponse {
        PatternResponse {
            name: name.to_string(),
            description: "Investigate, then read, then query, then summarize".to_string(),
            trigger_pattern: "investigate postgres * issue".to_string(),
            parameters: vec!["target".to_string()],
            steps: vec![
                PatternStep {
                    action: "search_docs".to_string(),
                    agent: None,
                    note: None,
                    task_template: None,
                },
                PatternStep {
                    action: "read_file".to_string(),
                    agent: None,
                    note: None,
                    task_template: None,
                },
                PatternStep {
                    action: "run_query".to_string(),
                    agent: None,
                    note: None,
                    task_template: None,
                },
                PatternStep {
                    action: "summarize".to_string(),
                    agent: None,
                    note: None,
                    task_template: None,
                },
            ],
        }
    }

    #[tokio::test]
    async fn extracts_pattern_across_two_sessions() {
        let h = setup();
        let agent_id = "agent-px";
        seed_pair(&h, agent_id);

        let mock = Arc::new(MockLlm::new(ok_response("investigate_postgres_issue")));
        let ext = PatternExtractor::new(
            h.knowledge_db.clone(),
            h.conversations_db.clone(),
            h.procedure_repo.clone(),
            h.compaction_repo.clone(),
            mock.clone(),
        );

        let stats = ext.run_cycle("run-pe-1").await.expect("run_cycle");
        assert_eq!(stats.episodes_considered, 2);
        assert_eq!(stats.pairs_evaluated, 1);
        assert_eq!(stats.pairs_matched, 1);
        assert_eq!(stats.llm_calls_made, 1);
        assert_eq!(stats.procedures_inserted, 1);
        assert_eq!(*mock.calls.lock().unwrap(), 1);

        let procs = h
            .procedure_repo
            .list_procedures(agent_id, Some("__global__"))
            .expect("list procs");
        assert_eq!(procs.len(), 1);
        assert_eq!(procs[0].name, "investigate_postgres_issue");
        assert!(procs[0].steps.contains("search_docs"));

        let rows = h.compaction_repo.list_run("run-pe-1").expect("list_run");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].operation, "pattern_extract");
        assert_eq!(rows[0].entity_id.as_deref(), Some(procs[0].id.as_str()));
    }

    #[tokio::test]
    async fn skips_when_existing_procedure_is_locked() {
        let h = setup();
        let agent_id = "agent-px-dup";
        seed_pair(&h, agent_id);

        // Pre-existing procedure with success_count >= DEDUP_SUCCESS_FLOOR.
        let existing = Procedure {
            id: "proc-existing".to_string(),
            agent_id: agent_id.to_string(),
            ward_id: Some("__global__".to_string()),
            name: "investigate_postgres_issue".to_string(),
            description: "locked".to_string(),
            trigger_pattern: None,
            steps: "[]".to_string(),
            parameters: None,
            success_count: 5,
            failure_count: 0,
            avg_duration_ms: None,
            avg_token_cost: None,
            last_used: None,
            embedding: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        h.procedure_repo
            .upsert_procedure(&existing)
            .expect("seed proc");

        let mock = Arc::new(MockLlm::new(ok_response("investigate_postgres_issue")));
        let ext = PatternExtractor::new(
            h.knowledge_db.clone(),
            h.conversations_db.clone(),
            h.procedure_repo.clone(),
            h.compaction_repo.clone(),
            mock,
        );
        let stats = ext.run_cycle("run-pe-2").await.expect("run_cycle");
        assert_eq!(stats.procedures_inserted, 0);
        assert_eq!(stats.skipped_existing, 1);

        let rows = h.compaction_repo.list_run("run-pe-2").expect("list_run");
        assert!(rows.is_empty());
    }

    #[test]
    fn longest_common_prefix_basics() {
        let a = vec!["x", "y", "z", "q"];
        let b = vec!["x", "y", "z", "r"];
        let lcp = longest_common_prefix(&a, &b);
        assert_eq!(lcp, vec!["x", "y", "z"]);
        let c = vec!["p", "y", "z"];
        assert!(longest_common_prefix(&a, &c).is_empty());
    }

    #[test]
    fn cosine_threshold_behavior() {
        let a = normalize(vec![1.0, 0.0, 0.0]);
        let b = normalize(vec![1.0, 0.0, 0.0]);
        assert!(cosine_similarity(&a, &b) >= PAIR_COSINE_THRESHOLD);
        let c = normalize(vec![0.0, 1.0, 0.0]);
        assert!(cosine_similarity(&a, &c) < PAIR_COSINE_THRESHOLD);
        // ~0.85 > threshold
        let d = normalize(vec![0.85, 0.5267, 0.0]);
        assert!(cosine_similarity(&a, &d) >= PAIR_COSINE_THRESHOLD);
    }

    #[test]
    fn sanitize_name_collapses_nonalnum() {
        assert_eq!(
            sanitize_name("Investigate Postgres!!Issue"),
            "investigate_postgres_issue"
        );
        assert_eq!(sanitize_name("   "), "");
        assert_eq!(sanitize_name("a b c"), "a_b_c");
    }

    #[test]
    fn extend_tool_names_parses_stored_format() {
        let blob = serde_json::json!([
            {"tool_id": "t1", "tool_name": "foo", "args": {}},
            {"tool_id": "t2", "tool_name": "bar", "args": {}}
        ])
        .to_string();
        let mut out = Vec::new();
        extend_tool_names(&blob, &mut out);
        assert_eq!(out, vec!["foo".to_string(), "bar".to_string()]);
    }
}
