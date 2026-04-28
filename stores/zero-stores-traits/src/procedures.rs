//! `ProcedureStore` trait — backend-agnostic interface for learned procedures.

use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcedureStats {
    pub total: i64,
}

#[async_trait]
pub trait ProcedureStore: Send + Sync {
    /// List procedures for a ward, capped at `limit`. Default empty.
    async fn list_by_ward(&self, _ward_id: &str, _limit: usize) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }

    /// Upsert a procedure. The `procedure` Value carries the full
    /// `Procedure` shape; `embedding` is optional.
    async fn upsert_procedure(
        &self,
        _procedure: Value,
        _embedding: Option<Vec<f32>>,
    ) -> Result<(), String> {
        Err("upsert_procedure not implemented for this store".to_string())
    }

    /// Vector-similarity search scoped to an agent (and optional ward).
    /// Each row carries a `procedure` field + `score` (cosine ∈ [0, 1]).
    async fn search_procedures_by_similarity(
        &self,
        _embedding: &[f32],
        _agent_id: &str,
        _ward_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }

    /// Bump success/failure counts after a run. No-op default.
    async fn increment_success(
        &self,
        _id: &str,
        _duration_ms: Option<i64>,
        _token_cost: Option<i64>,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn increment_failure(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn procedure_stats(&self) -> Result<ProcedureStats, String> {
        Ok(ProcedureStats::default())
    }
}
