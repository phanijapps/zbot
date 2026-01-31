// ============================================================================
// PROVIDERS HTTP ENDPOINTS
// REST API for LLM provider management
// ============================================================================

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};

use crate::services::providers::Provider;
use crate::state::AppState;

// ============================================================================
// Routes
// ============================================================================

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_providers).post(create_provider))
        .route(
            "/:id",
            get(get_provider).put(update_provider).delete(delete_provider),
        )
        .route("/:id/test", post(test_provider))
        .route("/:id/default", post(set_default_provider))
        .route("/test", post(test_provider_inline))
}

// ============================================================================
// Handlers
// ============================================================================

/// List all providers
async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    match state.provider_service.list() {
        Ok(providers) => Json(providers).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

/// Get a single provider
async fn get_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.provider_service.get(&id) {
        Ok(provider) => Json(provider).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e).into_response(),
    }
}

/// Create a new provider
async fn create_provider(
    State(state): State<AppState>,
    Json(provider): Json<Provider>,
) -> impl IntoResponse {
    match state.provider_service.create(provider) {
        Ok(created) => (StatusCode::CREATED, Json(created)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

/// Update an existing provider
async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(provider): Json<Provider>,
) -> impl IntoResponse {
    match state.provider_service.update(&id, provider) {
        Ok(updated) => Json(updated).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e).into_response(),
    }
}

/// Delete a provider
async fn delete_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.provider_service.delete(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e).into_response(),
    }
}

/// Set a provider as the default
async fn set_default_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.provider_service.set_default(&id) {
        Ok(provider) => Json(provider).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e).into_response(),
    }
}

/// Test a provider connection (by ID)
async fn test_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.provider_service.get(&id) {
        Ok(provider) => {
            let result = state.provider_service.test(&provider).await;
            Json(result).into_response()
        }
        Err(e) => (StatusCode::NOT_FOUND, e).into_response(),
    }
}

/// Test a provider connection (inline, without saving)
async fn test_provider_inline(
    State(state): State<AppState>,
    Json(provider): Json<Provider>,
) -> impl IntoResponse {
    let result = state.provider_service.test(&provider).await;
    Json(result).into_response()
}
