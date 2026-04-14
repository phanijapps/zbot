//! # Embedding Endpoints
//!
//! Phase 1 HTTP surface for the embedding backend selection feature.
//!
//! Endpoints:
//! - `GET  /api/embeddings/health` — current backend + dim + status
//! - `GET  /api/embeddings/models` — curated Ollama dropdown entries
//! - `POST /api/embeddings/configure` — apply a new [`EmbeddingConfig`];
//!   responds with an SSE stream of [`Health`] events terminating in
//!   `ready` or `error`.

use std::convert::Infallible;

use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::stream::Stream;
use gateway_services::{CuratedModel, EmbeddingConfig, Health, CURATED_MODELS};
use serde::Serialize;

// ============================================================================
// GET /api/embeddings/health
// ============================================================================

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub backend: String,
    pub model: Option<String>,
    pub dim: usize,
    pub status: String,
    pub indexed_count: usize,
    pub needs_reindex: bool,
}

pub async fn get_health(State(state): State<AppState>) -> Json<HealthResponse> {
    let svc = &state.embedding_service;
    let client = svc.client();
    let status_str = match svc.health() {
        Health::Ready => "ready".to_string(),
        Health::Reindexing { .. } => "reindexing".to_string(),
        Health::Pulling { .. } => "pulling".to_string(),
        Health::OllamaUnreachable => "ollama_unreachable".to_string(),
        Health::ModelMissing => "model_missing".to_string(),
        Health::Misconfigured(_) => "misconfigured".to_string(),
    };
    Json(HealthResponse {
        backend: client.model_name().to_string(),
        model: Some(client.model_name().to_string()),
        dim: svc.dimensions(),
        status: status_str,
        indexed_count: 0,
        needs_reindex: svc.needs_reindex(),
    })
}

// ============================================================================
// GET /api/embeddings/models
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ModelEntry {
    pub tag: &'static str,
    pub dim: usize,
    pub size_mb: u32,
    pub mteb: u32,
}

impl From<&CuratedModel> for ModelEntry {
    fn from(m: &CuratedModel) -> Self {
        Self {
            tag: m.tag,
            dim: m.dim,
            size_mb: m.size_mb,
            mteb: m.mteb,
        }
    }
}

pub async fn list_models() -> Json<Vec<ModelEntry>> {
    Json(CURATED_MODELS.iter().map(ModelEntry::from).collect())
}

// ============================================================================
// POST /api/embeddings/configure — SSE stream
// ============================================================================

pub async fn configure(
    State(state): State<AppState>,
    Json(new): Json<EmbeddingConfig>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let svc = state.embedding_service.clone();
    // Persist the intent first so a daemon restart will honor the selection.
    svc.persist_settings(&new)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Collect progress events into a channel.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Health>();
    let tx2 = tx.clone();
    let cb = move |h: Health| {
        let _ = tx2.send(h);
    };
    let svc_clone = svc.clone();
    let new_clone = new.clone();
    tokio::spawn(async move {
        let res = svc_clone.reconfigure(new_clone, cb).await;
        match res {
            Ok(()) => {
                let _ = tx.send(Health::Ready);
            }
            Err(e) => {
                let _ = tx.send(Health::Misconfigured(e));
            }
        }
    });

    let s = async_stream::stream! {
        while let Some(h) = rx.recv().await {
            let (ev_name, terminal) = match &h {
                Health::Ready => ("ready", true),
                Health::Misconfigured(_) => ("error", true),
                Health::Reindexing { .. } => ("reindexing", false),
                Health::Pulling { .. } => ("pulling", false),
                Health::OllamaUnreachable => ("error", true),
                Health::ModelMissing => ("error", true),
            };
            let payload = serde_json::to_string(&h).unwrap_or_else(|_| "{}".into());
            let ev = Event::default().event(ev_name).data(payload);
            yield Ok::<_, Infallible>(ev);
            if terminal {
                break;
            }
        }
    };
    Ok(Sse::new(s).keep_alive(KeepAlive::default()))
}
