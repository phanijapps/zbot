//! `POST /api/memory/search` — unified hybrid search across memory types.
//!
//! Fans out a single query to up to four content types (facts, wiki,
//! procedures, episodes) in parallel and returns one block per type with
//! `hits` and a `latency_ms` timer. Modes:
//! - `hybrid` (default): FTS + vec with RRF where available; vec-only for
//!   procedures (no FTS table); vec + LIKE fallback for episodes
//! - `fts`: never embeds the query; procedures return empty; episodes use a
//!   LIKE fallback over `task_summary`/`key_learnings`
//! - `semantic`: embedding required (returns 400 if unavailable); vec-only
//!   across all selected types
//!
//! The `filters` field is accepted but ignored in v1. `limit` applies
//! per-type, not globally.
//!
//! ## Migration status (TD-023)
//!
//! - The episode FTS-fallback path used to inline raw LIKE SQL via
//!   `state.knowledge_db.with_connection`. It now calls
//!   `EpisodeRepository::keyword_search`, so the handler no longer
//!   touches the connection pool directly for that path.
//! - Wiki / procedure / episode repos are built on demand from
//!   `state.knowledge_db`. That's still a typed-repo construction
//!   (not a raw SQL reach-in) — `WardWikiRepository`,
//!   `ProcedureRepository`, and `EpisodeRepository` haven't been
//!   migrated to `zero-stores` traits yet. Tracked under TD-023's
//!   HTTP-handler retirement follow-up.
//! - Memory-fact search continues to call `MemoryRepository`
//!   directly because the `MemoryFactStore` trait surface is JSON-
//!   oriented; converting these handlers requires hoisting `MemoryFact`
//!   to `zero-stores`, which is a separate workstream.
//!
//! Phase E: when the user has opted into the SurrealDB backend,
//! `state.knowledge_db` is `None` and this handler returns
//! `503 Service Unavailable` rather than reach for a SQLite handle
//! that wasn't initialized. Migrating the unified search to trait
//! stores is the natural follow-up — every type already has a
//! `*Store` trait method covering the per-type query.

use crate::state::AppState;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;
use zero_stores_domain::{SessionEpisode, WikiHit};

/// Request body for unified search.
#[derive(Debug, Deserialize)]
pub struct SearchBody {
    pub query: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_types")]
    pub types: Vec<String>,
    #[serde(default)]
    pub ward_ids: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub filters: Option<Value>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Optional agent scope. When present, memory-fact queries are restricted
    /// to facts the given agent can see (its private `scope='agent'` facts
    /// plus all `scope='global'` facts). When absent, no agent filter is
    /// applied — useful for admin/debug views of the full pool.
    #[serde(default)]
    pub agent_id: Option<String>,
}

fn default_mode() -> String {
    "hybrid".into()
}

fn default_types() -> Vec<String> {
    vec![
        "facts".into(),
        "wiki".into(),
        "procedures".into(),
        "episodes".into(),
    ]
}

fn default_limit() -> usize {
    10
}

/// One type's block in the response.
#[derive(Debug, Serialize, Default)]
pub struct TypeBlock {
    pub hits: Vec<Value>,
    pub latency_ms: u64,
}

/// Unified response with one block per content type.
#[derive(Debug, Serialize, Default)]
pub struct UnifiedResponse {
    pub facts: TypeBlock,
    pub wiki: TypeBlock,
    pub procedures: TypeBlock,
    pub episodes: TypeBlock,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}

pub type HandlerError = (StatusCode, Json<ErrorBody>);

fn err(status: StatusCode, msg: impl Into<String>) -> HandlerError {
    (status, Json(ErrorBody { error: msg.into() }))
}

/// Error helper: this handler is not yet migrated to trait stores;
/// when SurrealDB-backend mode disables the SQLite knowledge DB the
/// caller gets a clear 503 instead of a silent SQL reach-in attempt.
fn surreal_unavailable() -> HandlerError {
    err(
        StatusCode::SERVICE_UNAVAILABLE,
        "unified search not yet migrated to trait stores; \
         toggle SurrealDB off in Settings to use the SQLite path",
    )
}

fn wiki_hit_to_value(hit: WikiHit) -> Value {
    let snippet: String = hit.article.content.chars().take(240).collect();
    json!({
        "id": hit.article.id,
        "ward_id": hit.article.ward_id,
        "title": hit.article.title,
        "snippet": snippet,
        "updated_at": hit.article.updated_at,
        "score": hit.score,
        "match_source": hit.match_source,
    })
}

fn procedure_to_value(proc: zero_stores_domain::Procedure, score: f64) -> Value {
    json!({
        "id": proc.id,
        "agent_id": proc.agent_id,
        "ward_id": proc.ward_id,
        "name": proc.name,
        "description": proc.description,
        "success_count": proc.success_count,
        "failure_count": proc.failure_count,
        "last_used": proc.last_used,
        "created_at": proc.created_at,
        "updated_at": proc.updated_at,
        "score": score,
        "match_source": "vec",
    })
}

fn episode_to_value(ep: SessionEpisode, score: Option<f64>, source: &str) -> Value {
    let mut v = json!({
        "id": ep.id,
        "session_id": ep.session_id,
        "agent_id": ep.agent_id,
        "ward_id": ep.ward_id,
        "task_summary": ep.task_summary,
        "outcome": ep.outcome,
        "strategy_used": ep.strategy_used,
        "key_learnings": ep.key_learnings,
        "token_cost": ep.token_cost,
        "created_at": ep.created_at,
        "match_source": source,
    });
    if let (Value::Object(ref mut m), Some(s)) = (&mut v, score) {
        m.insert("score".into(), json!(s));
    }
    v
}

fn fact_to_value(fact: zero_stores_domain::MemoryFact, source: &str, score: Option<f64>) -> Value {
    let mut v = json!({
        "id": fact.id,
        "session_id": fact.session_id,
        "agent_id": fact.agent_id,
        "scope": fact.scope,
        "category": fact.category,
        "key": fact.key,
        "content": fact.content,
        "confidence": fact.confidence,
        "mention_count": fact.mention_count,
        "ward_id": fact.ward_id,
        "created_at": fact.created_at,
        "updated_at": fact.updated_at,
        "pinned": fact.pinned,
        "epistemic_class": fact.epistemic_class,
        "match_source": source,
    });
    if let (Value::Object(ref mut m), Some(s)) = (&mut v, score) {
        m.insert("score".into(), json!(s));
    }
    v
}

/// Fallback agent id used when scoping procedure / episode searches in v1
/// and no `agent_id` was supplied on the request. Procedure/episode repos
/// still require `&str`; the unified-search API will pass the caller's agent
/// when provided and fall back to `"root"` (the production root agent id)
/// otherwise. Memory-fact queries no longer rely on this — they accept
/// `Option<&str>` directly.
const FALLBACK_AGENT: &str = "root";

/// POST /api/memory/search — unified hybrid/fts/semantic search.
pub async fn memory_search(
    State(state): State<AppState>,
    Json(req): Json<SearchBody>,
) -> Result<Json<UnifiedResponse>, HandlerError> {
    let memory_repo = state
        .memory_repo
        .as_ref()
        .ok_or_else(|| err(StatusCode::SERVICE_UNAVAILABLE, "memory repo unavailable"))?
        .clone();
    let wiki_store = state
        .wiki_store
        .as_ref()
        .ok_or_else(surreal_unavailable)?
        .clone();
    let proc_store = state
        .procedure_store
        .as_ref()
        .ok_or_else(surreal_unavailable)?
        .clone();
    let episode_store = state
        .episode_store
        .as_ref()
        .ok_or_else(surreal_unavailable)?
        .clone();

    let ward: Option<String> = req.ward_ids.first().cloned();
    let ward_ref: Option<&str> = ward.as_deref();

    // Optional caller-scoped agent. `None` → no agent/scope gate (admin/debug).
    let agent: Option<String> = req.agent_id.clone();

    // Single embedding attempt, mode-dependent.
    let embedding: Option<Vec<f32>> = match req.mode.as_str() {
        "fts" => None,
        "semantic" => {
            let v = state
                .embedding_service
                .client()
                .embed(&[req.query.as_str()])
                .await
                .map_err(|e| {
                    err(
                        StatusCode::BAD_REQUEST,
                        format!("embedding backend unavailable: {e}"),
                    )
                })?;
            Some(v.into_iter().next().unwrap_or_default())
        }
        _ => match state
            .embedding_service
            .client()
            .embed(&[req.query.as_str()])
            .await
        {
            Ok(v) => v.into_iter().next(),
            Err(e) => {
                tracing::debug!("unified search: embedding unavailable ({e}); FTS-only");
                None
            }
        },
    };

    let want_facts = req.types.iter().any(|t| t == "facts");
    let want_wiki = req.types.iter().any(|t| t == "wiki");
    let want_proc = req.types.iter().any(|t| t == "procedures");
    let want_eps = req.types.iter().any(|t| t == "episodes");

    let query = req.query.clone();
    let mode = req.mode.clone();
    let limit = req.limit;
    let ward_owned = ward.clone();
    let agent_owned = agent.clone();

    let facts_fut = {
        let memory_repo = memory_repo.clone();
        let query = query.clone();
        let mode = mode.clone();
        let emb = embedding.clone();
        let ward = ward_owned.clone();
        let agent = agent_owned.clone();
        async move {
            if !want_facts {
                return TypeBlock::default();
            }
            let t0 = Instant::now();
            let hits = run_facts(
                &memory_repo,
                &query,
                &mode,
                emb.as_deref(),
                agent.as_deref(),
                ward.as_deref(),
                limit,
            );
            TypeBlock {
                hits,
                latency_ms: t0.elapsed().as_millis() as u64,
            }
        }
    };

    let wiki_fut = {
        let wiki_store = wiki_store.clone();
        let query = query.clone();
        let mode = mode.clone();
        let emb = embedding.clone();
        let ward = ward_owned.clone();
        async move {
            if !want_wiki {
                return TypeBlock::default();
            }
            let t0 = Instant::now();
            // In semantic-only mode, pass embedding but use a synthetic query
            // the FTS arm won't match (search_hybrid still runs FTS with it —
            // acceptable: no harm since RRF will favor vec hits).
            let pass_query = if mode == "semantic" {
                ""
            } else {
                query.as_str()
            };
            let hits = wiki_store
                .search_wiki_hybrid_typed(ward.as_deref(), pass_query, limit, emb.as_deref())
                .await
                .unwrap_or_default()
                .into_iter()
                .map(wiki_hit_to_value)
                .collect();
            TypeBlock {
                hits,
                latency_ms: t0.elapsed().as_millis() as u64,
            }
        }
    };

    let proc_fut = {
        let proc_store = proc_store.clone();
        let mode = mode.clone();
        let emb = embedding.clone();
        let ward = ward_owned.clone();
        let agent = agent_owned.clone();
        async move {
            if !want_proc {
                return TypeBlock::default();
            }
            let t0 = Instant::now();
            let scope_agent = agent.as_deref().unwrap_or(FALLBACK_AGENT);
            let hits: Vec<Value> = match (mode.as_str(), emb.as_ref()) {
                // No FTS table for procedures: fts mode returns empty.
                ("fts", _) => Vec::new(),
                (_, Some(emb)) => proc_store
                    .search_procedures_by_similarity_typed(
                        emb,
                        scope_agent,
                        ward.as_deref(),
                        limit,
                    )
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(p, s)| procedure_to_value(p, s))
                    .collect(),
                (_, None) => Vec::new(),
            };
            TypeBlock {
                hits,
                latency_ms: t0.elapsed().as_millis() as u64,
            }
        }
    };

    let eps_fut = {
        let episode_store = episode_store.clone();
        let query = query.clone();
        let mode = mode.clone();
        let emb = embedding.clone();
        let ward = ward_owned.clone();
        let agent = agent_owned.clone();
        async move {
            if !want_eps {
                return TypeBlock::default();
            }
            let t0 = Instant::now();
            let scope_agent = agent.as_deref().unwrap_or(FALLBACK_AGENT);
            let hits: Vec<Value> = match (mode.as_str(), emb.as_ref()) {
                // Pure FTS path → LIKE fallback (no FTS5 partner table for episodes).
                ("fts", _) => episode_store
                    .keyword_search_episodes(&query, ward.as_deref(), limit)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|ep| episode_to_value(ep, None, "fts"))
                    .collect(),
                (_, Some(emb)) => episode_store
                    .search_episodes_by_similarity_typed(scope_agent, emb, 0.0, limit)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    // Ward-scope filter in Rust since the trait method does
                    // not filter by ward.
                    .filter(|(ep, _)| ward.as_deref().is_none_or(|w| ep.ward_id == w))
                    .map(|(ep, s)| episode_to_value(ep, Some(s), "vec"))
                    .collect(),
                (_, None) => episode_store
                    .keyword_search_episodes(&query, ward.as_deref(), limit)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|ep| episode_to_value(ep, None, "fts"))
                    .collect(),
            };
            TypeBlock {
                hits,
                latency_ms: t0.elapsed().as_millis() as u64,
            }
        }
    };

    let _ = ward_ref; // silence unused if branches skip
    let (facts, wiki, procedures, episodes) = tokio::join!(facts_fut, wiki_fut, proc_fut, eps_fut);

    Ok(Json(UnifiedResponse {
        facts,
        wiki,
        procedures,
        episodes,
    }))
}

/// Route a fact search through the memory repo according to mode. Returns
/// JSON-ready hits.
///
/// `agent_id` is threaded straight into the repo methods — `Some(a)` yields
/// scope-aware results (agent's private + global), `None` returns the
/// unfiltered pool (admin/debug).
#[allow(clippy::too_many_arguments)]
fn run_facts(
    memory_repo: &zero_stores_sqlite::MemoryRepository,
    query: &str,
    mode: &str,
    embedding: Option<&[f32]>,
    agent_id: Option<&str>,
    ward: Option<&str>,
    limit: usize,
) -> Vec<Value> {
    match mode {
        "fts" => memory_repo
            .search_memory_facts_fts(query, agent_id, limit, ward)
            .unwrap_or_default()
            .into_iter()
            .map(|sf| fact_to_value(sf.fact, "fts", Some(sf.score)))
            .collect(),
        "semantic" => {
            let Some(emb) = embedding else {
                return Vec::new();
            };
            memory_repo
                .search_similar_facts(emb, agent_id, 0.0, limit, ward)
                .unwrap_or_default()
                .into_iter()
                .map(|sf| fact_to_value(sf.fact, "vec", Some(sf.score)))
                .collect()
        }
        _ => {
            let (rows, sources) = memory_repo
                .search_memory_facts_hybrid(query, embedding, agent_id, limit, 0.7, 0.3, ward)
                .unwrap_or_default();
            let src_map: std::collections::HashMap<String, &'static str> =
                sources.into_iter().collect();
            rows.into_iter()
                .map(|sf| {
                    let src = src_map.get(&sf.fact.id).copied().unwrap_or("fts");
                    fact_to_value(sf.fact, src, Some(sf.score))
                })
                .collect()
        }
    }
}
