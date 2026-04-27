use knowledge_graph::types::{Entity, Relationship};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedKnowledge {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}
