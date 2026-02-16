//! # HTTP Module
//!
//! RESTful HTTP API for the gateway.

mod agents;
mod bridge;
mod connectors;
mod conversations;
mod cron;
mod events;
mod gateway_bus;
mod health;
mod memory;
mod mcps;
mod openapi;
mod providers;
mod settings;
mod skills;
mod tools;
mod webhooks;

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
        // OpenAPI documentation
        .route("/api/openapi.yaml", get(openapi::openapi_yaml))
        .route("/api/openapi.json", get(openapi::openapi_json))
        .route("/api/docs", get(openapi::swagger_ui))
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
        // MCP endpoints
        .route("/api/mcps", get(mcps::list_mcps))
        .route("/api/mcps", post(mcps::create_mcp))
        .route("/api/mcps/:id", get(mcps::get_mcp))
        .route("/api/mcps/:id", put(mcps::update_mcp))
        .route("/api/mcps/:id", delete(mcps::delete_mcp))
        .route("/api/mcps/:id/test", post(mcps::test_mcp))
        // Connector endpoints
        .route("/api/connectors", get(connectors::list_connectors))
        .route("/api/connectors", post(connectors::create_connector))
        .route("/api/connectors/:id", get(connectors::get_connector))
        .route("/api/connectors/:id", put(connectors::update_connector))
        .route("/api/connectors/:id", delete(connectors::delete_connector))
        .route("/api/connectors/:id/metadata", get(connectors::get_connector_metadata))
        .route("/api/connectors/:id/test", post(connectors::test_connector))
        .route("/api/connectors/:id/enable", post(connectors::enable_connector))
        .route("/api/connectors/:id/disable", post(connectors::disable_connector))
        .route("/api/connectors/:id/inbound", post(connectors::inbound))
        .route("/api/connectors/:id/inbound-log", get(connectors::get_inbound_log))
        // Cron job endpoints
        .route("/api/cron", get(cron::list_cron_jobs))
        .route("/api/cron", post(cron::create_cron_job))
        .route("/api/cron/:id", get(cron::get_cron_job))
        .route("/api/cron/:id", put(cron::update_cron_job))
        .route("/api/cron/:id", delete(cron::delete_cron_job))
        .route("/api/cron/:id/trigger", post(cron::trigger_cron_job))
        .route("/api/cron/:id/enable", post(cron::enable_cron_job))
        .route("/api/cron/:id/disable", post(cron::disable_cron_job))
        // Webhook endpoints
        .route(
            "/api/webhooks/:hook_type/:hook_id",
            post(webhooks::handle_webhook),
        )
        .route(
            "/api/webhooks/:hook_type/:hook_id/verify",
            get(webhooks::verify_webhook),
        )
        .route(
            "/api/webhooks/whatsapp/:phone_number_id/messages",
            post(webhooks::handle_whatsapp_webhook),
        )
        .route(
            "/api/webhooks/telegram/:bot_id",
            post(webhooks::handle_telegram_webhook),
        )
        // SSE Events endpoints
        .route("/api/events", get(events::all_events_stream))
        .route("/api/events/:conversation_id", get(events::event_stream))
        // Settings endpoints
        .route("/api/settings/tools", get(settings::get_tool_settings))
        .route("/api/settings/tools", put(settings::update_tool_settings))
        .route("/api/settings/logs", get(settings::get_log_settings))
        .route("/api/settings/logs", put(settings::update_log_settings))
        // Memory endpoints
        .route("/api/memory", get(memory::list_all_memory_facts))
        .route("/api/memory/:agent_id", get(memory::list_memory_facts))
        .route("/api/memory/:agent_id/search", get(memory::search_memory_facts))
        .route("/api/memory/:agent_id/facts/:fact_id", get(memory::get_memory_fact))
        .route("/api/memory/:agent_id/facts/:fact_id", delete(memory::delete_memory_fact))
        // Logs endpoints (from api-logs crate)
        .nest_service("/api/logs", api_logs::routes(state.log_service.clone()))
        // Execution state endpoints (from execution-state crate)
        .nest_service("/api/executions", execution_state::routes(state.state_service.clone()))
        // Gateway Bus endpoints (for external connectors and API integrations)
        .nest("/api/gateway", gateway_bus::routes())
        // Bridge endpoints
        .route("/api/bridge/workers", get(bridge::list_workers))
        .route("/bridge/ws", get(bridge::ws_upgrade))
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
