//! # Knowledge Graph Endpoints
//!
//! HTTP API for querying the knowledge graph.

use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use gateway_database::{DistillationStats, UndistilledSession};
use knowledge_graph::{Direction, Entity, GraphStats, Relationship, Subgraph};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Query parameters for listing entities.
#[derive(Debug, Deserialize)]
pub struct EntityListQuery {
    /// Filter by entity type (e.g., "person", "tool", "project")
    pub entity_type: Option<String>,
    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
}

/// Query parameters for listing relationships.
#[derive(Debug, Deserialize)]
pub struct RelationshipListQuery {
    /// Filter by relationship type (e.g., "uses", "created", "part_of")
    pub relationship_type: Option<String>,
    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
}

/// Query parameters for neighbor queries.
#[derive(Debug, Deserialize)]
pub struct NeighborQuery {
    /// Direction of relationships to follow
    #[serde(default)]
    pub direction: Option<String>,
    /// Maximum number of neighbors
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// Query parameters for subgraph queries.
#[derive(Debug, Deserialize)]
pub struct SubgraphQuery {
    /// Maximum number of hops from center entity
    #[serde(default = "default_hops")]
    pub max_hops: usize,
}

fn default_limit() -> usize {
    50
}

fn default_hops() -> usize {
    2
}

// ============================================================================
// RESPONSE TYPES
// ============================================================================

/// Entity response for API.
#[derive(Debug, Serialize)]
pub struct EntityResponse {
    pub id: String,
    pub agent_id: String,
    pub entity_type: String,
    pub name: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub mention_count: i64,
    pub first_seen_at: String,
    pub last_seen_at: String,
}

impl From<Entity> for EntityResponse {
    fn from(entity: Entity) -> Self {
        Self {
            id: entity.id,
            agent_id: entity.agent_id,
            entity_type: entity.entity_type.as_str().to_string(),
            name: entity.name,
            properties: entity.properties,
            mention_count: entity.mention_count,
            first_seen_at: entity.first_seen_at.to_rfc3339(),
            last_seen_at: entity.last_seen_at.to_rfc3339(),
        }
    }
}

/// Relationship response for API.
#[derive(Debug, Serialize)]
pub struct RelationshipResponse {
    pub id: String,
    pub agent_id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relationship_type: String,
    pub mention_count: i64,
}

impl From<Relationship> for RelationshipResponse {
    fn from(rel: Relationship) -> Self {
        Self {
            id: rel.id,
            agent_id: rel.agent_id,
            source_entity_id: rel.source_entity_id,
            target_entity_id: rel.target_entity_id,
            relationship_type: rel.relationship_type.as_str().to_string(),
            mention_count: rel.mention_count,
        }
    }
}

/// Graph statistics response.
#[derive(Debug, Serialize)]
pub struct GraphStatsResponse {
    pub entity_count: usize,
    pub relationship_count: usize,
    pub entity_types: HashMap<String, usize>,
    pub relationship_types: HashMap<String, usize>,
    pub most_connected_entities: Vec<(String, usize)>,
}

impl From<GraphStats> for GraphStatsResponse {
    fn from(stats: GraphStats) -> Self {
        Self {
            entity_count: stats.entity_count,
            relationship_count: stats.relationship_count,
            entity_types: stats.entity_types,
            relationship_types: stats.relationship_types,
            most_connected_entities: stats.most_connected_entities,
        }
    }
}

/// Entity list response.
#[derive(Debug, Serialize)]
pub struct EntityListResponse {
    pub entities: Vec<EntityResponse>,
    pub total: usize,
}

/// Relationship list response.
#[derive(Debug, Serialize)]
pub struct RelationshipListResponse {
    pub relationships: Vec<RelationshipResponse>,
    pub total: usize,
}

/// Neighbor response.
#[derive(Debug, Serialize)]
pub struct NeighborResponse {
    pub entity_id: String,
    pub neighbors: Vec<NeighborEntry>,
}

/// Single neighbor entry.
#[derive(Debug, Serialize)]
pub struct NeighborEntry {
    pub entity: EntityResponse,
    pub relationship: RelationshipResponse,
    pub direction: String,
}

/// Subgraph response.
#[derive(Debug, Serialize)]
pub struct SubgraphResponse {
    pub entities: Vec<EntityResponse>,
    pub relationships: Vec<RelationshipResponse>,
    pub center: String,
    pub max_hops: usize,
}

impl From<Subgraph> for SubgraphResponse {
    fn from(subgraph: Subgraph) -> Self {
        Self {
            entities: subgraph
                .entities
                .into_iter()
                .map(EntityResponse::from)
                .collect(),
            relationships: subgraph
                .relationships
                .into_iter()
                .map(RelationshipResponse::from)
                .collect(),
            center: subgraph.center,
            max_hops: subgraph.max_hops,
        }
    }
}

/// Error response.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============================================================================
// HANDLERS
// ============================================================================

/// GET /api/graph/:agent_id/stats
/// Get graph statistics for an agent.
pub async fn get_graph_stats(
    Path(agent_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<GraphStatsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let graph_service = match &state.graph_service {
        Some(service) => service,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Knowledge graph service not available".to_string(),
                }),
            ));
        }
    };

    match graph_service.get_stats(&agent_id).await {
        Ok(stats) => Ok(Json(GraphStatsResponse::from(stats))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get graph stats: {}", e),
            }),
        )),
    }
}

/// GET /api/graph/:agent_id/entities
/// List entities for an agent.
pub async fn list_entities(
    Path(agent_id): Path<String>,
    Query(query): Query<EntityListQuery>,
    State(state): State<AppState>,
) -> Result<Json<EntityListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let graph_service = match &state.graph_service {
        Some(service) => service,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Knowledge graph service not available".to_string(),
                }),
            ));
        }
    };

    match graph_service
        .list_entities(
            &agent_id,
            query.entity_type.as_deref(),
            query.limit,
            query.offset,
        )
        .await
    {
        Ok(entities) => {
            let total = entities.len();
            Ok(Json(EntityListResponse {
                entities: entities.into_iter().map(EntityResponse::from).collect(),
                total,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to list entities: {}", e),
            }),
        )),
    }
}

/// GET /api/graph/:agent_id/relationships
/// List relationships for an agent.
pub async fn list_relationships(
    Path(agent_id): Path<String>,
    Query(query): Query<RelationshipListQuery>,
    State(state): State<AppState>,
) -> Result<Json<RelationshipListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let graph_service = match &state.graph_service {
        Some(service) => service,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Knowledge graph service not available".to_string(),
                }),
            ));
        }
    };

    match graph_service
        .list_relationships(
            &agent_id,
            query.relationship_type.as_deref(),
            query.limit,
            query.offset,
        )
        .await
    {
        Ok(relationships) => {
            let total = relationships.len();
            Ok(Json(RelationshipListResponse {
                relationships: relationships
                    .into_iter()
                    .map(RelationshipResponse::from)
                    .collect(),
                total,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to list relationships: {}", e),
            }),
        )),
    }
}

/// GET /api/graph/:agent_id/entities/:entity_id/neighbors
/// Get neighbors of an entity.
pub async fn get_entity_neighbors(
    Path((agent_id, entity_id)): Path<(String, String)>,
    Query(query): Query<NeighborQuery>,
    State(state): State<AppState>,
) -> Result<Json<NeighborResponse>, (StatusCode, Json<ErrorResponse>)> {
    let graph_service = match &state.graph_service {
        Some(service) => service,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Knowledge graph service not available".to_string(),
                }),
            ));
        }
    };

    // Parse direction
    let direction = match query.direction.as_deref() {
        Some("outgoing") => Direction::Outgoing,
        Some("incoming") => Direction::Incoming,
        _ => Direction::Both,
    };

    // Get neighbors through GraphService
    match graph_service
        .get_neighbors(&agent_id, &entity_id, direction, query.limit)
        .await
    {
        Ok(neighbors) => {
            let neighbor_entries: Vec<NeighborEntry> = neighbors
                .into_iter()
                .map(|n| NeighborEntry {
                    entity: EntityResponse::from(n.entity),
                    relationship: RelationshipResponse::from(n.relationship),
                    direction: match n.direction {
                        Direction::Outgoing => "outgoing".to_string(),
                        Direction::Incoming => "incoming".to_string(),
                        Direction::Both => "both".to_string(),
                    },
                })
                .collect();

            Ok(Json(NeighborResponse {
                entity_id,
                neighbors: neighbor_entries,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get neighbors: {}", e),
            }),
        )),
    }
}

/// GET /api/graph/:agent_id/entities/:entity_id/subgraph
/// Get subgraph around an entity.
pub async fn get_entity_subgraph(
    Path((agent_id, entity_id)): Path<(String, String)>,
    Query(query): Query<SubgraphQuery>,
    State(state): State<AppState>,
) -> Result<Json<SubgraphResponse>, (StatusCode, Json<ErrorResponse>)> {
    let graph_service = match &state.graph_service {
        Some(service) => service,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Knowledge graph service not available".to_string(),
                }),
            ));
        }
    };

    match graph_service
        .get_subgraph(&agent_id, &entity_id, query.max_hops)
        .await
    {
        Ok(subgraph) => Ok(Json(SubgraphResponse::from(subgraph))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get subgraph: {}", e),
            }),
        )),
    }
}

/// GET /api/graph/:agent_id/search
/// Search entities by name.
pub async fn search_entities(
    Path(agent_id): Path<String>,
    Query(query): Query<SearchQuery>,
    State(state): State<AppState>,
) -> Result<Json<EntityListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let graph_service = match &state.graph_service {
        Some(service) => service,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Knowledge graph service not available".to_string(),
                }),
            ));
        }
    };

    match graph_service
        .search_entities(&agent_id, &query.q, query.limit.unwrap_or(20))
        .await
    {
        Ok(entities) => {
            let total = entities.len();
            Ok(Json(EntityListResponse {
                entities: entities.into_iter().map(EntityResponse::from).collect(),
                total,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to search entities: {}", e),
            }),
        )),
    }
}

/// Query parameters for entity search.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search query string
    pub q: String,
    /// Maximum number of results
    pub limit: Option<usize>,
}

/// Query parameters for cross-agent entity listing.
#[derive(Debug, Deserialize)]
pub struct AllEntitiesQuery {
    /// Filter by ward/agent ID
    pub ward_id: Option<String>,
    /// Filter by entity type
    pub entity_type: Option<String>,
    /// Maximum number of results
    #[serde(default = "default_all_entities_limit")]
    pub limit: usize,
}

fn default_all_entities_limit() -> usize {
    200
}

/// Aggregate graph statistics for the Observatory health bar.
#[derive(Debug, Serialize)]
pub struct AggregateGraphStats {
    pub entities: usize,
    pub relationships: usize,
    pub facts: usize,
    pub episodes: i64,
    pub distillation: Option<DistillationStats>,
}

// ============================================================================
// DISTILLATION STATUS
// ============================================================================

/// GET /api/distillation/status
/// Get aggregate distillation statistics.
pub async fn distillation_status(
    State(state): State<AppState>,
) -> Result<Json<DistillationStats>, (StatusCode, Json<ErrorResponse>)> {
    let repo = match &state.distillation_repo {
        Some(repo) => repo,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Distillation repository not available".to_string(),
                }),
            ));
        }
    };

    match repo.get_stats() {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get distillation stats: {}", e),
            }),
        )),
    }
}

/// GET /api/distillation/undistilled
/// Returns undistilled sessions (session_id + agent_id pairs).
pub async fn undistilled_sessions(
    State(state): State<AppState>,
) -> Result<Json<Vec<UndistilledSession>>, (StatusCode, Json<ErrorResponse>)> {
    let repo = match &state.distillation_repo {
        Some(repo) => repo,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Distillation repository not available".to_string(),
                }),
            ));
        }
    };

    match repo.get_undistilled_sessions() {
        Ok(sessions) => Ok(Json(sessions)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get undistilled sessions: {}", e),
            }),
        )),
    }
}

/// Response for the trigger distillation endpoint.
#[derive(Debug, Serialize)]
pub struct TriggerDistillationResponse {
    pub session_id: String,
    pub status: String,
    pub facts_upserted: usize,
    pub error: Option<String>,
}

/// POST /api/distillation/trigger/:session_id
/// Trigger distillation for a specific session.
pub async fn trigger_distillation(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<TriggerDistillationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let distiller = match &state.distiller {
        Some(d) => d.clone(),
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Distillation service not available".to_string(),
                }),
            ));
        }
    };

    // Look up the root_agent_id for this session from the database
    let agent_id = match state.conversations.get_session_agent_id(&session_id) {
        Ok(Some(aid)) => aid,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Session '{}' not found", session_id),
                }),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to look up session: {}", e),
                }),
            ));
        }
    };

    match distiller.distill(&session_id, &agent_id).await {
        Ok(facts_upserted) => Ok(Json(TriggerDistillationResponse {
            session_id,
            status: "success".to_string(),
            facts_upserted,
            error: None,
        })),
        Err(e) => Ok(Json(TriggerDistillationResponse {
            session_id,
            status: "failed".to_string(),
            facts_upserted: 0,
            error: Some(e),
        })),
    }
}

// ============================================================================
// OBSERVATORY ENDPOINTS
// ============================================================================

/// GET /api/graph/stats
/// Aggregate graph statistics for the Observatory health bar.
pub async fn graph_stats(
    State(state): State<AppState>,
) -> Result<Json<AggregateGraphStats>, (StatusCode, Json<ErrorResponse>)> {
    // Entity + relationship counts from graph service
    let (entities, relationships) = match &state.graph_service {
        Some(service) => {
            let e = service.count_all_entities().await.unwrap_or(0);
            let r = service.count_all_relationships().await.unwrap_or(0);
            (e, r)
        }
        None => (0, 0),
    };

    // Fact count from memory repo
    let facts = match &state.memory_repo {
        Some(repo) => repo.count_all_memory_facts(None).unwrap_or(0),
        None => 0,
    };

    // Episode count from episode repo
    let episodes = match &state.episode_repo {
        Some(repo) => repo.count().unwrap_or(0),
        None => 0,
    };

    // Distillation stats
    let distillation = match &state.distillation_repo {
        Some(repo) => repo.get_stats().ok(),
        None => None,
    };

    Ok(Json(AggregateGraphStats {
        entities,
        relationships,
        facts,
        episodes,
        distillation,
    }))
}

/// GET /api/graph/all/relationships
/// Cross-agent relationship listing for the Observatory "All Agents" mode.
pub async fn all_relationships(
    Query(query): Query<AllEntitiesQuery>,
    State(state): State<AppState>,
) -> Result<Json<RelationshipListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let graph_service = match &state.graph_service {
        Some(service) => service,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Knowledge graph service not available".to_string(),
                }),
            ));
        }
    };

    match graph_service.list_all_relationships(query.limit).await {
        Ok(relationships) => {
            let total = relationships.len();
            Ok(Json(RelationshipListResponse {
                relationships: relationships
                    .into_iter()
                    .map(RelationshipResponse::from)
                    .collect(),
                total,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to list all relationships: {}", e),
            }),
        )),
    }
}

/// GET /api/graph/all/entities
/// Cross-agent entity listing for the Observatory "All Agents" mode.
pub async fn all_entities(
    Query(query): Query<AllEntitiesQuery>,
    State(state): State<AppState>,
) -> Result<Json<EntityListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let graph_service = match &state.graph_service {
        Some(service) => service,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Knowledge graph service not available".to_string(),
                }),
            ));
        }
    };

    match graph_service
        .list_all_entities(
            query.ward_id.as_deref(),
            query.entity_type.as_deref(),
            query.limit,
        )
        .await
    {
        Ok(entities) => {
            let total = entities.len();
            Ok(Json(EntityListResponse {
                entities: entities.into_iter().map(EntityResponse::from).collect(),
                total,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to list all entities: {}", e),
            }),
        )),
    }
}
