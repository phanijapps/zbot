//! `Procedure` domain type.

use serde::{Deserialize, Serialize};

/// A learned procedure pattern with execution statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Procedure {
    pub id: String,
    pub agent_id: String,
    pub ward_id: Option<String>,
    pub name: String,
    pub description: String,
    pub trigger_pattern: Option<String>,
    pub steps: String,
    pub parameters: Option<String>,
    pub success_count: i32,
    pub failure_count: i32,
    pub avg_duration_ms: Option<i64>,
    pub avg_token_cost: Option<i64>,
    pub last_used: Option<String>,
    /// Raw f32 embedding. Always `None` when loaded from a backend that
    /// stores vectors out-of-row.
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
    pub created_at: String,
    pub updated_at: String,
}

/// Result of `ProcedureStore::get_procedure_summary_by_name`. Captures
/// only the dedup-relevant fields so callers don't pay for hydrating
/// the full row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureSummary {
    pub id: String,
    pub name: String,
    pub success_count: i32,
}

/// Request shape for `ProcedureStore::insert_pattern_procedure`.
/// Pre-built from the LLM's structured response by `PatternExtractor`.
/// Flat field set so backends materialise their canonical procedure
/// row without callers needing to know the full `Procedure` shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
