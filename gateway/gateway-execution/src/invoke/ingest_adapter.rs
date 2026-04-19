//! # Ingestion Adapter
//!
//! Bridges [`gateway_database::KgEpisodeRepository`] + [`IngestionQueue`] +
//! [`knowledge_graph::GraphStorage`] to [`agent_tools::IngestionAccess`].
//! Wired into the agent tool registry so the `ingest` tool can both
//! (a) enqueue text chunks for background LLM extraction, and
//! (b) bulk-upsert structured entities and relationships synchronously.

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

use agent_tools::{IngestionAccess, StructuredCounts, StructuredEntity, StructuredRelationship};
use chrono::Utc;
use gateway_database::KgEpisodeRepository;
use knowledge_graph::{
    Entity, EntityType, ExtractedKnowledge, GraphStorage, Relationship, RelationshipType,
};

use crate::ingest::{
    chunker::{chunk_text, ChunkOptions},
    IngestionQueue,
};

/// Adapter that implements [`IngestionAccess`] for both text and structured
/// ingestion paths.
pub struct IngestionAdapter {
    queue: Arc<IngestionQueue>,
    episode_repo: Arc<KgEpisodeRepository>,
    graph: Arc<GraphStorage>,
}

impl IngestionAdapter {
    pub fn new(
        queue: Arc<IngestionQueue>,
        episode_repo: Arc<KgEpisodeRepository>,
        graph: Arc<GraphStorage>,
    ) -> Self {
        Self {
            queue,
            episode_repo,
            graph,
        }
    }
}

#[async_trait]
impl IngestionAccess for IngestionAdapter {
    async fn enqueue(
        &self,
        source_id: &str,
        source_type: &str,
        text: &str,
        session_id: Option<&str>,
        agent_id: &str,
    ) -> std::result::Result<(String, usize), String> {
        let chunks = chunk_text(text, ChunkOptions::default());
        let mut enqueued = 0usize;
        for chunk in &chunks {
            let source_ref = format!("{}#chunk-{}", source_id, chunk.index);
            let mut hasher = Sha256::new();
            hasher.update(chunk.text.as_bytes());
            let content_hash = format!("{:x}", hasher.finalize());
            let episode_id = self.episode_repo.upsert_pending(
                source_type,
                &source_ref,
                &content_hash,
                session_id,
                agent_id,
            )?;
            self.episode_repo.set_payload(&episode_id, &chunk.text)?;
            enqueued += 1;
        }
        self.queue.notify();
        Ok((source_id.to_string(), enqueued))
    }

    async fn ingest_structured(
        &self,
        agent_id: &str,
        entities: Vec<StructuredEntity>,
        relationships: Vec<StructuredRelationship>,
    ) -> std::result::Result<StructuredCounts, String> {
        let entity_count = entities.len();
        let relationship_count = relationships.len();
        let agent_id_owned = agent_id.to_string();
        let graph = self.graph.clone();

        // GraphStorage is sync; drop into spawn_blocking so we don't stall the
        // tokio reactor during the INSERT..ON CONFLICT sequence.
        tokio::task::spawn_blocking(move || -> std::result::Result<(), String> {
            let knowledge = build_knowledge(&agent_id_owned, entities, relationships);
            graph
                .store_knowledge(&agent_id_owned, knowledge)
                .map_err(|e| format!("store_knowledge: {e}"))
        })
        .await
        .map_err(|e| format!("ingest_structured join: {e}"))??;

        Ok(StructuredCounts {
            entities_upserted: entity_count,
            relationships_upserted: relationship_count,
        })
    }
}

/// Map the generic agent-tools shapes onto `knowledge_graph::ExtractedKnowledge`.
fn build_knowledge(
    agent_id: &str,
    entities: Vec<StructuredEntity>,
    relationships: Vec<StructuredRelationship>,
) -> ExtractedKnowledge {
    let now = Utc::now();

    let kg_entities: Vec<Entity> = entities
        .into_iter()
        .map(|e| {
            let mut props: HashMap<String, serde_json::Value> = HashMap::new();
            for (k, v) in e.properties {
                props.insert(k, v);
            }
            Entity {
                id: e.id,
                agent_id: agent_id.to_string(),
                entity_type: EntityType::from_str(&e.entity_type),
                name: e.name,
                properties: props,
                first_seen_at: now,
                last_seen_at: now,
                mention_count: 1,
                name_embedding: None,
            }
        })
        .collect();

    let kg_relationships: Vec<Relationship> = relationships
        .into_iter()
        .map(|r| {
            let mut props: HashMap<String, serde_json::Value> = HashMap::new();
            for (k, v) in r.properties {
                props.insert(k, v);
            }
            Relationship {
                id: format!("rel-{}", uuid::Uuid::new_v4()),
                agent_id: agent_id.to_string(),
                source_entity_id: r.from,
                target_entity_id: r.to,
                relationship_type: RelationshipType::from_str(&r.rel_type),
                properties: props,
                first_seen_at: now,
                last_seen_at: now,
                mention_count: 1,
            }
        })
        .collect();

    ExtractedKnowledge {
        entities: kg_entities,
        relationships: kg_relationships,
    }
}
