//! # Memory Endpoints
//!
//! CRUD and search operations for agent memory facts.

use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use gateway_database::MemoryFact;
use serde::{Deserialize, Serialize};

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Memory fact response for API.
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryFactResponse {
    pub id: String,
    pub agent_id: String,
    pub scope: String,
    pub category: String,
    pub key: String,
    pub content: String,
    pub confidence: f64,
    pub mention_count: i32,
    pub source_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Which search arm matched this fact: `"fts"`, `"vec"`, or `"hybrid"`.
    /// Only populated on search responses; `None` elsewhere and omitted from JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_source: Option<String>,
}

impl From<MemoryFact> for MemoryFactResponse {
    fn from(fact: MemoryFact) -> Self {
        Self {
            id: fact.id,
            agent_id: fact.agent_id,
            scope: fact.scope,
            category: fact.category,
            key: fact.key,
            content: fact.content,
            confidence: fact.confidence,
            mention_count: fact.mention_count,
            source_summary: fact.source_summary,
            created_at: fact.created_at,
            updated_at: fact.updated_at,
            match_source: None,
        }
    }
}

/// Query parameters for listing memory facts.
#[derive(Debug, Deserialize)]
pub struct MemoryListQuery {
    pub category: Option<String>,
    pub scope: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

/// Query parameters for listing ALL memory facts (across all agents).
#[derive(Debug, Deserialize)]
pub struct AllMemoryListQuery {
    pub agent_id: Option<String>,
    pub category: Option<String>,
    pub scope: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

/// Query parameters for searching memory facts.
#[derive(Debug, Deserialize)]
pub struct MemorySearchQuery {
    pub q: String,
    pub category: Option<String>,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    /// `"hybrid"` (default), `"fts"`, or `"semantic"`.
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub ward_id: Option<String>,
}

fn default_search_limit() -> usize {
    20
}

/// Response for list operations with pagination info.
#[derive(Debug, Serialize)]
pub struct MemoryListResponse {
    pub facts: Vec<MemoryFactResponse>,
    pub total: usize,
}

/// Error response.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============================================================================
// HANDLERS
// ============================================================================

/// GET /api/memory/:agent_id - List memory facts for an agent.
pub async fn list_memory_facts(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<MemoryListQuery>,
) -> Result<Json<MemoryListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let memory_repo = match &state.memory_repo {
        Some(repo) => repo,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Memory service not available".to_string(),
                }),
            ));
        }
    };

    let facts = memory_repo
        .list_memory_facts(
            &agent_id,
            query.category.as_deref(),
            query.scope.as_deref(),
            query.limit,
            query.offset,
        )
        .map_err(|e| {
            tracing::error!("Failed to list memory facts: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to list memory facts: {}", e),
                }),
            )
        })?;

    let total = memory_repo.count_memory_facts(&agent_id).unwrap_or(0);

    Ok(Json(MemoryListResponse {
        facts: facts.into_iter().map(MemoryFactResponse::from).collect(),
        total,
    }))
}

/// GET /api/memory/:agent_id/search - Search memory facts.
pub async fn search_memory_facts(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<MemorySearchQuery>,
) -> Result<Json<MemoryListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let memory_repo = match &state.memory_repo {
        Some(repo) => repo,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Memory service not available".to_string(),
                }),
            ));
        }
    };

    let mode = query.mode.as_deref().unwrap_or("hybrid");
    let ward_id = query.ward_id.as_deref();

    // Build (fact, source) pairs according to the requested mode.
    let scope_agent: Option<&str> = Some(agent_id.as_str());
    let scored: Vec<(MemoryFact, &'static str)> = match mode {
        "fts" => {
            let rows = memory_repo
                .search_memory_facts_fts(&query.q, scope_agent, query.limit, ward_id)
                .map_err(|e| search_err("Failed to search memory facts (fts)", e))?;
            rows.into_iter().map(|sf| (sf.fact, "fts")).collect()
        }
        "semantic" => {
            // Bubble embedding errors as 400 — caller asked for a mode that
            // requires an embedding backend.
            let emb = state
                .embedding_service
                .client()
                .embed(&[query.q.as_str()])
                .await
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: format!("Embedding backend unavailable: {}", e),
                        }),
                    )
                })?;
            let qe = emb.into_iter().next().unwrap_or_default();
            let rows = memory_repo
                .search_similar_facts(&qe, scope_agent, 0.0, query.limit, ward_id)
                .map_err(|e| search_err("Failed to search memory facts (semantic)", e))?;
            rows.into_iter().map(|sf| (sf.fact, "vec")).collect()
        }
        // Default: hybrid. If embedding fails, degrade to FTS-only (embedding=None).
        _ => {
            let qe_opt: Option<Vec<f32>> = match state
                .embedding_service
                .client()
                .embed(&[query.q.as_str()])
                .await
            {
                Ok(v) => v.into_iter().next(),
                Err(e) => {
                    tracing::debug!("hybrid search: embedding unavailable ({e}); FTS-only");
                    None
                }
            };
            let (rows, sources) = memory_repo
                .search_memory_facts_hybrid(
                    &query.q,
                    qe_opt.as_deref(),
                    scope_agent,
                    query.limit,
                    0.7,
                    0.3,
                    ward_id,
                )
                .map_err(|e| search_err("Failed to search memory facts (hybrid)", e))?;
            let src_map: std::collections::HashMap<String, &'static str> =
                sources.into_iter().collect();
            rows.into_iter()
                .map(|sf| {
                    let src = src_map.get(&sf.fact.id).copied().unwrap_or("fts");
                    (sf.fact, src)
                })
                .collect()
        }
    };

    // Filter by category if specified; attach match_source.
    let facts: Vec<MemoryFactResponse> = scored
        .into_iter()
        .filter(|(fact, _)| {
            query
                .category
                .as_ref()
                .map(|c| fact.category == *c)
                .unwrap_or(true)
        })
        .map(|(fact, src)| {
            let mut r = MemoryFactResponse::from(fact);
            r.match_source = Some(src.to_string());
            r
        })
        .collect();

    let total = facts.len();

    Ok(Json(MemoryListResponse { facts, total }))
}

fn search_err(context: &str, e: String) -> (StatusCode, Json<ErrorResponse>) {
    tracing::error!("{}: {}", context, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: format!("{}: {}", context, e),
        }),
    )
}

/// GET /api/memory/:agent_id/facts/:fact_id - Get a single memory fact.
pub async fn get_memory_fact(
    State(state): State<AppState>,
    Path((agent_id, fact_id)): Path<(String, String)>,
) -> Result<Json<MemoryFactResponse>, (StatusCode, Json<ErrorResponse>)> {
    let memory_repo = match &state.memory_repo {
        Some(repo) => repo,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Memory service not available".to_string(),
                }),
            ));
        }
    };

    let fact = memory_repo.get_memory_fact_by_id(&fact_id).map_err(|e| {
        tracing::error!("Failed to get memory fact: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get memory fact: {}", e),
            }),
        )
    })?;

    match fact {
        Some(f) if f.agent_id == agent_id => Ok(Json(MemoryFactResponse::from(f))),
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Fact does not belong to this agent".to_string(),
            }),
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Memory fact not found".to_string(),
            }),
        )),
    }
}

/// DELETE /api/memory/:agent_id/facts/:fact_id - Delete a memory fact.
pub async fn delete_memory_fact(
    State(state): State<AppState>,
    Path((agent_id, fact_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let memory_repo = match &state.memory_repo {
        Some(repo) => repo,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Memory service not available".to_string(),
                }),
            ));
        }
    };

    // First verify the fact belongs to this agent
    let fact = memory_repo.get_memory_fact_by_id(&fact_id).map_err(|e| {
        tracing::error!("Failed to get memory fact for deletion: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get memory fact: {}", e),
            }),
        )
    })?;

    match fact {
        Some(f) if f.agent_id == agent_id => {
            let deleted = memory_repo.delete_memory_fact(&fact_id).map_err(|e| {
                tracing::error!("Failed to delete memory fact: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to delete memory fact: {}", e),
                    }),
                )
            })?;

            if deleted {
                Ok(StatusCode::NO_CONTENT)
            } else {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "Memory fact not found".to_string(),
                    }),
                ))
            }
        }
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Fact does not belong to this agent".to_string(),
            }),
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Memory fact not found".to_string(),
            }),
        )),
    }
}

/// POST /api/memory/:agent_id — Create a new memory fact (policy, instruction, or about-me).
#[derive(Debug, Deserialize)]
pub struct CreateMemoryFactRequest {
    pub category: String,
    pub key: String,
    pub content: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default)]
    pub ward_id: Option<String>,
    #[serde(default = "default_true")]
    pub pinned: bool,
}

fn default_confidence() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}

pub async fn create_memory_fact(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(request): Json<CreateMemoryFactRequest>,
) -> Result<(StatusCode, Json<MemoryFactResponse>), (StatusCode, Json<ErrorResponse>)> {
    let memory_repo = match &state.memory_repo {
        Some(repo) => repo,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Memory service not available".to_string(),
                }),
            ));
        }
    };

    let now = chrono::Utc::now().to_rfc3339();
    let fact = MemoryFact {
        id: format!("fact-{}", uuid::Uuid::new_v4()),
        session_id: None,
        agent_id: agent_id.clone(),
        scope: "agent".to_string(),
        category: request.category.clone(),
        key: request.key.clone(),
        content: request.content.clone(),
        confidence: request.confidence,
        mention_count: 5,
        source_summary: Some("User-created via UI".to_string()),
        embedding: None,
        ward_id: request.ward_id.unwrap_or_else(|| "__global__".to_string()),
        contradicted_by: None,
        created_at: now.clone(),
        updated_at: now,
        expires_at: None,
        valid_from: None,
        valid_until: None,
        superseded_by: None,
        pinned: request.pinned,
        epistemic_class: Some("current".to_string()),
        source_episode_id: None,
        source_ref: None,
    };

    memory_repo.upsert_memory_fact(&fact).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create fact: {}", e),
            }),
        )
    })?;

    Ok((StatusCode::CREATED, Json(MemoryFactResponse::from(fact))))
}

/// GET /api/memory/search — Search ALL memory facts across all agents (server-side FTS5).
#[derive(Debug, Deserialize)]
pub struct GlobalMemorySearchQuery {
    pub q: String,
    #[serde(default = "default_global_search_limit")]
    pub limit: usize,
    pub category: Option<String>,
}

fn default_global_search_limit() -> usize {
    50
}

pub async fn search_all_memory_facts(
    State(state): State<AppState>,
    Query(query): Query<GlobalMemorySearchQuery>,
) -> Result<Json<MemoryListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let memory_repo = match &state.memory_repo {
        Some(repo) => repo,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Memory service not available".to_string(),
                }),
            ));
        }
    };

    let results = memory_repo
        .search_all_memory_facts_fts(&query.q, query.limit, query.category.as_deref())
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Search failed: {}", e),
                }),
            )
        })?;

    let facts: Vec<MemoryFactResponse> = results
        .into_iter()
        .map(|sf| MemoryFactResponse::from(sf.fact))
        .collect();
    let total = facts.len();

    Ok(Json(MemoryListResponse { facts, total }))
}

/// GET /api/memory - List ALL memory facts across all agents.
pub async fn list_all_memory_facts(
    Query(query): Query<AllMemoryListQuery>,
    State(state): State<AppState>,
) -> Result<Json<MemoryListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let memory_repo = match &state.memory_repo {
        Some(repo) => repo,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Memory service not available".to_string(),
                }),
            ));
        }
    };

    let facts = memory_repo
        .list_all_memory_facts(
            query.agent_id.as_deref(),
            query.category.as_deref(),
            query.scope.as_deref(),
            query.limit,
            query.offset,
        )
        .map_err(|e| {
            tracing::error!("Failed to list all memory facts: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to list memory facts: {}", e),
                }),
            )
        })?;

    let total = memory_repo
        .count_all_memory_facts(query.agent_id.as_deref())
        .unwrap_or(0);

    Ok(Json(MemoryListResponse {
        facts: facts.into_iter().map(MemoryFactResponse::from).collect(),
        total,
    }))
}

// ============================================================================
// PHASE 4: CONSOLIDATE / STATS / HEALTH
// ============================================================================

/// Response for `POST /api/memory/consolidate`.
#[derive(Debug, Serialize)]
pub struct ConsolidateResponse {
    pub status: &'static str,
}

/// Trigger a sleep-time consolidation cycle.
///
/// Returns `503 Service Unavailable` when the worker has not been wired
/// into `AppState` yet.
pub async fn consolidate(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<ConsolidateResponse>), (StatusCode, String)> {
    let worker = state.sleep_time_worker.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "sleep-time worker not initialized".to_string(),
    ))?;
    worker.trigger();
    Ok((
        StatusCode::ACCEPTED,
        Json(ConsolidateResponse {
            status: "triggered",
        }),
    ))
}

/// Memory subsystem stats response.
#[derive(Debug, Serialize, Default)]
pub struct MemoryStats {
    pub entities: i64,
    pub relationships: i64,
    pub facts: i64,
    pub episodes: i64,
    pub procedures: i64,
    pub wiki_articles: i64,
    pub goals_active: i64,
    pub db_size_mb: f64,
}

fn count_row(conn: &rusqlite::Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0)
}

/// `GET /api/memory/stats` — aggregate counts across memory subsystems.
pub async fn stats(State(state): State<AppState>) -> Json<MemoryStats> {
    let mut stats = MemoryStats::default();

    if let Some(graph_service) = state.graph_service.as_ref() {
        let storage = graph_service.storage();
        if let Ok(entities) = storage.get_entities("root") {
            stats.entities = entities.len() as i64;
        }
        if let Ok(rels) = storage.get_relationships("root") {
            stats.relationships = rels.len() as i64;
        }
    }

    let _ = state.knowledge_db.with_connection(|conn| {
        stats.facts = count_row(conn, "SELECT COUNT(*) FROM memory_facts");
        stats.episodes = count_row(conn, "SELECT COUNT(*) FROM kg_episodes");
        stats.procedures = count_row(conn, "SELECT COUNT(*) FROM procedures");
        stats.wiki_articles = count_row(conn, "SELECT COUNT(*) FROM ward_wiki_articles");
        stats.goals_active =
            count_row(conn, "SELECT COUNT(*) FROM kg_goals WHERE state = 'active'");
        Ok(())
    });

    let knowledge_path = state.paths.knowledge_db();
    if let Ok(meta) = std::fs::metadata(&knowledge_path) {
        // Safe: file sizes fit in f64 precision well within petabyte range.
        stats.db_size_mb = (meta.len() as f64) / (1024.0 * 1024.0);
    }

    Json(stats)
}

/// Memory subsystem health response.
#[derive(Debug, Serialize, Default)]
pub struct MemoryHealth {
    pub ingestion_queue_pending: u64,
    pub ingestion_queue_running: u64,
    pub failed_episodes_recent: u64,
    pub last_compaction_run_id: Option<String>,
    pub last_compaction_merges: u64,
    pub last_compaction_prunes: u64,
    pub last_compaction_at: Option<String>,
}

/// `GET /api/memory/health` — queue depth, recent failures, last compaction.
pub async fn health(State(state): State<AppState>) -> Json<MemoryHealth> {
    let mut health = MemoryHealth::default();

    if let Some(repo) = state.kg_episode_repo.as_ref() {
        if let Ok(n) = repo.count_pending_global() {
            health.ingestion_queue_pending = n;
        }
        let _ = state.knowledge_db.with_connection(|conn| {
            let failed = count_row(
                conn,
                "SELECT COUNT(*) FROM kg_episodes WHERE status = 'failed'",
            );
            let running = count_row(
                conn,
                "SELECT COUNT(*) FROM kg_episodes WHERE status = 'running'",
            );
            health.failed_episodes_recent = failed.max(0) as u64;
            health.ingestion_queue_running = running.max(0) as u64;
            Ok(())
        });
    }

    if let Some(compaction_repo) = state.compaction_repo.as_ref() {
        if let Ok(Some(summary)) = compaction_repo.latest_run_summary() {
            health.last_compaction_run_id = Some(summary.run_id);
            health.last_compaction_merges = summary.merges;
            health.last_compaction_prunes = summary.prunes;
            health.last_compaction_at = Some(summary.latest_at);
        }
    }

    Json(health)
}
