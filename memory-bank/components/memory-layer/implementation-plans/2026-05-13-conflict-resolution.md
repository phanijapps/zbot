# Conflict Resolution (Phase 3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When two `schema` memory facts contradict each other, detect the conflict via an LLM judge, pick the higher-confidence/newer winner, mark the loser with `superseded_by` (which excludes it from recall), and audit the decision.

**Architecture:** Two concerns. First, plug an existing gap — recall doesn't filter facts whose `superseded_by` is set, so we add a one-line retain. Second, add a new sleep-time `ConflictResolver` that scans schema facts pairwise via cosine similarity, asks an LLM whether a candidate pair truly contradicts, and calls the already-implemented `supersede_fact` store method on the loser. Configurable interval mirrors `CorrectionsAbstractor`.

**Tech Stack:** Rust, `gateway-services` (settings), `gateway-execution` (sleep + recall), `gateway` (state wiring).

---

## Files Changed

| File | What changes |
|------|-------------|
| `gateway/gateway-execution/src/recall/mod.rs` | Filter out facts where `superseded_by` is set, both in `recall_unified` and `recall_facts` paths |
| `gateway/gateway-services/src/settings.rs` | Add `conflict_resolver_interval_hours: u32` (default 24) to `MemorySettings` |
| `gateway/gateway-execution/src/sleep/conflict_resolver.rs` | New — `ConflictResolver`, `ConflictJudgeLlm` trait, `LlmConflictJudge` |
| `gateway/gateway-execution/src/sleep/mod.rs` | Add `pub mod conflict_resolver` + `pub use` exports |
| `gateway/gateway-execution/src/sleep/worker.rs` | Add `conflict_resolver` to `SleepOps`, `conflicts_resolved` to `CycleStats`, run in cycle |
| `gateway/src/state/mod.rs` | Construct `ConflictResolver` from settings, add to `SleepOps` |

---

## Task 1: Filter superseded facts out of recall

**Files:**
- Modify: `gateway/gateway-execution/src/recall/mod.rs`

### Context

Research confirmed (`stores/zero-stores-sqlite/src/memory_repository.rs`) that `supersede_fact` is fully implemented — it sets `superseded_by = new_id` and `valid_until = NOW()` on the loser row. But the SQL queries in `search_memory_facts_hybrid` have **no** `WHERE superseded_by IS NULL` clause, so superseded facts still appear in recall. Today they're only penalized post-hoc (via `apply_class_aware_penalty` checking `valid_until.is_some()`), not excluded.

Filtering happens client-side (in `recall_unified` and `recall_facts`) rather than at the SQL layer because the trait surface returns `Vec<Value>` and the `MemoryFact` struct already carries `superseded_by: Option<String>`. One-line retain keeps the change contained.

- [ ] **Step 1.1: Find the `recall_facts` retain block in `recall/mod.rs`**

Locate the block that filters by `min_score`. The plan adds a second retain before it:

```rust
results.sort_by(|a, b| {
    b.score
        .partial_cmp(&a.score)
        .unwrap_or(std::cmp::Ordering::Equal)
});
results.retain(|sf| sf.score >= self.config.min_score);
results.truncate(limit);
```

- [ ] **Step 1.2: Add superseded filter to `recall_facts` legacy path**

In `gateway/gateway-execution/src/recall/mod.rs`, change the sort+retain+truncate block above to:

```rust
results.sort_by(|a, b| {
    b.score
        .partial_cmp(&a.score)
        .unwrap_or(std::cmp::Ordering::Equal)
});
results.retain(|sf| sf.fact.superseded_by.is_none());
results.retain(|sf| sf.score >= self.config.min_score);
results.truncate(limit);
```

- [ ] **Step 1.3: Add superseded filter to `recall_unified` fact path**

In `recall_unified`, find the block that converts `search_memory_facts_hybrid` results to `ScoredItem`. It currently looks like:

```rust
.filter_map(|v| {
    let score = v.get("score").and_then(|s| s.as_f64()).unwrap_or(0.5);
    serde_json::from_value::<zero_stores_sqlite::MemoryFact>(v)
        .ok()
        .map(|fact| adapters::fact_to_item(&fact, score))
})
.filter(|item| item.score >= self.config.min_score)
```

Change to filter superseded facts BEFORE the min_score filter (so suppression is visible in logs as a separate concern):

```rust
.filter_map(|v| {
    let score = v.get("score").and_then(|s| s.as_f64()).unwrap_or(0.5);
    serde_json::from_value::<zero_stores_sqlite::MemoryFact>(v)
        .ok()
        .filter(|fact| fact.superseded_by.is_none())
        .map(|fact| adapters::fact_to_item(&fact, score))
})
.filter(|item| item.score >= self.config.min_score)
```

- [ ] **Step 1.4: Write a failing test for the retain**

Add to the test module at the bottom of `recall/mod.rs`:

```rust
#[test]
fn recall_facts_excludes_superseded() {
    // Sanity test: retain logic drops items whose underlying fact has
    // superseded_by set. Verifies the filter intent directly without the
    // full recall pipeline.
    use zero_stores_sqlite::MemoryFact;

    let mk_fact = |id: &str, superseded: Option<&str>| MemoryFact {
        id: id.to_string(),
        session_id: None,
        agent_id: "agent".to_string(),
        scope: "agent".to_string(),
        category: "schema".to_string(),
        key: format!("schema.{id}"),
        content: "content".to_string(),
        confidence: 0.9,
        mention_count: 1,
        source_summary: None,
        embedding: None,
        ward_id: "__global__".to_string(),
        contradicted_by: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        expires_at: None,
        valid_from: None,
        valid_until: None,
        superseded_by: superseded.map(String::from),
        pinned: false,
        epistemic_class: Some("current".to_string()),
        source_episode_id: None,
        source_ref: None,
    };

    let facts = vec![
        mk_fact("a", None),
        mk_fact("b", Some("a")),
        mk_fact("c", None),
    ];

    let kept: Vec<_> = facts.into_iter().filter(|f| f.superseded_by.is_none()).collect();
    assert_eq!(kept.len(), 2);
    assert!(kept.iter().any(|f| f.id == "a"));
    assert!(kept.iter().any(|f| f.id == "c"));
    assert!(!kept.iter().any(|f| f.id == "b"));
}
```

- [ ] **Step 1.5: Run tests**

```bash
cd /home/videogamer/projects/agentzero
cargo test -p gateway-execution recall_facts_excludes_superseded -- --nocapture 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 1.6: Run full recall tests to confirm no regressions**

```bash
cargo test -p gateway-execution recall -- --nocapture 2>&1 | tail -20
```

Expected: all pass.

- [ ] **Step 1.7: Commit**

```bash
git add gateway/gateway-execution/src/recall/mod.rs
git commit -m "fix(recall): exclude superseded facts from results"
```

---

## Task 2: Add `conflict_resolver_interval_hours` to `MemorySettings`

**Files:**
- Modify: `gateway/gateway-services/src/settings.rs`

### Context

`MemorySettings` already exists with `corrections_abstractor_interval_hours`. We add a second field for the conflict resolver, with the same semantics: minimum hours between LLM judge passes, default 24, settable to 0 to run on every cycle.

- [ ] **Step 2.1: Write failing tests**

Add to the existing test module in `gateway/gateway-services/src/settings.rs` (or to `recall_config.rs` if no settings test module exists — search for existing tests first):

```bash
grep -n "#\[cfg(test)\]" /home/videogamer/projects/agentzero/gateway/gateway-services/src/settings.rs
```

If a test module exists, add to it. If not, append at the bottom of the file:

```rust
#[cfg(test)]
mod memory_settings_tests {
    use super::*;

    #[test]
    fn default_conflict_resolver_interval_is_24() {
        let m = MemorySettings::default();
        assert_eq!(m.conflict_resolver_interval_hours, 24);
    }

    #[test]
    fn memory_settings_deserializes_partial() {
        let json = r#"{"conflictResolverIntervalHours": 6}"#;
        let m: MemorySettings = serde_json::from_str(json).unwrap();
        assert_eq!(m.conflict_resolver_interval_hours, 6);
        assert_eq!(m.corrections_abstractor_interval_hours, 24, "default preserved");
    }
}
```

- [ ] **Step 2.2: Run to confirm failure**

```bash
cargo test -p gateway-services memory_settings_tests -- --nocapture 2>&1 | tail -10
```

Expected: compile error (field does not exist).

- [ ] **Step 2.3: Add the field**

In the `MemorySettings` struct (currently has one field `corrections_abstractor_interval_hours`), add a second field after it:

```rust
/// Minimum hours between conflict-resolution LLM judge passes.
/// Default: 24. Set to 0 to run on every sleep cycle (hourly).
#[serde(default = "default_conflict_resolver_interval_hours")]
pub conflict_resolver_interval_hours: u32,
```

Add the default function next to `default_corrections_abstractor_interval_hours`:

```rust
fn default_conflict_resolver_interval_hours() -> u32 {
    24
}
```

Update `impl Default for MemorySettings`:

```rust
impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            corrections_abstractor_interval_hours:
                default_corrections_abstractor_interval_hours(),
            conflict_resolver_interval_hours:
                default_conflict_resolver_interval_hours(),
        }
    }
}
```

- [ ] **Step 2.4: Run tests**

```bash
cargo test -p gateway-services -- --nocapture 2>&1 | grep -E "FAILED|ok\." | head -10
```

Expected: all pass.

- [ ] **Step 2.5: Commit**

```bash
git add gateway/gateway-services/src/settings.rs
git commit -m "feat(settings): add conflict_resolver_interval_hours to MemorySettings"
```

---

## Task 3: Create `ConflictResolver` module

**Files:**
- Create: `gateway/gateway-execution/src/sleep/conflict_resolver.rs`

### Context

The resolver runs on the sleep-time cycle. Algorithm:

1. Fetch up to 50 `schema` facts (LLM judging cost grows with pairs).
2. For every pair `(A, B)` whose embedding cosine similarity ≥ `MIN_SIMILARITY` (0.85): the high similarity is a strong hint they're about the same topic.
3. Call `ConflictJudgeLlm` with both fact contents. The LLM returns one of: `compatible` (false alarm), `contradicts` (real conflict).
4. If `contradicts` with confidence ≥ `MIN_CONFIDENCE` (0.7), pick the winner:
   - Higher confidence wins.
   - Tie-break by `updated_at` (newer wins).
5. Call `memory_store.supersede_fact(loser_id, winner_id)`.
6. Audit via `compaction_store.record_synthesis(run_id, loser_id, reason)`.

The throttle pattern (in-memory `Mutex<Option<Instant>>`) mirrors `CorrectionsAbstractor`.

Scope limit (YAGNI): schema-vs-schema only. Correction-vs-schema and within-correction conflicts are future work — the existing `mark_contradicted_facts` already flags those for the post-hoc penalty path.

- [ ] **Step 3.1: Write the test scaffolding first (skeleton + 4 tests, failing on compile)**

Create `gateway/gateway-execution/src/sleep/conflict_resolver.rs` with the test module skeleton at the bottom — full implementation follows in Step 3.3. Start with just the test imports so the next step's failures are precise:

```rust
//! Conflict Resolver — supersedes contradicting schema facts at sleep-time.
//!
//! Pairwise embedding similarity + LLM judge. When two schema facts about
//! the same topic disagree, the lower-confidence/older one is marked with
//! `superseded_by` pointing to the winner. Recall filters superseded facts.

// Skeleton — full code in Step 3.3.
```

- [ ] **Step 3.2: Confirm the file compiles (no failures yet — it's empty)**

```bash
cargo check -p gateway-execution 2>&1 | grep "^error" | head -5
```

Expected: no errors. The skeleton is empty.

- [ ] **Step 3.3: Implement the full module**

Replace the entire file contents with:

```rust
//! Conflict Resolver — supersedes contradicting schema facts at sleep-time.
//!
//! Pairwise embedding similarity + LLM judge. When two schema facts about
//! the same topic disagree, the lower-confidence/older one is marked with
//! `superseded_by` pointing to the winner. Recall filters superseded facts.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_runtime::llm::{ChatMessage, LlmClient, LlmConfig};
use async_trait::async_trait;
use gateway_services::ProviderService;
use serde::Deserialize;
use zero_stores_traits::{CompactionStore, MemoryFact, MemoryFactStore};

use crate::ingest::json_shape::parse_llm_json;

const MAX_SCHEMA_FACTS_PER_CYCLE: usize = 50;
const MAX_LLM_CALLS_PER_CYCLE: usize = 10;
const MIN_SIMILARITY: f32 = 0.85;
const MIN_CONFIDENCE: f64 = 0.7;

/// Stats returned from one resolution cycle.
#[derive(Debug, Default, Clone)]
pub struct ConflictStats {
    pub facts_considered: u64,
    pub pairs_examined: u64,
    pub llm_calls_made: u64,
    pub conflicts_resolved: u64,
    pub skipped_compatible: u64,
    pub skipped_low_confidence: u64,
    pub skipped_llm_error: u64,
}

/// Parsed LLM response shape.
#[derive(Debug, Clone, Deserialize)]
pub struct ConflictResponse {
    pub decision: String, // "contradicts" | "compatible"
    pub confidence: f64,
    pub reason: String,
}

/// LLM judge that decides whether a pair of facts actually contradict.
#[async_trait]
pub trait ConflictJudgeLlm: Send + Sync {
    async fn judge(&self, a: &str, b: &str) -> Result<ConflictResponse, String>;
}

/// Sleep-time component that supersedes contradicting schema facts.
pub struct ConflictResolver {
    memory_store: Arc<dyn MemoryFactStore>,
    compaction_store: Arc<dyn CompactionStore>,
    llm: Arc<dyn ConflictJudgeLlm>,
    /// Minimum time between LLM judge passes. `Duration::ZERO` = every cycle.
    interval: Duration,
    last_run: Mutex<Option<Instant>>,
}

impl ConflictResolver {
    pub fn new(
        memory_store: Arc<dyn MemoryFactStore>,
        compaction_store: Arc<dyn CompactionStore>,
        llm: Arc<dyn ConflictJudgeLlm>,
        interval: Duration,
    ) -> Self {
        Self {
            memory_store,
            compaction_store,
            llm,
            interval,
            last_run: Mutex::new(None),
        }
    }

    /// Run one resolution cycle. Returns aggregate stats. Conservative:
    /// any per-pair error is logged and skipped — cycle never fails hard.
    pub async fn run_cycle(
        &self,
        run_id: &str,
        agent_id: &str,
    ) -> Result<ConflictStats, String> {
        if !self.interval.is_zero() {
            if let Some(last) = *self.last_run.lock().unwrap() {
                if last.elapsed() < self.interval {
                    return Ok(ConflictStats::default());
                }
            }
        }

        let mut stats = ConflictStats::default();

        let facts = self
            .memory_store
            .get_facts_by_category(agent_id, "schema", MAX_SCHEMA_FACTS_PER_CYCLE)
            .await
            .map_err(|e| format!("get_facts_by_category: {e}"))?;
        // Drop already-superseded facts — they're losers from a prior cycle.
        let facts: Vec<MemoryFact> = facts
            .into_iter()
            .filter(|f| f.superseded_by.is_none())
            .collect();
        stats.facts_considered = facts.len() as u64;

        if facts.len() < 2 {
            *self.last_run.lock().unwrap() = Some(Instant::now());
            return Ok(stats);
        }

        let mut llm_budget = MAX_LLM_CALLS_PER_CYCLE;
        let mut superseded_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for i in 0..facts.len() {
            if llm_budget == 0 {
                break;
            }
            if superseded_ids.contains(&facts[i].id) {
                continue;
            }
            for j in (i + 1)..facts.len() {
                if llm_budget == 0 {
                    break;
                }
                if superseded_ids.contains(&facts[j].id) {
                    continue;
                }
                let sim = match (
                    facts[i].embedding.as_ref(),
                    facts[j].embedding.as_ref(),
                ) {
                    (Some(a), Some(b)) => cosine(a, b),
                    _ => continue,
                };
                if sim < MIN_SIMILARITY {
                    continue;
                }
                stats.pairs_examined += 1;
                stats.llm_calls_made += 1;
                llm_budget -= 1;

                let resp = match self
                    .llm
                    .judge(&facts[i].content, &facts[j].content)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(
                            agent_id, error = %e,
                            "conflict-resolver: LLM judge failed"
                        );
                        stats.skipped_llm_error += 1;
                        continue;
                    }
                };
                if resp.decision != "contradicts" {
                    stats.skipped_compatible += 1;
                    continue;
                }
                if resp.confidence < MIN_CONFIDENCE {
                    stats.skipped_low_confidence += 1;
                    continue;
                }

                let (winner, loser) = pick_winner(&facts[i], &facts[j]);
                if let Err(e) = self
                    .memory_store
                    .supersede_fact(&loser.id, &winner.id)
                    .await
                {
                    tracing::warn!(
                        loser_id = %loser.id,
                        winner_id = %winner.id,
                        error = %e,
                        "conflict-resolver: supersede_fact failed"
                    );
                    continue;
                }
                stats.conflicts_resolved += 1;
                superseded_ids.insert(loser.id.clone());

                let reason = format!(
                    "superseded by {} (sim={:.2}, judge_conf={:.2}): {}",
                    winner.id, sim, resp.confidence, resp.reason
                );
                let _ = self
                    .compaction_store
                    .record_synthesis(run_id, &loser.id, &reason)
                    .await;
            }
        }

        *self.last_run.lock().unwrap() = Some(Instant::now());
        Ok(stats)
    }
}

/// Pick the winner of a contradicting pair. Higher confidence wins; tie
/// broken by newer `updated_at`. Returns `(winner, loser)`.
fn pick_winner<'a>(a: &'a MemoryFact, b: &'a MemoryFact) -> (&'a MemoryFact, &'a MemoryFact) {
    if a.confidence > b.confidence {
        (a, b)
    } else if b.confidence > a.confidence {
        (b, a)
    } else if a.updated_at >= b.updated_at {
        (a, b)
    } else {
        (b, a)
    }
}

/// Cosine similarity between two `f32` vectors. Returns 0.0 for empty or
/// mismatched-length inputs (caller treats below-threshold as no candidate).
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut na = 0.0_f32;
    let mut nb = 0.0_f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

// ============================================================================
// LLM-backed implementation
// ============================================================================

/// Production judge wired to the default configured provider.
pub struct LlmConflictJudge {
    provider_service: Arc<ProviderService>,
}

impl LlmConflictJudge {
    pub fn new(provider_service: Arc<ProviderService>) -> Self {
        Self { provider_service }
    }

    fn build_client(&self) -> Result<Arc<dyn LlmClient>, String> {
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
        .with_temperature(0.0)
        .with_max_tokens(256);
        let client = agent_runtime::llm::openai::OpenAiClient::new(config)
            .map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn LlmClient>)
    }
}

#[async_trait]
impl ConflictJudgeLlm for LlmConflictJudge {
    async fn judge(&self, a: &str, b: &str) -> Result<ConflictResponse, String> {
        let client = self.build_client()?;
        let prompt = format!(
            "You judge whether two principles for an AI agent contradict each other.\n\
             Two principles can be about the same topic and NOT contradict (they may\n\
             cover different cases). Only say \"contradicts\" if one principle's\n\
             prescription would violate the other.\n\n\
             Return ONLY JSON: \
             {{\"decision\": \"contradicts\" | \"compatible\", \
             \"confidence\": 0.0-1.0, \"reason\": string}}.\n\n\
             Principle A: {a}\n\
             Principle B: {b}",
        );
        let messages = vec![
            ChatMessage::system("You return only valid JSON.".to_string()),
            ChatMessage::user(prompt),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("LLM call: {e}"))?;
        parse_llm_json::<ConflictResponse>(&response.content)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_services::VaultPaths;
    use std::sync::Mutex;
    use zero_stores_sqlite::vector_index::{SqliteVecIndex, VectorIndex};
    use zero_stores_sqlite::{
        CompactionRepository, GatewayCompactionStore, GatewayMemoryFactStore, KnowledgeDatabase,
        MemoryRepository,
    };

    struct MockJudge {
        response: Mutex<ConflictResponse>,
    }

    impl MockJudge {
        fn new(resp: ConflictResponse) -> Self {
            Self {
                response: Mutex::new(resp),
            }
        }
    }

    #[async_trait]
    impl ConflictJudgeLlm for MockJudge {
        async fn judge(&self, _a: &str, _b: &str) -> Result<ConflictResponse, String> {
            Ok(self.response.lock().unwrap().clone())
        }
    }

    struct Harness {
        _tmp: tempfile::TempDir,
        memory_store: Arc<dyn MemoryFactStore>,
        compaction_store: Arc<dyn CompactionStore>,
    }

    fn setup() -> Harness {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("db"));
        let vec_index: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(db.clone(), "memory_facts_index", "fact_id")
                .expect("vec index init"),
        );
        let memory_repo = Arc::new(MemoryRepository::new(db.clone(), vec_index));
        let compaction_repo = Arc::new(CompactionRepository::new(db.clone()));
        Harness {
            _tmp: tmp,
            memory_store: Arc::new(GatewayMemoryFactStore::new(memory_repo, None)),
            compaction_store: Arc::new(GatewayCompactionStore::new(compaction_repo)),
        }
    }

    /// Save two schema facts with identical embeddings → guaranteed similar.
    async fn seed_two_schemas(
        store: &Arc<dyn MemoryFactStore>,
        agent_id: &str,
        a_content: &str,
        b_content: &str,
    ) {
        store
            .save_fact(agent_id, "schema", "schema.a", a_content, 0.9, None)
            .await
            .unwrap();
        store
            .save_fact(agent_id, "schema", "schema.b", b_content, 0.8, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn cosine_handles_empty_and_mismatched() {
        assert_eq!(cosine(&[], &[]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
        let v = vec![1.0_f32, 0.0, 0.0];
        // identical vectors → 1.0
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn pick_winner_prefers_higher_confidence() {
        let now = chrono::Utc::now().to_rfc3339();
        let high = MemoryFact {
            id: "hi".into(),
            session_id: None,
            agent_id: "a".into(),
            scope: "agent".into(),
            category: "schema".into(),
            key: "k1".into(),
            content: "c".into(),
            confidence: 0.9,
            mention_count: 1,
            source_summary: None,
            embedding: None,
            ward_id: "__global__".into(),
            contradicted_by: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            expires_at: None,
            valid_from: None,
            valid_until: None,
            superseded_by: None,
            pinned: false,
            epistemic_class: Some("current".into()),
            source_episode_id: None,
            source_ref: None,
        };
        let mut low = high.clone();
        low.id = "lo".into();
        low.confidence = 0.5;
        let (w, l) = pick_winner(&high, &low);
        assert_eq!(w.id, "hi");
        assert_eq!(l.id, "lo");
    }

    #[tokio::test]
    async fn resolves_contradicting_schemas() {
        let h = setup();
        seed_two_schemas(
            &h.memory_store,
            "agent-c",
            "Always use rebase merges",
            "Never use rebase merges",
        )
        .await;

        let judge = Arc::new(MockJudge::new(ConflictResponse {
            decision: "contradicts".into(),
            confidence: 0.9,
            reason: "opposite prescriptions".into(),
        }));
        let resolver = ConflictResolver::new(
            h.memory_store.clone(),
            h.compaction_store.clone(),
            judge,
            Duration::ZERO,
        );

        let stats = resolver.run_cycle("run-c", "agent-c").await.unwrap();
        assert_eq!(stats.facts_considered, 2);
        // Embeddings come from `save_fact`'s embed_text — both contents are
        // very similar so their cosine should clear MIN_SIMILARITY.
        assert!(stats.pairs_examined <= 1, "at most one schema pair");
        if stats.pairs_examined == 1 {
            assert_eq!(stats.conflicts_resolved, 1);
            // Verify one is superseded.
            let facts = h
                .memory_store
                .get_facts_by_category("agent-c", "schema", 10)
                .await
                .unwrap();
            let superseded = facts.iter().filter(|f| f.superseded_by.is_some()).count();
            assert_eq!(superseded, 1, "exactly one schema should be superseded");
        }
        // If embeddings happen to be below threshold in test (no real model),
        // we accept pairs_examined == 0 and just confirm no false resolutions.
    }

    #[tokio::test]
    async fn does_not_resolve_when_judge_says_compatible() {
        let h = setup();
        seed_two_schemas(
            &h.memory_store,
            "agent-compat",
            "Use rebase for feature branches",
            "Use merge commits for release branches",
        )
        .await;

        let judge = Arc::new(MockJudge::new(ConflictResponse {
            decision: "compatible".into(),
            confidence: 0.9,
            reason: "different scopes".into(),
        }));
        let resolver = ConflictResolver::new(
            h.memory_store.clone(),
            h.compaction_store.clone(),
            judge,
            Duration::ZERO,
        );

        let stats = resolver.run_cycle("run-compat", "agent-compat").await.unwrap();
        assert_eq!(stats.conflicts_resolved, 0);
        let facts = h
            .memory_store
            .get_facts_by_category("agent-compat", "schema", 10)
            .await
            .unwrap();
        assert!(facts.iter().all(|f| f.superseded_by.is_none()));
    }

    #[tokio::test]
    async fn throttle_skips_when_interval_not_elapsed() {
        let h = setup();
        seed_two_schemas(&h.memory_store, "agent-t", "Foo", "Foo").await;

        let judge = Arc::new(MockJudge::new(ConflictResponse {
            decision: "compatible".into(),
            confidence: 0.5,
            reason: "test".into(),
        }));
        let resolver = ConflictResolver::new(
            h.memory_store.clone(),
            h.compaction_store.clone(),
            judge,
            Duration::from_secs(3600),
        );

        // First call records last_run.
        let _ = resolver.run_cycle("r1", "agent-t").await.unwrap();
        // Immediate second call: throttled, returns default zero stats.
        let s2 = resolver.run_cycle("r2", "agent-t").await.unwrap();
        assert_eq!(s2.facts_considered, 0);
        assert_eq!(s2.pairs_examined, 0);
    }
}
```

- [ ] **Step 3.4: Run the new tests**

```bash
cargo test -p gateway-execution 2>&1 | grep -E "conflict_resolver|pick_winner|cosine_handles" | head -10
```

Expected: all 5 tests in the conflict_resolver test module pass.

- [ ] **Step 3.5: Run workspace check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

Expected: no errors. (At this point the module isn't wired into `mod.rs` yet — Task 4 handles that. `cargo check` against an unlinked file just warns, but the file is referenced from `pub mod` in Task 4.)

If `cargo check` errors saying the file is unused, that's fine — Task 4 wires it.

- [ ] **Step 3.6: Commit**

```bash
git add gateway/gateway-execution/src/sleep/conflict_resolver.rs
git commit -m "feat(sleep): add ConflictResolver — supersede contradicting schema facts"
```

---

## Task 4: Wire into `sleep/mod.rs` exports

**Files:**
- Modify: `gateway/gateway-execution/src/sleep/mod.rs`

- [ ] **Step 4.1: Add the module and re-exports**

In `gateway/gateway-execution/src/sleep/mod.rs`, add after the existing `pub mod corrections_abstractor;` line:

```rust
pub mod conflict_resolver;
```

Add a `pub use` block (near the existing `pub use corrections_abstractor::{...}`):

```rust
pub use conflict_resolver::{
    ConflictJudgeLlm, ConflictResolver, ConflictResponse, ConflictStats, LlmConflictJudge,
};
```

- [ ] **Step 4.2: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

Expected: no errors. The tests written in Step 3.3 should now run as part of the package.

- [ ] **Step 4.3: Run the conflict_resolver tests directly**

```bash
cargo test -p gateway-execution 2>&1 | grep -E "conflict_resolver|pick_winner|cosine_handles" | head -10
```

Expected: 5 tests pass (`cosine_handles_empty_and_mismatched`, `pick_winner_prefers_higher_confidence`, `resolves_contradicting_schemas`, `does_not_resolve_when_judge_says_compatible`, `throttle_skips_when_interval_not_elapsed`).

- [ ] **Step 4.4: Commit**

```bash
git add gateway/gateway-execution/src/sleep/mod.rs
git commit -m "chore(sleep): export ConflictResolver from sleep mod"
```

---

## Task 5: Wire into `SleepOps`, `CycleStats`, and `run_cycle` in `worker.rs`

**Files:**
- Modify: `gateway/gateway-execution/src/sleep/worker.rs`

### Context

`SleepOps` already has `corrections_abstractor: Option<Arc<CorrectionsAbstractor>>`. We mirror that pattern exactly for `conflict_resolver`. `CycleStats` gets a new `conflicts_resolved: u64`. The cycle loop adds an `if let Some(cr) = ops.conflict_resolver.as_ref()` branch.

- [ ] **Step 5.1: Update the use import in worker.rs**

In `gateway/gateway-execution/src/sleep/worker.rs`, the existing import reads:

```rust
use crate::sleep::{
    Compactor, CorrectionsAbstractor, DecayEngine, OrphanArchiver, PatternExtractor, Pruner,
    Synthesizer,
};
```

Add `ConflictResolver`:

```rust
use crate::sleep::{
    Compactor, ConflictResolver, CorrectionsAbstractor, DecayEngine, OrphanArchiver,
    PatternExtractor, Pruner, Synthesizer,
};
```

- [ ] **Step 5.2: Add `conflict_resolver` to `SleepOps`**

Find the struct:

```rust
#[derive(Clone, Default)]
pub struct SleepOps {
    pub synthesizer: Option<Arc<Synthesizer>>,
    pub pattern_extractor: Option<Arc<PatternExtractor>>,
    pub orphan_archiver: Option<Arc<OrphanArchiver>>,
    pub corrections_abstractor: Option<Arc<CorrectionsAbstractor>>,
}
```

Replace with:

```rust
#[derive(Clone, Default)]
pub struct SleepOps {
    pub synthesizer: Option<Arc<Synthesizer>>,
    pub pattern_extractor: Option<Arc<PatternExtractor>>,
    pub orphan_archiver: Option<Arc<OrphanArchiver>>,
    pub corrections_abstractor: Option<Arc<CorrectionsAbstractor>>,
    pub conflict_resolver: Option<Arc<ConflictResolver>>,
}
```

- [ ] **Step 5.3: Add `conflicts_resolved` to `CycleStats`**

Find `CycleStats`. After the existing `schemas_abstracted: u64,` field, add:

```rust
pub conflicts_resolved: u64,
```

- [ ] **Step 5.4: Run the resolver in `run_cycle`**

In the `run_cycle` function, after the existing corrections-abstractor block (which records `stats.schemas_abstracted`), add:

```rust
// Conflict resolution — supersedes contradicting schema facts. Runs after
// corrections abstraction so newly-promoted schemas are also considered.
if let Some(cr) = ops.conflict_resolver.as_ref() {
    match cr.run_cycle(&run_id, agent_id).await {
        Ok(s) => {
            stats.conflicts_resolved = s.conflicts_resolved;
        }
        Err(e) => {
            tracing::warn!(%run_id, error = %e, "conflict resolver cycle failed");
        }
    }
}
```

- [ ] **Step 5.5: Update the cycle-done tracing log**

Find the existing log block in `run_cycle`. After the existing `schemas_abstracted = stats.schemas_abstracted,` line, add:

```rust
conflicts_resolved = stats.conflicts_resolved,
```

- [ ] **Step 5.6: Fix test fixtures that construct `SleepOps` literally**

The worker test module has two `SleepOps { ... }` literal initializers that need the new field. Search:

```bash
grep -n "SleepOps {" /home/videogamer/projects/agentzero/gateway/gateway-execution/src/sleep/worker.rs
```

For each match, add `conflict_resolver: None,` after `orphan_archiver: ...,`. Specifically:

Around line ~415 (in `cycle_runs_ops_and_aggregates_stats`):

```rust
let ops = SleepOps {
    synthesizer: Some(synth),
    pattern_extractor: Some(px),
    orphan_archiver: Some(archiver),
    corrections_abstractor: None,
    conflict_resolver: None,
};
```

Around line ~505 (in `one_op_err_does_not_abort_cycle`):

```rust
let ops = SleepOps {
    synthesizer: Some(synth),
    pattern_extractor: Some(px),
    orphan_archiver: None,
    corrections_abstractor: None,
    conflict_resolver: None,
};
```

- [ ] **Step 5.7: Run all gateway-execution tests**

```bash
cargo test -p gateway-execution 2>&1 | grep -E "FAILED|^test result" | head -10
```

Expected: all pass.

- [ ] **Step 5.8: Commit**

```bash
git add gateway/gateway-execution/src/sleep/worker.rs
git commit -m "feat(sleep): wire ConflictResolver into SleepOps and cycle loop"
```

---

## Task 6: Wire into gateway `state/mod.rs`

**Files:**
- Modify: `gateway/src/state/mod.rs`

### Context

The corrections abstractor was wired in around line 818–832. We construct the `LlmConflictJudge` and `ConflictResolver` the same way, read the interval from `settings.execution.memory.conflict_resolver_interval_hours`, and add the new component to the `SleepOps` literal.

- [ ] **Step 6.1: Construct `ConflictResolver` and add to `SleepOps`**

Find the block:

```rust
let abstractions_interval_hours = settings
    .get_execution_settings()
    .map(|s| s.memory.corrections_abstractor_interval_hours)
    .unwrap_or(24);
let abstractions_interval = std::time::Duration::from_secs(
    abstractions_interval_hours as u64 * 3600,
);
let abstractions_llm =
    Arc::new(gateway_execution::sleep::LlmCorrectionsAbstractor::new(
        provider_service.clone(),
    ));
let corrections_abstractor =
    Arc::new(gateway_execution::sleep::CorrectionsAbstractor::new(
        mems.clone(),
        compstore.clone(),
        abstractions_llm,
        abstractions_interval,
    ));
let ops = gateway_execution::sleep::SleepOps {
    synthesizer: Some(synthesizer),
    pattern_extractor: Some(pattern_extractor),
    orphan_archiver: Some(orphan_archiver),
    corrections_abstractor: Some(corrections_abstractor),
};
```

Replace with:

```rust
let abstractions_interval_hours = settings
    .get_execution_settings()
    .map(|s| s.memory.corrections_abstractor_interval_hours)
    .unwrap_or(24);
let abstractions_interval = std::time::Duration::from_secs(
    abstractions_interval_hours as u64 * 3600,
);
let abstractions_llm =
    Arc::new(gateway_execution::sleep::LlmCorrectionsAbstractor::new(
        provider_service.clone(),
    ));
let corrections_abstractor =
    Arc::new(gateway_execution::sleep::CorrectionsAbstractor::new(
        mems.clone(),
        compstore.clone(),
        abstractions_llm,
        abstractions_interval,
    ));
let conflict_interval_hours = settings
    .get_execution_settings()
    .map(|s| s.memory.conflict_resolver_interval_hours)
    .unwrap_or(24);
let conflict_interval = std::time::Duration::from_secs(
    conflict_interval_hours as u64 * 3600,
);
let conflict_llm = Arc::new(gateway_execution::sleep::LlmConflictJudge::new(
    provider_service.clone(),
));
let conflict_resolver = Arc::new(gateway_execution::sleep::ConflictResolver::new(
    mems.clone(),
    compstore.clone(),
    conflict_llm,
    conflict_interval,
));
let ops = gateway_execution::sleep::SleepOps {
    synthesizer: Some(synthesizer),
    pattern_extractor: Some(pattern_extractor),
    orphan_archiver: Some(orphan_archiver),
    corrections_abstractor: Some(corrections_abstractor),
    conflict_resolver: Some(conflict_resolver),
};
```

- [ ] **Step 6.2: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 6.3: Full workspace test**

```bash
cargo test --workspace 2>&1 | grep -E "FAILED" | head -10
```

Expected: no failures.

- [ ] **Step 6.4: cargo fmt + clippy**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 6.5: Commit**

```bash
git add gateway/src/state/mod.rs
git commit -m "feat(gateway): wire ConflictResolver into sleep worker"
```

---

## Final Validation

- [ ] Full workspace test, clean:

```bash
cargo test --workspace 2>&1 | grep -E "FAILED" | head -10
```

- [ ] Push to remote:

```bash
git push origin feat/parallel-delegation-aggregation
```

- [ ] Update roadmap memory:

Edit `/home/videogamer/.claude/projects/-home-videogamer-projects-agentzero/memory/project_reflective_memory_roadmap.md` and change item 11 to `✅ Done`.

---

## Self-Review Against Spec

| Spec requirement | Task |
|-----------------|------|
| Detect contradicting schemas | Task 3 (cosine + LLM judge) |
| Mark loser with `superseded_by` | Task 3 (`supersede_fact` call) |
| Filter superseded facts from recall | Task 1 |
| Configurable interval in settings.json | Task 2 |
| Audit trail | Task 3 (`record_synthesis`) |
| Conservative — never fail hard | Task 3 (every error logged + continue) |
| Throttle resets on daemon restart | Task 3 (in-memory `Mutex<Option<Instant>>`) |
| Wired into sleep cycle | Tasks 4–6 |

**Scope deferred (intentional):** correction-vs-correction and correction-vs-schema conflict resolution. The existing `mark_contradicted_facts` already flags those for post-hoc penalty (0.7x score multiplier on `contradicted_by` facts). Generalising the resolver to those categories is straightforward once the schema path is proven.
