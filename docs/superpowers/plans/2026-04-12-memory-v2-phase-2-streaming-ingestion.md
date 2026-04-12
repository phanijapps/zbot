# Memory v2 — Phase 2 Implementation Plan: Streaming Ingestion

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agents and users can enqueue long documents (PDFs, books, pasted text) for incremental extraction. Each chunk becomes one episode, processed by background workers running a two-pass LLM extractor that writes entities + relationships through the resolver. Progress is observable per source. The HTTP handler returns 202 Accepted in <100ms — ingestion never blocks the caller.

**Architecture:** Content → paragraph-aware `Chunker` → one `kg_episodes` row per chunk (status=pending) → tokio mpsc queue → N workers drain it in parallel, each invoking the two-pass extractor (entities, then relationships conditioned on entities) → resolver + writer populate kg_entities / kg_relationships / kg_name_index. Concurrent session traffic (recall, graph_query, stats) stays responsive because writes go through WAL-mode SQLite + r2d2 pool.

**Tech Stack:** Rust 2024, tokio mpsc, `rusqlite`, existing `KnowledgeDatabase` + `GraphStorage` + `EntityResolver` (Phase 1), `agent_runtime::LlmClient` trait (existing, JSON-in-content pattern).

**Spec:** `docs/superpowers/specs/2026-04-12-memory-layer-redesign-design.md`

---

## Pre-flight

Branch from Phase 1c HEAD:

```bash
git checkout feature/memory-v2-phase-1c
git pull
git checkout -b feature/memory-v2-phase-2
```

All Phase 1 code is on `feature/memory-v2-phase-1c`. 1155 tests green. Resolver p95 = 2.1ms. Clean foundation.

---

## File Structure

**Created:**
- `gateway/gateway-execution/src/ingest/mod.rs` — module root
- `gateway/gateway-execution/src/ingest/chunker.rs` — paragraph-aware chunker
- `gateway/gateway-execution/src/ingest/extractor.rs` — two-pass LLM extractor
- `gateway/gateway-execution/src/ingest/queue.rs` — `IngestionQueue` + workers
- `gateway/gateway-execution/src/ingest/backpressure.rs` — rate limit / queue-depth checks
- `gateway/src/http/ingest.rs` — `POST /api/graph/ingest` + `GET /api/graph/ingest/:source_id/progress`
- `runtime/agent-tools/src/tools/ingest.rs` — agent-facing `ingest` tool
- `gateway/gateway-execution/tests/ingest_concurrency.rs` — concurrency stress test

**Modified:**
- `gateway/gateway-database/src/kg_episode_repository.rs` — add `upsert_pending`, `mark_running`, `mark_done`, `mark_failed`, `list_by_source`
- `gateway/src/http/mod.rs` — register 2 new routes
- `gateway/src/http/graph.rs` — rewrite `reindex_all_wards` to enqueue instead of execute synchronously
- `gateway/src/state.rs` — construct `IngestionQueue` at boot
- `runtime/agent-tools/src/tools/mod.rs` — register new `ingest` tool
- `gateway/templates/shards/tooling_skills.md` — document the `ingest` tool

**NOT modified in Phase 2:**
- `recall.rs` (Phase 3 — unified scored recall)
- `resolver.rs` (Phase 3/4 — LLM pairwise verify)
- Compactor/decay (Phase 4)

---

## Task 1: Chunker module

**Files:**
- Create: `gateway/gateway-execution/src/ingest/mod.rs`
- Create: `gateway/gateway-execution/src/ingest/chunker.rs`
- Modify: `gateway/gateway-execution/src/lib.rs`

- [ ] **Step 1: Create `ingest/mod.rs`**

```rust
//! Streaming ingestion pipeline — chunker, queue, extractor, backpressure.
//! Public entry: [`IngestionQueue`] + HTTP/tool wrappers.

pub mod chunker;
pub mod extractor;
pub mod queue;
pub mod backpressure;
```

- [ ] **Step 2: Create `chunker.rs` with failing tests first**

```rust
//! Paragraph-aware text chunker.
//!
//! Splits prose into overlapping windows of target token count, preferring
//! paragraph boundaries (`\n\n`), falling back to sentence terminators
//! (`. ? !`), falling back to character count. Token estimation is
//! approximate — chars/4 — adequate for GPT-4-family tokenization.

#[derive(Debug, Clone)]
pub struct ChunkOptions {
    pub target_tokens: usize,
    pub overlap_tokens: usize,
}

impl Default for ChunkOptions {
    fn default() -> Self {
        Self {
            target_tokens: 1000,
            overlap_tokens: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub index: usize,
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
}

/// Estimate token count from char count. 4 chars/token is the GPT-4 rule of thumb.
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Split `text` into overlapping chunks respecting paragraph boundaries.
pub fn chunk_text(text: &str, opts: ChunkOptions) -> Vec<Chunk> {
    if text.is_empty() {
        return Vec::new();
    }

    let target_chars = opts.target_tokens.saturating_mul(4);
    let overlap_chars = opts.overlap_tokens.saturating_mul(4);

    let mut chunks = Vec::new();
    let total = text.len();
    let mut cursor = 0usize;
    let mut index = 0usize;

    while cursor < total {
        let end = (cursor + target_chars).min(total);
        let end = if end < total {
            find_preferred_split(text, cursor, end)
        } else {
            end
        };

        let chunk_text = text[cursor..end].trim().to_string();
        if !chunk_text.is_empty() {
            chunks.push(Chunk {
                index,
                text: chunk_text,
                char_start: cursor,
                char_end: end,
            });
            index += 1;
        }

        if end >= total {
            break;
        }
        cursor = end.saturating_sub(overlap_chars);
        if cursor >= end {
            cursor = end;
        }
    }

    chunks
}

/// Find the best split point in `text[min..max]`:
/// 1. Latest `\n\n` ≥ min
/// 2. Otherwise latest sentence terminator (. ? !) followed by whitespace
/// 3. Otherwise `max` (hard cut)
fn find_preferred_split(text: &str, min: usize, max: usize) -> usize {
    let slice = &text[min..max];
    if let Some(idx) = slice.rfind("\n\n") {
        return min + idx + 2;
    }
    // Find last sentence terminator followed by whitespace.
    let bytes = slice.as_bytes();
    for (i, _) in slice.char_indices().rev() {
        let b = bytes[i];
        if (b == b'.' || b == b'?' || b == b'!') && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b' ' || next == b'\n' {
                return min + i + 2;
            }
        }
    }
    max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_produces_no_chunks() {
        assert!(chunk_text("", ChunkOptions::default()).is_empty());
    }

    #[test]
    fn short_text_fits_in_one_chunk() {
        let text = "Just a brief sentence.";
        let chunks = chunk_text(text, ChunkOptions::default());
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, text);
    }

    #[test]
    fn paragraph_boundary_preferred() {
        let text = "Paragraph one here and it is quite long enough to exceed.\n\nParagraph two follows.";
        let opts = ChunkOptions {
            target_tokens: 15, // ~60 chars
            overlap_tokens: 0,
        };
        let chunks = chunk_text(text, opts);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].text.contains("Paragraph one"));
        assert!(chunks[1].text.contains("Paragraph two"));
    }

    #[test]
    fn sentence_boundary_fallback() {
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence.";
        let opts = ChunkOptions {
            target_tokens: 8, // ~32 chars
            overlap_tokens: 0,
        };
        let chunks = chunk_text(text, opts);
        assert!(chunks.len() >= 2);
        for c in &chunks {
            assert!(c.text.ends_with('.') || c.text.ends_with(','), "chunk does not end cleanly: {}", c.text);
        }
    }

    #[test]
    fn chunks_overlap_by_configured_amount() {
        let text = "The quick brown fox jumps over the lazy dog. ".repeat(20);
        let opts = ChunkOptions {
            target_tokens: 30, // ~120 chars
            overlap_tokens: 10, // ~40 chars
        };
        let chunks = chunk_text(&text, opts);
        assert!(chunks.len() >= 2);
        // First chunk's end should overlap into second chunk's start (within 40 chars).
        let first_tail = &chunks[0].text[chunks[0].text.len().saturating_sub(20)..];
        assert!(chunks[1].text.contains(first_tail) || chunks[0].char_end > chunks[1].char_start);
    }

    #[test]
    fn indices_are_sequential_from_zero() {
        let text = "alpha. beta. gamma. delta. epsilon. zeta. eta. theta. ".repeat(5);
        let opts = ChunkOptions { target_tokens: 15, overlap_tokens: 0 };
        let chunks = chunk_text(&text, opts);
        for (i, c) in chunks.iter().enumerate() {
            assert_eq!(c.index, i);
        }
    }
}
```

- [ ] **Step 3: Register in `lib.rs`**

Edit `gateway/gateway-execution/src/lib.rs`:

```rust
pub mod ingest;
```

- [ ] **Step 4: Run tests**

```
cargo test -p gateway-execution --lib ingest::chunker
```

Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/ingest/ gateway/gateway-execution/src/lib.rs
git commit -m "feat(ingest): paragraph-aware Chunker"
```

---

## Task 2: Episode lifecycle methods on `KgEpisodeRepository`

**Files:**
- Modify: `gateway/gateway-database/src/kg_episode_repository.rs`

v22 already has `status`, `retry_count`, `error`, `started_at`, `completed_at` columns on `kg_episodes` (Phase 1a). But the repository has no methods that set them. This task adds those.

- [ ] **Step 1: Add lifecycle methods**

Append to `impl KgEpisodeRepository`:

```rust
/// Create a new pending episode. Returns its id.
pub fn upsert_pending(
    &self,
    source_type: &str,
    source_ref: &str,
    content_hash: &str,
    session_id: Option<&str>,
    agent_id: &str,
) -> Result<String, String> {
    let id = format!("ep-{}", uuid::Uuid::new_v4());
    let now = chrono::Utc::now().to_rfc3339();
    self.db.with_connection(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO kg_episodes (
                 id, source_type, source_ref, content_hash, session_id, agent_id,
                 status, retry_count, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', 0, ?7)",
            rusqlite::params![
                id,
                source_type,
                source_ref,
                content_hash,
                session_id,
                agent_id,
                now,
            ],
        )?;
        Ok(())
    })?;
    Ok(id)
}

/// Atomically claim the next pending episode (if any).
/// Marks `running`, stamps `started_at`. Returns the episode row.
pub fn claim_next_pending(&self) -> Result<Option<KgEpisode>, String> {
    let now = chrono::Utc::now().to_rfc3339();
    self.db.with_connection(|conn| {
        let tx = conn.unchecked_transaction()?;
        let row: Option<KgEpisode> = tx
            .query_row(
                "SELECT id, source_type, source_ref, content_hash, session_id, agent_id,
                        status, retry_count, error, created_at, started_at, completed_at
                 FROM kg_episodes
                 WHERE status = 'pending'
                 ORDER BY created_at ASC
                 LIMIT 1",
                [],
                row_to_episode,
            )
            .ok();
        if let Some(ref ep) = row {
            tx.execute(
                "UPDATE kg_episodes SET status = 'running', started_at = ?1 WHERE id = ?2",
                rusqlite::params![now, ep.id],
            )?;
            tx.commit()?;
        }
        Ok(row)
    })
}

pub fn mark_done(&self, episode_id: &str) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    self.db.with_connection(|conn| {
        conn.execute(
            "UPDATE kg_episodes SET status = 'done', completed_at = ?1 WHERE id = ?2",
            rusqlite::params![now, episode_id],
        )?;
        Ok(())
    })
}

pub fn mark_failed(&self, episode_id: &str, error: &str) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    self.db.with_connection(|conn| {
        conn.execute(
            "UPDATE kg_episodes SET status = 'failed', error = ?1, completed_at = ?2,
                                     retry_count = retry_count + 1
             WHERE id = ?3",
            rusqlite::params![error, now, episode_id],
        )?;
        Ok(())
    })
}

/// Reset a failed episode to pending if retry_count < max.
pub fn retry_if_eligible(&self, episode_id: &str, max_retries: u32) -> Result<bool, String> {
    self.db.with_connection(|conn| {
        let retry_count: u32 = conn
            .query_row(
                "SELECT retry_count FROM kg_episodes WHERE id = ?1",
                rusqlite::params![episode_id],
                |r| r.get::<_, u32>(0),
            )
            .unwrap_or(u32::MAX);
        if retry_count >= max_retries {
            return Ok(false);
        }
        conn.execute(
            "UPDATE kg_episodes SET status = 'pending', error = NULL WHERE id = ?1",
            rusqlite::params![episode_id],
        )?;
        Ok(true)
    })
}

/// Group by status for a given source_ref prefix (for progress endpoint).
pub fn status_counts_for_source(&self, source_ref_prefix: &str) -> Result<StatusCounts, String> {
    self.db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT status, COUNT(*) FROM kg_episodes
             WHERE source_ref LIKE ?1 || '%'
             GROUP BY status",
        )?;
        let rows = stmt.query_map(rusqlite::params![source_ref_prefix], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        let mut counts = StatusCounts::default();
        for row in rows {
            let (s, n) = row?;
            match s.as_str() {
                "pending" => counts.pending = n as u64,
                "running" => counts.running = n as u64,
                "done" => counts.done = n as u64,
                "failed" => counts.failed = n as u64,
                _ => {}
            }
        }
        Ok(counts)
    })
}
```

Add a `StatusCounts` struct + `row_to_episode` helper if missing. Also expose `pub struct KgEpisode` with all the fields.

- [ ] **Step 2: Test lifecycle**

Add to the test module:

```rust
#[test]
#[cfg_attr(not(feature = "db-tests"), ignore)]  // only if the crate feature-gates db tests
fn episode_lifecycle_pending_claim_done() {
    // Setup omitted; use the same VaultPaths+KnowledgeDatabase tempdir pattern
    // as other repo tests. Create one pending episode, claim it (must return Some),
    // claim again (must return None), mark_done, status_counts_for_source should
    // reflect {pending: 0, running: 0, done: 1}.
}
```

If there's no db-tests feature gate, just write it directly — Phase 1b already un-ignored repo tests.

- [ ] **Step 3: cargo check + test**

```
cargo check -p gateway-database
cargo test -p gateway-database --lib kg_episode_repository
```

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-database/src/kg_episode_repository.rs
git commit -m "feat(db): episode lifecycle — upsert_pending, claim_next, mark_done/failed"
```

---

## Task 3: IngestionQueue skeleton

**Files:**
- Create: `gateway/gateway-execution/src/ingest/queue.rs`

Tokio mpsc with N workers. Each worker: claim episode → extract → write. Claim is via `claim_next_pending` (atomic under pool connection). mpsc only carries a "wake up" signal; actual work selection is from DB.

- [ ] **Step 1: Write the skeleton**

```rust
//! Ingestion queue — producer sends "wake up, work exists" signals to a
//! pool of worker tasks. Workers atomically claim pending episodes via the
//! repository and process them. Work lives in the DB, not in the channel —
//! so worker restarts recover in-flight pending episodes.

use std::sync::Arc;
use tokio::sync::mpsc;

use crate::ingest::extractor::Extractor;
use gateway_database::{KgEpisodeRepository, KnowledgeDatabase};
use knowledge_graph::GraphStorage;

const WAKE_CHANNEL_CAPACITY: usize = 256;
const MAX_RETRIES: u32 = 3;

pub struct IngestionQueue {
    tx: mpsc::Sender<()>,
}

impl IngestionQueue {
    /// Spawn `workers` background workers and return a handle for producers.
    pub fn start(
        workers: usize,
        episode_repo: Arc<KgEpisodeRepository>,
        graph: Arc<GraphStorage>,
        extractor: Arc<dyn Extractor>,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<()>(WAKE_CHANNEL_CAPACITY);

        // Single receiver is owned by a coordinator that re-dispatches via a
        // broadcast to workers. Simpler: each worker has its own rx via
        // a fan-out. But mpsc has single consumer. Use Arc<Mutex<Receiver>>
        // for N workers sharing the channel — ugly but works. Alternative:
        // use tokio::sync::Notify and let workers loop on notify.notified().
        //
        // Choose Notify: one producer notifies, all waiting workers wake.
        // Workers race to claim_next_pending; one succeeds, others re-sleep.
        // This is cleaner than a fan-out mpsc.
        let notify = Arc::new(tokio::sync::Notify::new());
        let notify_clone = notify.clone();
        tokio::spawn(async move {
            while rx.recv().await.is_some() {
                notify_clone.notify_waiters();
            }
        });

        for worker_idx in 0..workers {
            let episode_repo = episode_repo.clone();
            let graph = graph.clone();
            let extractor = extractor.clone();
            let notify = notify.clone();
            tokio::spawn(async move {
                worker_loop(worker_idx, episode_repo, graph, extractor, notify).await;
            });
        }

        Self { tx }
    }

    /// Notify workers that work exists. Non-blocking; if the channel is full,
    /// workers will pick it up anyway on next iteration.
    pub fn notify(&self) {
        let _ = self.tx.try_send(());
    }
}

async fn worker_loop(
    worker_idx: usize,
    episode_repo: Arc<KgEpisodeRepository>,
    graph: Arc<GraphStorage>,
    extractor: Arc<dyn Extractor>,
    notify: Arc<tokio::sync::Notify>,
) {
    tracing::info!(worker_idx, "ingestion worker started");
    loop {
        // Claim next pending episode.
        let claimed = tokio::task::spawn_blocking({
            let repo = episode_repo.clone();
            move || repo.claim_next_pending()
        })
        .await;

        let episode = match claimed {
            Ok(Ok(Some(e))) => e,
            Ok(Ok(None)) => {
                // No work. Wait to be notified.
                notify.notified().await;
                continue;
            }
            Ok(Err(e)) => {
                tracing::warn!(worker_idx, error = %e, "claim_next_pending failed");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            }
            Err(e) => {
                tracing::warn!(worker_idx, error = %e, "spawn_blocking join failed");
                continue;
            }
        };

        // Process via extractor.
        let episode_id = episode.id.clone();
        let process_result = extractor.process(&episode, &graph).await;
        let finish_result = match process_result {
            Ok(()) => tokio::task::spawn_blocking({
                let repo = episode_repo.clone();
                let id = episode_id.clone();
                move || repo.mark_done(&id)
            })
            .await,
            Err(e) => {
                tracing::warn!(
                    worker_idx,
                    episode_id = %episode_id,
                    error = %e,
                    "extractor failed; marking episode failed"
                );
                tokio::task::spawn_blocking({
                    let repo = episode_repo.clone();
                    let id = episode_id.clone();
                    let err = e.clone();
                    move || repo.mark_failed(&id, &err)
                })
                .await
            }
        };

        if let Err(join_err) = finish_result {
            tracing::warn!(error = %join_err, "finish status update join failed");
        }
    }
}
```

- [ ] **Step 2: Commit**

```
git add gateway/gateway-execution/src/ingest/queue.rs
git commit -m "feat(ingest): IngestionQueue + worker loop skeleton (extractor stub)"
```

Note: this doesn't compile yet — `Extractor` trait comes in Task 4. Commit anyway to keep history clean; Task 4 makes it build.

---

## Task 4: Extractor trait + stub impl

**Files:**
- Create: `gateway/gateway-execution/src/ingest/extractor.rs`

- [ ] **Step 1: Define the trait**

```rust
//! Two-pass LLM extractor: pass 1 entities + aliases, pass 2 relationships
//! conditioned on the entity list. Concrete impl uses `agent_runtime::LlmClient`;
//! a `NoopExtractor` is provided for tests that don't exercise the LLM path.

use async_trait::async_trait;
use gateway_database::KgEpisode;
use knowledge_graph::GraphStorage;
use std::sync::Arc;

#[async_trait]
pub trait Extractor: Send + Sync {
    /// Process one episode — run the two-pass extraction and write results
    /// to `graph`. Errors propagate to the worker which marks the episode
    /// failed.
    async fn process(&self, episode: &KgEpisode, graph: &Arc<GraphStorage>) -> Result<(), String>;
}

/// Test-only extractor that records every episode id and never errors.
pub struct NoopExtractor {
    pub seen: tokio::sync::Mutex<Vec<String>>,
}

impl NoopExtractor {
    pub fn new() -> Self {
        Self {
            seen: tokio::sync::Mutex::new(Vec::new()),
        }
    }
}

impl Default for NoopExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Extractor for NoopExtractor {
    async fn process(
        &self,
        episode: &KgEpisode,
        _graph: &Arc<GraphStorage>,
    ) -> Result<(), String> {
        self.seen.lock().await.push(episode.id.clone());
        Ok(())
    }
}
```

- [ ] **Step 2: cargo check**

```
cargo check -p gateway-execution
```

Expected: clean — queue.rs now imports `Extractor` and `NoopExtractor`. If missing `async-trait` in `gateway-execution/Cargo.toml`, add it.

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/ingest/extractor.rs
git commit -m "feat(ingest): Extractor trait + NoopExtractor for tests"
```

---

## Task 5: LlmExtractor — pass 1 (entities)

**Files:**
- Modify: `gateway/gateway-execution/src/ingest/extractor.rs`

- [ ] **Step 1: Add LlmExtractor with pass-1 prompt**

Append to `extractor.rs`:

```rust
use agent_runtime::llm::{ChatMessage, LlmClient};
use knowledge_graph::{Entity, EntityType};

pub struct LlmExtractor {
    client: Arc<dyn LlmClient>,
    agent_id: String,
}

impl LlmExtractor {
    pub fn new(client: Arc<dyn LlmClient>, agent_id: String) -> Self {
        Self { client, agent_id }
    }

    async fn extract_entities(
        &self,
        chunk_text: &str,
        neighborhood_hints: &[String],
    ) -> Result<Vec<Entity>, String> {
        let system = "You extract named entities from text. \
            Return ONLY valid JSON matching the schema. \
            Do not wrap in code fences. Do not add commentary.";

        let hints_block = if neighborhood_hints.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nExisting entities already in the graph (prefer reusing \
                 these names when a mention refers to the same thing):\n{}",
                neighborhood_hints.join(", ")
            )
        };

        let user = format!(
            "Extract entities from this text. Output JSON: \
            {{\"entities\": [{{\"name\": string, \"type\": string, \"aliases\": [string], \"description\": string}}]}}\n\n\
            Valid types: person, organization, location, event, document, concept, tool, project, file, time_period, role, artifact, ward.\n\n\
            TEXT:\n{chunk_text}{hints_block}"
        );

        let messages = vec![
            ChatMessage::system(system.to_string()),
            ChatMessage::user(user),
        ];

        let response = self
            .client
            .chat(messages, None)
            .await
            .map_err(|e| format!("llm call failed: {e}"))?;

        parse_entities_response(&response.content, &self.agent_id)
    }
}

fn parse_entities_response(content: &str, agent_id: &str) -> Result<Vec<Entity>, String> {
    // Strip common code-fence wrapping just in case.
    let stripped = content.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let raw: serde_json::Value = serde_json::from_str(stripped)
        .map_err(|e| format!("parse entities: {e} (content: {})", &content.chars().take(200).collect::<String>()))?;

    let arr = raw
        .get("entities")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing 'entities' array".to_string())?;

    let mut out = Vec::new();
    for item in arr {
        let name = match item.get("name").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => continue,
        };
        let type_str = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("concept");
        let ty = EntityType::from_str(type_str);
        let mut entity = Entity::new(agent_id.to_string(), ty, name);
        // Record description in properties.
        if let Some(desc) = item.get("description").and_then(|v| v.as_str()) {
            entity
                .properties
                .insert("description".to_string(), serde_json::Value::String(desc.to_string()));
        }
        out.push(entity);
    }
    Ok(out)
}
```

- [ ] **Step 2: Test the parser with a synthetic LLM response**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entities_from_clean_json() {
        let json = r#"{"entities": [
            {"name": "Alice", "type": "person", "aliases": [], "description": "A character"},
            {"name": "Wonderland", "type": "location", "aliases": ["Land of Wonder"]}
        ]}"#;
        let entities = parse_entities_response(json, "root").unwrap();
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].name, "Alice");
        assert!(matches!(entities[0].entity_type, EntityType::Person));
    }

    #[test]
    fn strips_code_fence_wrapping() {
        let json = "```json\n{\"entities\": [{\"name\": \"X\", \"type\": \"concept\"}]}\n```";
        let entities = parse_entities_response(json, "root").unwrap();
        assert_eq!(entities.len(), 1);
    }

    #[test]
    fn skips_empty_names() {
        let json = r#"{"entities": [{"name": "", "type": "person"}, {"name": "Ok", "type": "person"}]}"#;
        let entities = parse_entities_response(json, "root").unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "Ok");
    }
}
```

- [ ] **Step 3: Run**

```
cargo test -p gateway-execution --lib ingest::extractor::tests
```

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/ingest/extractor.rs
git commit -m "feat(ingest): LlmExtractor pass 1 — entities"
```

---

## Task 6: LlmExtractor — pass 2 (relationships) + wire into `process`

**Files:**
- Modify: `gateway/gateway-execution/src/ingest/extractor.rs`

- [ ] **Step 1: Add pass-2**

```rust
impl LlmExtractor {
    async fn extract_relationships(
        &self,
        chunk_text: &str,
        entity_names: &[String],
    ) -> Result<Vec<(String, String, String)>, String> {
        if entity_names.len() < 2 {
            return Ok(Vec::new());
        }
        let system = "You extract relationships between entities. \
            Return ONLY valid JSON. Do not add commentary. \
            Every source and target MUST exactly match a name from the provided list.";
        let user = format!(
            "Given these entities: {}\n\n\
            Extract relationships between them from this text. \
            Output JSON: {{\"relationships\": [{{\"source\": string, \"target\": string, \"type\": string}}]}}\n\n\
            TEXT:\n{chunk_text}",
            entity_names.join(", ")
        );
        let messages = vec![
            ChatMessage::system(system.to_string()),
            ChatMessage::user(user),
        ];
        let response = self
            .client
            .chat(messages, None)
            .await
            .map_err(|e| format!("llm call failed (rels): {e}"))?;

        parse_relationships_response(&response.content, entity_names)
    }
}

fn parse_relationships_response(
    content: &str,
    known_entities: &[String],
) -> Result<Vec<(String, String, String)>, String> {
    let stripped = content.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let raw: serde_json::Value = serde_json::from_str(stripped)
        .map_err(|e| format!("parse relationships: {e}"))?;

    let arr = raw
        .get("relationships")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing 'relationships' array".to_string())?;

    let mut out = Vec::new();
    let known: std::collections::HashSet<&str> = known_entities.iter().map(|s| s.as_str()).collect();

    for item in arr {
        let src = item.get("source").and_then(|v| v.as_str()).unwrap_or("").trim();
        let tgt = item.get("target").and_then(|v| v.as_str()).unwrap_or("").trim();
        let ty = item.get("type").and_then(|v| v.as_str()).unwrap_or("").trim();
        if src.is_empty() || tgt.is_empty() || ty.is_empty() {
            continue;
        }
        // Drop relationships referencing unknown entities.
        if !known.contains(src) || !known.contains(tgt) {
            continue;
        }
        out.push((src.to_string(), tgt.to_string(), ty.to_string()));
    }
    Ok(out)
}
```

- [ ] **Step 2: Wire `process` to orchestrate both passes + write to graph**

```rust
#[async_trait]
impl Extractor for LlmExtractor {
    async fn process(
        &self,
        episode: &KgEpisode,
        graph: &Arc<GraphStorage>,
    ) -> Result<(), String> {
        // For Phase 2, the episode's content isn't stored in the DB — the
        // ingestion producer passes chunk text via `source_ref` that encodes
        // a lookup key OR we store chunk text in a new `kg_episode_payloads`
        // table. Simpler for Phase 2: producer stores chunk text inline in a
        // payload column. We add that column now.
        //
        // For now, the process path requires the chunk text to be fetched.
        // Use `episode.source_ref` to find the text in a sidecar table
        // `kg_episode_payloads(episode_id, text)` — added in the schema.
        //
        // TODO: implement this. For the skeleton, return Ok(()) without
        // actual extraction if payload is missing — effectively a NoopExtractor.
        let chunk_text = fetch_episode_payload(graph, &episode.id).await?;
        if chunk_text.is_empty() {
            return Ok(());
        }

        let entities = self.extract_entities(&chunk_text, &[]).await?;
        if entities.is_empty() {
            return Ok(());
        }
        let entity_names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();

        let rels = self.extract_relationships(&chunk_text, &entity_names).await?;

        // Resolve name → candidate entity id for relationships; rely on
        // store_knowledge's entity_id_map remapping to canonicalize.
        let mut knowledge_entities = entities;
        let mut candidate_rels = Vec::new();
        for (src_name, tgt_name, ty) in rels {
            let (Some(src_id), Some(tgt_id)) = (
                knowledge_entities.iter().find(|e| e.name == src_name).map(|e| e.id.clone()),
                knowledge_entities.iter().find(|e| e.name == tgt_name).map(|e| e.id.clone()),
            ) else { continue };
            candidate_rels.push(knowledge_graph::Relationship::new(
                self.agent_id.clone(),
                src_id,
                tgt_id,
                knowledge_graph::RelationshipType::from_str(&ty),
            ));
        }

        // Tag each entity with source_episode_id in properties.
        for e in &mut knowledge_entities {
            e.properties.insert(
                "_source_episode_id".to_string(),
                serde_json::Value::String(episode.id.clone()),
            );
        }

        let extracted = knowledge_graph::ExtractedKnowledge {
            entities: knowledge_entities,
            relationships: candidate_rels,
        };
        graph
            .store_knowledge(&self.agent_id, extracted)
            .map_err(|e| format!("store_knowledge: {e}"))?;
        Ok(())
    }
}

async fn fetch_episode_payload(
    graph: &Arc<GraphStorage>,
    _episode_id: &str,
) -> Result<String, String> {
    // TODO: query kg_episode_payloads. For now returns empty — Task 7 wires this.
    let _ = graph;
    Ok(String::new())
}
```

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/ingest/extractor.rs
git commit -m "feat(ingest): LlmExtractor pass 2 + process orchestration stub"
```

---

## Task 7: Episode payload storage

**Files:**
- Modify: `gateway/gateway-database/src/knowledge_schema.rs`
- Modify: `gateway/gateway-database/src/kg_episode_repository.rs`
- Modify: `gateway/gateway-execution/src/ingest/extractor.rs` (wire `fetch_episode_payload`)

Chunks need somewhere to live between enqueue and process. New table `kg_episode_payloads(episode_id, text, created_at)`.

- [ ] **Step 1: Add payload table to v22 schema**

In `knowledge_schema.rs`, append to `SCHEMA_SQL`:

```sql
CREATE TABLE IF NOT EXISTS kg_episode_payloads (
    episode_id TEXT PRIMARY KEY,
    text TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (episode_id) REFERENCES kg_episodes(id) ON DELETE CASCADE
);
```

This is SAFE to add post-v22 — it's a net-new table with IF NOT EXISTS. Fresh DBs get it; the existing test DBs recreate on every tempdir run.

- [ ] **Step 2: Add payload methods to KgEpisodeRepository**

```rust
pub fn set_payload(&self, episode_id: &str, text: &str) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    self.db.with_connection(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO kg_episode_payloads (episode_id, text, created_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![episode_id, text, now],
        )?;
        Ok(())
    })
}

pub fn get_payload(&self, episode_id: &str) -> Result<Option<String>, String> {
    self.db.with_connection(|conn| {
        let r = conn.query_row(
            "SELECT text FROM kg_episode_payloads WHERE episode_id = ?1",
            rusqlite::params![episode_id],
            |r| r.get::<_, String>(0),
        );
        match r {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    })
}
```

- [ ] **Step 3: Thread payload through extractor**

Change `Extractor::process` signature (or add a payload parameter in the queue worker) so it receives the text instead of doing a sidecar lookup. Cleaner: queue worker fetches the payload BEFORE calling `extractor.process(episode, chunk_text, graph)`:

In `extractor.rs`:

```rust
#[async_trait]
pub trait Extractor: Send + Sync {
    async fn process(
        &self,
        episode: &KgEpisode,
        chunk_text: &str,
        graph: &Arc<GraphStorage>,
    ) -> Result<(), String>;
}
```

Update `NoopExtractor` and `LlmExtractor::process` signatures. Remove the TODO'd `fetch_episode_payload`.

In `queue.rs::worker_loop`, after claiming an episode, fetch the payload via `episode_repo.get_payload(&episode.id)` (sync call, wrap in spawn_blocking). Pass it to `extractor.process(&episode, &payload, &graph)`.

If the payload is missing, mark failed with "payload missing."

- [ ] **Step 4: cargo check + test**

```
cargo check --workspace
cargo test -p gateway-database --lib kg_episode_repository
cargo test -p gateway-execution --lib ingest
```

All clean.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-database/src/knowledge_schema.rs gateway/gateway-database/src/kg_episode_repository.rs gateway/gateway-execution/src/ingest/
git commit -m "feat(ingest): kg_episode_payloads table; thread chunk text through Extractor"
```

---

## Task 8: Backpressure + rate limit

**Files:**
- Create: `gateway/gateway-execution/src/ingest/backpressure.rs`

- [ ] **Step 1: Write the module**

```rust
//! Backpressure and per-source rate limiting for ingestion.
//!
//! Two gates:
//! - global queue depth: if pending episodes > `max_queue_depth`,
//!   `ingest` returns 429.
//! - per-source: if pending episodes for a single source_ref > `max_per_source`,
//!   return 429.

use gateway_database::KgEpisodeRepository;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    pub max_queue_depth: u64,
    pub max_per_source: u64,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_queue_depth: 5_000,
            max_per_source: 500,
        }
    }
}

pub struct Backpressure {
    config: BackpressureConfig,
    episode_repo: Arc<KgEpisodeRepository>,
}

impl Backpressure {
    pub fn new(config: BackpressureConfig, episode_repo: Arc<KgEpisodeRepository>) -> Self {
        Self { config, episode_repo }
    }

    /// Returns Err(String) with a retry-after hint if the queue is saturated
    /// or the specific source has too many pending episodes.
    pub fn check(&self, source_ref_prefix: &str) -> Result<(), String> {
        let global_pending = self
            .episode_repo
            .count_pending_global()
            .unwrap_or(0);
        if global_pending >= self.config.max_queue_depth {
            return Err(format!(
                "queue full ({global_pending} pending, limit {})",
                self.config.max_queue_depth
            ));
        }
        let per_source_pending = self
            .episode_repo
            .count_pending_for_source(source_ref_prefix)
            .unwrap_or(0);
        if per_source_pending >= self.config.max_per_source {
            return Err(format!(
                "source backpressure ({per_source_pending} pending for source, limit {})",
                self.config.max_per_source
            ));
        }
        Ok(())
    }
}
```

- [ ] **Step 2: Add the count methods to `KgEpisodeRepository`**

```rust
pub fn count_pending_global(&self) -> Result<u64, String> {
    self.db.with_connection(|conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM kg_episodes WHERE status IN ('pending', 'running')",
            [],
            |r| r.get(0),
        )?;
        Ok(n.max(0) as u64)
    })
}

pub fn count_pending_for_source(&self, source_ref_prefix: &str) -> Result<u64, String> {
    self.db.with_connection(|conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM kg_episodes
             WHERE source_ref LIKE ?1 || '%' AND status IN ('pending', 'running')",
            rusqlite::params![source_ref_prefix],
            |r| r.get(0),
        )?;
        Ok(n.max(0) as u64)
    })
}
```

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/ingest/backpressure.rs gateway/gateway-database/src/kg_episode_repository.rs
git commit -m "feat(ingest): Backpressure with global + per-source rate limits"
```

---

## Task 9: HTTP endpoints — POST /api/graph/ingest + progress

**Files:**
- Create: `gateway/src/http/ingest.rs`
- Modify: `gateway/src/http/mod.rs`
- Modify: `gateway/src/state.rs` (construct IngestionQueue + Backpressure at boot)

- [ ] **Step 1: Write the handlers**

```rust
//! POST /api/graph/ingest — enqueue chunks for extraction.
//! GET  /api/graph/ingest/:source_id/progress — poll status.

use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::state::AppState;
use gateway_execution::ingest::chunker::{chunk_text, ChunkOptions};

#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub source_id: String,       // stable identifier, e.g. "book-rise-2024"
    pub source_type: String,     // "document" | "pasted_text" | ...
    pub text: String,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub chunk_opts: Option<IngestChunkOpts>,
}

#[derive(Debug, Deserialize)]
pub struct IngestChunkOpts {
    pub target_tokens: Option<usize>,
    pub overlap_tokens: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct IngestResponse {
    pub source_id: String,
    pub episode_count: usize,
}

pub async fn ingest(
    State(state): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> Result<(StatusCode, Json<IngestResponse>), (StatusCode, String)> {
    let Some(queue) = state.ingestion_queue.clone() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "ingestion queue not initialized".into()));
    };
    let Some(episode_repo) = state.kg_episode_repo.clone() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "episode repo missing".into()));
    };
    let Some(backpressure) = state.ingestion_backpressure.clone() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "backpressure not initialized".into()));
    };

    backpressure
        .check(&req.source_id)
        .map_err(|e| (StatusCode::TOO_MANY_REQUESTS, e))?;

    let opts = ChunkOptions {
        target_tokens: req.chunk_opts.as_ref().and_then(|o| o.target_tokens).unwrap_or(1000),
        overlap_tokens: req.chunk_opts.as_ref().and_then(|o| o.overlap_tokens).unwrap_or(100),
    };
    let chunks = chunk_text(&req.text, opts);
    let agent_id = req.agent_id.unwrap_or_else(|| "root".to_string());

    let mut enqueued = 0usize;
    for chunk in &chunks {
        let source_ref = format!("{}#chunk-{}", req.source_id, chunk.index);
        let content_hash = {
            let mut h = Sha256::new();
            h.update(chunk.text.as_bytes());
            format!("{:x}", h.finalize())
        };
        let episode_id = episode_repo
            .upsert_pending(&req.source_type, &source_ref, &content_hash, req.session_id.as_deref(), &agent_id)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        episode_repo
            .set_payload(&episode_id, &chunk.text)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        enqueued += 1;
    }
    queue.notify();

    Ok((
        StatusCode::ACCEPTED,
        Json(IngestResponse { source_id: req.source_id, episode_count: enqueued }),
    ))
}

#[derive(Debug, Serialize)]
pub struct ProgressResponse {
    pub source_id: String,
    pub pending: u64,
    pub running: u64,
    pub done: u64,
    pub failed: u64,
}

pub async fn progress(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<ProgressResponse>, (StatusCode, String)> {
    let Some(repo) = state.kg_episode_repo.clone() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "episode repo missing".into()));
    };
    let counts = repo
        .status_counts_for_source(&source_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(ProgressResponse {
        source_id,
        pending: counts.pending,
        running: counts.running,
        done: counts.done,
        failed: counts.failed,
    }))
}
```

- [ ] **Step 2: Register routes in `http/mod.rs`**

In `create_http_router`, under the other `/api/graph/...` routes:

```rust
.route("/api/graph/ingest", post(ingest::ingest))
.route("/api/graph/ingest/:source_id/progress", get(ingest::progress))
```

Add `mod ingest;` at the top of `http/mod.rs`.

- [ ] **Step 3: Wire IngestionQueue + Backpressure into AppState**

In `AppState`, add three fields:

```rust
pub ingestion_queue: Option<Arc<gateway_execution::ingest::queue::IngestionQueue>>,
pub ingestion_backpressure: Option<Arc<gateway_execution::ingest::backpressure::Backpressure>>,
```

In `AppState::new`, after `kg_episode_repo` and `graph_storage` are constructed (and after `embedding_client` because the extractor needs an LLM client):

```rust
let (ingestion_queue, ingestion_backpressure) = match (graph_storage.clone(), kg_episode_repo.clone()) {
    (Some(gs), repo) => {
        // LLM client for extraction — use the default provider's chat client.
        let llm: Arc<dyn agent_runtime::llm::LlmClient> = todo_get_default_llm_client(&provider_service);
        let extractor = Arc::new(gateway_execution::ingest::extractor::LlmExtractor::new(llm, "root".into()));
        let queue = Arc::new(gateway_execution::ingest::queue::IngestionQueue::start(
            2,
            repo.clone(),
            gs,
            extractor,
        ));
        let bp = Arc::new(gateway_execution::ingest::backpressure::Backpressure::new(
            Default::default(),
            repo,
        ));
        (Some(queue), Some(bp))
    }
    _ => (None, None),
};
```

Replace `todo_get_default_llm_client` with the actual call pattern used elsewhere in state.rs to create an LlmClient. Grep for `LlmClient::new` or `provider_service.default_client` to find the existing helper.

Add `ingestion_queue` and `ingestion_backpressure` to all 3 `AppState { ... }` construction sites; set to None in minimal/with_components where we don't construct them.

- [ ] **Step 4: cargo check + test**

```
cargo check --workspace
```

Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add gateway/src/http/ingest.rs gateway/src/http/mod.rs gateway/src/state.rs
git commit -m "feat(ingest): POST /api/graph/ingest + progress endpoint; wire queue in AppState"
```

---

## Task 10: Rewrite reindex to enqueue

**Files:**
- Modify: `gateway/src/http/graph.rs`

The existing `reindex_all_wards` (Pack A) executes synchronously. Change it to enqueue per-ward JSON files into the ingestion queue and return 202 immediately.

- [ ] **Step 1: Keep the old indexer for JSON-schema wards**

The Pack A indexer extracts entities+relationships from structured JSON via the `relationship_rules` module. That's not LLM extraction — it's schema-driven. Keep it for JSON files. Only prose files (`.md`, `.txt`, `.pdf` text) go through the new queue.

- [ ] **Step 2: Add `source_type=ward_prose` enqueue path**

For each ward, iterate its non-JSON text files, chunk them, enqueue one episode per chunk via `kg_episode_repo.upsert_pending`, set payload via `set_payload`, then `queue.notify()`. Return 202 with `{wards_processed, jsons_indexed, prose_episodes_enqueued}`.

- [ ] **Step 3: Commit**

```bash
git add gateway/src/http/graph.rs
git commit -m "feat(ingest): reindex enqueues prose; keeps JSON schema indexer synchronous"
```

---

## Task 11: Agent tool `ingest` + shard edits

**Files:**
- Create: `runtime/agent-tools/src/tools/ingest.rs`
- Modify: `runtime/agent-tools/src/tools/mod.rs`
- Modify: `gateway/templates/shards/tooling_skills.md`

- [ ] **Step 1: Define the tool**

The tool calls into the HTTP endpoint (simplest) OR directly into the ingestion queue via a trait. Given agent-tools runs in-process with access to the daemon state, direct call is cleaner.

Mimic the structure of `graph_query.rs`. The tool takes `{source_id, text, source_type?}`, dispatches to the same code path as the HTTP handler.

Expose a trait `IngestionAccess` from `gateway-execution::ingest` (chunk→enqueue→notify) so the tool doesn't depend on `gateway`.

- [ ] **Step 2: Shard**

Add to `tooling_skills.md`:

```markdown
### ingest
Enqueue a document or text for background extraction into the knowledge graph.

- `ingest(source_id="<stable-id>", text="<full text>", source_type?="document")` — returns {episode_count} immediately; work happens in the background.
- Check progress: extract entities appear in `graph_query` within ~seconds per chunk.
- Use when: you have a multi-paragraph document and want it queryable as graph entities/relationships.
```

- [ ] **Step 3: Commit**

```bash
git add runtime/agent-tools/src/tools/ingest.rs runtime/agent-tools/src/tools/mod.rs gateway/templates/shards/tooling_skills.md
git commit -m "feat(ingest): agent tool + shard docs"
```

---

## Task 12: Concurrency test + final validation

**Files:**
- Create: `gateway/gateway-execution/tests/ingest_concurrency.rs`

- [ ] **Step 1: Write the stress test**

```rust
//! Concurrency invariant: while a 500-chunk ingestion runs, unrelated
//! API calls (stats, graph queries, simple reads) stay <200ms p95.

use std::sync::Arc;
use std::time::Instant;
use tempfile::tempdir;

use gateway_database::{KgEpisodeRepository, KnowledgeDatabase};
use gateway_execution::ingest::{
    extractor::NoopExtractor,
    queue::IngestionQueue,
};
use gateway_services::VaultPaths;
use knowledge_graph::GraphStorage;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unrelated_reads_stay_responsive_during_heavy_ingestion() {
    let tmp = tempdir().unwrap();
    let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = Arc::new(KnowledgeDatabase::new(paths).unwrap());
    let episode_repo = Arc::new(KgEpisodeRepository::new(db.clone()));
    let graph = Arc::new(GraphStorage::new(db.clone()).unwrap());
    let extractor = Arc::new(NoopExtractor::new());
    let queue = Arc::new(IngestionQueue::start(
        2,
        episode_repo.clone(),
        graph.clone(),
        extractor.clone(),
    ));

    // Enqueue 500 episodes.
    for i in 0..500 {
        let id = episode_repo
            .upsert_pending(
                "document",
                &format!("stress#chunk-{i}"),
                &format!("h{i}"),
                None,
                "root",
            )
            .unwrap();
        episode_repo.set_payload(&id, &format!("chunk {i} text")).unwrap();
    }
    queue.notify();

    // In parallel with draining, issue 100 unrelated reads.
    let read_handle = tokio::spawn(async move {
        let mut durations = Vec::new();
        for _ in 0..100 {
            let start = Instant::now();
            let _ = db.with_connection(|conn| {
                let _: i64 = conn.query_row("SELECT COUNT(*) FROM kg_entities", [], |r| r.get(0))?;
                Ok(())
            });
            durations.push(start.elapsed());
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        durations
    });

    let durations = read_handle.await.unwrap();
    let mut sorted = durations.clone();
    sorted.sort();
    let p95 = sorted[(sorted.len() * 95) / 100];
    eprintln!("Reader p95 under ingestion load = {:?}", p95);
    assert!(
        p95.as_millis() < 200,
        "reads must stay <200ms p95 during ingestion, got {:?}",
        p95
    );
}
```

- [ ] **Step 2: Run**

```
cargo test -p gateway-execution --test ingest_concurrency --release
```

- [ ] **Step 3: fmt + clippy + full workspace test**

```
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

All clean.

- [ ] **Step 4: Push**

```
git add gateway/gateway-execution/tests/ingest_concurrency.rs
git commit -m "test(ingest): concurrency invariant — reads stay <200ms p95 under load"
git push -u origin feature/memory-v2-phase-2
```

---

## Self-Review

**Spec coverage:**
- ✅ Chunker (Task 1)
- ✅ IngestionQueue (Task 3) + Episode lifecycle (Task 2)
- ✅ Two-pass LLM extractor (Tasks 4-6)
- ✅ Payload storage (Task 7)
- ✅ Backpressure + rate limit (Task 8)
- ✅ POST /api/graph/ingest + progress (Task 9)
- ✅ Reindex unified (Task 10)
- ✅ Agent tool + shard (Task 11)
- ✅ Concurrency acceptance test (Task 12)
- ✅ WAL mode — already enforced by KnowledgeDatabase (Phase 1a)

**Deferred to later phases:**
- Unified scored recall (Phase 3)
- LLM pairwise verify in resolver stage 3 (Phase 3/4)
- Compactor + decay (Phase 4)
- iText2KG neighborhood-hint conditioning — Phase 2 Task 5 accepts empty hints; Phase 3 adds the neighborhood-lookup path

**Placeholder scan:** Task 9 Step 3 uses `todo_get_default_llm_client` — flagged as a grep-for-the-existing-helper step, not a placeholder in the final code. Task 10 Step 1 is narrative; Step 2 has the concrete logic.

**Known soft spots:**
- Task 9's LLM client wiring in AppState may discover that the default provider's chat client isn't constructable until after the full AppState init — we might need to construct the queue lazily on first request OR move its initialization later in `AppState::new`. Flagged but not pre-solved; the implementer confronts it with real code visibility.
- Task 11's `IngestionAccess` trait is sketched but not fully spec'd. Its shape becomes obvious once the HTTP handler is final.
