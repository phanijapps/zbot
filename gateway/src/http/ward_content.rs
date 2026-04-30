//! `GET /api/wards/:ward_id/content` — ward content aggregator.
//!
//! Returns a single snapshot of everything that belongs to one ward: the four
//! content types (facts, wiki, procedures, episodes), counts per type, and a
//! derived summary sourced from the ward's `__index__` wiki article (if any).
//! Each item is stamped with a server-computed `age_bucket` using the helper
//! in [`zero_stores_sqlite::age_bucket`] so the UI doesn't need to reimplement
//! recency classification.
//!
//! Limits: facts, wiki and procedures are capped at 100 rows; episodes at 50.
//! The episode/wiki/procedure paths are trait-routed (`state.episode_store`,
//! `state.wiki_store`, `state.procedure_store`) so the SurrealDB backend
//! is honored when opted in.
//!
//! ## Migration status (TD-023)
//!
//! Episode / wiki / procedure listings route through the trait stores;
//! the legacy SQLite-backed repositories are built lazily as a
//! fallback ONLY when both the trait store is unwired AND
//! `state.knowledge_db` is `Some`. In SurrealDB-backend mode the
//! SQLite handle is `None` so the trait stores are the only path —
//! requests that arrive before stores are wired return
//! `503 Service Unavailable` rather than panic.
//!
//! Memory-fact listings (`facts` field, `list_wards` endpoint)
//! still depend on the concrete `MemoryRepository`. Migrating those
//! requires hoisting `MemoryFact` from `zero-stores-sqlite` up to
//! `zero-stores`, which has a large blast radius (11 import sites)
//! and is intentionally a separate workstream — when the user is
//! on the SurrealDB backend these endpoints return 503.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use zero_stores_domain::{MemoryFact, Procedure, SessionEpisode, WikiArticle};
use zero_stores_sqlite::{
    age_bucket, vector_index::VectorIndex, EpisodeRepository, ProcedureRepository, SqliteVecIndex,
    WardWikiRepository,
};

const FACT_LIMIT: usize = 100;
const WIKI_LIMIT: usize = 100;
const PROCEDURE_LIMIT: usize = 100;
const EPISODE_LIMIT: usize = 50;

/// Response body for `GET /api/wards/:ward_id/content`.
#[derive(Debug, Serialize)]
pub struct WardContentResponse {
    pub ward_id: String,
    pub summary: WardSummary,
    pub facts: Vec<Value>,
    pub wiki: Vec<Value>,
    pub procedures: Vec<Value>,
    pub episodes: Vec<Value>,
    pub counts: Counts,
}

/// Derived summary for a ward — title/description/updated_at pulled from the
/// ward's `__index__` wiki article when present; falls back to the ward id.
#[derive(Debug, Serialize)]
pub struct WardSummary {
    pub title: String,
    pub description: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct Counts {
    pub facts: usize,
    pub wiki: usize,
    pub procedures: usize,
    pub episodes: usize,
}

/// Error response shape (matches the convention used by other HTTP modules).
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}

pub type HandlerError = (StatusCode, Json<ErrorBody>);

fn internal(context: &str, e: impl std::fmt::Display) -> HandlerError {
    tracing::error!("{}: {}", context, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody {
            error: format!("{}: {}", context, e),
        }),
    )
}

/// Parse an RFC-3339 timestamp into UTC; returns `None` for blanks or malformed
/// input.
fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Attach `age_bucket` to a serialized item relative to `now`, using
/// `created_at` as the recency anchor when `anchor` is `None`.
fn stamp(mut value: Value, now: DateTime<Utc>, anchor: Option<&str>) -> Value {
    let bucket = anchor
        .and_then(parse_ts)
        .map(|ts| age_bucket(now, ts))
        .unwrap_or("historical");
    if let Value::Object(ref mut map) = value {
        map.insert("age_bucket".to_string(), Value::String(bucket.to_string()));
    }
    value
}

fn first_non_empty_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn build_summary(ward_id: &str, wiki: &[WikiArticle]) -> WardSummary {
    if let Some(idx) = wiki.iter().find(|a| a.title == "__index__") {
        return WardSummary {
            title: ward_id.to_string(),
            description: first_non_empty_line(&idx.content),
            updated_at: Some(idx.updated_at.clone()),
        };
    }
    WardSummary {
        title: ward_id.to_string(),
        description: None,
        updated_at: None,
    }
}

/// Error helper for the SurrealDB-backend path: the SQLite trait stores are
/// unwired AND the SQLite knowledge DB is disabled, so this listing has
/// nowhere to come from.
fn surreal_unavailable(what: &str) -> HandlerError {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorBody {
            error: format!(
                "{what} listing not yet migrated to trait stores; \
                 toggle SurrealDB off in Settings to use the SQLite path"
            ),
        }),
    )
}

/// Build the legacy SQLite-backed wiki repository. Returns `None` when
/// `state.knowledge_db` is `None` (SurrealDB mode) so callers can decide
/// whether to fall back to the trait store or 503.
fn build_wiki_repo(state: &AppState) -> Result<Option<Arc<WardWikiRepository>>, HandlerError> {
    let Some(knowledge_db) = state.knowledge_db.as_ref() else {
        return Ok(None);
    };
    let idx = SqliteVecIndex::new(knowledge_db.clone(), "wiki_articles_index", "article_id")
        .map_err(|e| internal("wiki vec index", e))?;
    let vec: Arc<dyn VectorIndex> = Arc::new(idx);
    Ok(Some(Arc::new(WardWikiRepository::new(
        knowledge_db.clone(),
        vec,
    ))))
}

fn build_procedure_repo(
    state: &AppState,
) -> Result<Option<Arc<ProcedureRepository>>, HandlerError> {
    let Some(knowledge_db) = state.knowledge_db.as_ref() else {
        return Ok(None);
    };
    let idx = SqliteVecIndex::new(knowledge_db.clone(), "procedures_index", "procedure_id")
        .map_err(|e| internal("procedure vec index", e))?;
    let vec: Arc<dyn VectorIndex> = Arc::new(idx);
    Ok(Some(Arc::new(ProcedureRepository::new(
        knowledge_db.clone(),
        vec,
    ))))
}

fn build_episode_repo(state: &AppState) -> Result<Option<Arc<EpisodeRepository>>, HandlerError> {
    let Some(knowledge_db) = state.knowledge_db.as_ref() else {
        return Ok(None);
    };
    let idx = SqliteVecIndex::new(
        knowledge_db.clone(),
        "session_episodes_index",
        "episode_id",
    )
    .map_err(|e| internal("episode vec index", e))?;
    let vec: Arc<dyn VectorIndex> = Arc::new(idx);
    Ok(Some(Arc::new(EpisodeRepository::new(
        knowledge_db.clone(),
        vec,
    ))))
}

fn fact_to_value(fact: MemoryFact, now: DateTime<Utc>) -> Value {
    let updated = fact.updated_at.clone();
    let body = json!({
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
    });
    stamp(body, now, Some(&updated))
}

fn wiki_to_value(article: WikiArticle, now: DateTime<Utc>) -> Value {
    let updated = article.updated_at.clone();
    let body = json!({
        "id": article.id,
        "ward_id": article.ward_id,
        "agent_id": article.agent_id,
        "title": article.title,
        "content": article.content,
        "tags": article.tags,
        "version": article.version,
        "created_at": article.created_at,
        "updated_at": article.updated_at,
    });
    stamp(body, now, Some(&updated))
}

fn procedure_to_value(proc: Procedure, now: DateTime<Utc>) -> Value {
    // Prefer `last_used` as the recency anchor if present, else fall back to
    // `created_at`.
    let anchor = proc
        .last_used
        .clone()
        .unwrap_or_else(|| proc.created_at.clone());
    let body = json!({
        "id": proc.id,
        "agent_id": proc.agent_id,
        "ward_id": proc.ward_id,
        "name": proc.name,
        "description": proc.description,
        "trigger_pattern": proc.trigger_pattern,
        "success_count": proc.success_count,
        "failure_count": proc.failure_count,
        "avg_duration_ms": proc.avg_duration_ms,
        "last_used": proc.last_used,
        "created_at": proc.created_at,
        "updated_at": proc.updated_at,
    });
    stamp(body, now, Some(&anchor))
}

fn episode_to_value(ep: SessionEpisode, now: DateTime<Utc>) -> Value {
    let created = ep.created_at.clone();
    let body = json!({
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
    });
    stamp(body, now, Some(&created))
}

/// GET /api/wards/:ward_id/content — aggregate all ward-scoped content.
pub async fn get_ward_content(
    State(state): State<AppState>,
    Path(ward_id): Path<String>,
) -> Result<Json<WardContentResponse>, HandlerError> {
    // Memory facts: SQLite path uses `MemoryRepository::list_by_ward` (a
    // single WHERE-on-index query). SurrealDB path streams via the trait
    // surface and filters in handler — the trait surface doesn't yet
    // accept a ward_id argument.
    let facts: Vec<MemoryFact> = if let Some(memory_repo) = state.memory_repo.as_ref() {
        memory_repo
            .list_by_ward(&ward_id, FACT_LIMIT)
            .map_err(|e| internal("list facts by ward", e))?
    } else if let Some(memory_store) = state.memory_store.as_ref() {
        const FACT_AGG_LIMIT: usize = 5000;
        let raw = memory_store
            .list_memory_facts(None, None, None, FACT_AGG_LIMIT, 0)
            .await
            .map_err(|e| internal("list facts by ward (trait)", e))?;
        raw.into_iter()
            .filter(|v| {
                v.get("ward_id").and_then(|w| w.as_str()) == Some(ward_id.as_str())
            })
            .filter_map(|v| serde_json::from_value::<MemoryFact>(v).ok())
            .take(FACT_LIMIT)
            .collect()
    } else {
        return Err(surreal_unavailable("ward facts"));
    };

    // Episode / wiki / procedure listings prefer the trait store when
    // it's wired (SurrealDB or SQLite). The SQLite-backed repo build
    // is a fallback only when the trait store is `None` — which only
    // happens for `AppState::minimal()` test builds today.
    let episode_values: Vec<Value> = match state.episode_store.as_ref() {
        Some(store) => store
            .list_by_ward(&ward_id, EPISODE_LIMIT)
            .await
            .map_err(|e| internal("list episodes by ward", e))?,
        None => match build_episode_repo(&state)? {
            Some(repo) => repo
                .list_by_ward(&ward_id, EPISODE_LIMIT)
                .map_err(|e| internal("list episodes by ward", e))?
                .into_iter()
                .map(|ep| serde_json::to_value(ep).unwrap_or(Value::Null))
                .collect(),
            None => return Err(surreal_unavailable("episodes")),
        },
    };

    let wiki_articles: Vec<WikiArticle> = match state.wiki_store.as_ref() {
        Some(store) => {
            let raw = store
                .list_articles(&ward_id)
                .await
                .map_err(|e| internal("list wiki by ward (trait)", e))?;
            raw.into_iter()
                .filter_map(|v| serde_json::from_value::<WikiArticle>(v).ok())
                .collect()
        }
        None => match build_wiki_repo(&state)? {
            Some(repo) => repo
                .list_articles(&ward_id)
                .map_err(|e| internal("list wiki by ward", e))?,
            None => return Err(surreal_unavailable("wiki articles")),
        },
    };

    let procedures: Vec<Procedure> = match state.procedure_store.as_ref() {
        Some(store) => {
            let raw = store
                .list_by_ward(&ward_id, PROCEDURE_LIMIT)
                .await
                .map_err(|e| internal("list procedures by ward (trait)", e))?;
            raw.into_iter()
                .filter_map(|v| serde_json::from_value::<Procedure>(v).ok())
                .collect()
        }
        None => match build_procedure_repo(&state)? {
            Some(repo) => repo
                .list_by_ward(&ward_id, PROCEDURE_LIMIT)
                .map_err(|e| internal("list procedures by ward", e))?,
            None => return Err(surreal_unavailable("procedures")),
        },
    };

    // Cap wiki at WIKI_LIMIT (list_articles has no LIMIT clause).
    let wiki_articles: Vec<WikiArticle> = wiki_articles.into_iter().take(WIKI_LIMIT).collect();

    let summary = build_summary(&ward_id, &wiki_articles);

    let now = Utc::now();

    let counts = Counts {
        facts: facts.len(),
        wiki: wiki_articles.len(),
        procedures: procedures.len(),
        episodes: episode_values.len(),
    };

    let facts_json: Vec<Value> = facts.into_iter().map(|f| fact_to_value(f, now)).collect();
    let wiki_json: Vec<Value> = wiki_articles
        .into_iter()
        .map(|a| wiki_to_value(a, now))
        .collect();
    let procedures_json: Vec<Value> = procedures
        .into_iter()
        .map(|p| procedure_to_value(p, now))
        .collect();
    // Episode values come from the trait already as MemoryFactResponse-style
    // JSON; deserialize each into SessionEpisode for the response decorator,
    // and skip rows that fail to decode.
    let episodes_json: Vec<Value> = episode_values
        .into_iter()
        .filter_map(|v| serde_json::from_value::<zero_stores_sqlite::SessionEpisode>(v).ok())
        .map(|e| episode_to_value(e, now))
        .collect();

    Ok(Json(WardContentResponse {
        ward_id,
        summary,
        facts: facts_json,
        wiki: wiki_json,
        procedures: procedures_json,
        episodes: episodes_json,
        counts,
    }))
}

/// Response item for `GET /api/wards` — one ward entry.
#[derive(Debug, Serialize)]
pub struct WardListItem {
    pub id: String,
    pub count: usize,
}

/// GET /api/wards — list distinct wards with fact counts.
///
/// SQLite path uses `MemoryRepository::list_wards` (a single GROUP BY).
/// SurrealDB path streams up to `WARD_AGG_LIMIT` facts via the trait
/// surface and aggregates `ward_id` distinct counts in the handler —
/// the trait does not expose a distinct-projection method yet.
pub async fn list_wards(
    State(state): State<AppState>,
) -> Result<Json<Vec<WardListItem>>, HandlerError> {
    if let Some(memory_repo) = state.memory_repo.as_ref() {
        let rows = memory_repo
            .list_wards()
            .map_err(|e| internal("list wards", e))?;
        return Ok(Json(
            rows.into_iter()
                .map(|(id, count)| WardListItem { id, count })
                .collect(),
        ));
    }

    let memory_store = state
        .memory_store
        .as_ref()
        .ok_or_else(|| surreal_unavailable("ward listing"))?;

    const WARD_AGG_LIMIT: usize = 5000;
    let rows = memory_store
        .list_memory_facts(None, None, None, WARD_AGG_LIMIT, 0)
        .await
        .map_err(|e| internal("list wards (trait)", e))?;

    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for row in rows {
        let ward = row
            .get("ward_id")
            .and_then(|v| v.as_str())
            .unwrap_or("__global__");
        *counts.entry(ward.to_string()).or_insert(0) += 1;
    }

    let mut items: Vec<WardListItem> = counts
        .into_iter()
        .map(|(id, count)| WardListItem { id, count })
        .collect();
    items.sort_by(|a, b| b.count.cmp(&a.count).then(a.id.cmp(&b.id)));

    Ok(Json(items))
}
