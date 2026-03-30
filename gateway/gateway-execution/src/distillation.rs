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
use gateway_database::{ConversationRepository, DistillationRepository, DistillationRun, EpisodeRepository, MemoryFact, MemoryRepository, SessionEpisode};
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
    episode_repo: Option<Arc<EpisodeRepository>>,
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

/// An episode assessment extracted by the distillation LLM call.
#[derive(Debug, Clone, Deserialize)]
struct ExtractedEpisode {
    task_summary: String,
    /// One of: 'success', 'partial', 'failed'
    outcome: String,
    strategy_used: Option<String>,
    key_learnings: Option<String>,
}

/// Full distillation response including facts, entities, relationships, and episode.
#[derive(Debug, Clone, Deserialize)]
struct DistillationResponse {
    #[serde(default)]
    facts: Vec<ExtractedFact>,
    #[serde(default)]
    entities: Vec<ExtractedEntity>,
    #[serde(default)]
    relationships: Vec<ExtractedRelationship>,
    #[serde(default)]
    episode: Option<ExtractedEpisode>,
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
        episode_repo: Option<Arc<EpisodeRepository>>,
        paths: Arc<VaultPaths>,
    ) -> Self {
        Self {
            provider_service,
            embedding_client,
            conversation_repo,
            memory_repo,
            graph_storage,
            distillation_repo,
            episode_repo,
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

        if response.facts.is_empty() && response.entities.is_empty() && response.episode.is_none() {
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
                ward_id: "__global__".to_string(),
                contradicted_by: None,
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

        // 6. Store episode if extracted
        let mut episode_created = false;
        if let Some(ref extracted_episode) = response.episode {
            match self.store_episode(session_id, agent_id, extracted_episode, &now).await {
                Ok(true) => {
                    episode_created = true;
                    tracing::info!(
                        session_id = %session_id,
                        outcome = %extracted_episode.outcome,
                        "Episode created from distillation"
                    );
                }
                Ok(false) => {
                    tracing::debug!(
                        session_id = %session_id,
                        "Episode extraction skipped — no episode repository"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        session_id = %session_id,
                        error = %e,
                        "Failed to store episode — continuing with distillation"
                    );
                }
            }
        }

        let duration_ms = started.elapsed().as_millis() as i64;

        // 7. Record success in distillation_runs
        self.record_success(
            session_id,
            response.facts.len() as i32,
            response.entities.len() as i32,
            response.relationships.len() as i32,
            episode_created,
            duration_ms,
        );

        tracing::info!(
            session_id = %session_id,
            upserted = upserted,
            episode_created = episode_created,
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
            "## Session Transcript\n\n{}\n\n---\nExtract durable facts, entities, relationships, and an episode assessment. Respond with ONLY the JSON object, nothing else.",
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
                    let content = &response.content;
                    tracing::debug!(
                        provider = %provider.name,
                        response_len = content.len(),
                        "Distillation LLM responded ({} chars)",
                        content.len()
                    );
                    match parse_distillation_response(content) {
                        Ok(parsed) => {
                            tracing::info!(
                                provider = %provider.name,
                                facts = parsed.facts.len(),
                                entities = parsed.entities.len(),
                                relationships = parsed.relationships.len(),
                                has_episode = parsed.episode.is_some(),
                                "Distillation parsed successfully"
                            );
                            return Ok(parsed);
                        }
                        Err(parse_err) => {
                            // Log the raw response so we can debug what the LLM returned
                            let preview = if content.len() > 800 { &content[..800] } else { content.as_str() };
                            tracing::warn!(
                                provider = %provider.name,
                                error = %parse_err,
                                response_preview = %preview,
                                "Distillation response could not be parsed — trying next provider"
                            );
                            last_error = format!("Provider '{}': parse failed: {}", provider.name, parse_err);
                            continue; // Try next provider — a different model might produce parseable JSON
                        }
                    }
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

    // =========================================================================
    // Episode storage and strategy emergence
    // =========================================================================

    /// Store an extracted episode and attempt strategy emergence.
    ///
    /// Returns `Ok(true)` if the episode was stored, `Ok(false)` if no repo,
    /// or `Err` on failure.
    async fn store_episode(
        &self,
        session_id: &str,
        agent_id: &str,
        extracted: &ExtractedEpisode,
        now: &str,
    ) -> Result<bool, String> {
        let episode_repo = match &self.episode_repo {
            Some(repo) => repo,
            None => return Ok(false),
        };

        // Look up ward_id from the sessions table
        let ward_id = self
            .conversation_repo
            .get_session_ward_id(session_id)
            .unwrap_or(None)
            .unwrap_or_else(|| "__global__".to_string());

        // Embed the task summary for similarity search
        let embedding = self.embed_text(&extracted.task_summary).await;

        let episode = SessionEpisode {
            id: format!("ep-{}", uuid::Uuid::new_v4()),
            session_id: session_id.to_string(),
            agent_id: agent_id.to_string(),
            ward_id: ward_id.clone(),
            task_summary: extracted.task_summary.clone(),
            outcome: extracted.outcome.clone(),
            strategy_used: extracted.strategy_used.clone(),
            key_learnings: extracted.key_learnings.clone(),
            token_cost: None,
            embedding: embedding.clone(),
            created_at: now.to_string(),
        };

        episode_repo.insert(&episode)?;

        // Attempt strategy emergence for successful episodes
        if extracted.outcome == "success" {
            if let Err(e) = self
                .try_emerge_strategy(agent_id, &ward_id, &episode, embedding.as_deref(), now)
                .await
            {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Strategy emergence failed — non-fatal"
                );
            }
        }

        // Attempt failure clustering for failed/partial episodes
        if extracted.outcome == "failed" || extracted.outcome == "partial" {
            if let Err(e) = self
                .try_cluster_failures(agent_id, &episode, &ward_id)
                .await
            {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Failure clustering failed (non-fatal)"
                );
            }
        }

        Ok(true)
    }

    /// Attempt to emerge a strategy from repeated successful episodes.
    ///
    /// If 2+ similar successful episodes exist for this agent, extract the
    /// common strategy pattern and upsert it as a `strategy` memory fact.
    async fn try_emerge_strategy(
        &self,
        agent_id: &str,
        ward_id: &str,
        episode: &SessionEpisode,
        embedding: Option<&[f32]>,
        now: &str,
    ) -> Result<(), String> {
        let episode_repo = match &self.episode_repo {
            Some(repo) => repo,
            None => return Ok(()),
        };

        let query_embedding = match embedding {
            Some(emb) => emb,
            None => return Ok(()), // No embedding — cannot search by similarity
        };

        // Search for similar episodes
        let similar = episode_repo.search_by_similarity(agent_id, query_embedding, 0.7, 10)?;

        // Filter to only successful episodes (excluding the one we just inserted)
        let successful_similar: Vec<_> = similar
            .into_iter()
            .filter(|(ep, _score)| ep.outcome == "success" && ep.id != episode.id)
            .collect();

        // Need at least 2 similar successful episodes (the new one + 2 existing = pattern)
        if successful_similar.len() < 2 {
            return Ok(());
        }

        // Extract strategy: use the most recent episode's strategy_used
        let strategy_description = episode
            .strategy_used
            .as_deref()
            .or_else(|| {
                successful_similar
                    .first()
                    .and_then(|(ep, _)| ep.strategy_used.as_deref())
            })
            .unwrap_or("Repeated successful approach")
            .to_string();

        // Derive a sanitized key from the task summary
        let task_type = sanitize_task_type(&episode.task_summary);
        let fact_key = format!("strategy.{}", task_type);

        tracing::info!(
            agent_id = %agent_id,
            key = %fact_key,
            similar_count = successful_similar.len(),
            "Strategy emerged from {} similar successful episodes",
            successful_similar.len()
        );

        // Upsert the strategy as a memory fact
        let fact = MemoryFact {
            id: format!("fact-{}", uuid::Uuid::new_v4()),
            session_id: Some(episode.session_id.clone()),
            agent_id: agent_id.to_string(),
            scope: "agent".to_string(),
            category: "strategy".to_string(),
            key: fact_key,
            content: strategy_description,
            confidence: 0.92,
            mention_count: 1,
            source_summary: Some(format!(
                "Emerged from {} similar successful episodes in ward '{}'",
                successful_similar.len() + 1,
                ward_id,
            )),
            embedding: embedding.map(|e| e.to_vec()),
            ward_id: ward_id.to_string(),
            contradicted_by: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
            expires_at: None,
        };

        self.memory_repo.upsert_memory_fact(&fact)?;

        Ok(())
    }

    /// Attempt to cluster repeated failures and generate a correction fact.
    ///
    /// If 3+ similar failed/partial episodes exist for this agent, extract the
    /// common failure pattern and upsert it as a `correction` memory fact.
    async fn try_cluster_failures(
        &self,
        agent_id: &str,
        episode: &SessionEpisode,
        ward_id: &str,
    ) -> Result<(), String> {
        let episode_repo = match &self.episode_repo {
            Some(repo) => repo,
            None => return Ok(()),
        };

        // Embed the task summary for similarity search
        let embedding = self.embed_text(&episode.task_summary).await;
        let query_embedding = match embedding.as_deref() {
            Some(emb) => emb,
            None => return Ok(()), // No embedding — cannot search by similarity
        };

        // Search for similar episodes (wider threshold than strategy: 0.6 vs 0.7)
        let similar = episode_repo.search_by_similarity(agent_id, query_embedding, 0.6, 20)?;

        // Filter to only failed/partial episodes (excluding the one we just inserted)
        let failed_similar: Vec<_> = similar
            .into_iter()
            .filter(|(ep, _score)| {
                (ep.outcome == "failed" || ep.outcome == "partial")
                    && ep.session_id != episode.session_id
            })
            .collect();

        // Need at least 3 similar failed episodes to form a cluster
        if failed_similar.len() < 3 {
            return Ok(());
        }

        let cluster_size = failed_similar.len();

        // Extract the common failure pattern from key_learnings
        let latest_key_learning = episode
            .key_learnings
            .as_deref()
            .or_else(|| {
                failed_similar
                    .first()
                    .and_then(|(ep, _)| ep.key_learnings.as_deref())
            })
            .unwrap_or("Repeated failure without specific learning")
            .to_string();

        // Derive a sanitized key from the task summary
        let task_type = sanitize_task_type(&episode.task_summary);
        let fact_key = format!("correction.recurring.{}", task_type);
        let now = chrono::Utc::now().to_rfc3339();

        tracing::info!(
            agent_id = %agent_id,
            key = %fact_key,
            cluster_size = cluster_size,
            "Failure cluster detected from {} similar failed episodes",
            cluster_size
        );

        // Upsert the correction as a memory fact
        let fact = MemoryFact {
            id: format!("fact-{}", uuid::Uuid::new_v4()),
            session_id: Some(episode.session_id.clone()),
            agent_id: agent_id.to_string(),
            scope: "agent".to_string(),
            category: "correction".to_string(),
            key: fact_key,
            content: format!("Recurring failure ({} episodes): {}", cluster_size, latest_key_learning),
            confidence: (0.85 + 0.02 * cluster_size as f64).min(0.98),
            mention_count: cluster_size as i32,
            source_summary: Some("Clustered from repeated failures".to_string()),
            embedding: embedding.clone(),
            ward_id: ward_id.to_string(),
            contradicted_by: None,
            created_at: now.clone(),
            updated_at: now,
            expires_at: None,
        };

        self.memory_repo.upsert_memory_fact(&fact)?;

        Ok(())
    }

    // =========================================================================
    // Embedding
    // =========================================================================

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

/// Derive a sanitized task type from a task summary for use as a fact key.
///
/// Takes the first few words, lowercases them, replaces spaces and dots with
/// underscores, and caps length to keep the key concise.
fn sanitize_task_type(task_summary: &str) -> String {
    task_summary
        .to_lowercase()
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join("_")
        .replace('.', "_")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .take(40)
        .collect()
}

/// Build a compact transcript from session messages.
fn build_transcript(messages: &[gateway_database::Message]) -> String {
    let mut parts = Vec::with_capacity(messages.len());

    for msg in messages {
        let role = match msg.role.as_str() {
            "user" => "USER",
            "assistant" => "ASSISTANT",
            "system" => continue, // Skip system messages — they're instructions, not conversation
            "tool" => "TOOL_RESULT",
            _ => &msg.role,
        };

        // For tool results, extract the meaningful content, not raw JSON
        let content = if msg.role == "tool" {
            summarize_tool_result(&msg.content)
        } else if msg.content.len() > 1000 {
            format!("{}... [truncated, {} chars total]", zero_core::truncate_str(&msg.content, 1000), msg.content.len())
        } else {
            msg.content.clone()
        };

        // For assistant messages with tool calls, show what tools were called
        let tool_info = if let Some(tc) = &msg.tool_calls {
            match serde_json::from_str::<Vec<serde_json::Value>>(tc) {
                Ok(calls) => {
                    let names: Vec<String> = calls.iter()
                        .filter_map(|c| c.get("tool_name").or(c.get("name")).and_then(|n| n.as_str()).map(String::from))
                        .collect();
                    if names.is_empty() {
                        String::new()
                    } else {
                        format!(" [called: {}]", names.join(", "))
                    }
                }
                Err(_) => String::new(),
            }
        } else {
            String::new()
        };

        // Skip empty content (sometimes tool call messages have "[tool calls]" as placeholder)
        if content.is_empty() || content == "[tool calls]" {
            if !tool_info.is_empty() {
                parts.push(format!("ASSISTANT:{}", tool_info));
            }
            continue;
        }

        parts.push(format!("{}: {}{}", role, content, tool_info));
    }

    parts.join("\n\n")
}

/// Summarize a tool result for the distillation transcript.
/// Extracts meaningful content from JSON tool responses, reducing noise.
fn summarize_tool_result(content: &str) -> String {
    // Try to parse as JSON and extract key fields
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(obj) = val.as_object() {
            // Common tool result patterns
            if let Some(stdout) = obj.get("stdout").and_then(|v| v.as_str()) {
                let exit_code = obj.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
                let stderr = obj.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
                let mut result = format!("[exit_code: {}]", exit_code);
                if !stdout.trim().is_empty() {
                    let truncated = if stdout.len() > 500 { &stdout[..500] } else { stdout };
                    result.push_str(&format!(" {}", truncated.trim()));
                }
                if !stderr.trim().is_empty() && exit_code != 0 {
                    let truncated = if stderr.len() > 300 { &stderr[..300] } else { stderr };
                    result.push_str(&format!(" STDERR: {}", truncated.trim()));
                }
                return result;
            }
            // Delegation result
            if let Some(message) = obj.get("message").and_then(|v| v.as_str()) {
                return format!("[delegation result] {}", if message.len() > 500 { &message[..500] } else { message });
            }
            // Ward change
            if obj.get("__ward_changed__").is_some() {
                let action = obj.get("action").and_then(|v| v.as_str()).unwrap_or("changed");
                return format!("[ward {}]", action);
            }
        }
    }
    // Fallback: truncate raw content
    if content.len() > 500 {
        format!("{}... [truncated]", zero_core::truncate_str(content, 500))
    } else {
        content.to_string()
    }
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
/// - JSON with surrounding explanation text
///
/// Returns Err if the response cannot be parsed at all — this is a real failure,
/// not "nothing worth remembering".
fn parse_distillation_response(content: &str) -> Result<DistillationResponse, String> {
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return Err("LLM returned empty response".to_string());
    }

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
            episode: None,
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
            episode: None,
        });
    }

    // Try a lenient parse — maybe the LLM returned valid JSON but with extra/different field names
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
        return parse_distillation_from_value(&val);
    }

    // All parsing failed — this is a real error, not "nothing to extract"
    let preview = if trimmed.len() > 500 { &trimmed[..500] } else { trimmed };
    Err(format!("Failed to parse distillation response. Preview: {}", preview))
}

/// Try to extract a DistillationResponse from an arbitrary JSON Value.
/// Handles cases where the LLM uses slightly different field names or structures.
fn parse_distillation_from_value(val: &serde_json::Value) -> Result<DistillationResponse, String> {
    let obj = val.as_object().ok_or("Response is not a JSON object")?;

    let facts: Vec<ExtractedFact> = obj.get("facts")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let entities: Vec<ExtractedEntity> = obj.get("entities")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let relationships: Vec<ExtractedRelationship> = obj.get("relationships")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let episode: Option<ExtractedEpisode> = obj.get("episode")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    if facts.is_empty() && entities.is_empty() && episode.is_none() {
        // The JSON parsed but all arrays were empty or had incompatible fields
        // Log what was actually in the JSON to help debug
        let keys: Vec<&String> = obj.keys().collect();
        tracing::warn!(
            keys = ?keys,
            "Parsed JSON but extracted nothing — check if LLM used unexpected field names"
        );
    }

    Ok(DistillationResponse { facts, entities, relationships, episode })
}

/// Extract JSON content from text that may contain markdown code blocks.
fn extract_json_from_content(content: &str) -> String {
    // 1. Try markdown code blocks first: ```json ... ``` or ``` ... ```
    let code_block_patterns = ["```json\n", "```json\r\n", "```JSON\n", "```\n", "```\r\n"];
    for pattern in &code_block_patterns {
        if let Some(start) = content.find(pattern) {
            let json_start = start + pattern.len();
            if let Some(end) = content[json_start..].find("```") {
                let extracted = content[json_start..json_start + end].trim();
                if !extracted.is_empty() {
                    return extracted.to_string();
                }
            }
        }
    }

    // 2. Try object brackets (full distillation response — most common expected format)
    if let Some(start) = content.find('{') {
        if let Some(end) = content.rfind('}') {
            if end > start {
                return content[start..=end].to_string();
            }
        }
    }

    // 3. Try array brackets (facts-only backward compat)
    if let Some(start) = content.find('[') {
        if let Some(end) = content.rfind(']') {
            if end > start {
                return content[start..=end].to_string();
            }
        }
    }

    content.to_string()
}

/// The distillation prompt sent as a system message.
/// The default distillation prompt (embedded fallback).
/// Can be overridden by creating `config/distillation_prompt.md` in the vault.
const DEFAULT_DISTILLATION_PROMPT: &str = r#"You are a memory extraction system. Analyze the session transcript and extract durable facts, entities, relationships, and an episode assessment worth remembering for FUTURE sessions.

IMPORTANT: Respond with ONLY a valid JSON object. No explanation, no markdown, no text before or after the JSON. Your entire response must be parseable JSON.

Return a JSON object with three arrays and one optional episode object:

{
  "facts": [
    {"category": "...", "key": "category.subdomain.topic", "content": "1-2 sentence fact", "confidence": 0.0-1.0}
  ],
  "entities": [
    {"name": "entity name", "type": "person|organization|project|tool|concept|file", "properties": {}}
  ],
  "relationships": [
    {"source": "entity name", "target": "entity name", "type": "relationship_type"}
  ],
  "episode": {
    "task_summary": "What the user was trying to accomplish (1-2 sentences)",
    "outcome": "success|partial|failed",
    "strategy_used": "What approach was taken (e.g., 'delegated to data-analyst for technicals')",
    "key_learnings": "What went well or poorly (1-2 sentences)"
  }
}

## Episode Assessment

Assess the session as a whole and return an "episode" object:
- task_summary: What was the user trying to accomplish? (1-2 sentences)
- outcome: Did the agent complete the goal? One of: success, partial, failed
- strategy_used: What approach was taken? (e.g., "delegated to data-analyst for technicals", "direct code generation", "multi-step research then implementation")
- key_learnings: What went well or poorly? (1-2 sentences)

If the session is too short or unclear to assess, omit the episode field.

## Fact Categories (6 types)

- `user` — user preferences, style, capabilities (e.g., coding style, language preferences, expertise areas)
- `pattern` — how-to knowledge, error workarounds, successful workflows (e.g., build steps, debug techniques)
- `domain` — domain knowledge with hierarchical keys (e.g., `domain.finance.lmnd.outlook`, `domain.rust.async_patterns`)
- `instruction` — standing orders, workflow rules (e.g., "always use X", "never do Y", "run tests before commit")
- `correction` — corrections to agent behavior (e.g., "don't suggest X because Y", mistakes and lessons learned)
- `strategy` — successful approaches for recurring task types (e.g., "for data analysis tasks, delegate to data-analyst subagent")

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
- Prefer fewer high-quality extractions over many low-value ones.

## Output Format

CRITICAL: Your ENTIRE response must be a single valid JSON object. Do NOT include any text, explanation, or markdown formatting. Start your response with { and end with }."#;

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
        // Unparseable text should now return Err, not Ok(empty)
        let result = parse_distillation_response("No facts to extract from this session.");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse"));
    }

    #[test]
    fn test_parse_facts_with_surrounding_text() {
        // JSON embedded in surrounding text should still be extracted
        let text = "Here are the extracted facts:\n{\"facts\": [{\"category\": \"pattern\", \"key\": \"pattern.workflow.test_before_commit\", \"content\": \"Always run tests before committing\", \"confidence\": 0.8}], \"entities\": [], \"relationships\": []}\nDone.";
        let resp = parse_distillation_response(text).unwrap();
        assert_eq!(resp.facts.len(), 1);
        assert_eq!(resp.facts[0].category, "pattern");
    }

    #[test]
    fn test_build_transcript_truncates() {
        let long_content = "x".repeat(2000);
        let messages = vec![gateway_database::Message {
            id: "msg-1".to_string(),
            execution_id: Some("exec-1".to_string()),
            session_id: Some("sess-1".to_string()),
            role: "user".to_string(),
            content: long_content,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            token_count: 500,
            tool_calls: None,
            tool_results: None,
            tool_call_id: None,
        }];

        let transcript = build_transcript(&messages);
        assert!(transcript.contains("truncated"));
        assert!(transcript.len() < 3000); // Truncated at 1000 chars + prefix
    }

    #[test]
    fn test_default_confidence() {
        let json = r#"[{"category": "domain", "key": "domain.zbot.project_name", "content": "Project is called AgentZero"}]"#;
        let resp = parse_distillation_response(json).unwrap();
        assert_eq!(resp.facts[0].confidence, 0.8);
    }

    #[test]
    fn test_parse_response_with_episode() {
        let json = r#"{
            "facts": [{"category": "domain", "key": "domain.test", "content": "Test fact", "confidence": 0.9}],
            "entities": [],
            "relationships": [],
            "episode": {
                "task_summary": "User asked to analyze portfolio data",
                "outcome": "success",
                "strategy_used": "delegated to data-analyst for technicals",
                "key_learnings": "CSV parsing worked well with pandas"
            }
        }"#;
        let resp = parse_distillation_response(json).unwrap();
        assert_eq!(resp.facts.len(), 1);
        let ep = resp.episode.unwrap();
        assert_eq!(ep.outcome, "success");
        assert_eq!(ep.task_summary, "User asked to analyze portfolio data");
        assert_eq!(ep.strategy_used.as_deref(), Some("delegated to data-analyst for technicals"));
        assert_eq!(ep.key_learnings.as_deref(), Some("CSV parsing worked well with pandas"));
    }

    #[test]
    fn test_parse_response_without_episode() {
        let json = r#"{"facts": [], "entities": [], "relationships": []}"#;
        let resp = parse_distillation_response(json).unwrap();
        assert!(resp.episode.is_none());
    }

    #[test]
    fn test_parse_response_episode_partial_fields() {
        let json = r#"{
            "facts": [],
            "entities": [],
            "relationships": [],
            "episode": {
                "task_summary": "Quick question about Rust",
                "outcome": "partial"
            }
        }"#;
        let resp = parse_distillation_response(json).unwrap();
        let ep = resp.episode.unwrap();
        assert_eq!(ep.outcome, "partial");
        assert!(ep.strategy_used.is_none());
        assert!(ep.key_learnings.is_none());
    }

    #[test]
    fn test_sanitize_task_type_basic() {
        assert_eq!(sanitize_task_type("Analyze portfolio data"), "analyze_portfolio_data");
    }

    #[test]
    fn test_sanitize_task_type_long_summary() {
        assert_eq!(
            sanitize_task_type("User asked the agent to analyze their entire stock portfolio and generate a report"),
            "user_asked_the_agent"
        );
    }

    #[test]
    fn test_sanitize_task_type_with_dots() {
        assert_eq!(sanitize_task_type("Fix config.toml parsing"), "fix_config_toml_parsing");
    }

    #[test]
    fn test_sanitize_task_type_special_chars() {
        assert_eq!(sanitize_task_type("Build & deploy (v2)"), "build__deploy_v2");
    }

    #[test]
    fn test_sanitize_task_type_empty() {
        assert_eq!(sanitize_task_type(""), "");
    }
}
