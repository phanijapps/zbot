# Pattern Abstraction (Phase 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When an agent has accumulated 3+ correction facts, run a sleep-time LLM pass that abstracts them into a single `schema` category fact — a distilled principle that ranks above raw corrections in recall.

**Architecture:** Three files. `RecallConfig` gets a `schema` weight. A new `CorrectionsAbstractor` runs during sleep-time. `SleepOps` and `CycleStats` are extended to wire it in. Gateway state/mod.rs constructs and registers it.

**Tech Stack:** Rust, `gateway-services` crate (RecallConfig), `gateway-execution` crate (sleep/corrections_abstractor.rs + worker.rs), `gateway` crate (state/mod.rs)

---

## Files Changed

| File | What changes |
|------|-------------|
| `gateway/gateway-services/src/recall_config.rs` | Add `"schema"` to `category_weights` with weight 1.6 |
| `gateway/gateway-execution/src/sleep/corrections_abstractor.rs` | New — `CorrectionsAbstractor` + `AbstractionLlm` trait + `LlmCorrectionsAbstractor` |
| `gateway/gateway-execution/src/sleep/mod.rs` | Add `pub mod corrections_abstractor` + pub use exports |
| `gateway/gateway-execution/src/sleep/worker.rs` | Add `corrections_abstractor` to `SleepOps`, `schemas_abstracted` to `CycleStats`, run in loop |
| `gateway/src/state/mod.rs` | Construct `CorrectionsAbstractor` and add to `SleepOps` |

---

## Task 1: Add `schema` weight to RecallConfig

**Files:**
- Modify: `gateway/gateway-services/src/recall_config.rs`

### Context

`RecallConfig` stores `category_weights: HashMap<String, f64>`. `correction` is 1.5. `schema` should be 1.6 — schemas are distilled rules that subsume the raw corrections.

- [ ] **Step 1.1: Write failing test**

Add to the `#[cfg(test)]` block at the bottom of `gateway/gateway-services/src/recall_config.rs`:

```rust
#[test]
fn schema_category_weight_is_higher_than_correction() {
    let config = RecallConfig::default();
    let schema_w = config.category_weight("schema");
    let correction_w = config.category_weight("correction");
    assert!(
        schema_w > correction_w,
        "schema weight ({schema_w}) must exceed correction weight ({correction_w})"
    );
}
```

- [ ] **Step 1.2: Run to confirm failure**

```bash
cd /home/videogamer/projects/agentzero
cargo test -p gateway-services schema_category_weight -- --nocapture 2>&1 | tail -10
```

Expected: FAIL — schema returns 1.0 fallback, correction is 1.5, assertion fails.

- [ ] **Step 1.3: Add `schema` to `category_weights`**

In `RecallConfig::default()`, inside the `HashMap::from([...])`, add after `("correction".to_string(), 1.5)`:

```rust
("schema".to_string(), 1.6),
```

Also update the `assert_eq!(config.category_weights.len(), 9)` test to `10`.

- [ ] **Step 1.4: Run tests**

```bash
cargo test -p gateway-services -- --nocapture 2>&1 | grep -E "FAILED|ok\." | head -20
```

Expected: all pass. The `default_config` test will fail if you forget to update the length assertion — fix it.

- [ ] **Step 1.5: Commit**

```bash
git add gateway/gateway-services/src/recall_config.rs
git commit -m "feat(recall): add schema category weight (1.6) to RecallConfig"
```

---

## Task 2: Create `corrections_abstractor.rs`

**Files:**
- Create: `gateway/gateway-execution/src/sleep/corrections_abstractor.rs`

### Context

This module runs during sleep-time. It fetches all `correction` facts for an agent, sends them to an LLM, and if a shared principle is found writes it as a `schema` category fact via `memory_store.save_fact`.

`save_fact` does upsert: if the same `(agent_id, scope, ward_id, key)` already exists, it bumps `mention_count`. So calling `run_cycle` twice with the same corrections is idempotent — the schema fact gets its count bumped.

The LLM response shape mirrors the `Synthesizer`'s `SynthesisResponse`.

- [ ] **Step 2.1: Write failing tests**

Create `gateway/gateway-execution/src/sleep/corrections_abstractor.rs` with just the test module to start:

```rust
use std::sync::Arc;
use async_trait::async_trait;
use gateway_services::VaultPaths;
use zero_stores_sqlite::{
    CompactionRepository, GatewayCompactionStore, GatewayMemoryFactStore,
    KnowledgeDatabase, MemoryRepository,
};
use zero_stores_sqlite::vector_index::{SqliteVecIndex, VectorIndex};
use zero_stores_traits::{CompactionStore, MemoryFactStore};

// ---- Stub types (will be replaced in Step 2.3) ----

pub struct AbstractionStats {
    pub corrections_considered: u64,
    pub schemas_abstracted: u64,
    pub skipped_low_confidence: u64,
    pub skipped_llm_error: u64,
}

impl Default for AbstractionStats {
    fn default() -> Self {
        Self {
            corrections_considered: 0,
            schemas_abstracted: 0,
            skipped_low_confidence: 0,
            skipped_llm_error: 0,
        }
    }
}

// ---- Tests ----

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn skips_when_fewer_than_three_corrections() {
        let h = setup();
        for i in 0..2_usize {
            h.memory_store
                .save_fact("agent-few", "correction", &format!("corr-{i}"),
                           "Don't do X when Y", 0.9, None)
                .await
                .unwrap();
        }
        // CorrectionsAbstractor doesn't exist yet — this won't compile.
        // That's the expected failure for TDD.
        let _stats: AbstractionStats = todo!();
    }
}
```

- [ ] **Step 2.2: Run to confirm compile failure**

```bash
cargo test -p gateway-execution corrections_abstractor -- --nocapture 2>&1 | tail -10
```

Expected: compile error (module not yet wired, `todo!()` macro, or missing type).

- [ ] **Step 2.3: Implement the full module**

Replace the entire file with:

```rust
//! Corrections Abstractor — promotes repeated correction facts to schema facts.
//!
//! Runs during sleep-time. When an agent has accumulated MIN_CORRECTIONS_TO_ABSTRACT
//! (3+) correction facts, asks an LLM to identify a shared principle. If found,
//! writes a `schema` category fact via `save_fact` (upsert — idempotent on
//! repeated calls with the same corrections cluster).
//!
//! Category weights: schema (1.6) > correction (1.5) — schemas are preferred
//! in recall over the raw corrections they distill.

use std::sync::Arc;

use agent_runtime::llm::{ChatMessage, LlmClient, LlmConfig};
use async_trait::async_trait;
use gateway_services::ProviderService;
use serde::Deserialize;
use zero_stores_traits::{CompactionStore, MemoryFactStore};

use crate::ingest::json_shape::parse_llm_json;

const MIN_CORRECTIONS_TO_ABSTRACT: usize = 3;
const MAX_CORRECTIONS_PER_CALL: usize = 20;
const MIN_CONFIDENCE: f64 = 0.7;

/// Stats returned from one abstraction cycle.
#[derive(Debug, Default, Clone)]
pub struct AbstractionStats {
    pub corrections_considered: u64,
    pub schemas_abstracted: u64,
    pub skipped_low_confidence: u64,
    pub skipped_llm_error: u64,
}

/// Parsed LLM response shape.
#[derive(Debug, Clone, Deserialize)]
pub struct AbstractionResponse {
    pub schema: String,
    pub confidence: f64,
    pub key_fact: String,
    pub decision: String, // "abstract" | "skip"
}

/// Abstraction so tests can inject a mock LLM.
#[async_trait]
pub trait AbstractionLlm: Send + Sync {
    async fn abstract_corrections(
        &self,
        corrections: &[String],
    ) -> Result<AbstractionResponse, String>;
}

/// Sleep-time component that distills correction facts into schema facts.
pub struct CorrectionsAbstractor {
    memory_store: Arc<dyn MemoryFactStore>,
    compaction_store: Arc<dyn CompactionStore>,
    llm: Arc<dyn AbstractionLlm>,
}

impl CorrectionsAbstractor {
    pub fn new(
        memory_store: Arc<dyn MemoryFactStore>,
        compaction_store: Arc<dyn CompactionStore>,
        llm: Arc<dyn AbstractionLlm>,
    ) -> Self {
        Self {
            memory_store,
            compaction_store,
            llm,
        }
    }

    /// Run one abstraction cycle. Returns aggregate stats. Any error is
    /// logged and the cycle returns partial stats — never fails hard.
    pub async fn run_cycle(
        &self,
        run_id: &str,
        agent_id: &str,
    ) -> Result<AbstractionStats, String> {
        let mut stats = AbstractionStats::default();

        let corrections = self
            .memory_store
            .get_facts_by_category(agent_id, "correction", MAX_CORRECTIONS_PER_CALL)
            .await
            .map_err(|e| format!("get_facts_by_category: {e}"))?;

        stats.corrections_considered = corrections.len() as u64;

        if corrections.len() < MIN_CORRECTIONS_TO_ABSTRACT {
            return Ok(stats);
        }

        let contents: Vec<String> = corrections.iter().map(|f| f.content.clone()).collect();

        let resp = match self.llm.abstract_corrections(&contents).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    agent_id,
                    error = %e,
                    "corrections-abstractor: LLM failed"
                );
                stats.skipped_llm_error += 1;
                return Ok(stats);
            }
        };

        if resp.decision != "abstract" || resp.confidence < MIN_CONFIDENCE {
            stats.skipped_low_confidence += 1;
            return Ok(stats);
        }

        let key = format!("schema.corrections.{}", short_hash(&resp.key_fact));

        match self
            .memory_store
            .save_fact(agent_id, "schema", &key, &resp.key_fact, resp.confidence, None)
            .await
        {
            Ok(_) => {
                stats.schemas_abstracted += 1;
                let reason = format!(
                    "abstracted from {} corrections (schema={}, confidence={:.2})",
                    corrections.len(),
                    resp.schema,
                    resp.confidence
                );
                // Best-effort audit via CompactionStore.
                if let Ok(Some(fact)) = self
                    .memory_store
                    .get_fact_by_key(agent_id, "global", "__global__", &key)
                    .await
                {
                    let _ = self
                        .compaction_store
                        .record_synthesis(run_id, &fact.id, &reason)
                        .await;
                }
            }
            Err(e) => {
                tracing::warn!(
                    agent_id,
                    key,
                    error = %e,
                    "corrections-abstractor: save_fact failed"
                );
                stats.skipped_llm_error += 1;
            }
        }

        Ok(stats)
    }
}

// ============================================================================
// LLM-backed implementation
// ============================================================================

/// Production `AbstractionLlm` wired to the default configured provider.
pub struct LlmCorrectionsAbstractor {
    provider_service: Arc<ProviderService>,
}

impl LlmCorrectionsAbstractor {
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
        .with_max_tokens(512);
        let client = agent_runtime::llm::openai::OpenAiClient::new(config)
            .map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn LlmClient>)
    }
}

#[async_trait]
impl AbstractionLlm for LlmCorrectionsAbstractor {
    async fn abstract_corrections(
        &self,
        corrections: &[String],
    ) -> Result<AbstractionResponse, String> {
        let client = self.build_client()?;
        let formatted = corrections
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {c}", i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "You identify common principles from an AI agent's correction history.\n\
             Below are {n} correction facts the agent has accumulated.\n\
             Decide if they share a common theme expressible as one imperative principle.\n\n\
             Return ONLY JSON: \
             {{\"schema\": string, \"confidence\": 0.0-1.0, \
             \"key_fact\": string, \"decision\": \"abstract\" | \"skip\"}}.\n\
             - \"schema\": theme name in snake_case (<5 words)\n\
             - \"key_fact\": the principle as a single imperative sentence\n\
             - \"decision\": \"abstract\" if clear shared principle, \"skip\" if too diverse\n\n\
             Corrections:\n{formatted}",
            n = corrections.len(),
        );
        let messages = vec![
            ChatMessage::system("You return only valid JSON.".to_string()),
            ChatMessage::user(prompt),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("LLM call: {e}"))?;
        parse_llm_json::<AbstractionResponse>(&response.content)
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn short_hash(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:08x}", (h.finish() & 0xFFFF_FFFF) as u32)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_services::VaultPaths;
    use std::sync::Mutex;
    use zero_stores_sqlite::{
        CompactionRepository, GatewayCompactionStore, GatewayMemoryFactStore, KnowledgeDatabase,
        MemoryRepository,
    };
    use zero_stores_sqlite::vector_index::{SqliteVecIndex, VectorIndex};

    struct MockLlm {
        response: Mutex<AbstractionResponse>,
    }

    impl MockLlm {
        fn new(resp: AbstractionResponse) -> Self {
            Self {
                response: Mutex::new(resp),
            }
        }

        fn always_skip() -> Self {
            Self::new(AbstractionResponse {
                schema: String::new(),
                confidence: 0.99,
                key_fact: String::new(),
                decision: "skip".into(),
            })
        }

        fn always_fail() -> Arc<MockFailLlm> {
            Arc::new(MockFailLlm)
        }
    }

    #[async_trait]
    impl AbstractionLlm for MockLlm {
        async fn abstract_corrections(
            &self,
            _corrections: &[String],
        ) -> Result<AbstractionResponse, String> {
            Ok(self.response.lock().unwrap().clone())
        }
    }

    struct MockFailLlm;

    #[async_trait]
    impl AbstractionLlm for MockFailLlm {
        async fn abstract_corrections(
            &self,
            _corrections: &[String],
        ) -> Result<AbstractionResponse, String> {
            Err("induced failure".into())
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

    async fn seed_corrections(store: &Arc<dyn MemoryFactStore>, agent_id: &str, n: usize) {
        for i in 0..n {
            store
                .save_fact(
                    agent_id,
                    "correction",
                    &format!("corr-{i}"),
                    &format!("Don't do X when Y — correction {i}"),
                    0.9,
                    None,
                )
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn inserts_schema_when_abstractions_found() {
        let h = setup();
        seed_corrections(&h.memory_store, "agent-abs", 3).await;

        let mock = Arc::new(MockLlm::new(AbstractionResponse {
            schema: "avoid-x-when-y".into(),
            confidence: 0.85,
            key_fact: "When Y is true, always avoid X".into(),
            decision: "abstract".into(),
        }));

        let abs = CorrectionsAbstractor::new(
            h.memory_store.clone(),
            h.compaction_store.clone(),
            mock,
        );

        let stats = abs.run_cycle("run-abs", "agent-abs").await.unwrap();

        assert_eq!(stats.corrections_considered, 3);
        assert_eq!(stats.schemas_abstracted, 1);
        assert_eq!(stats.skipped_low_confidence, 0);
        assert_eq!(stats.skipped_llm_error, 0);

        let schema_facts = h
            .memory_store
            .get_facts_by_category("agent-abs", "schema", 10)
            .await
            .unwrap();
        assert_eq!(schema_facts.len(), 1);
        assert!(schema_facts[0].content.contains("avoid X"));
        assert_eq!(schema_facts[0].category, "schema");
    }

    #[tokio::test]
    async fn skips_when_fewer_than_three_corrections() {
        let h = setup();
        seed_corrections(&h.memory_store, "agent-few", 2).await;

        let abs = CorrectionsAbstractor::new(
            h.memory_store.clone(),
            h.compaction_store.clone(),
            MockLlm::always_fail(),
        );

        let stats = abs.run_cycle("run-few", "agent-few").await.unwrap();

        assert_eq!(stats.corrections_considered, 2);
        assert_eq!(stats.schemas_abstracted, 0);
        // LLM was never called, so no error either
        assert_eq!(stats.skipped_llm_error, 0);
    }

    #[tokio::test]
    async fn skips_when_decision_is_skip() {
        let h = setup();
        seed_corrections(&h.memory_store, "agent-skip", 4).await;

        let abs = CorrectionsAbstractor::new(
            h.memory_store.clone(),
            h.compaction_store.clone(),
            Arc::new(MockLlm::always_skip()),
        );

        let stats = abs.run_cycle("run-skip", "agent-skip").await.unwrap();

        assert_eq!(stats.corrections_considered, 4);
        assert_eq!(stats.schemas_abstracted, 0);
        assert_eq!(stats.skipped_low_confidence, 1);

        let schema_facts = h
            .memory_store
            .get_facts_by_category("agent-skip", "schema", 10)
            .await
            .unwrap();
        assert!(schema_facts.is_empty());
    }

    #[tokio::test]
    async fn skips_when_confidence_below_threshold() {
        let h = setup();
        seed_corrections(&h.memory_store, "agent-lowconf", 3).await;

        let mock = Arc::new(MockLlm::new(AbstractionResponse {
            schema: "something".into(),
            confidence: 0.5,
            key_fact: "some principle".into(),
            decision: "abstract".into(),
        }));

        let abs = CorrectionsAbstractor::new(
            h.memory_store.clone(),
            h.compaction_store.clone(),
            mock,
        );

        let stats = abs.run_cycle("run-lowconf", "agent-lowconf").await.unwrap();

        assert_eq!(stats.schemas_abstracted, 0);
        assert_eq!(stats.skipped_low_confidence, 1);
    }

    #[tokio::test]
    async fn idempotent_on_second_call() {
        // Calling run_cycle twice with the same corrections produces one schema fact
        // (upsert/mention_count bump, not duplicate insertion).
        let h = setup();
        seed_corrections(&h.memory_store, "agent-idem", 3).await;

        let mock = Arc::new(MockLlm::new(AbstractionResponse {
            schema: "principle-x".into(),
            confidence: 0.9,
            key_fact: "Always do X before Y".into(),
            decision: "abstract".into(),
        }));

        let abs = CorrectionsAbstractor::new(
            h.memory_store.clone(),
            h.compaction_store.clone(),
            mock,
        );

        abs.run_cycle("run-idem-1", "agent-idem").await.unwrap();
        abs.run_cycle("run-idem-2", "agent-idem").await.unwrap();

        let schema_facts = h
            .memory_store
            .get_facts_by_category("agent-idem", "schema", 10)
            .await
            .unwrap();
        assert_eq!(schema_facts.len(), 1, "upsert must not create duplicate schema facts");
    }

    #[test]
    fn short_hash_is_deterministic() {
        assert_eq!(short_hash("hello"), short_hash("hello"));
        assert_ne!(short_hash("hello"), short_hash("world"));
    }
}
```

- [ ] **Step 2.4: Run tests**

```bash
cargo test -p gateway-execution corrections_abstractor -- --nocapture 2>&1 | tail -20
```

Expected: all 6 tests pass. If `get_facts_by_category` doesn't support `"schema"` category — it does, the query is category-agnostic.

- [ ] **Step 2.5: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

- [ ] **Step 2.6: Commit**

```bash
git add gateway/gateway-execution/src/sleep/corrections_abstractor.rs
git commit -m "feat(sleep): add CorrectionsAbstractor — distill correction facts into schema facts"
```

---

## Task 3: Wire into `sleep/mod.rs` exports

**Files:**
- Modify: `gateway/gateway-execution/src/sleep/mod.rs`

- [ ] **Step 3.1: Add mod + pub use**

In `gateway/gateway-execution/src/sleep/mod.rs`, add after `pub mod compactor;`:

```rust
pub mod corrections_abstractor;
```

And add a `pub use` line after the existing `pub use verifier::LlmPairwiseVerifier;`:

```rust
pub use corrections_abstractor::{
    AbstractionLlm, AbstractionStats, CorrectionsAbstractor, LlmCorrectionsAbstractor,
};
```

- [ ] **Step 3.2: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

- [ ] **Step 3.3: Commit**

```bash
git add gateway/gateway-execution/src/sleep/mod.rs
git commit -m "chore(sleep): export CorrectionsAbstractor from sleep mod"
```

---

## Task 4: Wire into `SleepOps`, `CycleStats`, and `run_cycle` in `worker.rs`

**Files:**
- Modify: `gateway/gateway-execution/src/sleep/worker.rs`

- [ ] **Step 4.1: Add `corrections_abstractor` to `SleepOps`**

In `gateway/gateway-execution/src/sleep/worker.rs`, find:

```rust
#[derive(Clone, Default)]
pub struct SleepOps {
    pub synthesizer: Option<Arc<Synthesizer>>,
    pub pattern_extractor: Option<Arc<PatternExtractor>>,
    pub orphan_archiver: Option<Arc<OrphanArchiver>>,
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
}
```

- [ ] **Step 4.2: Add `schemas_abstracted` to `CycleStats`**

Find the `CycleStats` struct and add a field after `patterns_inserted`:

```rust
pub schemas_abstracted: u64,
```

- [ ] **Step 4.3: Add the `use` import**

At the top of `worker.rs`, the existing import line reads:

```rust
use crate::sleep::{Compactor, DecayEngine, OrphanArchiver, PatternExtractor, Pruner, Synthesizer};
```

Replace with:

```rust
use crate::sleep::{
    Compactor, CorrectionsAbstractor, DecayEngine, OrphanArchiver, PatternExtractor, Pruner,
    Synthesizer,
};
```

- [ ] **Step 4.4: Run corrections_abstractor in `run_cycle`**

In the `run_cycle` function, after the orphan archiver block, add:

```rust
// Corrections abstraction — runs after compaction so corrections are stable.
if let Some(ca) = ops.corrections_abstractor.as_ref() {
    match ca.run_cycle(&run_id, agent_id).await {
        Ok(s) => {
            stats.schemas_abstracted = s.schemas_abstracted;
        }
        Err(e) => {
            tracing::warn!(%run_id, error = %e, "corrections abstractor cycle failed");
        }
    }
}
```

Also add `schemas_abstracted` to the final `tracing::info!` log at the end of `run_cycle`. Find the existing block:

```rust
tracing::info!(
    kind,
    %run_id,
    candidates_considered = stats.candidates_considered,
    merges = stats.merges_performed,
    synthesis_inserted = stats.synthesis_facts_inserted,
    synthesis_bumped = stats.synthesis_facts_bumped,
    patterns_inserted = stats.patterns_inserted,
    prune_candidates = stats.prune_candidates,
    pruned = stats.pruned,
    pruned_failed = stats.pruned_failed,
    orphans_scanned = stats.orphans_scanned,
    orphans_archived = stats.orphans_archived,
    orphans_failed = stats.orphans_failed,
    "sleep-time cycle done"
);
```

Add `schemas_abstracted = stats.schemas_abstracted,` before `"sleep-time cycle done"`.

- [ ] **Step 4.5: Run tests**

```bash
cargo test -p gateway-execution -- --nocapture 2>&1 | grep -E "FAILED|ok\." | head -30
```

Expected: all pass (including existing worker tests — the new field has no effect on them because `SleepOps::default()` leaves it `None`).

- [ ] **Step 4.6: Commit**

```bash
git add gateway/gateway-execution/src/sleep/worker.rs
git commit -m "feat(sleep): wire CorrectionsAbstractor into SleepOps and cycle loop"
```

---

## Task 5: Wire into gateway `state/mod.rs`

**Files:**
- Modify: `gateway/src/state/mod.rs`

### Context

The sleep worker is constructed around line 793 in `gateway/src/state/mod.rs`. The `SleepOps` struct is built at line 818 with three fields. We add a fourth.

`CorrectionsAbstractor` only needs `mems` (the memory fact store) and `compstore` (compaction store), plus the LLM provider. Both are already wired into the surrounding code.

- [ ] **Step 5.1: Add `corrections_abstractor` to the `SleepOps` construction**

Find the block (around line 793–822) in `gateway/src/state/mod.rs`:

```rust
let synth_llm = Arc::new(gateway_execution::sleep::LlmSynthesizer::new(
    provider_service.clone(),
));
let synthesizer = Arc::new(gateway_execution::sleep::Synthesizer::new(
    kgs.clone(),
    eps.clone(),
    mems.clone(),
    compstore.clone(),
    synth_llm,
    embedding_client.clone(),
));
let pattern_llm = Arc::new(gateway_execution::sleep::LlmPatternExtractor::new(
    provider_service.clone(),
));
let pattern_extractor = Arc::new(gateway_execution::sleep::PatternExtractor::new(
    eps.clone(),
    conversation_store_for_state.clone(),
    prs.clone(),
    compstore.clone(),
    pattern_llm,
));
let orphan_archiver = Arc::new(gateway_execution::sleep::OrphanArchiver::new(
    kgs.clone(),
    compstore.clone(),
));
let ops = gateway_execution::sleep::SleepOps {
    synthesizer: Some(synthesizer),
    pattern_extractor: Some(pattern_extractor),
    orphan_archiver: Some(orphan_archiver),
};
```

Replace with (add 5 lines before the `SleepOps` construction):

```rust
let synth_llm = Arc::new(gateway_execution::sleep::LlmSynthesizer::new(
    provider_service.clone(),
));
let synthesizer = Arc::new(gateway_execution::sleep::Synthesizer::new(
    kgs.clone(),
    eps.clone(),
    mems.clone(),
    compstore.clone(),
    synth_llm,
    embedding_client.clone(),
));
let pattern_llm = Arc::new(gateway_execution::sleep::LlmPatternExtractor::new(
    provider_service.clone(),
));
let pattern_extractor = Arc::new(gateway_execution::sleep::PatternExtractor::new(
    eps.clone(),
    conversation_store_for_state.clone(),
    prs.clone(),
    compstore.clone(),
    pattern_llm,
));
let orphan_archiver = Arc::new(gateway_execution::sleep::OrphanArchiver::new(
    kgs.clone(),
    compstore.clone(),
));
let abstractions_llm = Arc::new(gateway_execution::sleep::LlmCorrectionsAbstractor::new(
    provider_service.clone(),
));
let corrections_abstractor = Arc::new(gateway_execution::sleep::CorrectionsAbstractor::new(
    mems.clone(),
    compstore.clone(),
    abstractions_llm,
));
let ops = gateway_execution::sleep::SleepOps {
    synthesizer: Some(synthesizer),
    pattern_extractor: Some(pattern_extractor),
    orphan_archiver: Some(orphan_archiver),
    corrections_abstractor: Some(corrections_abstractor),
};
```

- [ ] **Step 5.2: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 5.3: Full workspace test**

```bash
cargo test --workspace 2>&1 | grep -E "FAILED" | head -10
```

Expected: no failures.

- [ ] **Step 5.4: cargo fmt + clippy**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings 2>&1 | grep "^error" | head -10
```

- [ ] **Step 5.5: Commit**

```bash
git add gateway/src/state/mod.rs
git commit -m "feat(gateway): wire CorrectionsAbstractor into sleep worker"
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

---

## Self-Review Against Spec

| Spec requirement | Task |
|-----------------|------|
| Pattern abstraction from N sessions (3+) | Task 2 |
| Schema category promoted above corrections in recall | Task 1 |
| LLM abstracting into a principle | Task 2 (`LlmCorrectionsAbstractor`) |
| Idempotent — same corrections → same key, upsert not duplicate | Task 2 (`short_hash` key) |
| Wired into sleep-time cycle | Tasks 3–5 |
| Audit trail via CompactionStore | Task 2 (`record_synthesis`) |
| Does not break existing cycle if abstractor errors | Task 4 (Err branch logs + continues) |
