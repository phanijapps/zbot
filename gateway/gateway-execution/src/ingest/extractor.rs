//! Two-pass LLM extractor: pass 1 entities, pass 2 relationships conditioned
//! on the entity list. Concrete LLM-backed impl lands in Tasks 5 and 6.
//! `NoopExtractor` provides a test-friendly no-op for queue-level tests.

use agent_runtime::llm::{ChatMessage, LlmClient};
use async_trait::async_trait;
use gateway_database::KgEpisode;
use knowledge_graph::{Entity, EntityType, GraphStorage};
use std::sync::Arc;
use tokio::sync::Mutex;

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
// Task 6 wires these fields into `Extractor::process`; allow(dead_code) is
// scoped to the struct until that lands in the same PR.
#[allow(dead_code)]
pub struct LlmExtractor {
    client: Arc<dyn LlmClient>,
    agent_id: String,
}

impl LlmExtractor {
    pub fn new(client: Arc<dyn LlmClient>, agent_id: String) -> Self {
        Self { client, agent_id }
    }

    #[allow(dead_code)] // used by Extractor::process in Task 6
    async fn extract_entities(&self, chunk_text: &str) -> Result<Vec<Entity>, String> {
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

        let response = self
            .client
            .chat(messages, None)
            .await
            .map_err(|e| format!("llm entity pass failed: {e}"))?;

        parse_entities_response(&response.content, &self.agent_id)
    }
}

/// Parse the LLM's JSON response into typed Entity values.
/// Strips optional code-fence wrapping. Silently skips malformed items.
#[allow(dead_code)] // used by Extractor::process in Task 6
fn parse_entities_response(content: &str, agent_id: &str) -> Result<Vec<Entity>, String> {
    let stripped = strip_code_fence(content);
    let raw: serde_json::Value = serde_json::from_str(stripped).map_err(|e| {
        let preview: String = content.chars().take(200).collect();
        format!("parse entities: {e} (preview: {preview})")
    })?;

    let arr = raw
        .get("entities")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing 'entities' array in LLM response".to_string())?;

    let mut out = Vec::new();
    for item in arr {
        let name = match item.get("name").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => continue,
        };
        let type_str = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("concept");
        let ty = EntityType::from_str(type_str);
        let mut entity = Entity::new(agent_id.to_string(), ty, name);
        if let Some(desc) = item.get("description").and_then(|v| v.as_str()) {
            entity.properties.insert(
                "description".to_string(),
                serde_json::Value::String(desc.to_string()),
            );
        }
        out.push(entity);
    }
    Ok(out)
}

#[allow(dead_code)] // used by parse_entities_response
fn strip_code_fence(s: &str) -> &str {
    let t = s.trim();
    let t = t
        .strip_prefix("```json")
        .or_else(|| t.strip_prefix("```"))
        .unwrap_or(t)
        .trim();
    t.strip_suffix("```").unwrap_or(t).trim()
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
}
