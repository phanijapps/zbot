//! # Agent Endpoints
//!
//! CRUD operations for agents.

use crate::services::agents::Agent;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

/// Agent response (full view for API).
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentResponse {
    pub id: String,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub model: String,
    pub temperature: f64,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(rename = "thinkingEnabled")]
    pub thinking_enabled: bool,
    #[serde(rename = "voiceRecordingEnabled")]
    pub voice_recording_enabled: bool,
    pub instructions: String,
    pub mcps: Vec<String>,
    pub skills: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middleware: Option<String>,
    #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

impl From<Agent> for AgentResponse {
    fn from(agent: Agent) -> Self {
        Self {
            id: agent.id,
            name: agent.name,
            display_name: agent.display_name,
            description: agent.description,
            provider_id: agent.provider_id,
            model: agent.model,
            temperature: agent.temperature,
            max_tokens: agent.max_tokens,
            thinking_enabled: agent.thinking_enabled,
            voice_recording_enabled: agent.voice_recording_enabled,
            instructions: agent.instructions,
            mcps: agent.mcps,
            skills: agent.skills,
            middleware: agent.middleware,
            created_at: agent.created_at,
        }
    }
}

/// Create agent request.
#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub model: String,
    pub temperature: Option<f64>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<u32>,
    pub instructions: Option<String>,
    pub mcps: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
}

/// Update agent request.
#[derive(Debug, Deserialize)]
pub struct UpdateAgentRequest {
    pub name: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "providerId")]
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<u32>,
    #[serde(rename = "thinkingEnabled")]
    pub thinking_enabled: Option<bool>,
    #[serde(rename = "voiceRecordingEnabled")]
    pub voice_recording_enabled: Option<bool>,
    pub instructions: Option<String>,
    pub mcps: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub middleware: Option<String>,
}

/// GET /api/agents - List all agents.
pub async fn list_agents(State(state): State<AppState>) -> Json<Vec<AgentResponse>> {
    match state.agents.list().await {
        Ok(agents) => Json(agents.into_iter().map(AgentResponse::from).collect()),
        Err(e) => {
            tracing::error!("Failed to list agents: {}", e);
            Json(vec![])
        }
    }
}

/// POST /api/agents - Create a new agent.
pub async fn create_agent(
    State(state): State<AppState>,
    Json(request): Json<CreateAgentRequest>,
) -> Result<Json<AgentResponse>, StatusCode> {
    let agent = Agent {
        id: String::new(),
        name: request.name.clone(),
        display_name: request.display_name.unwrap_or_else(|| request.name.clone()),
        description: request.description.unwrap_or_default(),
        agent_type: Some("llm".to_string()),
        provider_id: request.provider_id,
        model: request.model,
        temperature: request.temperature.unwrap_or(0.7),
        max_tokens: request.max_tokens.unwrap_or(2000),
        thinking_enabled: false,
        voice_recording_enabled: true,
        system_instruction: None,
        instructions: request.instructions.unwrap_or_else(|| "You are a helpful AI assistant.".to_string()),
        mcps: request.mcps.unwrap_or_default(),
        skills: request.skills.unwrap_or_default(),
        middleware: None,
        created_at: None,
    };

    match state.agents.create(agent).await {
        Ok(created) => Ok(Json(AgentResponse::from(created))),
        Err(e) => {
            tracing::error!("Failed to create agent: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/agents/:id - Get an agent by ID.
pub async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentResponse>, StatusCode> {
    match state.agents.get(&id).await {
        Ok(agent) => Ok(Json(AgentResponse::from(agent))),
        Err(e) => {
            tracing::warn!("Agent not found: {} - {}", id, e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

/// PUT /api/agents/:id - Update an agent.
pub async fn update_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateAgentRequest>,
) -> Result<Json<AgentResponse>, StatusCode> {
    // Get existing agent
    let existing = match state.agents.get(&id).await {
        Ok(a) => a,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    // Merge updates
    let updated = Agent {
        id: existing.id,
        name: request.name.unwrap_or(existing.name),
        display_name: request.display_name.unwrap_or(existing.display_name),
        description: request.description.unwrap_or(existing.description),
        agent_type: existing.agent_type,
        provider_id: request.provider_id.unwrap_or(existing.provider_id),
        model: request.model.unwrap_or(existing.model),
        temperature: request.temperature.unwrap_or(existing.temperature),
        max_tokens: request.max_tokens.unwrap_or(existing.max_tokens),
        thinking_enabled: request.thinking_enabled.unwrap_or(existing.thinking_enabled),
        voice_recording_enabled: request.voice_recording_enabled.unwrap_or(existing.voice_recording_enabled),
        system_instruction: existing.system_instruction,
        instructions: request.instructions.unwrap_or(existing.instructions),
        mcps: request.mcps.unwrap_or(existing.mcps),
        skills: request.skills.unwrap_or(existing.skills),
        middleware: request.middleware.or(existing.middleware),
        created_at: existing.created_at,
    };

    match state.agents.update(&id, updated).await {
        Ok(agent) => Ok(Json(AgentResponse::from(agent))),
        Err(e) => {
            tracing::error!("Failed to update agent: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// DELETE /api/agents/:id - Delete an agent.
pub async fn delete_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> StatusCode {
    match state.agents.delete(&id).await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => {
            tracing::warn!("Failed to delete agent: {} - {}", id, e);
            StatusCode::NOT_FOUND
        }
    }
}
