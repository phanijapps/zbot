//! # Connector HTTP Endpoints
//!
//! REST API for managing external connectors.

use crate::connectors::{CreateConnectorRequest, UpdateConnectorRequest};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use tracing::{error, info};

/// Error response for connector operations.
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    code: String,
}

impl ErrorResponse {
    fn not_found(id: &str) -> Self {
        Self {
            error: format!("Connector not found: {}", id),
            code: "CONNECTOR_NOT_FOUND".to_string(),
        }
    }

    fn already_exists(id: &str) -> Self {
        Self {
            error: format!("Connector already exists: {}", id),
            code: "CONNECTOR_EXISTS".to_string(),
        }
    }

    fn invalid_id(msg: &str) -> Self {
        Self {
            error: msg.to_string(),
            code: "INVALID_ID".to_string(),
        }
    }

    fn internal(msg: &str) -> Self {
        Self {
            error: msg.to_string(),
            code: "INTERNAL_ERROR".to_string(),
        }
    }
}

/// GET /api/connectors - List all connectors.
pub async fn list_connectors(State(state): State<AppState>) -> impl IntoResponse {
    match state.connector_registry.list().await {
        Ok(connectors) => Json(connectors).into_response(),
        Err(e) => {
            error!(error = %e, "Failed to list connectors");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::internal(&e.to_string())),
            )
                .into_response()
        }
    }
}

/// POST /api/connectors - Create a new connector.
pub async fn create_connector(
    State(state): State<AppState>,
    Json(request): Json<CreateConnectorRequest>,
) -> impl IntoResponse {
    info!(connector_id = %request.id, "Creating connector");

    match state.connector_registry.create(request).await {
        Ok(connector) => (StatusCode::CREATED, Json(connector)).into_response(),
        Err(e) => {
            use crate::connectors::ConnectorServiceError;
            match &e {
                ConnectorServiceError::AlreadyExists(id) => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse::already_exists(id)),
                )
                    .into_response(),
                ConnectorServiceError::InvalidId(msg) => {
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse::invalid_id(msg))).into_response()
                }
                _ => {
                    error!(error = %e, "Failed to create connector");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::internal(&e.to_string())),
                    )
                        .into_response()
                }
            }
        }
    }
}

/// GET /api/connectors/:id - Get a connector by ID.
pub async fn get_connector(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.connector_registry.get(&id).await {
        Ok(connector) => Json(connector).into_response(),
        Err(e) => {
            use crate::connectors::ConnectorServiceError;
            match &e {
                ConnectorServiceError::NotFound(id) => {
                    (StatusCode::NOT_FOUND, Json(ErrorResponse::not_found(id))).into_response()
                }
                _ => {
                    error!(error = %e, "Failed to get connector");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::internal(&e.to_string())),
                    )
                        .into_response()
                }
            }
        }
    }
}

/// PUT /api/connectors/:id - Update a connector.
pub async fn update_connector(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateConnectorRequest>,
) -> impl IntoResponse {
    info!(connector_id = %id, "Updating connector");

    match state.connector_registry.update(&id, request).await {
        Ok(connector) => Json(connector).into_response(),
        Err(e) => {
            use crate::connectors::ConnectorServiceError;
            match &e {
                ConnectorServiceError::NotFound(id) => {
                    (StatusCode::NOT_FOUND, Json(ErrorResponse::not_found(id))).into_response()
                }
                _ => {
                    error!(error = %e, "Failed to update connector");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::internal(&e.to_string())),
                    )
                        .into_response()
                }
            }
        }
    }
}

/// DELETE /api/connectors/:id - Delete a connector.
pub async fn delete_connector(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    info!(connector_id = %id, "Deleting connector");

    match state.connector_registry.delete(&id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            use crate::connectors::ConnectorServiceError;
            match &e {
                ConnectorServiceError::NotFound(id) => {
                    (StatusCode::NOT_FOUND, Json(ErrorResponse::not_found(id))).into_response()
                }
                _ => {
                    error!(error = %e, "Failed to delete connector");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::internal(&e.to_string())),
                    )
                        .into_response()
                }
            }
        }
    }
}

/// GET /api/connectors/:id/metadata - Get connector capabilities and metadata.
pub async fn get_connector_metadata(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.connector_registry.get(&id).await {
        Ok(connector) => Json(connector.metadata).into_response(),
        Err(e) => {
            use crate::connectors::ConnectorServiceError;
            match &e {
                ConnectorServiceError::NotFound(id) => {
                    (StatusCode::NOT_FOUND, Json(ErrorResponse::not_found(id))).into_response()
                }
                _ => {
                    error!(error = %e, "Failed to get connector metadata");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::internal(&e.to_string())),
                    )
                        .into_response()
                }
            }
        }
    }
}

/// POST /api/connectors/:id/test - Test connector connectivity.
pub async fn test_connector(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    info!(connector_id = %id, "Testing connector connectivity");

    match state.connector_registry.test(&id).await {
        Ok(result) => {
            let status = if result.success {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            (status, Json(result)).into_response()
        }
        Err(e) => {
            use crate::connectors::ConnectorServiceError;
            match &e {
                ConnectorServiceError::NotFound(id) => {
                    (StatusCode::NOT_FOUND, Json(ErrorResponse::not_found(id))).into_response()
                }
                _ => {
                    error!(error = %e, "Failed to test connector");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::internal(&e.to_string())),
                    )
                        .into_response()
                }
            }
        }
    }
}

/// POST /api/connectors/:id/enable - Enable a connector.
pub async fn enable_connector(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    info!(connector_id = %id, "Enabling connector");

    match state
        .connector_registry
        .update(
            &id,
            UpdateConnectorRequest {
                enabled: Some(true),
                ..Default::default()
            },
        )
        .await
    {
        Ok(connector) => Json(connector).into_response(),
        Err(e) => {
            use crate::connectors::ConnectorServiceError;
            match &e {
                ConnectorServiceError::NotFound(id) => {
                    (StatusCode::NOT_FOUND, Json(ErrorResponse::not_found(id))).into_response()
                }
                _ => {
                    error!(error = %e, "Failed to enable connector");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::internal(&e.to_string())),
                    )
                        .into_response()
                }
            }
        }
    }
}

/// POST /api/connectors/:id/disable - Disable a connector.
pub async fn disable_connector(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    info!(connector_id = %id, "Disabling connector");

    match state
        .connector_registry
        .update(
            &id,
            UpdateConnectorRequest {
                enabled: Some(false),
                ..Default::default()
            },
        )
        .await
    {
        Ok(connector) => Json(connector).into_response(),
        Err(e) => {
            use crate::connectors::ConnectorServiceError;
            match &e {
                ConnectorServiceError::NotFound(id) => {
                    (StatusCode::NOT_FOUND, Json(ErrorResponse::not_found(id))).into_response()
                }
                _ => {
                    error!(error = %e, "Failed to disable connector");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::internal(&e.to_string())),
                    )
                        .into_response()
                }
            }
        }
    }
}
