//! Auxiliary store traits batched in Phase D5.
//!
//! Smaller surface areas than the main MemoryFactStore / KnowledgeGraphStore;
//! grouped here to keep the file count tractable while still giving each one
//! a clean trait. Each is sized to its actual usage in the runtime.

use async_trait::async_trait;
use serde_json::Value;

// ============================================================================
// GoalStore
// ============================================================================

/// Backend-agnostic interface for goals — agent intents with lifecycle.
#[async_trait]
pub trait GoalStore: Send + Sync {
    /// Get a goal by id.
    async fn get_goal(&self, _goal_id: &str) -> Result<Option<Value>, String> {
        Ok(None)
    }

    /// Active goals for an agent — used for intent boost in unified recall.
    async fn list_active_goals(&self, _agent_id: &str) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }

    /// Create a new goal. Returns the persisted id.
    async fn create_goal(&self, _goal: Value) -> Result<String, String> {
        Err("create_goal not implemented for this store".to_string())
    }

    /// Move a goal to a new state (active / blocked / satisfied / abandoned).
    async fn update_goal_state(&self, _goal_id: &str, _new_state: &str) -> Result<(), String> {
        Ok(())
    }
}

// ============================================================================
// RecallLogStore
// ============================================================================

/// Per-session log of which fact-keys were recalled. Drives predictive
/// recall (don't re-surface the same fact every turn).
#[async_trait]
pub trait RecallLogStore: Send + Sync {
    async fn log_recall(&self, _session_id: &str, _fact_key: &str) -> Result<(), String> {
        Ok(())
    }

    async fn get_keys_for_session(&self, _session_id: &str) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }

    async fn get_keys_for_sessions(&self, _session_ids: &[String]) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
}

// ============================================================================
// DistillationStore
// ============================================================================

/// Distillation run lifecycle: tracks which sessions have been distilled,
/// retry counts, and per-run statistics.
#[async_trait]
pub trait DistillationStore: Send + Sync {
    /// Insert a new distillation run row.
    async fn insert_run(&self, _run: Value) -> Result<(), String> {
        Err("insert_run not implemented for this store".to_string())
    }

    /// Get the run row for a session (if any).
    async fn get_run_by_session(&self, _session_id: &str) -> Result<Option<Value>, String> {
        Ok(None)
    }

    /// Bump the retry counter for a failed run.
    async fn update_retry(&self, _session_id: &str) -> Result<(), String> {
        Ok(())
    }

    /// Mark a run successful. `summary` is a free-form post-mortem string.
    async fn update_success(
        &self,
        _session_id: &str,
        _summary: Option<String>,
    ) -> Result<(), String> {
        Ok(())
    }
}
