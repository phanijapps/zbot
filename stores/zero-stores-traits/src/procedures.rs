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

    // ---- Sleep-time pattern extraction (Phase D4) ----------------------

    /// Look up a procedure by `(agent_id, name)`. Returns just the
    /// dedup-relevant fields (id + success_count) so callers don't pay
    /// for hydrating the full row. Used by `PatternExtractor` to skip
    /// candidates whose name is already locked-in by a successful
    /// existing procedure. Default: not found.
    async fn get_procedure_summary_by_name(
        &self,
        _agent_id: &str,
        _name: &str,
    ) -> Result<Option<ProcedureSummary>, String> {
        Ok(None)
    }

    /// Insert a synthesised procedure pattern. Pre-built from the
    /// LLM's structured response by `PatternExtractor`. Returns the
    /// procedure id used. Default: no-op error so misuse is loud.
    async fn insert_pattern_procedure(
        &self,
        _req: PatternProcedureInsert,
    ) -> Result<String, String> {
        Err("insert_pattern_procedure not implemented for this store".to_string())
    }
}

/// Result of `get_procedure_summary_by_name`. Captures only the
/// dedup-relevant fields.
#[derive(Debug, Clone)]
pub struct ProcedureSummary {
    pub id: String,
    pub name: String,
    pub success_count: i32,
}

/// Request shape for `insert_pattern_procedure`. Flat field set so
/// the trait crate doesn't need the full `Procedure` type from
/// `zero-stores-sqlite`.
#[derive(Debug, Clone)]
pub struct PatternProcedureInsert {
    pub agent_id: String,
    pub ward_id: Option<String>,
    pub name: String,
    pub description: String,
    pub trigger_pattern: Option<String>,
    /// JSON-serialised `Vec<PatternStep>`.
    pub steps_json: String,
    /// JSON-serialised `Vec<String>`, or `None` if no parameters.
    pub parameters_json: Option<String>,
}
