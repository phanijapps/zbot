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
    let snapshot = svc.config_snapshot();
    let status_str = match svc.health() {
        Health::Ready => "ready".to_string(),
        Health::Reindexing { .. } => "reindexing".to_string(),
        Health::Pulling { .. } => "pulling".to_string(),
        Health::OllamaUnreachable => "ollama_unreachable".to_string(),
        Health::ModelMissing => "model_missing".to_string(),
        Health::Misconfigured(_) => "misconfigured".to_string(),
    };
    Json(HealthResponse {
        backend: snapshot.backend.as_str().to_string(),
        model: Some(client.model_name().to_string()),
        dim: svc.dimensions(),
        status: status_str,
        indexed_count: count_indexed_rows(&state.knowledge_db),
        needs_reindex: svc.needs_reindex(),
    })
}

/// Sum of indexed rows across the three sqlite-vec tables. Each
/// `*_index_rowids` aux table holds one row per indexed item, so this is
/// a faithful count of what's actually searchable. Returns 0 if any query
/// fails (e.g., aux tables not yet created on a fresh install).
fn count_indexed_rows(db: &gateway_database::KnowledgeDatabase) -> usize {
    const TABLES: &[&str] = &[
        "memory_facts_index_rowids",
        "kg_name_index_rowids",
        "session_episodes_index_rowids",
    ];
    db.with_connection(|conn| {
        let mut total = 0usize;
        for tbl in TABLES {
            let n: i64 = conn
                .query_row(&format!("SELECT count(*) FROM {tbl}"), [], |r| r.get(0))
                .unwrap_or(0);
            total = total.saturating_add(n as usize);
        }
        Ok(total)
    })
    .unwrap_or(0)
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
    let knowledge_db = state.knowledge_db.clone();
    // Persist the intent first so a daemon restart will honor the selection.
    svc.persist_settings(&new)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Collect progress events into a channel.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Health>();
    let tx_cb = tx.clone();
    let cb = move |h: Health| {
        let _ = tx_cb.send(h);
    };
    let svc_clone = svc.clone();
    let new_clone = new.clone();
    tokio::spawn(async move {
        // Phase 1: reconfigure (ping + pull + client swap).
        if let Err(e) = svc_clone.reconfigure(new_clone.clone(), cb).await {
            let _ = tx.send(Health::Misconfigured(e));
            return;
        }

        // Phase 2: reindex if required (dim or model changed).
        if svc_clone.needs_reindex() {
            let current_dim = svc_clone.dimensions();
            let client = svc_clone.client();
            let tx_reindex = tx.clone();
            let on_progress = move |table: &'static str, current: usize, total: usize| {
                let ev = Health::Reindexing {
                    table: table.to_string(),
                    current,
                    total,
                };
                // Also publish into service.health so pollers see it.
                let _ = tx_reindex.send(ev);
            };
            match gateway_execution::sleep::embedding_reindex::reindex_all(
                &knowledge_db,
                client,
                current_dim,
                &on_progress,
            )
            .await
            {
                Ok(_) => {
                    if let Err(e) = svc_clone.mark_indexed(current_dim) {
                        let _ = tx.send(Health::Misconfigured(format!(
                            "reindex ok but mark_indexed failed: {e}"
                        )));
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Health::Misconfigured(format!("reindex failed: {e}")));
                    return;
                }
            }
        }

        let _ = tx.send(Health::Ready);
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
