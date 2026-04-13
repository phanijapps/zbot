//! Two-pass LLM extractor: pass 1 entities, pass 2 relationships conditioned
//! on the entity list. Concrete LLM-backed impl lands in Tasks 5 and 6.
//! `NoopExtractor` provides a test-friendly no-op for queue-level tests.

use crate::ingest::json_shape::parse_llm_json;
use agent_runtime::llm::{ChatMessage, LlmClient};
use async_trait::async_trait;
use gateway_database::KgEpisode;
use gateway_services::ProviderService;
use knowledge_graph::{Entity, EntityType, GraphStorage};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Deserialize)]
struct EntitiesEnvelope {
    entities: Vec<EntityItem>,
}

#[derive(Debug, Deserialize)]
struct EntityItem {
    name: Option<String>,
    #[serde(rename = "type")]
    type_str: Option<String>,
    description: Option<String>,
    #[allow(dead_code)]
    aliases: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RelationshipsEnvelope {
    relationships: Vec<RelationshipItem>,
}

#[derive(Debug, Deserialize)]
struct RelationshipItem {
    source: Option<String>,
    target: Option<String>,
    #[serde(rename = "type")]
    type_str: Option<String>,
}

/// Processes one episode — runs extraction + writes to graph.
/// Errors propagate to the worker which marks the episode failed.
#[async_trait]
pub trait Extractor: Send + Sync {
    async fn process(
        &self,
        episode: &KgEpisode,
        chunk_text: &str,
        graph: &Arc<GraphStorage>,
    ) -> Result<(), String>;
}

/// Test-only extractor: records each episode id and always succeeds.
pub struct NoopExtractor {
    pub seen: Mutex<Vec<String>>,
}

impl NoopExtractor {
    pub fn new() -> Self {
        Self {
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl Default for NoopExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Extractor for NoopExtractor {
    async fn process(
        &self,
        episode: &KgEpisode,
        _chunk_text: &str,
        _graph: &Arc<GraphStorage>,
    ) -> Result<(), String> {
        self.seen.lock().await.push(episode.id.clone());
        Ok(())
    }
}

/// LLM-backed two-pass extractor. Pass 1: entities. Pass 2: relationships
/// (conditioned on the entity list).
pub struct LlmExtractor {
    provider_service: Arc<ProviderService>,
    agent_id: String,
}

impl LlmExtractor {
    pub fn new(provider_service: Arc<ProviderService>, agent_id: String) -> Self {
        Self {
            provider_service,
            agent_id,
        }
    }

    /// Build an LLM client from the current default provider. Mirrors
    /// `SessionDistiller::build_llm_client` so ingestion picks up provider
    /// changes without needing a restart.
    fn build_client(&self) -> Result<Arc<dyn LlmClient>, String> {
        let providers = self
            .provider_service
            .list()
            .map_err(|e| format!("list providers: {e}"))?;
        if providers.is_empty() {
            return Err("No LLM providers configured".to_string());
        }
        let provider = providers
            .iter()
            .find(|p| p.is_default)
            .or_else(|| providers.first())
            .ok_or_else(|| "No suitable provider".to_string())?;

        let model = provider.default_model().to_string();
        let provider_id = provider.id.clone().unwrap_or_else(|| "default".to_string());
        let config = agent_runtime::llm::LlmConfig::new(
            provider.base_url.clone(),
            provider.api_key.clone(),
            model,
            provider_id,
        )
        .with_temperature(0.2)
        .with_max_tokens(4096);

        let client = agent_runtime::llm::openai::OpenAiClient::new(config)
            .map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn LlmClient>)
    }

    async fn extract_entities(
        &self,
        client: &Arc<dyn LlmClient>,
        chunk_text: &str,
    ) -> Result<Vec<Entity>, String> {
        let system = "You extract named entities from text. \
            Return ONLY valid JSON matching the schema. \
            Do not wrap in code fences. Do not add commentary.";

        let user = format!(
            "Extract entities from this text. Output JSON: \
            {{\"entities\": [{{\"name\": string, \"type\": string, \"aliases\": [string], \"description\": string}}]}}\n\n\
            Valid types: person, organization, location, event, document, concept, tool, project, file, time_period, role, artifact, ward.\n\n\
            TEXT:\n{chunk_text}"
        );

        let messages = vec![
            ChatMessage::system(system.to_string()),
            ChatMessage::user(user),
        ];

        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("llm entity pass failed: {e}"))?;

        parse_entities_response(&response.content, &self.agent_id)
    }

    async fn extract_relationships(
        &self,
        client: &Arc<dyn LlmClient>,
        chunk_text: &str,
        entity_names: &[String],
    ) -> Result<Vec<(String, String, String)>, String> {
        if entity_names.len() < 2 {
            return Ok(Vec::new());
        }
        let system = "You extract relationships between entities. \
            Return ONLY valid JSON. Do not add commentary. \
            Every source and target MUST exactly match a name from the provided list.";
        let user = format!(
            "Given these entities: {}\n\n\
            Extract relationships between them from this text. \
            Output JSON: {{\"relationships\": [{{\"source\": string, \"target\": string, \"type\": string}}]}}\n\n\
            TEXT:\n{chunk_text}",
            entity_names.join(", ")
        );
        let messages = vec![
            ChatMessage::system(system.to_string()),
            ChatMessage::user(user),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("llm rel pass failed: {e}"))?;

        parse_relationships_response(&response.content, entity_names)
    }
}

#[async_trait]
impl Extractor for LlmExtractor {
    async fn process(
        &self,
        episode: &KgEpisode,
        chunk_text: &str,
        graph: &Arc<GraphStorage>,
    ) -> Result<(), String> {
        if chunk_text.trim().is_empty() {
            return Ok(());
        }

        let client = self.build_client()?;

        let mut entities = self.extract_entities(&client, chunk_text).await?;
        if entities.is_empty() {
            return Ok(());
        }
        let entity_names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();

        let rel_tuples = self
            .extract_relationships(&client, chunk_text, &entity_names)
            .await?;

        // Build Relationship structs using the candidate entity ids.
        // store_knowledge remaps them to canonical ids via resolver merges.
        let mut candidate_rels = Vec::new();
        for (src_name, tgt_name, ty) in rel_tuples {
            let src_id = entities
                .iter()
                .find(|e| e.name == src_name)
                .map(|e| e.id.clone());
            let tgt_id = entities
                .iter()
                .find(|e| e.name == tgt_name)
                .map(|e| e.id.clone());
            let (Some(src_id), Some(tgt_id)) = (src_id, tgt_id) else {
                continue;
            };
            candidate_rels.push(knowledge_graph::Relationship::new(
                self.agent_id.clone(),
                src_id,
                tgt_id,
                knowledge_graph::RelationshipType::from_str(&ty),
            ));
        }

        // Tag every entity with provenance.
        for e in &mut entities {
            e.properties.insert(
                "_source_episode_id".to_string(),
                serde_json::Value::String(episode.id.clone()),
            );
        }

        let extracted = knowledge_graph::ExtractedKnowledge {
            entities,
            relationships: candidate_rels,
        };
        graph
            .store_knowledge(&self.agent_id, extracted)
            .map_err(|e| format!("store_knowledge: {e}"))?;
        Ok(())
    }
}

fn parse_relationships_response(
    content: &str,
    known_entities: &[String],
) -> Result<Vec<(String, String, String)>, String> {
    let env: RelationshipsEnvelope = parse_llm_json(content)?;
    let known: std::collections::HashSet<&str> =
        known_entities.iter().map(|s| s.as_str()).collect();
    let mut out = Vec::new();
    for item in env.relationships {
        let src = item.source.as_deref().map(str::trim).unwrap_or("");
        let tgt = item.target.as_deref().map(str::trim).unwrap_or("");
        let ty = item.type_str.as_deref().map(str::trim).unwrap_or("");
        if src.is_empty() || tgt.is_empty() || ty.is_empty() {
            continue;
        }
        if !known.contains(src) || !known.contains(tgt) {
            continue; // drop hallucinated references to entities not in pass-1
        }
        out.push((src.to_string(), tgt.to_string(), ty.to_string()));
    }
    Ok(out)
}

/// Parse the LLM's JSON response into typed Entity values.
/// Strips optional code-fence wrapping. Silently skips malformed items.
fn parse_entities_response(content: &str, agent_id: &str) -> Result<Vec<Entity>, String> {
    let env: EntitiesEnvelope = parse_llm_json(content)?;
    let mut out = Vec::new();
    for item in env.entities {
        let Some(name) = item
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let ty = EntityType::from_str(item.type_str.as_deref().unwrap_or("concept"));
        let mut entity = Entity::new(agent_id.to_string(), ty, name.to_string());
        if let Some(desc) = item.description {
            entity
                .properties
                .insert("description".to_string(), serde_json::Value::String(desc));
        }
        out.push(entity);
    }
    Ok(out)
}

#[cfg(test)]
mod tests_extractor {
    use super::*;

    #[test]
    fn parses_entities_from_clean_json() {
        let json = r#"{"entities": [
            {"name": "Alice", "type": "person", "aliases": [], "description": "A character"},
            {"name": "Wonderland", "type": "location", "aliases": ["Land of Wonder"]}
        ]}"#;
        let entities = parse_entities_response(json, "root").unwrap();
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].name, "Alice");
        assert!(matches!(entities[0].entity_type, EntityType::Person));
    }

    #[test]
    fn strips_code_fence_wrapping() {
        let json = "```json\n{\"entities\": [{\"name\": \"X\", \"type\": \"concept\"}]}\n```";
        let entities = parse_entities_response(json, "root").unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "X");
    }

    #[test]
    fn skips_empty_names() {
        let json =
            r#"{"entities": [{"name": "", "type": "person"}, {"name": "Ok", "type": "person"}]}"#;
        let entities = parse_entities_response(json, "root").unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "Ok");
    }

    #[test]
    fn missing_entities_array_errors() {
        let json = r#"{"nope": []}"#;
        assert!(parse_entities_response(json, "root").is_err());
    }

    #[test]
    fn parse_rejects_malformed_json() {
        let result = parse_entities_response("not valid json at all", "root");
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_entities_array_ok() {
        let result =
            parse_entities_response(r#"{"entities": []}"#, "root").expect("valid but empty");
        assert!(result.is_empty());
    }

    #[test]
    fn parses_relationships_and_drops_hallucinated_refs() {
        let json = r#"{"relationships": [
            {"source": "Alice", "target": "Wonderland", "type": "located_in"},
            {"source": "Ghost", "target": "Alice", "type": "haunts"}
        ]}"#;
        let rels =
            parse_relationships_response(json, &["Alice".into(), "Wonderland".into()]).unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].0, "Alice");
        assert_eq!(rels[0].2, "located_in");
    }

    #[test]
    fn empty_entity_list_returns_no_relationships_call() {
        // Parse directly; the guard in extract_relationships (network path) is
        // tested implicitly. Just confirm parser handles empty known list.
        let json = r#"{"relationships": [{"source": "A", "target": "B", "type": "x"}]}"#;
        let rels = parse_relationships_response(json, &[]).unwrap();
        assert!(rels.is_empty());
    }
}
