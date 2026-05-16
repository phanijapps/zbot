//! Self-RAG retrieval gate.
//!
//! Before hybrid search runs, a small LLM call decides whether retrieval is
//! needed and reformulates the query if it is. Reduces dilution on multi-topic
//! queries; saves latency when context already suffices.
//!
//! Failure-safe: any LLM error or malformed response falls back to passing
//! the raw input through as `Direct(raw_input)` — gate failure never blocks
//! recall.
//!
//! Borrowed from Du's agent-memory survey (arxiv 2603.07670, Self-RAG section).

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::util::parse_llm_json;
use crate::{LlmClientConfig, MemoryLlmFactory, QueryGateConfig};
use agent_runtime::llm::ChatMessage;

/// Decision returned by the gate. Drives whether (and how) hybrid search runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetrievalDecision {
    /// Skip hybrid search entirely. Always-inject corrections still fire
    /// from the separate bootstrap path.
    Skip,
    /// Run hybrid search using this reformulated query.
    Direct(String),
    /// Run hybrid search per subquery, then dedup-merge by fact key.
    Split(Vec<String>),
}

/// Raw JSON shape the LLM is asked to return.
///
/// Public so test mocks of [`QueryGateLlm`] can construct it. Not part of the
/// gate's outward contract — callers see only [`RetrievalDecision`].
#[derive(Debug, Clone, Deserialize)]
pub struct GateResponse {
    pub decision: String,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub subqueries: Option<Vec<String>>,
}

/// LLM abstraction so tests can inject a mock. Returns the raw `GateResponse`
/// shape so validation (decision-string + field consistency) lives in one
/// place inside `QueryGate::reformulate`.
#[async_trait]
pub trait QueryGateLlm: Send + Sync {
    async fn reformulate(&self, raw_input: &str) -> Result<GateResponse, String>;
}

/// Self-RAG retrieval gate. Construct via [`QueryGate::new`].
pub struct QueryGate {
    llm: Arc<dyn QueryGateLlm>,
    config: QueryGateConfig,
}

impl QueryGate {
    /// Build a new gate with the given LLM and config.
    pub fn new(llm: Arc<dyn QueryGateLlm>, config: QueryGateConfig) -> Self {
        Self { llm, config }
    }

    /// Decide how to retrieve memory for `raw_input`.
    ///
    /// - Returns `Direct(raw_input)` if the gate is disabled.
    /// - Calls the LLM and parses the response.
    /// - Validates the decision; any error → `Direct(raw_input)` (failure-safe).
    pub async fn reformulate(&self, raw_input: &str) -> RetrievalDecision {
        if !self.config.enabled {
            return RetrievalDecision::Direct(raw_input.to_string());
        }
        match self.llm.reformulate(raw_input).await {
            Ok(resp) => validate(resp, raw_input, &self.config),
            Err(e) => {
                tracing::warn!("query gate LLM error — falling back to direct: {e}");
                RetrievalDecision::Direct(raw_input.to_string())
            }
        }
    }
}

/// Validate the LLM's decision against the config. Returns `Direct(raw)` for
/// any structural problem (missing field, unknown decision string, empty
/// subqueries, etc.). Truncates over-long subqueries / over-many subqueries
/// to the config limits.
fn validate(resp: GateResponse, raw_input: &str, cfg: &QueryGateConfig) -> RetrievalDecision {
    let direct_fallback = || RetrievalDecision::Direct(raw_input.to_string());
    match resp.decision.as_str() {
        "skip" => RetrievalDecision::Skip,
        "direct" => match resp.query {
            Some(q) if !q.trim().is_empty() => {
                RetrievalDecision::Direct(truncate(q.trim(), cfg.max_subquery_len))
            }
            _ => direct_fallback(),
        },
        "split" => {
            let raw_subs = resp.subqueries.unwrap_or_default();
            let cleaned: Vec<String> = raw_subs
                .into_iter()
                .map(|s| truncate(s.trim(), cfg.max_subquery_len))
                .filter(|s| !s.is_empty())
                .take(cfg.max_subqueries)
                .collect();
            if cleaned.len() < 2 {
                // Split is only meaningful for 2+ topics — otherwise treat as direct.
                direct_fallback()
            } else {
                RetrievalDecision::Split(cleaned)
            }
        }
        _ => direct_fallback(),
    }
}

/// Byte-wise truncation that respects UTF-8 char boundaries.
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        return s.to_string();
    }
    s.chars().take(max_len).collect()
}

// ============================================================================
// LLM-backed implementation
// ============================================================================

const GATE_SYSTEM_PROMPT: &str = "You decide how to retrieve memory for an agent.\n\
\n\
Given the user's input, return a JSON object with one of three decisions:\n\
- \"skip\" — input is small talk, a thanks, or already self-contained; no recall needed\n\
- \"direct\" — input is a clear single-topic question; reformulate into a clean retrieval query\n\
- \"split\" — input mixes 2+ distinct topics; split into focused subqueries (max 4)\n\
\n\
Output ONLY valid JSON. No prose.\n\
\n\
Examples:\n\
Input: \"thanks!\"\n\
Output: {\"decision\": \"skip\"}\n\
\n\
Input: \"what's my preferred coffee order\"\n\
Output: {\"decision\": \"direct\", \"query\": \"user preferred coffee order\"}\n\
\n\
Input: \"where do I live, what are my dietary restrictions, and which restaurants did I save\"\n\
Output: {\"decision\": \"split\", \"subqueries\": [\"user location address\", \"user dietary restrictions allergies\", \"saved restaurants\"]}";

/// Production `QueryGateLlm` wired to the injected `MemoryLlmFactory`.
///
/// Note on `model_id`: the current `MemoryLlmFactory` trait doesn't expose a
/// per-call model override — it always uses the default-provider model
/// (same path the corrections abstractor and conflict resolver use).
/// `QueryGateConfig::model_id` is parsed and held for the future when the
/// factory grows that knob, but it has no effect today.
pub struct LlmQueryGate {
    factory: Arc<dyn MemoryLlmFactory>,
}

impl LlmQueryGate {
    pub fn new(factory: Arc<dyn MemoryLlmFactory>) -> Self {
        Self { factory }
    }
}

#[async_trait]
impl QueryGateLlm for LlmQueryGate {
    async fn reformulate(&self, raw_input: &str) -> Result<GateResponse, String> {
        // Small token budget: the JSON envelope is tiny (~80 tokens worst case).
        let client = self
            .factory
            .build_client(LlmClientConfig::new(0.0, 256))
            .await?;
        let messages = vec![
            ChatMessage::system(GATE_SYSTEM_PROMPT.to_string()),
            ChatMessage::user(raw_input.to_string()),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("query gate LLM call: {e}"))?;
        parse_llm_json::<GateResponse>(&response.content)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mock LLM whose response is either pre-canned or a forced error.
    struct MockGateLlm {
        result: Mutex<Result<GateResponse, String>>,
    }

    impl MockGateLlm {
        fn ok(resp: GateResponse) -> Self {
            Self {
                result: Mutex::new(Ok(resp)),
            }
        }

        fn err(msg: &str) -> Self {
            Self {
                result: Mutex::new(Err(msg.to_string())),
            }
        }
    }

    #[async_trait]
    impl QueryGateLlm for MockGateLlm {
        async fn reformulate(&self, _raw: &str) -> Result<GateResponse, String> {
            // Clone for repeatability (tests only call once but be safe).
            match &*self.result.lock().unwrap() {
                Ok(r) => Ok(GateResponse {
                    decision: r.decision.clone(),
                    query: r.query.clone(),
                    subqueries: r.subqueries.clone(),
                }),
                Err(e) => Err(e.clone()),
            }
        }
    }

    fn gate_with(llm: Arc<dyn QueryGateLlm>, mut cfg: QueryGateConfig) -> QueryGate {
        cfg.enabled = true;
        QueryGate::new(llm, cfg)
    }

    /// Test A — disabled returns Direct(raw).
    #[tokio::test]
    async fn disabled_returns_direct_raw() {
        let llm: Arc<dyn QueryGateLlm> = Arc::new(MockGateLlm::ok(GateResponse {
            decision: "skip".to_string(),
            query: None,
            subqueries: None,
        }));
        // Use default config — `enabled: false` — to verify disabled short-circuits.
        let gate = QueryGate::new(llm, QueryGateConfig::default());
        let out = gate.reformulate("anything").await;
        assert_eq!(out, RetrievalDecision::Direct("anything".to_string()));
    }

    /// Test B — valid Direct decision is returned verbatim.
    #[tokio::test]
    async fn valid_direct_decision_returned() {
        let llm: Arc<dyn QueryGateLlm> = Arc::new(MockGateLlm::ok(GateResponse {
            decision: "direct".to_string(),
            query: Some("user preferred coffee".to_string()),
            subqueries: None,
        }));
        let gate = gate_with(llm, QueryGateConfig::default());
        let out = gate.reformulate("what's my coffee").await;
        assert_eq!(
            out,
            RetrievalDecision::Direct("user preferred coffee".to_string())
        );
    }

    /// Test C — valid Split decision with 3 subqueries is returned verbatim.
    #[tokio::test]
    async fn valid_split_decision_returned() {
        let subs = vec![
            "user location address".to_string(),
            "user dietary restrictions allergies".to_string(),
            "saved restaurants".to_string(),
        ];
        let llm: Arc<dyn QueryGateLlm> = Arc::new(MockGateLlm::ok(GateResponse {
            decision: "split".to_string(),
            query: None,
            subqueries: Some(subs.clone()),
        }));
        let gate = gate_with(llm, QueryGateConfig::default());
        let out = gate.reformulate("multi-topic input").await;
        assert_eq!(out, RetrievalDecision::Split(subs));
    }

    /// Test D — valid Skip decision is returned verbatim.
    #[tokio::test]
    async fn valid_skip_decision_returned() {
        let llm: Arc<dyn QueryGateLlm> = Arc::new(MockGateLlm::ok(GateResponse {
            decision: "skip".to_string(),
            query: None,
            subqueries: None,
        }));
        let gate = gate_with(llm, QueryGateConfig::default());
        let out = gate.reformulate("thanks!").await;
        assert_eq!(out, RetrievalDecision::Skip);
    }

    /// Test E — LLM error falls back to Direct(raw).
    #[tokio::test]
    async fn llm_error_falls_back_to_direct() {
        let llm: Arc<dyn QueryGateLlm> = Arc::new(MockGateLlm::err("network timeout"));
        let gate = gate_with(llm, QueryGateConfig::default());
        let out = gate.reformulate("anything goes here").await;
        assert_eq!(
            out,
            RetrievalDecision::Direct("anything goes here".to_string())
        );
    }

    /// Test F — malformed response (unknown decision string) falls back to Direct(raw).
    #[tokio::test]
    async fn malformed_decision_falls_back_to_direct() {
        let llm: Arc<dyn QueryGateLlm> = Arc::new(MockGateLlm::ok(GateResponse {
            decision: "maybe_skip".to_string(), // not one of the three valid values
            query: None,
            subqueries: None,
        }));
        let gate = gate_with(llm, QueryGateConfig::default());
        let out = gate.reformulate("raw input here").await;
        assert_eq!(out, RetrievalDecision::Direct("raw input here".to_string()));
    }

    /// Direct with missing/empty query field also falls back.
    #[tokio::test]
    async fn direct_without_query_field_falls_back() {
        let llm: Arc<dyn QueryGateLlm> = Arc::new(MockGateLlm::ok(GateResponse {
            decision: "direct".to_string(),
            query: None,
            subqueries: None,
        }));
        let gate = gate_with(llm, QueryGateConfig::default());
        let out = gate.reformulate("raw").await;
        assert_eq!(out, RetrievalDecision::Direct("raw".to_string()));
    }

    /// Test G — Split with 6 subqueries when `max_subqueries: 4` gets truncated
    /// to the first 4. (We pick truncation over rejection: still useful signal
    /// even when the LLM over-splits.)
    #[tokio::test]
    async fn split_over_max_subqueries_truncates_to_first_n() {
        let subs: Vec<String> = (1..=6).map(|i| format!("sub {i}")).collect();
        let llm: Arc<dyn QueryGateLlm> = Arc::new(MockGateLlm::ok(GateResponse {
            decision: "split".to_string(),
            query: None,
            subqueries: Some(subs),
        }));
        let cfg = QueryGateConfig {
            max_subqueries: 4,
            ..QueryGateConfig::default()
        };
        let gate = gate_with(llm, cfg);
        let out = gate.reformulate("multi").await;
        match out {
            RetrievalDecision::Split(v) => {
                assert_eq!(v.len(), 4, "must truncate to first 4");
                assert_eq!(v[0], "sub 1");
                assert_eq!(v[3], "sub 4");
            }
            other => panic!("expected Split, got {other:?}"),
        }
    }

    /// Split with only one (non-empty) subquery is not a true split — falls back.
    #[tokio::test]
    async fn split_with_single_subquery_falls_back_to_direct() {
        let llm: Arc<dyn QueryGateLlm> = Arc::new(MockGateLlm::ok(GateResponse {
            decision: "split".to_string(),
            query: None,
            subqueries: Some(vec!["only one".to_string()]),
        }));
        let gate = gate_with(llm, QueryGateConfig::default());
        let out = gate.reformulate("raw text").await;
        assert_eq!(out, RetrievalDecision::Direct("raw text".to_string()));
    }

    /// Subquery length over `max_subquery_len` is truncated.
    #[tokio::test]
    async fn subquery_length_truncated_to_config_max() {
        let long = "x".repeat(500);
        let subs = vec!["short".to_string(), long.clone()];
        let llm: Arc<dyn QueryGateLlm> = Arc::new(MockGateLlm::ok(GateResponse {
            decision: "split".to_string(),
            query: None,
            subqueries: Some(subs),
        }));
        let cfg = QueryGateConfig {
            max_subquery_len: 50,
            ..QueryGateConfig::default()
        };
        let gate = gate_with(llm, cfg);
        let out = gate.reformulate("raw").await;
        match out {
            RetrievalDecision::Split(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], "short");
                assert_eq!(v[1].chars().count(), 50);
            }
            other => panic!("expected Split, got {other:?}"),
        }
    }
}
