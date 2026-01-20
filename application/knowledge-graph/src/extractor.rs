//! # Entity Extractor
//!
//! Extracts entities and relationships from conversation messages.

use crate::error::{GraphError, GraphResult};
use crate::types::{Entity, EntityType, ExtractedKnowledge};
use regex::Regex;

/// Entity extractor for conversations
pub struct EntityExtractor {
    /// Agent ID for this extractor
    agent_id: String,
}

impl EntityExtractor {
    /// Create a new entity extractor
    pub fn new(agent_id: String) -> Self {
        Self { agent_id }
    }

    /// Extract entities and relationships from a message
    pub fn extract_from_message(&self, _role: &str, content: &str) -> GraphResult<ExtractedKnowledge> {
        let mut entities = Vec::new();
        let relationships = Vec::new();

        // Extract different entity types
        entities.extend(self.extract_people(content)?);
        entities.extend(self.extract_organizations(content)?);
        entities.extend(self.extract_locations(content)?);
        entities.extend(self.extract_tools(content)?);
        entities.extend(self.extract_projects(content)?);

        // Extract relationships (simplified for now)
        // Full relationship extraction would require NLP/LLM

        Ok(ExtractedKnowledge {
            entities,
            relationships,
        })
    }

    /// Extract person names (simple heuristic)
    fn extract_people(&self, content: &str) -> GraphResult<Vec<Entity>> {
        let mut entities = Vec::new();

        // Simple pattern: capitalized words that might be names
        // This is a basic implementation; real-world would use NLP
        let re = Regex::new(r"\b([A-Z][a-z]+)\s+([A-Z][a-z]+)\b").map_err(|e| {
            GraphError::Config(format!("Failed to compile regex: {}", e))
        })?;

        for caps in re.captures_iter(content) {
            if let Some(full_name) = caps.get(0) {
                let name = full_name.as_str().to_string();
                // Filter out common words
                if !is_common_word(&name) {
                    entities.push(Entity::new(
                        self.agent_id.clone(),
                        EntityType::Person,
                        name,
                    ));
                }
            }
        }

        Ok(entities)
    }

    /// Extract organizations (common tech companies, etc.)
    fn extract_organizations(&self, content: &str) -> GraphResult<Vec<Entity>> {
        let mut entities = Vec::new();

        // Common tech organizations
        let orgs = vec![
            "Google", "Microsoft", "Amazon", "Apple", "Meta", "OpenAI",
            "GitHub", "GitLab", "Stripe", "Shopify", "Twitter", "LinkedIn",
            "Facebook", "Instagram", "WhatsApp", "Telegram", "Discord",
            "React", "Vue", "Angular", "Svelte", "Next.js", "Vite",
        ];

        for org in orgs {
            if content.contains(org) {
                entities.push(Entity::new(
                    self.agent_id.clone(),
                    EntityType::Organization,
                    org.to_string(),
                ));
            }
        }

        Ok(entities)
    }

    /// Extract locations
    fn extract_locations(&self, content: &str) -> GraphResult<Vec<Entity>> {
        let mut entities = Vec::new();

        // Common locations
        let locations = vec![
            "San Francisco", "New York", "London", "Paris", "Berlin",
            "Tokyo", "Singapore", "Bangalore", "Seattle", "Boston",
        ];

        for location in locations {
            if content.contains(location) {
                entities.push(Entity::new(
                    self.agent_id.clone(),
                    EntityType::Location,
                    location.to_string(),
                ));
            }
        }

        Ok(entities)
    }

    /// Extract tools/technologies
    fn extract_tools(&self, content: &str) -> GraphResult<Vec<Entity>> {
        let mut entities = Vec::new();

        // Programming languages and frameworks
        let tools = vec![
            "Rust", "Python", "JavaScript", "TypeScript", "Go", "Java",
            "React", "Vue", "Angular", "Svelte", "Node.js", "Deno",
            "Docker", "Kubernetes", "AWS", "Azure", "GCP",
            "PostgreSQL", "MongoDB", "Redis", "SQLite",
            "Git", "GitHub", "GitLab", "VS Code",
        ];

        for tool in tools {
            if content.contains(tool) {
                entities.push(Entity::new(
                    self.agent_id.clone(),
                    EntityType::Tool,
                    tool.to_string(),
                ));
            }
        }

        Ok(entities)
    }

    /// Extract project names (capitalized phrases that might be projects)
    fn extract_projects(&self, content: &str) -> GraphResult<Vec<Entity>> {
        let mut entities = Vec::new();

        // Pattern: "Project X" or similar
        let re = Regex::new(r"[Pp]roject\s+([A-Z][a-zA-Z0-9]+)").map_err(|e| {
            GraphError::Config(format!("Failed to compile regex: {}", e))
        })?;

        for caps in re.captures_iter(content) {
            if let Some(project_name) = caps.get(1) {
                entities.push(Entity::new(
                    self.agent_id.clone(),
                    EntityType::Project,
                    format!("Project {}", project_name.as_str()),
                ));
            }
        }

        Ok(entities)
    }
}

/// Check if a word is too common to be an entity
fn is_common_word(word: &str) -> bool {
    let common = vec![
        "The", "This", "That", "These", "Those", "Is", "Are", "Was",
        "Were", "Be", "Been", "Being", "Have", "Has", "Had", "Do", "Does",
        "Will", "Would", "Could", "Should", "May", "Might", "Can", "Just",
        "Also", "Very", "More", "Some", "Such", "What", "Which", "Who",
        "When", "Where", "Why", "How", "All", "Each", "Every", "Both",
        "Few", "Many", "Much", "Own", "Same", "So", "Than", "Too", "Very",
    ];
    common.contains(&word)
}

/// LLM-powered entity extractor for more accurate extraction
pub struct LlmEntityExtractor {
    agent_id: String,
}

impl LlmEntityExtractor {
    pub fn new(agent_id: String) -> Self {
        Self { agent_id }
    }

    /// Extract entities using LLM
    pub async fn extract_from_message(
        &self,
        _role: &str,
        content: &str,
    ) -> GraphResult<ExtractedKnowledge> {
        // For now, fall back to rule-based extraction
        // LLM-based extraction would require async LLM calls
        let extractor = EntityExtractor::new(self.agent_id.clone());
        extractor.extract_from_message("user", content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_people() {
        let extractor = EntityExtractor::new("test_agent".to_string());
        let content = "I talked to John Smith and Mary Johnson about the project.";
        let knowledge = extractor.extract_from_message("user", content).unwrap();

        assert!(!knowledge.entities.is_empty());
        // Should find "John Smith" and "Mary Johnson"
    }

    #[test]
    fn test_extract_organizations() {
        let extractor = EntityExtractor::new("test_agent".to_string());
        let content = "We're using Google Cloud and AWS for infrastructure.";
        let knowledge = extractor.extract_from_message("user", content).unwrap();

        assert!(!knowledge.entities.is_empty());
    }

    #[test]
    fn test_entity_type_conversion() {
        assert_eq!(EntityType::from_str("person"), EntityType::Person);
        assert_eq!(EntityType::from_str("Person"), EntityType::Person);
        assert_eq!(EntityType::from_str("PERSON"), EntityType::Person);
        assert_eq!(EntityType::from_str("custom_type"), EntityType::Custom("custom_type".to_string()));
    }
}
