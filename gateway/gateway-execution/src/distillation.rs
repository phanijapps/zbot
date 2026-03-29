// ============================================================================
// SESSION DISTILLATION
// Automatically extract durable facts from completed sessions
// ============================================================================

//! The `SessionDistiller` analyzes completed session transcripts and extracts
//! structured facts worth remembering for future sessions. This is AgentZero's
//! key advantage over other memory frameworks — we have full tool call history
//! (Session Tree) and can automatically distill it without the agent needing
//! to explicitly save.
//!
//! ## Flow
//!
//! 1. Load last N messages from a completed session
//! 2. Build a distillation prompt
//! 3. Call the LLM to extract structured facts as JSON
//! 4. Upsert each fact into `memory_facts` with embedding
//! 5. Cache the embedding for hash-based dedup

use std::sync::Arc;

use agent_runtime::llm::client::LlmClient;
use agent_runtime::llm::config::LlmConfig;
use agent_runtime::llm::embedding::EmbeddingClient;
use agent_runtime::llm::openai::OpenAiClient;
use agent_runtime::types::ChatMessage;
use gateway_database::{ConversationRepository, DistillationRepository, DistillationRun, MemoryFact, MemoryRepository};
use gateway_services::{ProviderService, VaultPaths};
use knowledge_graph::{GraphStorage, Entity, EntityType, Relationship, RelationshipType};
use serde::Deserialize;

/// Distills completed sessions into structured memory facts.
pub struct SessionDistiller {
    provider_service: Arc<ProviderService>,
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
    conversation_repo: Arc<ConversationRepository>,
    memory_repo: Arc<MemoryRepository>,
    graph_storage: Option<Arc<GraphStorage>>,
    distillation_repo: Option<Arc<DistillationRepository>>,
    paths: Arc<VaultPaths>,
}

/// A single fact extracted by the distillation LLM call.
#[derive(Debug, Clone, Deserialize)]
struct ExtractedFact {
    category: String,
    key: String,
    content: String,
    #[serde(default = "default_confidence")]
    confidence: f64,
}

/// An entity extracted by the distillation LLM call.
#[derive(Debug, Clone, Deserialize)]
struct ExtractedEntity {
    name: String,
    #[serde(rename = "type")]
    entity_type: String,
    #[serde(default)]
    properties: std::collections::HashMap<String, serde_json::Value>,
}

/// A relationship extracted by the distillation LLM call.
#[derive(Debug, Clone, Deserialize)]
struct ExtractedRelationship {
    source: String,
    target: String,
    #[serde(rename = "type")]
    relationship_type: String,
}

/// Full distillation response including facts, entities, and relationships.
#[derive(Debug, Clone, Deserialize)]
struct DistillationResponse {
    #[serde(default)]
    facts: Vec<ExtractedFact>,
    #[serde(default)]
    entities: Vec<ExtractedEntity>,
    #[serde(default)]
    relationships: Vec<ExtractedRelationship>,
}

fn default_confidence() -> f64 {
    0.8
}

/// Minimum number of messages in a session to trigger distillation.
/// Set low to capture learnings from even short sessions.
const MIN_MESSAGES_FOR_DISTILLATION: usize = 4;

/// Maximum messages to load for distillation (to stay within LLM context).
const MAX_MESSAGES_FOR_DISTILLATION: usize = 100;

impl SessionDistiller {
    /// Create a new session distiller with lazy LLM client resolution.
    ///
    /// The distiller resolves the default provider and creates a lightweight
    /// LLM client on-demand in `distill()`, avoiding the need for a concrete
    /// LLM client at construction time.
    pub fn new(
        provider_service: Arc<ProviderService>,
        embedding_client: Option<Arc<dyn EmbeddingClient>>,
        conversation_repo: Arc<ConversationRepository>,
        memory_repo: Arc<MemoryRepository>,
        graph_storage: Option<Arc<GraphStorage>>,
        distillation_repo: Option<Arc<DistillationRepository>>,
        paths: Arc<VaultPaths>,
    ) -> Self {
        Self {
            provider_service,
            embedding_client,
            conversation_repo,
            memory_repo,
            graph_storage,
            distillation_repo,
            paths,
        }
    }

    /// Load the distillation prompt from filesystem or use embedded default.
    ///
    /// Checks for `config/distillation_prompt.md` in the vault directory.
    /// Falls back to the embedded DEFAULT_DISTILLATION_PROMPT if not found.
    fn load_distillation_prompt(&self) -> String {
        let prompt_path = self.paths.distillation_prompt();

        match std::fs::read_to_string(&prompt_path) {
            Ok(content) if !content.trim().is_empty() => {
                tracing::info!("Loaded distillation prompt from {:?}", prompt_path);
                content
            }
            Ok(_) => {
                tracing::debug!("Distillation prompt file is empty, using default");
                DEFAULT_DISTILLATION_PROMPT.to_string()
            }
            Err(_) => {
                // Write default to disk so user can customize it
                if let Some(parent) = prompt_path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                if let Err(e) = std::fs::write(&prompt_path, DEFAULT_DISTILLATION_PROMPT) {
                    tracing::debug!("Failed to write default distillation prompt: {}", e);
                } else {
                    tracing::info!("Created default distillation prompt at {:?}", prompt_path);
                }
                DEFAULT_DISTILLATION_PROMPT.to_string()
            }
        }
    }

    /// Distill a completed session into memory facts.
    ///
    /// Returns the number of facts upserted. Records a `distillation_runs`
    /// entry when the repository is available — optimistic-failure pattern:
    /// insert with `status = 'failed'` up front, then update to `'success'`
    /// or `'skipped'` when the outcome is known.
    pub async fn distill(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> Result<usize, String> {
        let started = std::time::Instant::now();

        // 1. Load session messages
        let messages = self
            .conversation_repo
            .get_session_conversation(session_id, MAX_MESSAGES_FOR_DISTILLATION)
            .map_err(|e| format!("Failed to load session messages: {}", e))?;

        if messages.len() < MIN_MESSAGES_FOR_DISTILLATION {
            tracing::debug!(
                session_id = %session_id,
                message_count = messages.len(),
                "Skipping distillation — too few messages"
            );
            // Record as skipped
            self.record_skipped(session_id);
            return Ok(0);
        }

        // Insert optimistic-failure record before attempting distillation
        self.record_pending(session_id);

        // 2. Build transcript for the LLM
        let transcript = build_transcript(&messages);

        // 3. Call LLM for fact and entity extraction (with provider fallback)
        let response = match self.extract_all(&transcript).await {
            Ok(resp) => resp,
            Err(e) => {
                // The initial 'failed' record stays — update with error message
                self.record_error(session_id, &e);
                return Err(e);
            }
        };

        if response.facts.is_empty() && response.entities.is_empty() {
            tracing::info!(
                session_id = %session_id,
                "Distillation found nothing worth remembering"
            );
            let duration_ms = started.elapsed().as_millis() as i64;
            self.record_success(session_id, 0, 0, 0, false, duration_ms);
            return Ok(0);
        }

        tracing::info!(
            session_id = %session_id,
            fact_count = response.facts.len(),
            entity_count = response.entities.len(),
            relationship_count = response.relationships.len(),
            "Distillation extracted {} facts, {} entities, {} relationships",
            response.facts.len(), response.entities.len(), response.relationships.len()
        );

        // 4. Upsert each fact with embedding
        let now = chrono::Utc::now().to_rfc3339();
        let mut upserted = 0;

        for ef in &response.facts {
            let fact_id = format!("fact-{}", uuid::Uuid::new_v4());

            // Embed the fact content
            let embedding = self.embed_text(&ef.content).await;

            let fact = MemoryFact {
                id: fact_id,
                session_id: Some(session_id.to_string()),
                agent_id: agent_id.to_string(),
                scope: "agent".to_string(),
                category: ef.category.clone(),
                key: ef.key.clone(),
                content: ef.content.clone(),
                confidence: ef.confidence,
                mention_count: 1,
                source_summary: Some(format!("Distilled from session {}", session_id)),
                embedding,
                created_at: now.clone(),
                updated_at: now.clone(),
                expires_at: None,
            };

            if let Err(e) = self.memory_repo.upsert_memory_fact(&fact) {
                tracing::warn!(
                    key = %ef.key,
                    error = %e,
                    "Failed to upsert distilled fact"
                );
            } else {
                upserted += 1;
            }
        }

        // 5. Store entities and relationships in knowledge graph
        if let Some(graph) = &self.graph_storage {
            // Build entity map for relationship resolution
            let mut entity_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

            for ee in &response.entities {
                // Check if entity already exists (dedup by name, case-insensitive)
                match graph.find_entity_by_name(agent_id, &ee.name).await {
                    Ok(Some(existing_id)) => {
                        // Entity already exists — bump mention count and reuse ID
                        if let Err(e) = graph.bump_entity_mention(&existing_id).await {
                            tracing::warn!(entity = %ee.name, error = %e, "Failed to bump entity mention");
                        }
                        entity_map.insert(ee.name.clone(), existing_id);
                    }
                    _ => {
                        // Entity not found — create new
                        let mut entity = Entity::new(
                            agent_id.to_string(),
                            EntityType::from_str(&ee.entity_type),
                            ee.name.clone(),
                        );
                        entity.properties = ee.properties.clone();
                        entity_map.insert(ee.name.clone(), entity.id.clone());

                        let knowledge = knowledge_graph::types::ExtractedKnowledge {
                            entities: vec![entity],
                            relationships: vec![],
                        };
                        if let Err(e) = graph.store_knowledge(agent_id, knowledge).await {
                            tracing::warn!(entity = %ee.name, error = %e, "Failed to store entity");
                        }
                    }
                }
            }

            for er in &response.relationships {
                // Resolve entity names to IDs (or use names as IDs if not found)
                let source_id = entity_map.get(&er.source)
                    .cloned()
                    .unwrap_or_else(|| er.source.clone());
                let target_id = entity_map.get(&er.target)
                    .cloned()
                    .unwrap_or_else(|| er.target.clone());

                let relationship = Relationship::new(
                    agent_id.to_string(),
                    source_id,
                    target_id,
                    RelationshipType::from_str(&er.relationship_type),
                );

                let knowledge = knowledge_graph::types::ExtractedKnowledge {
                    entities: vec![],
                    relationships: vec![relationship],
                };
                if let Err(e) = graph.store_knowledge(agent_id, knowledge).await {
                    tracing::warn!(
                        source = %er.source, target = %er.target,
                        error = %e, "Failed to store relationship"
                    );
                }
            }
        }

        let duration_ms = started.elapsed().as_millis() as i64;

        // 6. Record success in distillation_runs
        self.record_success(
            session_id,
            response.facts.len() as i32,
            response.entities.len() as i32,
            response.relationships.len() as i32,
            false, // episode_created — will be set by episode extraction (Task 8)
            duration_ms,
        );

        tracing::info!(
            session_id = %session_id,
            upserted = upserted,
            duration_ms = duration_ms,
            "Session distillation complete"
        );

        Ok(upserted)
    }

    // =========================================================================
    // Health-reporting helpers
    // =========================================================================

    /// Insert a pending/failed distillation run (optimistic failure).
    fn record_pending(&self, session_id: &str) {
        if let Some(repo) = &self.distillation_repo {
            let run = DistillationRun {
                id: format!("dr-{}", uuid::Uuid::new_v4()),
                session_id: session_id.to_string(),
                status: "failed".to_string(),
                error: Some("Distillation in progress".to_string()),
                created_at: chrono::Utc::now().to_rfc3339(),
                ..Default::default()
            };
            if let Err(e) = repo.insert(&run) {
                tracing::warn!(session_id = %session_id, error = %e, "Failed to insert distillation run record");
            }
        }
    }

    /// Record a skipped distillation (too few messages).
    fn record_skipped(&self, session_id: &str) {
        if let Some(repo) = &self.distillation_repo {
            let run = DistillationRun {
                id: format!("dr-{}", uuid::Uuid::new_v4()),
                session_id: session_id.to_string(),
                status: "skipped".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                ..Default::default()
            };
            if let Err(e) = repo.insert(&run) {
                tracing::warn!(session_id = %session_id, error = %e, "Failed to record skipped distillation");
            }
        }
    }

    /// Update an existing distillation run to success.
    fn record_success(
        &self,
        session_id: &str,
        facts: i32,
        entities: i32,
        rels: i32,
        episode_created: bool,
        duration_ms: i64,
    ) {
        if let Some(repo) = &self.distillation_repo {
            if let Err(e) = repo.update_success(session_id, facts, entities, rels, episode_created, duration_ms) {
                tracing::warn!(session_id = %session_id, error = %e, "Failed to record distillation success");
            }
        }
    }

    /// Update an existing distillation run with an error message.
    fn record_error(&self, session_id: &str, error: &str) {
        if let Some(repo) = &self.distillation_repo {
            if let Err(e) = repo.update_retry(session_id, "failed", 0, Some(error)) {
                tracing::warn!(session_id = %session_id, error = %e, "Failed to record distillation error");
            }
        }
    }

    /// Call the LLM to extract facts, entities, and relationships.
    ///
    /// Implements a provider fallback chain: tries the default provider first,
    /// then iterates through remaining providers if the LLM call fails.
    async fn extract_all(&self, transcript: &str) -> Result<DistillationResponse, String> {
        let providers = self.provider_service.list()
            .map_err(|e| format!("Failed to list providers: {}", e))?;

        if providers.is_empty() {
            return Err("No providers configured — cannot distill session".to_string());
        }

        // Load prompt once (shared across attempts)
        let system = self.load_distillation_prompt();
        let user = format!(
            "## Session Transcript\n\n{}\n\n---\nExtract durable facts, entities, and relationships as JSON.",
            transcript
        );

        // Order: default provider first, then the rest
        let default_idx = providers.iter().position(|p| p.is_default);
        let ordered_indices: Vec<usize> = match default_idx {
            Some(idx) => std::iter::once(idx)
                .chain((0..providers.len()).filter(move |&i| i != idx))
                .collect(),
            None => (0..providers.len()).collect(),
        };

        let mut last_error = String::new();

        for idx in ordered_indices {
            let provider = &providers[idx];
            let model = provider.default_model();
            let provider_id = provider.id.clone().unwrap_or_else(|| "default".to_string());

            let config = LlmConfig::new(
                provider.base_url.clone(),
                provider.api_key.clone(),
                model.to_string(),
                provider_id.clone(),
            )
            .with_temperature(0.3)
            .with_max_tokens(4096);

            let client = match OpenAiClient::new(config) {
                Ok(c) => Arc::new(c) as Arc<dyn LlmClient>,
                Err(e) => {
                    last_error = format!("Provider '{}': client creation failed: {}", provider.name, e);
                    tracing::warn!(
                        provider = %provider.name,
                        error = %e,
                        "Distillation: failed to create LLM client, trying next provider"
                    );
                    continue;
                }
            };

            let messages = vec![
                ChatMessage::system(system.clone()),
                ChatMessage::user(user.clone()),
            ];

            match client.chat(messages, None).await {
                Ok(response) => {
                    return parse_distillation_response(&response.content);
                }
                Err(e) => {
                    last_error = format!("Provider '{}' ({}): LLM call failed: {}", provider.name, provider_id, e);
                    tracing::warn!(
                        provider = %provider.name,
                        provider_id = %provider_id,
                        error = %e,
                        "Distillation LLM call failed, trying next provider"
                    );
                }
            }
        }

        Err(format!("All providers failed for distillation. Last error: {}", last_error))
    }

    /// Embed a single text, with caching.
    async fn embed_text(&self, text: &str) -> Option<Vec<f32>> {
        let client = self.embedding_client.as_ref()?;
        let model_name = client.model_name().to_string();

        // Check cache
        let hash = agent_runtime::content_hash(text);
        if let Ok(Some(cached)) = self.memory_repo.get_cached_embedding(&hash, &model_name) {
            return Some(cached);
        }

        // Generate embedding
        match client.embed(&[text]).await {
            Ok(mut embeddings) if !embeddings.is_empty() => {
                let emb = embeddings.remove(0);
                // Cache it
                let _ = self.memory_repo.cache_embedding(&hash, &model_name, &emb);
                Some(emb)
            }
            Ok(_) => None,
            Err(e) => {
                tracing::warn!("Failed to embed text for distillation: {}", e);
                None
            }
        }
    }
}

/// Build a compact transcript from session messages.
fn build_transcript(messages: &[gateway_database::Message]) -> String {
    let mut parts = Vec::with_capacity(messages.len());

    for msg in messages {
        let role = match msg.role.as_str() {
            "user" => "USER",
            "assistant" => "ASSISTANT",
            "system" => "SYSTEM",
            "tool" => "TOOL",
            _ => &msg.role,
        };

        // Truncate very long messages for the distillation context
        let content = if msg.content.len() > 500 {
            format!("{}... [truncated, {} chars total]", zero_core::truncate_str(&msg.content, 500), msg.content.len())
        } else {
            msg.content.clone()
        };

        // Include tool info if present
        let tool_info = if let Some(tc) = &msg.tool_calls {
            format!(" [tool_calls: {}]", truncate_json(tc, 200))
        } else {
            String::new()
        };

        parts.push(format!("{}: {}{}", role, content, tool_info));
    }

    parts.join("\n\n")
}

/// Truncate a JSON string to a max length for display.
fn truncate_json(json_str: &str, max_len: usize) -> String {
    if json_str.len() <= max_len {
        json_str.to_string()
    } else {
        format!("{}...", zero_core::truncate_str(json_str, max_len))
    }
}

/// Parse the full distillation response (facts + entities + relationships).
///
/// The LLM might return:
/// - A JSON object: `{"facts": [...], "entities": [...], "relationships": [...]}`
/// - Just a JSON array of facts (backward compat): `[{...}, ...]`
/// - JSON wrapped in markdown code block
fn parse_distillation_response(content: &str) -> Result<DistillationResponse, String> {
    let trimmed = content.trim();

    // Try parsing as full distillation response (object with facts/entities/relationships)
    if let Ok(resp) = serde_json::from_str::<DistillationResponse>(trimmed) {
        return Ok(resp);
    }

    // Try parsing as just a facts array (backward compat)
    if let Ok(facts) = serde_json::from_str::<Vec<ExtractedFact>>(trimmed) {
        return Ok(DistillationResponse {
            facts,
            entities: Vec::new(),
            relationships: Vec::new(),
        });
    }

    // Try extracting JSON from markdown code blocks or surrounding text
    let json_str = extract_json_from_content(trimmed);

    if let Ok(resp) = serde_json::from_str::<DistillationResponse>(&json_str) {
        return Ok(resp);
    }

    if let Ok(facts) = serde_json::from_str::<Vec<ExtractedFact>>(&json_str) {
        return Ok(DistillationResponse {
            facts,
            entities: Vec::new(),
            relationships: Vec::new(),
        });
    }

    // Return empty if unparseable
    tracing::debug!("Could not parse distillation response, treating as empty");
    Ok(DistillationResponse {
        facts: Vec::new(),
        entities: Vec::new(),
        relationships: Vec::new(),
    })
}

/// Extract JSON content from text that may contain markdown code blocks.
fn extract_json_from_content(content: &str) -> String {
    // Try array brackets first (more common for facts-only responses)
    if let Some(start) = content.find('[') {
        if let Some(end) = content.rfind(']') {
            return content[start..=end].to_string();
        }
    }
    // Then try object brackets (for full distillation response)
    if let Some(start) = content.find('{') {
        if let Some(end) = content.rfind('}') {
            return content[start..=end].to_string();
        }
    }
    content.to_string()
}

/// The distillation prompt sent as a system message.
/// The default distillation prompt (embedded fallback).
/// Can be overridden by creating `config/distillation_prompt.md` in the vault.
const DEFAULT_DISTILLATION_PROMPT: &str = r#"You are a memory extraction system. Analyze the session transcript and extract durable facts, entities, and relationships worth remembering for FUTURE sessions.

Return a JSON object with three arrays:

{
  "facts": [
    {"category": "...", "key": "category.subdomain.topic", "content": "1-2 sentence fact", "confidence": 0.0-1.0}
  ],
  "entities": [
    {"name": "entity name", "type": "person|organization|project|tool|concept|file", "properties": {}}
  ],
  "relationships": [
    {"source": "entity name", "target": "entity name", "type": "relationship_type"}
  ]
}

## Fact Categories (5 types)

- `user` — user preferences, style, capabilities (e.g., coding style, language preferences, expertise areas)
- `pattern` — how-to knowledge, error workarounds, successful workflows (e.g., build steps, debug techniques)
- `domain` — domain knowledge with hierarchical keys (e.g., `domain.finance.lmnd.outlook`, `domain.rust.async_patterns`)
- `instruction` — standing orders, workflow rules (e.g., "always use X", "never do Y", "run tests before commit")
- `correction` — corrections to agent behavior (e.g., "don't suggest X because Y", mistakes and lessons learned)

## Key Format

Use dot-notation hierarchy: `{category}.{subdomain}.{topic}`
Examples: `user.preferred_language`, `pattern.rust.error_handling`, `domain.finance.lmnd.outlook`, `instruction.testing.always_run_cargo_check`, `correction.code_style.no_unwrap`

If a fact updates something already known, use the SAME key so it overwrites.

## Entity Types

- `person` — people mentioned by name
- `organization` — companies and organizations (use "organization", NOT "company")
- `project` — software projects, repos, products
- `tool` — tools, libraries, frameworks, technologies
- `concept` — abstract concepts, topics, methodologies
- `file` — important ward files (core modules, config files, data files)

Include `properties` where relevant:
- Organizations: sector, ticker, industry
- Projects: language, framework, ward (workspace path)
- Files: ward (workspace path), exports, purpose
- Tools: version, usage context

## Relationship Types

`related_to`, `uses`, `created`, `part_of`, `is_in`, `has_module`, `exports`, `prefers`, `analyzed_by`

## Ward File Summaries

When a session analyzes or works with files in a ward (workspace), include a `domain.{subdomain}.data_available` fact summarizing what data/files are available (e.g., `domain.finance.portfolio_data_available`).

## Rules

- Maximum 20 facts, 20 entities, 20 relationships per session.
- Only extract facts useful in FUTURE sessions. Skip ephemeral details (one-off questions, transient errors, session-specific data).
- Confidence: 0.9+ = explicitly stated, 0.7-0.9 = strongly implied, 0.5-0.7 = inferred from context.
- If nothing worth remembering, return {"facts": [], "entities": [], "relationships": []}.
- Prefer fewer high-quality extractions over many low-value ones."#;

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_facts_from_json_array() {
        // Backward compat: plain array of facts
        let json = r#"[{"category": "user", "key": "user.preferred_language", "content": "User prefers Rust", "confidence": 0.9}]"#;
        let resp = parse_distillation_response(json).unwrap();
        assert_eq!(resp.facts.len(), 1);
        assert_eq!(resp.facts[0].key, "user.preferred_language");
        assert_eq!(resp.facts[0].confidence, 0.9);
    }

    #[test]
    fn test_parse_full_response() {
        let json = r#"{"facts": [{"category": "domain", "key": "domain.zbot.db_engine", "content": "Using SQLite", "confidence": 0.85}], "entities": [{"name": "SQLite", "type": "tool", "properties": {"usage": "embedded database"}}], "relationships": [{"source": "AgentZero", "target": "SQLite", "type": "uses"}]}"#;
        let resp = parse_distillation_response(json).unwrap();
        assert_eq!(resp.facts.len(), 1);
        assert_eq!(resp.entities.len(), 1);
        assert_eq!(resp.entities[0].name, "SQLite");
        assert_eq!(resp.relationships.len(), 1);
    }

    #[test]
    fn test_parse_facts_from_markdown() {
        let md = "```json\n[{\"category\": \"domain\", \"key\": \"domain.zbot.db_engine\", \"content\": \"Using SQLite\", \"confidence\": 0.85}]\n```";
        let resp = parse_distillation_response(md).unwrap();
        assert_eq!(resp.facts.len(), 1);
        assert_eq!(resp.facts[0].key, "domain.zbot.db_engine");
    }

    #[test]
    fn test_parse_facts_empty_array() {
        let resp = parse_distillation_response("[]").unwrap();
        assert!(resp.facts.is_empty());
    }

    #[test]
    fn test_parse_facts_unparseable() {
        let resp = parse_distillation_response("No facts to extract from this session.").unwrap();
        assert!(resp.facts.is_empty());
    }

    #[test]
    fn test_parse_facts_with_surrounding_text() {
        let text = "Here are the extracted facts:\n[{\"category\": \"pattern\", \"key\": \"pattern.workflow.test_before_commit\", \"content\": \"Always run tests before committing\", \"confidence\": 0.8}]\nDone.";
        let resp = parse_distillation_response(text).unwrap();
        assert_eq!(resp.facts.len(), 1);
        assert_eq!(resp.facts[0].category, "pattern");
    }

    #[test]
    fn test_build_transcript_truncates() {
        let long_content = "x".repeat(1000);
        let messages = vec![gateway_database::Message {
            id: "msg-1".to_string(),
            execution_id: Some("exec-1".to_string()),
            session_id: Some("sess-1".to_string()),
            role: "user".to_string(),
            content: long_content,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            token_count: 250,
            tool_calls: None,
            tool_results: None,
            tool_call_id: None,
        }];

        let transcript = build_transcript(&messages);
        assert!(transcript.contains("truncated"));
        assert!(transcript.len() < 2000);
    }

    #[test]
    fn test_default_confidence() {
        let json = r#"[{"category": "domain", "key": "domain.zbot.project_name", "content": "Project is called AgentZero"}]"#;
        let resp = parse_distillation_response(json).unwrap();
        assert_eq!(resp.facts[0].confidence, 0.8);
    }
}
