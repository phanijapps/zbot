//! HandoffWriter — engine + production LLM impl moved to gateway-memory.
//!
//! Moved to `gateway_memory::sleep::handoff_writer`: data types
//! (`HandoffEntry`, `HandoffInput`), the `HandoffLlm` trait, `should_inject`,
//! `read_handoff_block`, the `HANDOFF_*` constants, the production
//! `LlmHandoffWriter`, and `format_conversation_for_summary`.
//!
//! Stays here: the `HandoffWriter` struct, which depends on the concrete
//! `zero_stores_sqlite::ConversationRepository` (moving it would create a
//! crate-dep cycle).

pub use gateway_memory::sleep::handoff_writer::*;

use std::sync::Arc;

use agent_runtime::ChatMessage;
use chrono::Utc;
use zero_stores_sqlite::ConversationRepository;

/// Writes session handoff to the memory fact store.
pub struct HandoffWriter {
    llm: Arc<dyn HandoffLlm>,
    fact_store: Arc<dyn zero_stores::MemoryFactStore>,
    conversation_repo: Arc<ConversationRepository>,
}

impl HandoffWriter {
    pub fn new(
        llm: Arc<dyn HandoffLlm>,
        fact_store: Arc<dyn zero_stores::MemoryFactStore>,
        conversation_repo: Arc<ConversationRepository>,
    ) -> Self {
        Self {
            llm,
            fact_store,
            conversation_repo,
        }
    }

    /// Fire-and-forget entry point: loads last 50 messages then calls
    /// `write_with_messages`. All errors are logged at warn and swallowed.
    pub async fn write(&self, session_id: &str, agent_id: &str, ward_id: &str) {
        let messages_raw = match self
            .conversation_repo
            .get_session_conversation(session_id, 50)
        {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(session_id, "handoff: failed to load messages: {e}");
                return;
            }
        };
        let messages = self
            .conversation_repo
            .session_messages_to_chat_format(&messages_raw);
        if let Err(e) = self
            .write_with_messages(session_id, agent_id, ward_id, messages)
            .await
        {
            tracing::warn!(session_id, "handoff: write failed: {e}");
        }
    }

    /// Testable core: accepts pre-loaded messages; returns Err on LLM or store failure.
    pub async fn write_with_messages(
        &self,
        session_id: &str,
        agent_id: &str,
        ward_id: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<(), String> {
        let turns = messages.iter().filter(|m| m.role == "user").count() as u32;
        let correction_count = self
            .fact_store
            .get_facts_by_category(agent_id, "correction", 200)
            .await
            .unwrap_or_default()
            .len() as u32;
        let input = HandoffInput {
            messages,
            ward_id: ward_id.to_string(),
        };
        let summary = self.llm.summarize(&input).await?;
        let entry = HandoffEntry {
            summary,
            session_id: session_id.to_string(),
            completed_at: Utc::now().to_rfc3339(),
            ward_id: ward_id.to_string(),
            intent_key: format!("ctx.{session_id}.intent"),
            goal_count: 0,
            open_task_count: 0,
            correction_count,
            turns,
        };
        self.persist(session_id, &entry).await
    }

    async fn persist(&self, session_id: &str, entry: &HandoffEntry) -> Result<(), String> {
        let json = serde_json::to_string(entry).map_err(|e| format!("serialize entry: {e}"))?;
        self.fact_store
            .save_fact(
                HANDOFF_AGENT_SENTINEL,
                HANDOFF_CATEGORY,
                "handoff.latest",
                &json,
                1.0,
                Some(session_id),
            )
            .await
            .map_err(|e| format!("save handoff.latest: {e}"))?;
        self.fact_store
            .save_fact(
                HANDOFF_AGENT_SENTINEL,
                HANDOFF_CATEGORY,
                &format!("handoff.{session_id}"),
                &json,
                1.0,
                Some(session_id),
            )
            .await
            .map_err(|e| format!("save handoff.{session_id}: {e}"))?;
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;
    use tempfile::TempDir;
    use zero_stores_domain::MemoryFact;
    use zero_stores_sqlite::{ConversationRepository, DatabaseManager};

    // ---- Mock LLM ----

    struct MockLlm {
        response: Mutex<Result<String, String>>,
        calls: Mutex<u32>,
    }

    impl MockLlm {
        fn ok(summary: &str) -> Arc<Self> {
            Arc::new(Self {
                response: Mutex::new(Ok(summary.to_string())),
                calls: Mutex::new(0),
            })
        }
        fn err() -> Arc<Self> {
            Arc::new(Self {
                response: Mutex::new(Err("mock LLM error".to_string())),
                calls: Mutex::new(0),
            })
        }
        fn call_count(&self) -> u32 {
            *self.calls.lock().unwrap()
        }
    }

    #[async_trait]
    impl HandoffLlm for MockLlm {
        async fn summarize(&self, _input: &HandoffInput) -> Result<String, String> {
            *self.calls.lock().unwrap() += 1;
            self.response.lock().unwrap().clone()
        }
    }

    // ---- Mock MemoryFactStore ----

    #[derive(Default)]
    struct MockFactStore {
        facts: Mutex<HashMap<String, String>>,
        corrections: Mutex<Vec<String>>,
    }

    impl MockFactStore {
        fn new() -> Arc<Self> {
            Arc::new(Self::default())
        }
        fn get(&self, key: &str) -> Option<String> {
            self.facts.lock().unwrap().get(key).cloned()
        }
        fn contains(&self, key: &str) -> bool {
            self.facts.lock().unwrap().contains_key(key)
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
            category: &str,
            _limit: usize,
        ) -> Result<Vec<MemoryFact>, String> {
            if category != "correction" {
                return Ok(Vec::new());
            }
            let items = self.corrections.lock().unwrap().clone();
            Ok(items
                .into_iter()
                .enumerate()
                .map(|(i, content)| MemoryFact {
                    id: format!("corr-{i}"),
                    session_id: None,
                    agent_id: "".to_string(),
                    scope: "".to_string(),
                    ward_id: "".to_string(),
                    category: "correction".to_string(),
                    key: format!("correction.{i}"),
                    content,
                    confidence: 1.0,
                    mention_count: 1,
                    source_summary: None,
                    embedding: None,
                    contradicted_by: None,
                    created_at: "".to_string(),
                    updated_at: "".to_string(),
                    expires_at: None,
                    valid_from: None,
                    valid_until: None,
                    superseded_by: None,
                    pinned: false,
                    epistemic_class: None,
                    source_episode_id: None,
                    source_ref: None,
                })
                .collect())
        }
    }

    // ---- Harness ----

    fn make_conversation_repo(tmp: &TempDir) -> Arc<ConversationRepository> {
        use gateway_services::VaultPaths;
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        paths.ensure_dirs_exist().expect("ensure vault dirs");
        let db = Arc::new(DatabaseManager::new(paths).expect("db"));
        Arc::new(ConversationRepository::new(db))
    }

    fn make_writer(
        tmp: &TempDir,
        llm: Arc<dyn HandoffLlm>,
        store: Arc<dyn zero_stores::MemoryFactStore>,
    ) -> HandoffWriter {
        HandoffWriter::new(llm, store, make_conversation_repo(tmp))
    }

    fn sample_messages(n: usize) -> Vec<ChatMessage> {
        (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    ChatMessage::user(format!("user message {i}"))
                } else {
                    ChatMessage::assistant(format!("assistant reply {i}"))
                }
            })
            .collect()
    }

    // ---- Test 1: generates_summary_from_messages ----

    #[tokio::test]
    async fn generates_summary_from_messages() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = MockLlm::ok(
            "User explored memory limits. Wrote reflective spec. Left impl incomplete.",
        );
        let store = MockFactStore::new();
        let writer = make_writer(&tmp, llm.clone(), store.clone());

        let result = writer
            .write_with_messages("sess-abc", "agent-root", "test-ward", sample_messages(6))
            .await;

        assert!(result.is_ok(), "write_with_messages failed: {result:?}");
        assert_eq!(llm.call_count(), 1, "LLM should be called exactly once");

        let raw = store.get("handoff.latest").expect("handoff.latest missing");
        let entry: HandoffEntry = serde_json::from_str(&raw).unwrap();
        assert!(!entry.summary.is_empty(), "summary should be non-empty");
        assert_eq!(entry.session_id, "sess-abc");
        assert_eq!(entry.ward_id, "test-ward");
        assert_eq!(entry.turns, 3, "3 user messages in sample_messages(6)");
    }

    // ---- Test 2: writes_latest_and_archived_keys ----

    #[tokio::test]
    async fn writes_latest_and_archived_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let store = MockFactStore::new();
        let writer = make_writer(&tmp, MockLlm::ok("Session summary here."), store.clone());

        writer
            .write_with_messages("sess-xyz", "agent-root", "my-ward", sample_messages(4))
            .await
            .unwrap();

        assert!(store.contains("handoff.latest"), "handoff.latest missing");
        assert!(
            store.contains("handoff.sess-xyz"),
            "handoff.sess-xyz missing"
        );

        let latest: HandoffEntry =
            serde_json::from_str(&store.get("handoff.latest").unwrap()).unwrap();
        let archived: HandoffEntry =
            serde_json::from_str(&store.get("handoff.sess-xyz").unwrap()).unwrap();
        assert_eq!(latest.summary, archived.summary);
    }

    // ---- Test 3: failure_is_silent ----

    #[tokio::test]
    async fn failure_is_silent() {
        let tmp = tempfile::tempdir().unwrap();
        let store = MockFactStore::new();
        let writer = make_writer(&tmp, MockLlm::err(), store.clone());

        let result = writer
            .write_with_messages("sess-fail", "agent-root", "ward", sample_messages(2))
            .await;

        assert!(result.is_err(), "expected Err when LLM fails");
        assert!(
            !store.contains("handoff.latest"),
            "no fact should be written on LLM failure"
        );
    }

    // ---- Test 9: real correction_count ----

    #[tokio::test]
    async fn correction_count_reflects_real_count() {
        let tmp = tempfile::tempdir().unwrap();
        let store = MockFactStore::new();
        store
            .corrections
            .lock()
            .unwrap()
            .push("Use write_file not shell".to_string());
        store
            .corrections
            .lock()
            .unwrap()
            .push("Never CPU offload".to_string());

        let writer = make_writer(&tmp, MockLlm::ok("Summary."), store.clone());
        writer
            .write_with_messages("sess-c", "agent-root", "test-ward", sample_messages(4))
            .await
            .unwrap();

        let raw = store.get("handoff.latest").unwrap();
        let entry: HandoffEntry = serde_json::from_str(&raw).unwrap();
        assert_eq!(entry.correction_count, 2);
    }
}
