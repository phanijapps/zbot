# Intent Analysis v2 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the intent analysis middleware from a function receiving pre-collected arrays to an autonomous agent that indexes and semantically searches skills, agents, and wards using local embeddings.

**Architecture:** The `analyze_intent` function takes service references instead of data arrays. It indexes resources into `memory_facts` with embeddings (via `MemoryFactStore`), semantically searches for relevant resources per user message, sends only top-N matches to the LLM, and logs every step with structured `tracing` output.

**Tech Stack:** Rust, `zero_core::MemoryFactStore` trait, `gateway_services::{SkillService, AgentService}`, fastembed local embeddings, serde_json

**Spec:** `docs/superpowers/specs/2026-03-20-intent-analysis-v2-design.md`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Types, LLM prompt, `analyze_intent()` (autonomous), `ensure_indexed()`, `search_resources()`, `format_user_template()`, `inject_intent_context()`, logging |
| `gateway/gateway-execution/src/runner.rs` | Simplified call: pass services not arrays, remove manual indexing block |
| `gateway/gateway-execution/tests/intent_analysis_tests.rs` | Updated integration tests with MockMemoryFactStore |

---

## Chunk 1: Autonomous analyze_intent with indexing and semantic search

### Task 1: Rewrite analyze_intent to take services

**Files:**
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs`

This task rewrites the core `analyze_intent` function and adds three new internal functions: `ensure_indexed`, `search_resources`, and updated `format_user_template`.

- [ ] **Step 1: Add new imports**

At top of `intent_analysis.rs`, replace the current imports with:

```rust
use agent_runtime::{ChatMessage, LlmClient};
use gateway_services::{AgentService, SharedVaultPaths, SkillService};
use serde::Deserialize;
use serde_json::Value;
use zero_core::MemoryFactStore;
```

- [ ] **Step 2: Add the `ensure_indexed` function**

This checks if skills/agents/wards are already in memory_facts. If not (or count mismatch), indexes them with embeddings.

```rust
/// Ensures skills, agents, and wards are indexed in memory_facts for semantic search.
/// Compares disk count vs indexed count; re-indexes on mismatch.
async fn ensure_indexed(
    fact_store: &dyn MemoryFactStore,
    skill_service: &SkillService,
    agent_service: &AgentService,
    vault_paths: &SharedVaultPaths,
) {
    // Index skills
    match skill_service.list().await {
        Ok(skills) => {
            let indexed = count_facts(fact_store, "skill").await;
            if indexed != skills.len() {
                tracing::info!(
                    disk = skills.len(),
                    indexed = indexed,
                    "Indexing skills into memory"
                );
                for skill in &skills {
                    let key = format!("skill:{}", skill.name);
                    let content = format!(
                        "{} | {} | category: {}",
                        skill.name, skill.description, skill.category
                    );
                    if let Err(e) = fact_store
                        .save_fact("root", "skill", &key, &content, 1.0, None)
                        .await
                    {
                        tracing::debug!("Failed to index skill {}: {}", skill.name, e);
                    }
                }
            }
        }
        Err(e) => tracing::warn!("Failed to list skills for indexing: {}", e),
    }

    // Index agents
    match agent_service.list().await {
        Ok(agents) => {
            let indexed = count_facts(fact_store, "agent").await;
            if indexed != agents.len() {
                tracing::info!(
                    disk = agents.len(),
                    indexed = indexed,
                    "Indexing agents into memory"
                );
                for agent in &agents {
                    let key = format!("agent:{}", agent.id);
                    let content = format!("{} | {}", agent.id, agent.description);
                    if let Err(e) = fact_store
                        .save_fact("root", "agent", &key, &content, 1.0, None)
                        .await
                    {
                        tracing::debug!("Failed to index agent {}: {}", agent.id, e);
                    }
                }
            }
        }
        Err(e) => tracing::warn!("Failed to list agents for indexing: {}", e),
    }

    // Index wards
    let wards_dir = vault_paths.vault_dir().join("wards");
    if let Ok(entries) = std::fs::read_dir(&wards_dir) {
        let ward_dirs: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        let indexed = count_facts(fact_store, "ward").await;
        if indexed != ward_dirs.len() {
            tracing::info!(
                disk = ward_dirs.len(),
                indexed = indexed,
                "Indexing wards into memory"
            );
            for entry in &ward_dirs {
                let name = entry.file_name().to_string_lossy().to_string();
                let agents_md_path = entry.path().join("AGENTS.md");
                let purpose = if agents_md_path.exists() {
                    std::fs::read_to_string(&agents_md_path)
                        .ok()
                        .and_then(|content| {
                            // Extract first non-empty, non-heading line as purpose
                            content
                                .lines()
                                .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
                                .map(|l| l.trim().to_string())
                        })
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                let key = format!("ward:{}", name);
                let content = if purpose.is_empty() {
                    name.clone()
                } else {
                    format!("{} | {}", name, purpose)
                };
                if let Err(e) = fact_store
                    .save_fact("root", "ward", &key, &content, 1.0, None)
                    .await
                {
                    tracing::debug!("Failed to index ward {}: {}", name, e);
                }
            }
        }
    }
}

/// Count indexed facts for a given category by recalling with a broad query.
async fn count_facts(fact_store: &dyn MemoryFactStore, category: &str) -> usize {
    // Recall with a broad query to get all facts, filter by category
    match fact_store.recall_facts("root", category, 100).await {
        Ok(result) => result
            .get("results")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|item| item.get("category").and_then(|c| c.as_str()) == Some(category))
                    .count()
            })
            .unwrap_or(0),
        Err(_) => 0,
    }
}
```

- [ ] **Step 3: Add the `search_resources` function**

Semantically searches memory_facts for resources relevant to the user message, returning separate lists.

```rust
/// Semantic search result grouped by resource type.
struct SearchResults {
    skills: Vec<Value>,
    agents: Vec<Value>,
    wards: Vec<String>,
}

/// Search memory_facts for resources semantically relevant to the user message.
async fn search_resources(
    fact_store: &dyn MemoryFactStore,
    user_message: &str,
    limit: usize,
) -> SearchResults {
    let mut skills = Vec::new();
    let mut agents = Vec::new();
    let mut wards = Vec::new();

    // Single recall with generous limit, then filter by category
    match fact_store.recall_facts("root", user_message, limit).await {
        Ok(result) => {
            if let Some(items) = result.get("results").and_then(|r| r.as_array()) {
                for item in items {
                    let category = item
                        .get("category")
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    let content = item
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    let key = item
                        .get("key")
                        .and_then(|k| k.as_str())
                        .unwrap_or("");

                    match category {
                        "skill" => {
                            let name = key.strip_prefix("skill:").unwrap_or(key);
                            let parts: Vec<&str> = content.splitn(2, " | ").collect();
                            let desc = parts.get(1).unwrap_or(&"");
                            skills.push(serde_json::json!({
                                "name": name,
                                "description": desc,
                            }));
                        }
                        "agent" => {
                            let name = key.strip_prefix("agent:").unwrap_or(key);
                            let parts: Vec<&str> = content.splitn(2, " | ").collect();
                            let desc = parts.get(1).unwrap_or(&"");
                            agents.push(serde_json::json!({
                                "name": name,
                                "description": desc,
                            }));
                        }
                        "ward" => {
                            wards.push(content.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("Semantic search failed: {}", e);
        }
    }

    SearchResults {
        skills,
        agents,
        wards,
    }
}
```

- [ ] **Step 4: Rewrite `analyze_intent` to use services**

Replace the current `analyze_intent` function:

```rust
/// Autonomous intent analysis: indexes resources, searches semantically, calls LLM.
pub async fn analyze_intent(
    llm_client: &dyn LlmClient,
    user_message: &str,
    fact_store: &dyn MemoryFactStore,
    skill_service: &SkillService,
    agent_service: &AgentService,
    vault_paths: &SharedVaultPaths,
) -> Result<IntentAnalysis, String> {
    tracing::info!("Starting intent analysis for root session");

    // Step 1: Ensure resources are indexed with embeddings
    ensure_indexed(fact_store, skill_service, agent_service, vault_paths).await;

    // Step 2: Semantic search for relevant resources
    let results = search_resources(fact_store, user_message, 30).await;

    tracing::info!(
        skills_matched = results.skills.len(),
        agents_matched = results.agents.len(),
        wards_matched = results.wards.len(),
        "Semantic search complete"
    );

    // Step 3: Build LLM prompt with only relevant resources
    let messages = vec![
        ChatMessage::system(INTENT_ANALYSIS_PROMPT.to_string()),
        ChatMessage::user(format_user_template(
            user_message,
            &results.skills,
            &results.agents,
            &results.wards,
        )),
    ];

    tracing::info!(
        skills = results.skills.len(),
        agents = results.agents.len(),
        wards = results.wards.len(),
        "LLM call — sending relevant resources"
    );

    // Step 4: Call LLM
    let response = llm_client
        .chat(messages, None)
        .await
        .map_err(|e| format!("Intent analysis LLM call failed: {}", e))?;

    tracing::debug!(raw_response = %response.content, "LLM raw response");

    let content = strip_markdown_fences(&response.content);

    // Step 5: Parse response
    let analysis = serde_json::from_str::<IntentAnalysis>(&content)
        .map_err(|e| format!("Failed to parse intent analysis JSON: {}", e))?;

    tracing::info!(
        primary_intent = %analysis.primary_intent,
        hidden_intents = analysis.hidden_intents.len(),
        ward = %analysis.ward_recommendation.ward_name,
        approach = %analysis.execution_strategy.approach,
        "Intent analysis complete"
    );

    Ok(analysis)
}
```

- [ ] **Step 5: Update `format_user_template` to accept `&[String]` for wards**

The wards parameter changes from directory names to enriched strings (e.g., "financial-analysis | Stock analysis workspace"). Update the formatting:

```rust
pub fn format_user_template(
    message: &str,
    skills: &[Value],
    agents: &[Value],
    wards: &[String],
) -> String {
    // ... (same as current implementation — no change needed)
    // wards are already strings like "financial-analysis | purpose from AGENTS.md"
}
```

Actually `format_user_template` signature stays the same. No code change needed here.

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p gateway-execution`
Expected: Will fail because runner.rs still calls old signature. That's ok — Task 2 fixes it.

- [ ] **Step 7: Commit (WIP — compiles with runner update pending)**

Don't commit yet. Proceed to Task 2.

---

### Task 2: Simplify runner.rs integration

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Remove manual indexing block**

In `create_executor()`, delete the manual indexing loop (the block that iterates `available_skills` and `available_agents` calling `save_fact`). This was ~30 lines added in the previous iteration.

Also delete the `existing_wards` filesystem scan (~10 lines) and `fact_store_for_indexing` clone.

- [ ] **Step 2: Update the `analyze_intent` call**

Change the enrichment block to pass services instead of arrays:

```rust
        // Intent analysis enrichment (root agent first turn only)
        let enriched_agent = if is_root {
            if let Some(msg) = user_message {
                let llm_config = agent_runtime::LlmConfig::new(
                    provider.base_url.clone(),
                    provider.api_key.clone(),
                    agent.model.clone(),
                    provider.id.clone().unwrap_or_else(|| provider.name.clone()),
                )
                .with_temperature(agent.temperature)
                .with_max_tokens(agent.max_tokens)
                .with_thinking(false);
                match agent_runtime::OpenAiClient::new(llm_config) {
                    Ok(raw_client) => {
                        let retry_client = agent_runtime::RetryingLlmClient::new(
                            std::sync::Arc::new(raw_client),
                            agent_runtime::RetryPolicy::default(),
                        );
                        let llm_client: std::sync::Arc<dyn agent_runtime::LlmClient> =
                            std::sync::Arc::new(retry_client);
                        // fact_store is already built above (line ~905)
                        // Use it for indexing + search before it's moved into builder
                        let fs_ref = fact_store.as_ref().map(|f| f.as_ref());
                        if let Some(fs) = fs_ref {
                            match analyze_intent(
                                llm_client.as_ref(),
                                msg,
                                fs,
                                &self.skill_service,
                                &self.agent_service,
                                &self.paths,
                            )
                            .await
                            {
                                Ok(analysis) => {
                                    let mut enriched = agent.clone();
                                    inject_intent_context(&mut enriched.instructions, &analysis);
                                    tracing::info!(
                                        chars = enriched.instructions.len() - agent.instructions.len(),
                                        "Enrichment injected into system prompt"
                                    );
                                    Some(enriched)
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Intent analysis failed, proceeding without enrichment: {}",
                                        e
                                    );
                                    None
                                }
                            }
                        } else {
                            tracing::warn!("No fact store available — skipping intent analysis");
                            None
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to create LLM client for intent analysis: {}",
                            e
                        );
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        let agent_for_build = enriched_agent.as_ref().unwrap_or(agent);
```

Key changes:
- `fact_store` is borrowed (not cloned) before it's moved into the builder
- Services passed by reference: `&self.skill_service`, `&self.agent_service`, `&self.paths`
- No more `available_skills`/`available_agents`/`existing_wards` in the call

**Important**: The `fact_store` variable is an `Option<Arc<dyn MemoryFactStore>>`. It gets moved into `builder.with_fact_store(fs)` at line ~936. The enrichment must use it BEFORE that move. The current code already has `fact_store_for_indexing` clone — keep using that clone for the enrichment call, then let the original `fact_store` move into the builder.

Actually, simplest: keep the `fact_store_for_indexing = fact_store.clone()` line and pass `fact_store_for_indexing.as_ref().unwrap()` to `analyze_intent`. Remove the manual indexing loop that used `fact_store_for_indexing`.

- [ ] **Step 3: Remove the `collect_skills_summary` / `collect_agents_summary` calls IF they are only used for enrichment**

Check: are `available_skills` and `available_agents` still needed for the executor's initial state? YES — `ListSkillsTool` and `ListAgentsTool` read them from state. Keep the collection but stop passing them to `analyze_intent`.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p gateway-execution`
Expected: No errors

- [ ] **Step 5: Run tests**

Run: `cargo test -p gateway-execution`
Expected: Unit tests for types/prompt/injection still pass. Async tests for `analyze_intent` will fail because the signature changed. Fix in Task 3.

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/middleware/intent_analysis.rs gateway/gateway-execution/src/runner.rs
git commit -m "feat(intent): autonomous intent analysis with indexing and semantic search"
```

---

### Task 3: Update all tests

**Files:**
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs` (unit tests)
- Modify: `gateway/gateway-execution/tests/intent_analysis_tests.rs` (integration tests)

- [ ] **Step 1: Create MockMemoryFactStore**

Add to the test module in `intent_analysis.rs`:

```rust
use std::sync::Mutex;
use std::collections::HashMap;

struct MockMemoryFactStore {
    facts: Mutex<HashMap<String, (String, String)>>, // key -> (category, content)
}

impl MockMemoryFactStore {
    fn new() -> Self {
        Self {
            facts: Mutex::new(HashMap::new()),
        }
    }

    fn with_facts(facts: Vec<(&str, &str, &str)>) -> Self {
        let store = Self::new();
        let mut map = store.facts.lock().unwrap();
        for (category, key, content) in facts {
            map.insert(key.to_string(), (category.to_string(), content.to_string()));
        }
        store
    }
}

#[async_trait]
impl MemoryFactStore for MockMemoryFactStore {
    async fn save_fact(
        &self,
        _agent_id: &str,
        category: &str,
        key: &str,
        content: &str,
        _confidence: f64,
        _session_id: Option<&str>,
    ) -> Result<Value, String> {
        self.facts.lock().unwrap().insert(
            key.to_string(),
            (category.to_string(), content.to_string()),
        );
        Ok(serde_json::json!({"success": true}))
    }

    async fn recall_facts(
        &self,
        _agent_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Value, String> {
        let facts = self.facts.lock().unwrap();
        let results: Vec<Value> = facts
            .iter()
            .filter(|(_, (_, content))| {
                // Simple substring match for tests
                content.to_lowercase().contains(&query.to_lowercase())
                    || query.to_lowercase().contains(&content.split(" | ").next().unwrap_or("").to_lowercase())
            })
            .take(limit)
            .map(|(key, (category, content))| {
                serde_json::json!({
                    "key": key,
                    "category": category,
                    "content": content,
                    "confidence": 1.0,
                    "score": 0.9,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "query": query,
            "results": results,
            "count": results.len(),
        }))
    }
}
```

- [ ] **Step 2: Update async tests to use new signature**

The `analyze_intent` tests need to pass a `MockMemoryFactStore`, mock `SkillService`, mock `AgentService`, and mock `SharedVaultPaths`. Since `SkillService` and `AgentService` are concrete types (not traits), we can't easily mock them.

**Alternative approach**: Since the async tests mock the LLM client (which returns a fixed JSON), and the `ensure_indexed` + `search_resources` calls happen before the LLM call, the tests can use a pre-populated `MockMemoryFactStore` that already has the indexed data. The `SkillService`/`AgentService`/`VaultPaths` won't be called if the fact store already has matching counts.

However, this requires creating real `SkillService`/`AgentService` instances pointing to temp dirs. Use `tempfile::TempDir`.

**Simplest approach for tests**: Create a helper that builds test services:

```rust
fn create_test_services() -> (Arc<SkillService>, Arc<AgentService>, SharedVaultPaths) {
    let temp = tempfile::TempDir::new().unwrap();
    let vault = temp.into_path();

    // Create wards dir
    std::fs::create_dir_all(vault.join("wards")).unwrap();
    // Create skills dir
    std::fs::create_dir_all(vault.join("skills")).unwrap();
    // Create agents dir
    std::fs::create_dir_all(vault.join("agents")).unwrap();

    let paths = Arc::new(gateway_services::VaultPaths::new(vault));
    let skill_service = Arc::new(SkillService::new(paths.clone()));
    let agent_service = Arc::new(AgentService::new(paths.clone()));

    (skill_service, agent_service, paths)
}
```

Then update each async test to use the new signature. The mock fact store can be pre-populated with skills/agents so `ensure_indexed` sees matching counts and skips indexing.

- [ ] **Step 3: Update integration tests similarly**

Same pattern in `tests/intent_analysis_tests.rs`.

- [ ] **Step 4: Run all tests**

Run: `cargo test -p gateway-execution`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/middleware/intent_analysis.rs gateway/gateway-execution/tests/intent_analysis_tests.rs
git commit -m "test(intent): update tests for autonomous intent analysis"
```

---

## Chunk 2: Documentation and verification

### Task 4: Update AGENTS.md files

**Files:**
- Modify: `gateway/gateway-execution/AGENTS.md` (if exists, otherwise skip)
- Check: `runtime/agent-tools/AGENTS.md` (note indexer module status)

- [ ] **Step 1: Check if `gateway/gateway-execution/AGENTS.md` exists**

If it exists, add a section documenting the middleware:

```markdown
## Middleware

### Intent Analysis (`src/middleware/intent_analysis.rs`)
Pre-execution enrichment that runs before root agent sessions:
1. Indexes skills, agents, and wards into memory_facts with local embeddings
2. Semantically searches for resources relevant to the user message
3. Calls LLM with top matches to produce an execution graph
4. Injects results into the system prompt

The middleware is autonomous — it discovers resources via services, not pre-collected arrays.
```

- [ ] **Step 2: Commit**

```bash
git add -A
git commit -m "docs: update AGENTS.md with intent analysis middleware architecture"
```

---

### Task 5: Final verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 2: Full test suite**

Run: `cargo test -p gateway-execution`
Expected: All tests pass

- [ ] **Step 3: Verify logging works**

Read through the code and confirm every step has a `tracing::info!` or `tracing::warn!` call. The key events:
- "Starting intent analysis for root session"
- "Indexing skills/agents/wards into memory"
- "Semantic search complete"
- "LLM call — sending relevant resources"
- "Intent analysis complete"
- "Enrichment injected into system prompt"

- [ ] **Step 4: Commit any remaining changes**

```bash
git status
# If clean, no commit needed
```
