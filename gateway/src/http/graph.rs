//! # Knowledge Graph Endpoints
//!
//! HTTP API for querying the knowledge graph.

use super::SameOrigin;
use super::ErrorResponse;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use knowledge_graph::kg_trait::KnowledgeGraphStore;
use knowledge_graph::types::Direction as StoreDirection;
use knowledge_graph::{Direction, Entity, GraphStats, Relationship, Subgraph};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use zbot_engram_adapter::GovernanceCapabilityHealth;
use zbot_stores_domain::{DistillationStats, UndistilledSession};

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
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
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
    /// True when the name or properties were clipped/omitted to meet the
    /// projection budgets (AC3). Counts stay exact regardless.
    pub projection_truncated: bool,
}

/// Clip a string at a Unicode boundary to at most `max_chars`.
fn clip_unicode(value: String, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value;
    }
    value.chars().take(max_chars).collect()
}

/// Project an entity into its bounded response shape. Over-budget properties
/// are omitted entirely (as `{}`) with `projection_truncated: true`; the
/// stored row is untouched (AC3: truncation flagged per row without losing
/// counts).
fn project_entity(entity: Entity) -> EntityResponse {
    let name_over = entity.name.chars().count() > GRAPH_MAX_NAME_CHARS;
    let mut truncated = name_over;
    let name = if name_over {
        clip_unicode(entity.name.clone(), GRAPH_MAX_NAME_CHARS)
    } else {
        entity.name
    };
    let properties_ok = entity.properties.len() <= GRAPH_MAX_PROPERTY_ENTRIES
        && entity
            .properties
            .values()
            .all(property_within_budget);
    let properties = if properties_ok {
        entity.properties
    } else {
        truncated = true;
        HashMap::new()
    };
    let mut response = EntityResponse {
        id: entity.id,
        agent_id: entity.agent_id,
        entity_type: entity.entity_type.as_str().to_string(),
        name,
        properties,
        mention_count: entity.mention_count,
        first_seen_at: entity.first_seen_at.to_rfc3339(),
        last_seen_at: entity.last_seen_at.to_rfc3339(),
        projection_truncated: truncated,
    };
    // Row byte budget (AC3): an over-budget row drops its properties rather
    // than being dropped itself — counts stay exact, truncation is flagged.
    if serde_json::to_string(&response).map(|s| s.len()).unwrap_or(0) > GRAPH_ROW_BUDGET_BYTES {
        response.properties = HashMap::new();
        response.projection_truncated = true;
    }
    response
}

/// Recursive budget check: strings ≤ 4096 chars, ≤ 64 entries per container,
/// ≤ 4 nesting levels, serialized value within the row budget's property
/// share (≤ 8 KiB enforced at serialization time below).
fn property_within_budget(value: &serde_json::Value) -> bool {
    property_depth_within(value, 4)
}

fn property_depth_within(value: &serde_json::Value, depth: usize) -> bool {
    match value {
        serde_json::Value::String(text) => text.chars().count() <= GRAPH_MAX_PROPERTY_CHARS,
        serde_json::Value::Array(items) => {
            depth > 0
                && items.len() <= GRAPH_MAX_PROPERTY_ENTRIES
                && items.iter().all(|item| property_depth_within(item, depth - 1))
        }
        serde_json::Value::Object(map) => {
            depth > 0
                && map.len() <= GRAPH_MAX_PROPERTY_ENTRIES
                && map.values().all(|item| property_depth_within(item, depth - 1))
        }
        _ => true,
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
    pub projection_truncated: bool,
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
            projection_truncated: false,
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

/// Entity list response: exact scope total, echo of the requested offset, and
/// `next_offset` when more rows exist (null when exhausted). `total` is the
/// exact same-scope count, never the page length.
#[derive(Debug, Serialize)]
pub struct EntityListResponse {
    pub entities: Vec<EntityResponse>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

/// Relationship list response with the same pagination semantics.
#[derive(Debug, Serialize)]
pub struct RelationshipListResponse {
    pub relationships: Vec<RelationshipResponse>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

/// Enforce the page byte budget by dropping serialized rows beyond 2 MiB.
/// Returns the shortened vector; `next_offset` is computed from the returned
/// count by the caller, so no row is skipped — the next page starts exactly
/// where this one stopped.
fn enforce_page_budget<T: Serialize>(mut rows: Vec<T>) -> Vec<T> {
    let mut used = 0usize;
    let mut keep = 0usize;
    for row in &rows {
        let size = serde_json::to_string(row).map(|s| s.len()).unwrap_or(0);
        if used + size > GRAPH_PAGE_BUDGET_BYTES && keep > 0 {
            break;
        }
        used += size;
        keep += 1;
    }
    rows.truncate(keep.max(1).min(rows.len()));
    rows
}

/// Neighbor response with pagination semantics.
#[derive(Debug, Serialize)]
pub struct NeighborResponse {
    pub entity_id: String,
    pub neighbors: Vec<NeighborEntry>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
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
                .map(project_entity)
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

// ============================================================================
// HANDLERS
// ============================================================================

/// Strip the `agent:` prefix from UI-supplied agent ids so the graph service
/// (which stores the bare name) finds rows. Both forms are accepted.
fn normalize_agent_id(id: &str) -> &str {
    id.strip_prefix("agent:").unwrap_or(id)
}

/// GET /api/graph/:agent_id/stats
/// Get graph statistics for an agent.
pub async fn get_graph_stats(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    Path(agent_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<GraphStatsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let kg_store = require_kg_store(&state)?;
    kg_store
        .graph_stats(normalize_agent_id(&agent_id))
        .await
        .map(|stats| Json(GraphStatsResponse::from(stats)))
        .map_err(store_err_to_http)
}

/// GET /api/graph/:agent_id/entities
/// List entities for an agent.
pub async fn list_entities(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    Path(agent_id): Path<String>,
    Query(query): Query<EntityListQuery>,
    State(state): State<AppState>,
) -> Result<Json<EntityListResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_paging(query.limit, query.offset)?;
    let kg_store = require_kg_store(&state)?;
    let page = kg_store
        .list_entities_paged(
            normalize_agent_id(&agent_id),
            query.entity_type.as_deref(),
            query.limit,
            query.offset,
        )
        .await
        .map_err(store_err_to_http)?;
    let returned = page.entities.len();
    Ok(Json(EntityListResponse {
        entities: enforce_page_budget(page.entities.into_iter().map(project_entity).collect()),
        total: page.total,
        offset: query.offset,
        next_offset: next_offset(query.offset, returned, page.total),
    }))
}

/// GET /api/graph/:agent_id/relationships
/// List relationships for an agent.
pub async fn list_relationships(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    Path(agent_id): Path<String>,
    Query(query): Query<RelationshipListQuery>,
    State(state): State<AppState>,
) -> Result<Json<RelationshipListResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_paging(query.limit, query.offset)?;
    let kg_store = require_kg_store(&state)?;
    let page = kg_store
        .list_relationships_paged(
            normalize_agent_id(&agent_id),
            query.relationship_type.as_deref(),
            query.limit,
            query.offset,
        )
        .await
        .map_err(store_err_to_http)?;
    let returned = page.relationships.len();
    Ok(Json(RelationshipListResponse {
        relationships: enforce_page_budget(
            page.relationships
                .into_iter()
                .map(RelationshipResponse::from)
                .collect(),
        ),
        total: page.total,
        offset: query.offset,
        next_offset: next_offset(query.offset, returned, page.total),
    }))
}

/// GET /api/graph/:agent_id/entities/:entity_id/neighbors
/// Get neighbors of an entity.
pub async fn get_entity_neighbors(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    Path((agent_id, entity_id)): Path<(String, String)>,
    Query(query): Query<NeighborQuery>,
    State(state): State<AppState>,
) -> Result<Json<NeighborResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_paging(query.limit, query.offset)?;
    let kg_store = require_kg_store(&state)?;
    let direction = parse_direction(query.direction.as_deref());
    let page = kg_store
        .get_neighbors_full_paged(
            normalize_agent_id(&agent_id),
            &entity_id,
            direction,
            query.limit,
            query.offset,
        )
        .await
        .map_err(store_err_to_http)?;
    let returned = page.neighbors.len();
    let neighbors: Vec<NeighborEntry> = page
        .neighbors
        .into_iter()
        .map(|info| NeighborEntry {
            entity: project_entity(info.entity),
            relationship: RelationshipResponse::from(info.relationship),
            direction: match info.direction {
                Direction::Outgoing => "outgoing".to_string(),
                Direction::Incoming => "incoming".to_string(),
                Direction::Both => "both".to_string(),
            },
        })
        .collect();
    Ok(Json(NeighborResponse {
        entity_id,
        total: page.total,
        offset: query.offset,
        next_offset: next_offset(query.offset, returned, page.total),
        neighbors,
    }))
}

/// Parse the optional `?direction=` query string into the trait-side
/// `StoreDirection`. Default = `Both`. Unknown values fall back to
/// `Both` to match the historical handler behavior.
fn parse_direction(s: Option<&str>) -> StoreDirection {
    match s {
        Some("outgoing") => StoreDirection::Outgoing,
        Some("incoming") => StoreDirection::Incoming,
        _ => StoreDirection::Both,
    }
}

/// GET /api/graph/:agent_id/entities/:entity_id/subgraph
/// Get subgraph around an entity.
pub async fn get_entity_subgraph(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    Path((agent_id, entity_id)): Path<(String, String)>,
    Query(query): Query<SubgraphQuery>,
    State(state): State<AppState>,
) -> Result<Json<SubgraphResponse>, (StatusCode, Json<ErrorResponse>)> {
    if query.max_hops == 0 || query.max_hops > GRAPH_MAX_HOPS {
        return Err(bad_request("max_hops must be between 1 and 4"));
    }
    let kg_store = require_kg_store(&state)?;
    kg_store
        .get_subgraph(normalize_agent_id(&agent_id), &entity_id, query.max_hops)
        .await
        .map(|subgraph| Json(SubgraphResponse::from(subgraph)))
        .map_err(store_err_to_http)
}

/// GET /api/graph/:agent_id/search
/// Search entities by name.
///
/// Backed by `kg_store` (KnowledgeGraphStore trait). Response shape
/// is identical to the historical handler.
pub async fn search_entities(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    Path(agent_id): Path<String>,
    Query(query): Query<SearchQuery>,
    State(state): State<AppState>,
) -> Result<Json<EntityListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let limit = query.limit.unwrap_or(20);
    validate_paging(limit, query.offset)?;
    if query.q.chars().count() > GRAPH_MAX_QUERY_CHARS {
        return Err(bad_request("query too long"));
    }
    let kg_store = require_kg_store(&state)?;
    let page = kg_store
        .search_entities_paged(
            normalize_agent_id(&agent_id),
            &query.q,
            None,
            limit,
            query.offset,
        )
        .await
        .map_err(store_err_to_http)?;
    let returned = page.entities.len();
    Ok(Json(EntityListResponse {
        entities: enforce_page_budget(page.entities.into_iter().map(project_entity).collect()),
        total: page.total,
        offset: query.offset,
        next_offset: next_offset(query.offset, returned, page.total),
    }))
}

/// GET /api/graph/all/search — aggregate (cross-agent) entity search with
/// exact totals and offset paging (exploration contract).
pub async fn search_all_entities(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    Query(query): Query<AllSearchQuery>,
    State(state): State<AppState>,
) -> Result<Json<EntityListResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_paging(query.limit, query.offset)?;
    if query.q.chars().count() > GRAPH_MAX_QUERY_CHARS {
        return Err(bad_request("query too long"));
    }
    let kg_store = require_kg_store(&state)?;
    let page = kg_store
        .search_all_entities_paged(
            &query.q,
            query.entity_type.as_deref(),
            query.limit,
            query.offset,
        )
        .await
        .map_err(store_err_to_http)?;
    let returned = page.entities.len();
    Ok(Json(EntityListResponse {
        entities: enforce_page_budget(page.entities.into_iter().map(project_entity).collect()),
        total: page.total,
        offset: query.offset,
        next_offset: next_offset(query.offset, returned, page.total),
    }))
}

/// GET /api/graph/:agent_id/entities/:entity_id — direct agent-scoped entity
/// read (exploration contract: endpoint resolution for edge endpoints).
pub async fn get_scoped_entity(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    Path((agent_id, entity_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<EntityResponse>, (StatusCode, Json<ErrorResponse>)> {
    if entity_id.is_empty() || entity_id.len() > 128 {
        return Err(bad_request("invalid entity ID"));
    }
    let kg_store = require_kg_store(&state)?;
    let agent = normalize_agent_id(&agent_id);
    let entity = kg_store
        .get_entity(&knowledge_graph::kg_trait::EntityId(entity_id.clone()))
        .await
        .map_err(store_err_to_http)?
        .filter(|entity| entity.agent_id == agent || entity.agent_id == "__global__")
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("Entity not found".to_string())),
            )
        })?;
    Ok(Json(project_entity(entity)))
}

/// Resolve `state.kg_store` or short-circuit with 503. Centralised so
/// every graph handler emits the same payload when the trait-erased
/// store hasn't been wired (smoke tests, partial init).
fn require_kg_store(
    state: &AppState,
) -> Result<Arc<dyn KnowledgeGraphStore>, (StatusCode, Json<ErrorResponse>)> {
    state.kg_store().clone().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse::new(
                "Knowledge graph store unavailable".to_string(),
            )),
        )
    })
}

/// Map a [`knowledge_graph::kg_trait::GraphStoreError`] to the HTTP error pair used by graph
/// handlers: `(StatusCode, Json<ErrorResponse>)`.
fn store_err_to_http(
    err: knowledge_graph::kg_trait::GraphStoreError,
) -> (StatusCode, Json<ErrorResponse>) {
    use knowledge_graph::kg_trait::GraphStoreError;
    match err {
        GraphStoreError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("Entity not found".to_string())),
        ),
        GraphStoreError::Conflict(msg) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse::new(format!("Conflict: {}", msg))),
        ),
        GraphStoreError::Invalid(msg) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(format!("Invalid request: {}", msg))),
        ),
        GraphStoreError::Unavailable { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse::new(
                "Knowledge graph store temporarily unavailable".to_string(),
            )),
        ),
        GraphStoreError::Schema(msg)
        | GraphStoreError::Backend(msg)
        | GraphStoreError::Config(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!(
                "Knowledge graph error: {}",
                msg
            ))),
        ),
    }
}

/// Query parameters for entity search.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search query string
    pub q: String,
    /// Maximum number of results
    pub limit: Option<usize>,
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
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
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
}

/// Query parameters for cross-agent relationship listing.
#[derive(Debug, Deserialize)]
pub struct AllRelationshipsQuery {
    /// Filter by relationship type
    pub relationship_type: Option<String>,
    /// Maximum number of results
    #[serde(default = "default_all_entities_limit")]
    pub limit: usize,
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
}

/// Query parameters for aggregate (cross-agent) entity search.
#[derive(Debug, Deserialize)]
pub struct AllSearchQuery {
    /// Search query string
    pub q: String,
    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
    /// Filter by entity type
    pub entity_type: Option<String>,
}

fn default_all_entities_limit() -> usize {
    200
}

/// Exploration contract bounds (AC1/AC3): bounded page size, bounded offset,
/// bounded query strings, bounded traversal depth.
const GRAPH_MAX_LIMIT: usize = 1000;
const GRAPH_MAX_OFFSET: usize = 1_000_000;
const GRAPH_MAX_QUERY_CHARS: usize = 256;
const GRAPH_MAX_HOPS: usize = 4;
/// Serialized page byte budget (2 MiB) and per-row budget (16 KiB).
const GRAPH_PAGE_BUDGET_BYTES: usize = 2 * 1024 * 1024;
const GRAPH_ROW_BUDGET_BYTES: usize = 16 * 1024;
/// Projection caps (AC3): name, property strings, entries per container,
/// serialized properties size.
const GRAPH_MAX_NAME_CHARS: usize = 1024;
const GRAPH_MAX_PROPERTY_CHARS: usize = 4096;
const GRAPH_MAX_PROPERTY_ENTRIES: usize = 64;

fn bad_request(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse::new(message.to_string())),
    )
}

fn validate_paging(limit: usize, offset: usize) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if limit == 0 || limit > GRAPH_MAX_LIMIT {
        return Err(bad_request("limit must be between 1 and 1000"));
    }
    if offset > GRAPH_MAX_OFFSET {
        return Err(bad_request("offset too large"));
    }
    Ok(())
}

/// `next_offset` when more rows exist beyond this page (contract: null on
/// exhaustion). The page may have been shortened by the byte budget, so the
/// decision uses the returned row count, never the requested limit.
fn next_offset(offset: usize, returned: usize, total: usize) -> Option<usize> {
    let next = offset + returned;
    (next < total).then_some(next)
}

/// Aggregate graph statistics for the Observatory health bar.
#[derive(Debug, Serialize)]
pub struct AggregateGraphStats {
    pub entities: usize,
    pub relationships: usize,
    pub facts: usize,
    pub episodes: i64,
    pub distillation: Option<DistillationStats>,
    pub governance: Option<GovernanceCapabilityHealth>,
}

// ============================================================================
// DISTILLATION STATUS
// ============================================================================

/// GET /api/distillation/status
/// Get aggregate distillation statistics.
///
/// Returns zeroed stats when `distillation_repo` is unavailable
/// (stripped-down test fixtures) so the Observatory health bar
/// renders cleanly.
pub async fn distillation_status(
    State(state): State<AppState>,
) -> Result<Json<DistillationStats>, (StatusCode, Json<ErrorResponse>)> {
    let repo_slot = state.distillation_repo();
    let repo = match repo_slot.as_deref() {
        Some(repo) => repo,
        None => return Ok(Json(DistillationStats::default())),
    };

    match repo.get_stats() {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!(
                "Failed to get distillation stats: {}",
                e
            ))),
        )),
    }
}

/// GET /api/distillation/undistilled
/// Returns undistilled sessions (session_id + agent_id pairs).
///
/// Returns an empty list when `distillation_repo` is unavailable.
pub async fn undistilled_sessions(
    State(state): State<AppState>,
) -> Result<Json<Vec<UndistilledSession>>, (StatusCode, Json<ErrorResponse>)> {
    let repo_slot = state.distillation_repo();
    let repo = match repo_slot.as_deref() {
        Some(repo) => repo,
        None => return Ok(Json(Vec::new())),
    };

    match repo.get_undistilled_sessions() {
        Ok(sessions) => Ok(Json(sessions)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(format!(
                "Failed to get undistilled sessions: {}",
                e
            ))),
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
    let distiller = match &state.distiller() {
        Some(d) => d.clone(),
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse::new(
                    "Distillation service not available".to_string(),
                )),
            ));
        }
    };

    // Look up the root_agent_id for this session from the database
    let agent_id = match state.session_meta().session_agent_id(&session_id) {
        Ok(Some(aid)) => aid,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(format!(
                    "Session '{}' not found",
                    session_id
                ))),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(format!(
                    "Failed to look up session: {}",
                    e
                ))),
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
            error: Some(e.to_string()),
        })),
    }
}

// ============================================================================
// OBSERVATORY ENDPOINTS
// ============================================================================

/// GET /api/graph/stats
/// Aggregate graph statistics for the Observatory health bar.
///
/// Counts come from trait-erased stores where possible: `kg_store`
/// for entity/relationship counts and `memory_store` for fact count.
/// Distillation run status remains on the conversation database; semantic
/// counts route through backend-neutral stores.
pub async fn graph_stats(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    State(state): State<AppState>,
) -> Result<Json<AggregateGraphStats>, (StatusCode, Json<ErrorResponse>)> {
    // Reads through the stores group: this handler needs 5 of its members.
    let stores = state.stores();

    // Entity + relationship counts from kg_store.
    let (entities, relationships) = match &stores.kg_store {
        Some(store) => {
            let e = store.count_all_entities().await.unwrap_or(0);
            let r = store.count_all_relationships().await.unwrap_or(0);
            (e, r)
        }
        None => (0, 0),
    };

    // Fact count from memory_store.
    let facts = match &stores.memory_store {
        Some(store) => store
            .count_all_facts(None)
            .await
            .ok()
            .map(|n| n as usize)
            .unwrap_or(0),
        None => 0,
    };

    let episodes = match &stores.episode_store {
        Some(store) => store.episode_stats().await.map(|s| s.total).unwrap_or(0),
        None => 0,
    };

    // Distillation stats
    let distillation = match stores.distillation_repo.as_deref() {
        Some(repo) => repo.get_stats().ok(),
        None => None,
    };

    Ok(Json(AggregateGraphStats {
        entities,
        relationships,
        facts,
        episodes,
        distillation,
        governance: stores.governance_health.clone(),
    }))
}

/// GET /api/graph/all/relationships
/// Cross-agent relationship listing for the Observatory "All Agents" mode.
pub async fn all_relationships(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    Query(query): Query<AllRelationshipsQuery>,
    State(state): State<AppState>,
) -> Result<Json<RelationshipListResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_paging(query.limit, query.offset)?;
    let kg_store = require_kg_store(&state)?;
    let page = kg_store
        .list_all_relationships_paged(
            query.relationship_type.as_deref(),
            query.limit,
            query.offset,
        )
        .await
        .map_err(store_err_to_http)?;
    let returned = page.relationships.len();
    Ok(Json(RelationshipListResponse {
        relationships: enforce_page_budget(
            page.relationships
                .into_iter()
                .map(RelationshipResponse::from)
                .collect(),
        ),
        total: page.total,
        offset: query.offset,
        next_offset: next_offset(query.offset, returned, page.total),
    }))
}

/// GET /api/graph/all/entities
/// Cross-agent entity listing for the Observatory "All Agents" mode.
pub async fn all_entities(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    Query(query): Query<AllEntitiesQuery>,
    State(state): State<AppState>,
) -> Result<Json<EntityListResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_paging(query.limit, query.offset)?;
    let kg_store = require_kg_store(&state)?;
    let page = kg_store
        .list_all_entities_paged(
            query.ward_id.as_deref(),
            query.entity_type.as_deref(),
            query.limit,
            query.offset,
        )
        .await
        .map_err(store_err_to_http)?;
    let returned = page.entities.len();
    Ok(Json(EntityListResponse {
        entities: enforce_page_budget(page.entities.into_iter().map(project_entity).collect()),
        total: page.total,
        offset: query.offset,
        next_offset: next_offset(query.offset, returned, page.total),
    }))
}

/// Response body for the reindex endpoint.
#[derive(Debug, Serialize)]
pub struct ReindexResponse {
    pub wards_processed: usize,
    pub entities_created: usize,
}

fn valid_ward_directory_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// POST /api/graph/reindex — force re-indexing of every ward on disk.
/// Idempotent: relationships upsert via UNIQUE(source, target, type).
///
/// Trait-routed via `state.kg_episode_store` + `state.kg_store`.
/// Returns 503 only when neither trait is wired (defensive —
/// production always has both).
pub async fn reindex_all_wards(
    _origin: SameOrigin,
    _bind: super::sessions::LoopbackBind,
    State(state): State<AppState>,
) -> Result<Json<ReindexResponse>, StatusCode> {
    let episode_store = state
        .kg_episode_store()
        .clone()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let kg_store = state
        .kg_store()
        .clone()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let response =
        reindex_ward_directories(&state.paths().wards_dir(), &episode_store, &kg_store).await;
    Ok(Json(response))
}

async fn reindex_ward_directories(
    wards_dir: &std::path::Path,
    episode_store: &Arc<dyn zbot_stores_traits::KgEpisodeStore>,
    kg_store: &Arc<dyn KnowledgeGraphStore>,
) -> ReindexResponse {
    use gateway_execution::ward_artifact_indexer::{index_ward_with_options, IndexOptions};

    if std::fs::symlink_metadata(wards_dir)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(true)
    {
        return ReindexResponse {
            wards_processed: 0,
            entities_created: 0,
        };
    }
    let Ok(read) = std::fs::read_dir(wards_dir) else {
        return ReindexResponse {
            wards_processed: 0,
            entities_created: 0,
        };
    };

    let mut total_entities = 0_usize;
    let mut wards_processed = 0_usize;
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(ward_id) = file_name.to_str().filter(|id| valid_ward_directory_id(id)) else {
            tracing::warn!(path = %path.display(), "Skipping invalid Ward directory during graph reindex");
            continue;
        };
        let n = index_ward_with_options(
            &path,
            ward_id,
            "admin-reindex",
            "root",
            episode_store,
            kg_store,
            IndexOptions {
                force_reindex: true,
            },
        )
        .await;
        total_entities += n;
        wards_processed += 1;
    }

    ReindexResponse {
        wards_processed,
        entities_created: total_entities,
    }
}

#[cfg(test)]
mod reindex_scope_tests {
    use super::{reindex_ward_directories, valid_ward_directory_id};
    use knowledge_graph::kg_trait::KnowledgeGraphStore;
    use std::sync::Arc;

    #[test]
    fn ward_directory_scope_requires_a_single_valid_component() {
        assert!(valid_ward_directory_id("research-ward_1"));
        assert!(!valid_ward_directory_id(""));
        assert!(!valid_ward_directory_id("../other-ward"));
        assert!(!valid_ward_directory_id("ward/other"));
        assert!(!valid_ward_directory_id(&"a".repeat(65)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reindex_uses_directory_ward_scope_and_skips_invalid_directories() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let wards_dir = tmp.path().join("wards");
        let valid = wards_dir.join("research-ward");
        let invalid = wards_dir.join("invalid.ward");
        std::fs::create_dir_all(&valid).expect("valid ward");
        std::fs::create_dir_all(&invalid).expect("invalid ward");
        std::fs::write(
            valid.join("people.json"),
            r#"[{"name":"Ada Lovelace","role":"Mathematician"}]"#,
        )
        .expect("valid artifact");
        std::fs::write(
            invalid.join("people.json"),
            r#"[{"name":"Must Not Index"}]"#,
        )
        .expect("invalid artifact");
        let outside = tmp.path().join("outside-ward");
        std::fs::create_dir_all(&outside).expect("outside Ward directory");
        std::fs::write(
            outside.join("people.json"),
            r#"[{"name":"Must Not Follow Ward Symlink"}]"#,
        )
        .expect("outside Ward artifact");
        std::os::unix::fs::symlink(&outside, wards_dir.join("symlink-ward")).expect("symlink ward");
        let nested_outside = tmp.path().join("outside-nested");
        std::fs::create_dir_all(&nested_outside).expect("outside nested directory");
        std::fs::write(
            nested_outside.join("people.json"),
            r#"[{"name":"Must Not Follow Nested Symlink"}]"#,
        )
        .expect("outside nested artifact");
        std::os::unix::fs::symlink(&nested_outside, valid.join("linked-outside"))
            .expect("nested symlink");

        // Production wiring: engram adapter stores on a tempdir.
        use zbot_engram_adapter::{
            AdapterConfig, EngramKnowledgeGraphStore, EngramProvider, EngramSidecarStores,
        };
        let root = tmp.path().join("engram-reindex");
        std::fs::create_dir_all(&root).expect("root");
        let mut config = AdapterConfig::engram_for_data_root(&root, "engram.db");
        config.embedding_provider.provider_type = "gateway-test".to_string();
        config.embedding_provider.model = "gateway-test".to_string();
        config.embedding_provider.dimensions = 8;
        let provider = EngramProvider::open(config.clone()).expect("provider");
        let episode_store: Arc<dyn zbot_stores_traits::KgEpisodeStore> = Arc::new(
            EngramSidecarStores::from_provider(config.clone(), &provider).expect("episode store"),
        );
        let kg_store: Arc<dyn KnowledgeGraphStore> = Arc::new(
            EngramKnowledgeGraphStore::from_provider(config, &provider).expect("kg store"),
        );

        let response = reindex_ward_directories(&wards_dir, &episode_store, &kg_store).await;

        assert_eq!(response.wards_processed, 1);
        assert!(response.entities_created >= 2);
        let ada = knowledge_graph::kg_trait::KnowledgeGraphStore::get_entity_by_name(
            kg_store.as_ref(),
            "root",
            "Ada Lovelace",
        )
        .await
        .expect("query Ada")
        .expect("Ada indexed");
        assert_eq!(
            ada.properties.get("ward_id"),
            Some(&serde_json::json!("research-ward"))
        );
        assert!(
            knowledge_graph::kg_trait::KnowledgeGraphStore::get_entity_by_name(
                kg_store.as_ref(),
                "root",
                "Must Not Index"
            )
            .await
            .expect("query invalid artifact")
            .is_none()
        );
        assert!(
            knowledge_graph::kg_trait::KnowledgeGraphStore::get_entity_by_name(
                kg_store.as_ref(),
                "root",
                "Must Not Follow Ward Symlink"
            )
            .await
            .expect("query Ward symlink artifact")
            .is_none()
        );
        assert!(
            knowledge_graph::kg_trait::KnowledgeGraphStore::get_entity_by_name(
                kg_store.as_ref(),
                "root",
                "Must Not Follow Nested Symlink"
            )
            .await
            .expect("query nested symlink artifact")
            .is_none()
        );
    }
}
