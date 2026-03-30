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

    // Use FTS5 search (hybrid requires embeddings which we don't have in HTTP API)
    let results = memory_repo
        .search_memory_facts_fts(&query.q, &agent_id, query.limit, None)
        .map_err(|e| {
            tracing::error!("Failed to search memory facts: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to search memory facts: {}", e),
                }),
            )
        })?;

    // Filter by category if specified
    let facts: Vec<MemoryFactResponse> = results
        .into_iter()
        .filter(|sf| {
            query
                .category
                .as_ref()
                .map(|c| sf.fact.category == *c)
                .unwrap_or(true)
        })
        .map(|sf| MemoryFactResponse::from(sf.fact))
        .collect();

    let total = facts.len();

    Ok(Json(MemoryListResponse { facts, total }))
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

    let fact = memory_repo
        .get_memory_fact_by_id(&fact_id)
        .map_err(|e| {
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
    let fact = memory_repo
        .get_memory_fact_by_id(&fact_id)
        .map_err(|e| {
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
            let deleted = memory_repo
                .delete_memory_fact(&fact_id)
                .map_err(|e| {
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
