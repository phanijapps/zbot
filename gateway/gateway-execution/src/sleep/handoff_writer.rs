//! HandoffWriter — writes session handoff to the memory fact store on completion,
//! and provides `read_handoff_block` for injection at session start.

use std::sync::Arc;

use agent_runtime::ChatMessage;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zero_stores_sqlite::ConversationRepository;

pub const HANDOFF_MAX_AGE_DAYS: i64 = 7;

pub(crate) const HANDOFF_AGENT_SENTINEL: &str = "__handoff__";
const HANDOFF_CATEGORY: &str = "handoff";
pub(crate) const HANDOFF_SCOPE: &str = "global";
pub(crate) const HANDOFF_WARD: &str = "__global__";

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
// Production LLM impl
// ============================================================================

/// Production `HandoffLlm` wired to the default configured provider.
pub struct LlmHandoffWriter {
    provider_service: Arc<gateway_services::ProviderService>,
}

impl LlmHandoffWriter {
    pub fn new(provider_service: Arc<gateway_services::ProviderService>) -> Self {
        Self { provider_service }
    }

    fn build_client(&self) -> Result<Arc<dyn agent_runtime::llm::LlmClient>, String> {
        use agent_runtime::llm::{openai::OpenAiClient, LlmConfig};
        let providers = self
            .provider_service
            .list()
            .map_err(|e| format!("list providers: {e}"))?;
        let provider = providers
            .iter()
            .find(|p| p.is_default)
            .or_else(|| providers.first())
            .ok_or_else(|| "no providers configured".to_string())?;
        let model = provider.default_model().to_string();
        let provider_id = provider.id.clone().unwrap_or_else(|| "default".to_string());
        let config = LlmConfig::new(
            provider.base_url.clone(),
            provider.api_key.clone(),
            model,
            provider_id,
        )
        .with_temperature(0.2)
        .with_max_tokens(256);
        let client = OpenAiClient::new(config).map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn agent_runtime::llm::LlmClient>)
    }
}

/// Formats conversation messages for the LLM summary prompt.
/// Includes user, assistant, and tool messages (up to 40).
/// Assistant messages with tool calls are annotated with `[called: name1, name2]`.
pub(crate) fn format_conversation_for_summary(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant" || m.role == "tool")
        .take(40)
        .map(|m| {
            if m.role == "assistant" {
                if let Some(calls) = &m.tool_calls {
                    let names: Vec<&str> = calls.iter().map(|c| c.name.as_str()).collect();
                    if !names.is_empty() {
                        let text = m.text_content();
                        return if text.is_empty() {
                            format!("assistant [called: {}]", names.join(", "))
                        } else {
                            format!("assistant [called: {}]: {}", names.join(", "), text)
                        };
                    }
                }
            }
            format!("{}: {}", m.role, m.text_content())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[async_trait]
impl HandoffLlm for LlmHandoffWriter {
    async fn summarize(&self, input: &HandoffInput) -> Result<String, String> {
        let client = self.build_client()?;
        let conversation = format_conversation_for_summary(&input.messages);
        let prompt = format!(
            "Summarize this conversation in 3-5 sentences. Cover:\n\
             - What was accomplished\n\
             - What was left incomplete or in progress\n\
             - What the user seemed most focused on or interested in next\n\n\
             Be specific. Do not use filler phrases like 'the user and assistant discussed'.\n\
             Use past tense. Write for an agent reading this at the start of the NEXT session.\n\n\
             Ward: {ward}\n\n\
             Conversation:\n{conversation}",
            ward = input.ward_id,
            conversation = conversation,
        );
        let messages = vec![
            ChatMessage::system(
                "You are a concise session summarizer. Return only the summary text, no JSON."
                    .to_string(),
            ),
            ChatMessage::user(prompt),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("LLM call: {e}"))?;
        Ok(response.content)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
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

    // ---- Test 10: format_conversation_for_summary includes tool call annotations ----

    #[test]
    fn format_conversation_includes_tool_annotations() {
        use agent_runtime::ToolCall;

        let mut assistant_msg = ChatMessage::assistant("Let me check.".to_string());
        assistant_msg.tool_calls = Some(vec![ToolCall {
            id: "tc-1".to_string(),
            name: "memory".to_string(),
            arguments: serde_json::json!({"action": "get_fact"}),
        }]);

        let messages = vec![
            ChatMessage::user("What do you know?".to_string()),
            assistant_msg,
            ChatMessage::system("memory result".to_string()), // system filtered out
        ];

        let text = format_conversation_for_summary(&messages);
        assert!(
            text.contains("[called: memory]"),
            "tool name missing: {text}"
        );
        assert!(
            !text.contains("memory result"),
            "system message should be excluded: {text}"
        );
        assert!(
            text.contains("What do you know"),
            "user message missing: {text}"
        );
    }
}
