// ============================================================================
// GATEWAY PROCEDURE STORE
// SQLite-backed implementation of the ProcedureStore trait.
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use zero_stores_domain::Procedure;
use zero_stores_traits::{ProcedureStats, ProcedureStore};

use crate::procedure_repository::ProcedureRepository;

pub struct GatewayProcedureStore {
    repo: Arc<ProcedureRepository>,
}

impl GatewayProcedureStore {
    pub fn new(repo: Arc<ProcedureRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ProcedureStore for GatewayProcedureStore {
    async fn list_by_ward(&self, ward_id: &str, limit: usize) -> Result<Vec<Value>, String> {
        let rows = self.repo.list_by_ward(ward_id, limit)?;
        rows.into_iter()
            .map(|p| serde_json::to_value(p).map_err(|e| e.to_string()))
            .collect()
    }

    async fn upsert_procedure(
        &self,
        procedure: Value,
        embedding: Option<Vec<f32>>,
    ) -> Result<(), String> {
        let mut typed: Procedure =
            serde_json::from_value(procedure).map_err(|e| format!("decode Procedure: {e}"))?;
        if embedding.is_some() {
            typed.embedding = embedding;
        }
        self.repo.upsert_procedure(&typed)
    }

    async fn search_procedures_by_similarity(
        &self,
        embedding: &[f32],
        agent_id: &str,
        ward_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        let scored = self
            .repo
            .search_by_similarity(embedding, agent_id, ward_id, limit)?;
        Ok(scored
            .into_iter()
            .map(|(p, score)| {
                serde_json::json!({
                    "procedure": p,
                    "score": score,
                })
            })
            .collect())
    }

    async fn increment_success(
        &self,
        id: &str,
        duration_ms: Option<i64>,
        token_cost: Option<i64>,
    ) -> Result<(), String> {
        self.repo.increment_success(id, duration_ms, token_cost)
    }

    async fn increment_failure(&self, id: &str) -> Result<(), String> {
        self.repo.increment_failure(id)
    }

    async fn procedure_stats(&self) -> Result<ProcedureStats, String> {
        // ProcedureRepository doesn't expose a global count; defer to default.
        Ok(ProcedureStats::default())
    }
}
