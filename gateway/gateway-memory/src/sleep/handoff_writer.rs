//! Session handoff types — `HandoffEntry`, `HandoffInput`, the `HandoffLlm`
//! trait, `should_inject`, and `read_handoff_block` injection helper.
//!
//! The `HandoffWriter` struct (which depends on the concrete
//! `zero_stores_sqlite::ConversationRepository`) stays in `gateway-execution`
//! to avoid a circular crate dependency
//! (`gateway-memory` → `zero-stores-sqlite` → `gateway-services` → `gateway-memory`).

use std::sync::Arc;

use agent_runtime::ChatMessage;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const HANDOFF_MAX_AGE_DAYS: i64 = 7;

pub const HANDOFF_AGENT_SENTINEL: &str = "__handoff__";
pub const HANDOFF_CATEGORY: &str = "handoff";
pub const HANDOFF_SCOPE: &str = "global";
pub const HANDOFF_WARD: &str = "__global__";

/// Stored JSON shape for each `handoff.*` fact in the DB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffEntry {
    pub summary: String,
    pub session_id: String,
    pub completed_at: String,
    pub ward_id: String,
    pub intent_key: String,
    pub goal_count: u32,
    pub open_task_count: u32,
    pub correction_count: u32,
    pub turns: u32,
}

/// Input passed to the LLM for summarisation.
#[derive(Debug, Clone)]
pub struct HandoffInput {
    pub messages: Vec<ChatMessage>,
    pub ward_id: String,
}

/// Mockable LLM interface for generating 3-5 sentence handoff summaries.
#[async_trait]
pub trait HandoffLlm: Send + Sync {
    async fn summarize(&self, input: &HandoffInput) -> Result<String, String>;
}

/// Returns false if the entry is older than `HANDOFF_MAX_AGE_DAYS` or unparseable.
pub fn should_inject(entry: &HandoffEntry) -> bool {
    let Ok(completed_at) = entry.completed_at.parse::<DateTime<Utc>>() else {
        return false;
    };
    (Utc::now() - completed_at).num_days() <= HANDOFF_MAX_AGE_DAYS
}

/// Reads `handoff.latest` from the fact store.
/// Returns `None` if absent, unparseable, older than `HANDOFF_MAX_AGE_DAYS`,
/// or if `current_ward` is `Some` and doesn't match the entry's ward.
pub async fn read_handoff_block(
    fact_store: &Arc<dyn zero_stores::MemoryFactStore>,
    current_ward: Option<&str>,
) -> Option<String> {
    let fact = fact_store
        .get_fact_by_key(
            HANDOFF_AGENT_SENTINEL,
            HANDOFF_SCOPE,
            HANDOFF_WARD,
            "handoff.latest",
        )
        .await
        .ok()??;
    let entry: HandoffEntry = serde_json::from_str(&fact.content).ok()?;
    let completed_at = entry.completed_at.parse::<DateTime<Utc>>().ok()?;
    if (Utc::now() - completed_at).num_days() > HANDOFF_MAX_AGE_DAYS {
        return None;
    }
    // Ward filter: skip if the new session is a different ward (None = accept any)
    if let Some(cw) = current_ward {
        if !entry.ward_id.is_empty() && entry.ward_id != cw {
            return None;
        }
    }
    let date_str = completed_at.format("%Y-%m-%d").to_string();
    Some(format!(
        "## Last Session  ({date} · ward: {ward} · {turns} turns)\n\
         {summary}\n\n\
         Corrections active: {corrections} · Goals: {goals}\n\
         Full context: memory(action=get_fact, key=handoff.{sid})\n\
         Last intent:  memory(action=get_fact, key={intent_key})",
        date = date_str,
        ward = entry.ward_id,
        turns = entry.turns,
        summary = entry.summary,
        corrections = entry.correction_count,
        goals = entry.goal_count,
        sid = entry.session_id,
        intent_key = entry.intent_key,
    ))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use zero_stores_domain::MemoryFact;

    // ---- Mock MemoryFactStore ----

    #[derive(Default)]
    struct MockFactStore {
        facts: Mutex<HashMap<String, String>>,
    }

    impl MockFactStore {
        fn new() -> Arc<Self> {
            Arc::new(Self::default())
        }
    }

    #[async_trait]
    impl zero_stores::MemoryFactStore for MockFactStore {
        async fn save_fact(
            &self,
            _agent_id: &str,
            _category: &str,
            key: &str,
            content: &str,
            _confidence: f64,
            _session_id: Option<&str>,
        ) -> Result<serde_json::Value, String> {
            self.facts
                .lock()
                .unwrap()
                .insert(key.to_string(), content.to_string());
            Ok(serde_json::json!({"ok": true}))
        }

        async fn recall_facts(
            &self,
            _agent_id: &str,
            _query: &str,
            _limit: usize,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!([]))
        }

        async fn get_fact_by_key(
            &self,
            _agent_id: &str,
            _scope: &str,
            _ward_id: &str,
            key: &str,
        ) -> Result<Option<MemoryFact>, String> {
            let content = self.facts.lock().unwrap().get(key).cloned();
            Ok(content.map(|c| MemoryFact {
                id: "mock".to_string(),
                session_id: None,
                agent_id: HANDOFF_AGENT_SENTINEL.to_string(),
                scope: HANDOFF_SCOPE.to_string(),
                category: HANDOFF_CATEGORY.to_string(),
                key: key.to_string(),
                content: c,
                confidence: 1.0,
                mention_count: 1,
                source_summary: None,
                embedding: None,
                ward_id: HANDOFF_WARD.to_string(),
                contradicted_by: None,
                created_at: String::new(),
                updated_at: String::new(),
                expires_at: None,
                valid_from: None,
                valid_until: None,
                superseded_by: None,
                pinned: false,
                epistemic_class: None,
                source_episode_id: None,
                source_ref: None,
            }))
        }

        async fn get_facts_by_category(
            &self,
            _agent_id: &str,
            _category: &str,
            _limit: usize,
        ) -> Result<Vec<MemoryFact>, String> {
            Ok(Vec::new())
        }
    }

    // ---- Test 4: stale_handoff_excluded ----

    #[test]
    fn stale_handoff_excluded() {
        let stale = HandoffEntry {
            summary: "old summary".to_string(),
            session_id: "sess-old".to_string(),
            completed_at: (Utc::now() - chrono::Duration::days(8)).to_rfc3339(),
            ward_id: "ward".to_string(),
            intent_key: "ctx.sess-old.intent".to_string(),
            goal_count: 0,
            open_task_count: 0,
            correction_count: 0,
            turns: 5,
        };
        assert!(
            !should_inject(&stale),
            "8-day-old handoff should be excluded"
        );

        let fresh = HandoffEntry {
            completed_at: (Utc::now() - chrono::Duration::days(6)).to_rfc3339(),
            ..stale.clone()
        };
        assert!(
            should_inject(&fresh),
            "6-day-old handoff should be injected"
        );
    }

    // ---- Test 5: read_handoff_block returns formatted block ----

    #[tokio::test]
    async fn read_handoff_block_returns_formatted_block() {
        let store = MockFactStore::new();
        let entry = HandoffEntry {
            summary: "User explored memory. Spec written.".to_string(),
            session_id: "sess-abc".to_string(),
            completed_at: Utc::now().to_rfc3339(),
            ward_id: "memory-ward".to_string(),
            intent_key: "ctx.sess-abc.intent".to_string(),
            goal_count: 2,
            open_task_count: 0,
            correction_count: 3,
            turns: 10,
        };
        store.facts.lock().unwrap().insert(
            "handoff.latest".to_string(),
            serde_json::to_string(&entry).unwrap(),
        );

        let store: Arc<dyn zero_stores::MemoryFactStore> = store;
        let block = read_handoff_block(&store, None)
            .await
            .expect("should return a block");
        assert!(block.contains("## Last Session"));
        assert!(block.contains("User explored memory"));
        assert!(block.contains("memory-ward"));
        assert!(block.contains("Corrections active: 3"));
        assert!(block.contains("Goals: 2"));
        assert!(block.contains("handoff.sess-abc"));
    }

    // ---- Test 6: read_handoff_block returns None for stale entry ----

    #[tokio::test]
    async fn read_handoff_block_returns_none_when_stale() {
        let store = MockFactStore::new();
        let entry = HandoffEntry {
            summary: "old".to_string(),
            session_id: "sess-old".to_string(),
            completed_at: (Utc::now() - chrono::Duration::days(10)).to_rfc3339(),
            ward_id: "ward".to_string(),
            intent_key: "ctx.sess-old.intent".to_string(),
            goal_count: 0,
            open_task_count: 0,
            correction_count: 0,
            turns: 5,
        };
        store.facts.lock().unwrap().insert(
            "handoff.latest".to_string(),
            serde_json::to_string(&entry).unwrap(),
        );

        let store: Arc<dyn zero_stores::MemoryFactStore> = store;
        assert!(read_handoff_block(&store, None).await.is_none());
    }

    // ---- Test 7: read_handoff_block returns None when absent ----

    #[tokio::test]
    async fn read_handoff_block_returns_none_when_absent() {
        let store: Arc<dyn zero_stores::MemoryFactStore> = MockFactStore::new();
        assert!(read_handoff_block(&store, None).await.is_none());
    }

    // ---- Test 8: ward filter ----

    #[tokio::test]
    async fn read_handoff_block_returns_none_when_ward_differs() {
        let store = MockFactStore::new();
        let entry = HandoffEntry {
            summary: "Done something.".to_string(),
            session_id: "sess-1".to_string(),
            completed_at: Utc::now().to_rfc3339(),
            ward_id: "coding-ward".to_string(),
            intent_key: "ctx.sess-1.intent".to_string(),
            goal_count: 0,
            open_task_count: 0,
            correction_count: 0,
            turns: 3,
        };
        store.facts.lock().unwrap().insert(
            "handoff.latest".to_string(),
            serde_json::to_string(&entry).unwrap(),
        );
        let store: Arc<dyn zero_stores::MemoryFactStore> = store;
        assert!(read_handoff_block(&store, Some("research-ward"))
            .await
            .is_none());
        assert!(read_handoff_block(&store, Some("coding-ward"))
            .await
            .is_some());
        assert!(read_handoff_block(&store, None).await.is_some());
    }
}
