//! # HTTP Module
//!
//! RESTful HTTP API for the gateway.

mod agents;
mod conversations;
mod health;
mod providers;
mod skills;
mod tools;

use crate::config::GatewayConfig;
use crate::state::AppState;
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::info;

/// Create the HTTP router with all endpoints.
pub fn create_http_router(config: GatewayConfig, state: AppState) -> Router {
    let cors = if config.cors_enabled {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
    };

    let mut router = Router::new()
        // Health endpoints
        .route("/api/health", get(health::health_check))
        .route("/api/status", get(health::status))
        // Agent endpoints
        .route("/api/agents", get(agents::list_agents))
        .route("/api/agents", post(agents::create_agent))
        .route("/api/agents/:id", get(agents::get_agent))
        .route("/api/agents/:id", put(agents::update_agent))
        .route("/api/agents/:id", delete(agents::delete_agent))
        // Conversation endpoints
        .route("/api/conversations", get(conversations::list_conversations))
        .route("/api/conversations", post(conversations::create_conversation))
        .route("/api/conversations/:id", get(conversations::get_conversation))
        .route(
            "/api/conversations/:id",
            delete(conversations::delete_conversation),
        )
        .route(
            "/api/conversations/:id/messages",
            get(conversations::list_messages),
        )
        // Tool endpoints
        .route("/api/tools", get(tools::list_tools))
        .route("/api/tools/:name", get(tools::get_tool))
        // Skill endpoints
        .route("/api/skills", get(skills::list_skills))
        .route("/api/skills", post(skills::create_skill))
        .route("/api/skills/:id", get(skills::get_skill))
        .route("/api/skills/:id", put(skills::update_skill))
        .route("/api/skills/:id", delete(skills::delete_skill))
        // Provider endpoints
        .nest("/api/providers", providers::routes())
        // State
        .with_state(state);

    // Add static file serving for web dashboard
    if config.serve_dashboard {
        if let Some(static_dir) = &config.static_dir {
            let path = PathBuf::from(static_dir);
            if path.exists() {
                info!("Serving dashboard from: {}", static_dir);
                let index_file = path.join("index.html");
                let serve_dir = ServeDir::new(&path)
                    .not_found_service(ServeFile::new(&index_file));
                router = router.fallback_service(serve_dir);
            } else {
                tracing::warn!("Static directory not found: {}", static_dir);
            }
        }
    }

    router
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}
