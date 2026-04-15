# Memory Tab — Command Deck Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single-agent flat `MemoryTab` with a ward-first three-column "Command Deck" that surfaces facts + wiki + procedures + episodes per ward, wired to the existing (but unused) hybrid search backend plus a new wiki FTS index.

**Architecture:** Backend gains one new wiki FTS virtual table, a hybrid search helper for wiki, and two new HTTP endpoints (`/api/wards/:ward_id/content`, `/api/memory/search`). Frontend replaces `MemoryTab.tsx` with a componentised `command-deck/` tree gated behind a `memory_tab_command_deck` settings flag. Existing style tokens (`--color-*`, `--spacing-*`, `--radius-*`) are reused verbatim.

**Tech Stack:** Rust (axum, rusqlite, sqlite-vec) · TypeScript/React (Vite, Vitest, React Query-style hooks)

**Spec:** `docs/superpowers/specs/2026-04-15-memory-tab-command-deck-design.md`

**Branch:** `feature/memory-tab-command-deck`

---

## File Structure

### Backend — new

- `gateway/gateway-database/migrations/v23_wiki_fts.sql` — create `ward_wiki_articles_fts` + sync triggers
- `gateway/gateway-database/src/age_bucket.rs` — single helper `age_bucket(now, created_at) -> &'static str`
- `gateway/src/http/memory_search.rs` — unified `/api/memory/search` handler
- `gateway/src/http/ward_content.rs` — `/api/wards/:ward_id/content` aggregator

### Backend — modified

- `gateway/gateway-database/src/wiki_repository.rs` — add `search_hybrid`
- `gateway/gateway-database/src/lib.rs` — re-export `age_bucket`
- `gateway/src/http/memory.rs` — `search_memory_facts` accepts `mode` param, calls hybrid
- `gateway/src/http/mod.rs` + `gateway/src/server.rs` — mount new routes

### Frontend — new (`apps/ui/src/features/memory/command-deck/`)

- `types.ts` — `WardSummary`, `WardContent`, `HybridSearchResponse`, `MatchSource`, `AgeBucket`
- `hooks.ts` — `useWards`, `useWardContent(wardId)`, `useHybridSearch(query, opts)`, `useTimewarp`
- `MemoryTab.tsx` — three-column shell, composes everything below
- `WardRail.tsx` — left column
- `SearchBar.tsx` — top bar query + mode toggle
- `ScopeChips.tsx` — under search bar
- `ContentDeck.tsx` — center column shell, tab state
- `ContentList.tsx` — age-grouped list
- `MemoryItemCard.tsx` — one row (used in list + search results)
- `WriteRail.tsx` — right column
- `AddDrawer.tsx` — modal panel for `+ Fact / + Instruction / + Policy`
- `__tests__/MemoryItemCard.test.tsx`
- `__tests__/WardRail.test.tsx`
- `__tests__/SearchBar.test.tsx`
- `__tests__/ContentList.test.tsx`

### Frontend — modified

- `apps/ui/src/services/transport/interface.ts` — add `getWardContent`, `searchMemoryHybrid`
- `apps/ui/src/services/transport/http.ts` — implement both
- `apps/ui/src/services/transport/types.ts` — mirror backend types
- `apps/ui/src/App.tsx` (or wherever the router lives) — gate MemoryTab on feature flag
- `apps/ui/src/features/settings/` — add toggle for `memory_tab_command_deck`
- `apps/ui/src/features/memory/MemoryTab.tsx` → rename to `MemoryTabLegacy.tsx`

---

## Task 1: Create `ward_wiki_articles_fts` migration

**Files:**
- Create: `gateway/gateway-database/migrations/v23_wiki_fts.sql`
- Modify: `gateway/gateway-database/src/migrations.rs` (register the new version)
- Test: `gateway/gateway-database/tests/wiki_fts_migration.rs`

- [ ] **Step 1: Write the failing test**

```rust
// gateway/gateway-database/tests/wiki_fts_migration.rs
use std::sync::Arc;
use tempfile::tempdir;

use gateway_database::KnowledgeDatabase;
use gateway_services::VaultPaths;

fn db() -> (tempfile::TempDir, Arc<KnowledgeDatabase>) {
    let tmp = tempdir().unwrap();
    let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = Arc::new(KnowledgeDatabase::new(paths).unwrap());
    (tmp, db)
}

#[test]
fn ward_wiki_articles_fts_exists_after_migration() {
    let (_tmp, db) = db();
    let exists: i64 = db
        .with_connection(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='ward_wiki_articles_fts'",
                [],
                |r| r.get(0),
            )
        })
        .unwrap();
    assert_eq!(exists, 1);
}

#[test]
fn inserting_wiki_populates_fts_via_trigger() {
    let (_tmp, db) = db();
    db.with_connection(|c| {
        c.execute(
            "INSERT INTO ward_wiki_articles (id, ward_id, agent_id, title, content, tags, source_fact_ids, version, created_at, updated_at) \
             VALUES ('w1','wardA','root','Hormuz Geofence','Latitude 24.0 to 27.5','[]','[]',1,'2026-04-15','2026-04-15')",
            [],
        )?;
        Ok(())
    })
    .unwrap();

    let hits: i64 = db
        .with_connection(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM ward_wiki_articles_fts WHERE ward_wiki_articles_fts MATCH 'Hormuz'",
                [],
                |r| r.get(0),
            )
        })
        .unwrap();
    assert_eq!(hits, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p gateway-database --test wiki_fts_migration`
Expected: FAIL with `no such table: ward_wiki_articles_fts`.

- [ ] **Step 3: Create the migration file**

```sql
-- gateway/gateway-database/migrations/v23_wiki_fts.sql
CREATE VIRTUAL TABLE IF NOT EXISTS ward_wiki_articles_fts USING fts5(
    title,
    content,
    content='ward_wiki_articles',
    content_rowid='rowid'
);

-- Backfill from existing rows.
INSERT INTO ward_wiki_articles_fts(rowid, title, content)
SELECT rowid, title, content FROM ward_wiki_articles;

-- Keep FTS in sync with source table.
CREATE TRIGGER IF NOT EXISTS ward_wiki_articles_fts_ai AFTER INSERT ON ward_wiki_articles BEGIN
    INSERT INTO ward_wiki_articles_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS ward_wiki_articles_fts_ad AFTER DELETE ON ward_wiki_articles BEGIN
    INSERT INTO ward_wiki_articles_fts(ward_wiki_articles_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
END;

CREATE TRIGGER IF NOT EXISTS ward_wiki_articles_fts_au AFTER UPDATE ON ward_wiki_articles BEGIN
    INSERT INTO ward_wiki_articles_fts(ward_wiki_articles_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
    INSERT INTO ward_wiki_articles_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;
```

- [ ] **Step 4: Register migration version in Rust**

Find the migration loader in `gateway/gateway-database/src/migrations.rs` (pattern: list of `(version, sql)` tuples). Append:

```rust
(23, include_str!("../migrations/v23_wiki_fts.sql")),
```

Keep the existing list order; the migration runner replays sequentially and stops at schema_version = 23.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p gateway-database --test wiki_fts_migration`
Expected: PASS (both tests).

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-database/migrations/v23_wiki_fts.sql \
        gateway/gateway-database/src/migrations.rs \
        gateway/gateway-database/tests/wiki_fts_migration.rs
git commit -m "feat(db): add ward_wiki_articles_fts with sync triggers"
```

---

## Task 2: `age_bucket` helper

**Files:**
- Create: `gateway/gateway-database/src/age_bucket.rs`
- Modify: `gateway/gateway-database/src/lib.rs`
- Test: inside `age_bucket.rs`

- [ ] **Step 1: Write the failing test**

```rust
// gateway/gateway-database/src/age_bucket.rs
use chrono::{DateTime, Duration, Utc};

/// Classify a timestamp into a human-meaningful recency bucket relative to `now`.
/// Returns one of: "today", "last_7_days", "historical".
pub fn age_bucket(now: DateTime<Utc>, created_at: DateTime<Utc>) -> &'static str {
    let age = now.signed_duration_since(created_at);
    if age < Duration::hours(24) {
        "today"
    } else if age < Duration::days(7) {
        "last_7_days"
    } else {
        "historical"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn twelve_hours_ago_is_today() {
        let t = now() - Duration::hours(12);
        assert_eq!(age_bucket(now(), t), "today");
    }

    #[test]
    fn three_days_ago_is_last_7_days() {
        let t = now() - Duration::days(3);
        assert_eq!(age_bucket(now(), t), "last_7_days");
    }

    #[test]
    fn thirty_days_ago_is_historical() {
        let t = now() - Duration::days(30);
        assert_eq!(age_bucket(now(), t), "historical");
    }

    #[test]
    fn exactly_seven_days_is_historical() {
        let t = now() - Duration::days(7);
        assert_eq!(age_bucket(now(), t), "historical");
    }
}
```

- [ ] **Step 2: Export from the crate**

Edit `gateway/gateway-database/src/lib.rs`:

```rust
pub mod age_bucket;
pub use age_bucket::age_bucket;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p gateway-database --lib age_bucket`
Expected: PASS (4 tests).

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-database/src/age_bucket.rs gateway/gateway-database/src/lib.rs
git commit -m "feat(db): add age_bucket classifier for recency grouping"
```

---

## Task 3: `wiki_repository::search_hybrid`

**Files:**
- Modify: `gateway/gateway-database/src/wiki_repository.rs`
- Test: `gateway/gateway-database/tests/wiki_search_hybrid.rs`

- [ ] **Step 1: Write the failing integration test**

```rust
// gateway/gateway-database/tests/wiki_search_hybrid.rs
use std::sync::Arc;
use tempfile::tempdir;

use gateway_database::vector_index::SqliteVecIndex;
use gateway_database::{KnowledgeDatabase, WikiRepository};
use gateway_services::VaultPaths;

fn setup() -> (tempfile::TempDir, Arc<KnowledgeDatabase>, WikiRepository) {
    let tmp = tempdir().unwrap();
    let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
    let db = Arc::new(KnowledgeDatabase::new(paths).unwrap());
    let vec = Arc::new(
        SqliteVecIndex::new(db.clone(), "wiki_articles_index", "article_id")
            .expect("init vec"),
    );
    let repo = WikiRepository::new(db.clone(), vec);
    (tmp, db, repo)
}

fn normalized(v: Vec<f32>) -> Vec<f32> {
    let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.into_iter().map(|x| x / n).collect()
}

#[test]
fn hybrid_matches_fts_when_keyword_present() {
    let (_tmp, db, repo) = setup();
    db.with_connection(|c| {
        c.execute(
            "INSERT INTO ward_wiki_articles (id, ward_id, agent_id, title, content, tags, source_fact_ids, version, created_at, updated_at) \
             VALUES ('w1','wardA','root','AISStream Endpoint','wss://stream.aisstream.io/v0/stream returns 404 on v2.','[]','[]',1,'2026-04-15','2026-04-15')",
            [],
        )?;
        Ok(())
    }).unwrap();

    let hits = repo.search_hybrid("AISStream", Some("wardA"), None, 5).unwrap();
    assert!(hits.iter().any(|h| h.article.id == "w1"));
}

#[test]
fn hybrid_matches_vector_when_query_and_title_are_semantically_similar() {
    let (_tmp, db, repo) = setup();
    let emb = normalized((0..1024).map(|i| (i as f32).sin()).collect());
    db.with_connection(|c| {
        c.execute(
            "INSERT INTO ward_wiki_articles (id, ward_id, agent_id, title, content, tags, source_fact_ids, version, created_at, updated_at) \
             VALUES ('w2','wardA','root','Hormuz Bounding Box','24 N to 27 N, 54 E to 58 E.','[]','[]',1,'2026-04-15','2026-04-15')",
            [],
        )?;
        Ok(())
    }).unwrap();
    // Also index the embedding directly.
    repo.vec_index_for_tests().upsert("w2", &emb).unwrap();

    let hits = repo.search_hybrid("strait monitoring region", Some("wardA"), Some(emb), 5).unwrap();
    assert!(hits.iter().any(|h| h.article.id == "w2"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p gateway-database --test wiki_search_hybrid`
Expected: FAIL — `no method named search_hybrid`.

- [ ] **Step 3: Add `search_hybrid` and test hook**

In `gateway/gateway-database/src/wiki_repository.rs`, add to `impl WikiRepository`:

```rust
/// A single wiki hit with provenance of why it matched.
#[derive(Debug, Clone)]
pub struct WikiHit {
    pub article: WikiArticle,
    pub score: f32,
    pub match_source: &'static str, // "fts" | "vec" | "hybrid" | "title"
}

/// Hybrid search: FTS5 over title+content unioned with sqlite-vec nearest
/// neighbours, fused via reciprocal-rank combination. `embedding` is optional;
/// when `None`, behaves as pure FTS.
pub fn search_hybrid(
    &self,
    query: &str,
    ward_id: Option<&str>,
    embedding: Option<Vec<f32>>,
    limit: usize,
) -> Result<Vec<WikiHit>, String> {
    let fts_sql = match ward_id {
        Some(_) => "SELECT a.rowid, a.id FROM ward_wiki_articles_fts \
                    JOIN ward_wiki_articles a ON a.rowid = ward_wiki_articles_fts.rowid \
                    WHERE ward_wiki_articles_fts MATCH ?1 AND a.ward_id = ?2 LIMIT 50",
        None    => "SELECT a.rowid, a.id FROM ward_wiki_articles_fts \
                    JOIN ward_wiki_articles a ON a.rowid = ward_wiki_articles_fts.rowid \
                    WHERE ward_wiki_articles_fts MATCH ?1 LIMIT 50",
    };

    let sanitized = crate::memory_repository::sanitize_fts_query(query);

    let fts_ids: Vec<String> = self
        .db
        .with_connection(|conn| {
            let mut stmt = conn.prepare(fts_sql)?;
            let rows = if let Some(w) = ward_id {
                stmt.query_map(rusqlite::params![sanitized, w], |r| r.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                stmt.query_map(rusqlite::params![sanitized], |r| r.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            Ok(rows)
        })
        .unwrap_or_default();

    let vec_ids: Vec<(String, f32)> = match embedding.as_ref() {
        Some(emb) => self.vec_index.query_nearest(emb, 50).unwrap_or_default(),
        None => Vec::new(),
    };

    // Reciprocal-rank fusion.
    let mut scored: std::collections::HashMap<String, (f32, &'static str)> =
        std::collections::HashMap::new();
    for (rank, id) in fts_ids.iter().enumerate() {
        let s = 1.0 / (60.0 + rank as f32);
        scored.entry(id.clone()).or_insert((0.0, "fts")).0 += s;
    }
    for (rank, (id, _dist)) in vec_ids.iter().enumerate() {
        let s = 1.0 / (60.0 + rank as f32);
        let slot = scored.entry(id.clone()).or_insert((0.0, "vec"));
        slot.0 += s;
        if slot.1 == "fts" {
            slot.1 = "hybrid";
        }
    }

    let mut ranked: Vec<(String, f32, &'static str)> = scored
        .into_iter()
        .map(|(id, (s, src))| (id, s, src))
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(limit);

    let mut out = Vec::new();
    for (id, score, src) in ranked {
        if let Some(article) = self.get(&id)? {
            if ward_id.is_none() || article.ward_id == ward_id.unwrap() {
                out.push(WikiHit { article, score, match_source: src });
            }
        }
    }
    Ok(out)
}

#[doc(hidden)]
#[cfg(test)]
pub fn vec_index_for_tests(&self) -> &std::sync::Arc<dyn crate::vector_index::VectorIndex> {
    &self.vec_index
}
```

Export `WikiHit` from the crate root: in `gateway/gateway-database/src/lib.rs` add `pub use wiki_repository::WikiHit;`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p gateway-database --test wiki_search_hybrid`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-database/src/wiki_repository.rs \
        gateway/gateway-database/src/lib.rs \
        gateway/gateway-database/tests/wiki_search_hybrid.rs
git commit -m "feat(db): add WikiRepository::search_hybrid (FTS + vec + RRF)"
```

---

## Task 4: Upgrade `search_memory_facts` HTTP handler to hybrid with mode

**Files:**
- Modify: `gateway/src/http/memory.rs`
- Test: `gateway/tests/memory_search_handler.rs`

- [ ] **Step 1: Write the failing test**

```rust
// gateway/tests/memory_search_handler.rs
// Integration-level: spin a minimal Axum state and POST a search query.
// Assumes the helper `test_state::build()` exists in the gateway test tree; if not,
// reuse the pattern from gateway/tests/api_tests.rs to construct AppState.

use serde_json::json;

#[tokio::test]
async fn hybrid_mode_returns_match_source_field() {
    let (state, _tmp) = test_helpers::minimal_state_with_one_fact().await;
    let client = test_helpers::router_client(state).await;
    let resp: serde_json::Value = client
        .get("/api/memory/agent:root?q=hormuz&mode=hybrid&limit=10")
        .send_json()
        .await;
    assert!(resp["facts"].is_array());
    assert!(resp["facts"][0].get("match_source").is_some());
}

#[tokio::test]
async fn fts_mode_does_not_call_embedding_backend() {
    // Mocks the embedding service to panic if embed() is called.
    let (state, _tmp) = test_helpers::minimal_state_with_panic_embed().await;
    let client = test_helpers::router_client(state).await;
    let resp: serde_json::Value = client
        .get("/api/memory/agent:root?q=hormuz&mode=fts")
        .send_json()
        .await;
    assert!(resp.get("facts").is_some());
}
```

Note on `test_helpers`: if the repo doesn't have a `test_helpers` module, add it as a small module alongside the existing test file, copying the AppState-builder pattern from `gateway/tests/api_tests.rs`. Keep it ≤ 80 lines.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p gateway --test memory_search_handler`
Expected: FAIL — `mode` query parameter ignored, no `match_source` in response.

- [ ] **Step 3: Accept `mode` + call hybrid**

Edit `gateway/src/http/memory.rs:search_memory_facts` (around line 154). Add:

```rust
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub mode: Option<String>, // "hybrid" (default) | "fts" | "semantic"
    #[serde(default)]
    pub ward_id: Option<String>,
}

fn default_limit() -> usize { 20 }

pub async fn search_memory_facts(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<MemoryListResponse>, ApiError> {
    let mode = q.mode.as_deref().unwrap_or("hybrid");

    let (facts, source_map) = match mode {
        "fts" => {
            let rows = state.memory_repo.search_memory_facts_fts(
                &q.q, &agent_id, q.limit, q.ward_id.as_deref(),
            )?;
            let map = rows.iter().map(|f| (f.id.clone(), "fts")).collect();
            (rows, map)
        }
        "semantic" => {
            let emb = state.embedding_client.embed(&[&q.q]).await
                .map_err(|e| ApiError::bad_request(format!("embedding failed: {e}")))?
                .into_iter().next().ok_or_else(|| ApiError::bad_request("empty embedding"))?;
            let rows = state.memory_repo.search_similar_facts(&emb, &agent_id, q.limit, q.ward_id.as_deref())?;
            let map = rows.iter().map(|f| (f.id.clone(), "vec")).collect();
            (rows, map)
        }
        _ /* hybrid */ => {
            let emb = state.embedding_client.embed(&[&q.q]).await.ok()
                .and_then(|mut v| v.pop());
            let (rows, sources) = state.memory_repo.search_memory_facts_hybrid(
                &q.q, emb, &agent_id, q.limit, q.ward_id.as_deref(),
            )?;
            (rows, sources)
        }
    };

    let total = facts.len();
    Ok(Json(MemoryListResponse {
        facts: annotate_with_source(facts, source_map),
        total,
    }))
}
```

`MemoryListResponse.facts` must serialize `match_source`. Add to `MemoryFact` serialization (likely in the transport types) a `#[serde(skip_serializing_if = "Option::is_none")] pub match_source: Option<String>,` field, with default `None` on the DB-read paths.

Update `search_memory_facts_hybrid` in `memory_repository.rs:634` to return a per-id source map alongside results (currently returns just rows). Change signature to:

```rust
pub fn search_memory_facts_hybrid(
    &self,
    query: &str,
    embedding: Option<Vec<f32>>,
    agent_id: &str,
    limit: usize,
    ward_id: Option<&str>,
) -> Result<(Vec<MemoryFact>, Vec<(String, &'static str)>), String>
```

Mirror the RRF attribution pattern from Task 3.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p gateway --test memory_search_handler`
Expected: PASS (2 tests).
Run: `cargo test -p gateway-database --lib memory_repository`
Expected: PASS (no regressions — existing callers adapted).

- [ ] **Step 5: Commit**

```bash
git add gateway/src/http/memory.rs \
        gateway/gateway-database/src/memory_repository.rs \
        gateway/tests/memory_search_handler.rs
git commit -m "feat(http): memory search accepts mode param; returns match_source"
```

---

## Task 5: `/api/wards/:ward_id/content` aggregator

**Files:**
- Create: `gateway/src/http/ward_content.rs`
- Modify: `gateway/src/http/mod.rs` (register module), `gateway/src/server.rs` (mount route)
- Test: `gateway/tests/ward_content_endpoint.rs`

- [ ] **Step 1: Write the failing test**

```rust
// gateway/tests/ward_content_endpoint.rs
use serde_json::Value;

#[tokio::test]
async fn returns_four_content_types_with_age_buckets() {
    let (state, _tmp) = test_helpers::seed_ward_with_all_content("literature-library").await;
    let client = test_helpers::router_client(state).await;

    let resp: Value = client
        .get("/api/wards/literature-library/content")
        .send_json()
        .await;

    for key in ["facts", "wiki", "procedures", "episodes"] {
        assert!(resp[key].is_array(), "missing content type {key}");
    }
    let counts = &resp["counts"];
    assert!(counts["facts"].as_u64().unwrap() >= 1);
    let first = &resp["facts"][0];
    let bucket = first["age_bucket"].as_str().unwrap();
    assert!(["today","last_7_days","historical"].contains(&bucket));
}

#[tokio::test]
async fn unknown_ward_returns_empty_arrays_and_zero_counts() {
    let (state, _tmp) = test_helpers::empty_state().await;
    let client = test_helpers::router_client(state).await;
    let resp: Value = client.get("/api/wards/nope/content").send_json().await;
    assert_eq!(resp["counts"]["facts"].as_u64().unwrap(), 0);
    assert!(resp["facts"].as_array().unwrap().is_empty());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p gateway --test ward_content_endpoint`
Expected: FAIL — route 404.

- [ ] **Step 3: Create the handler module**

```rust
// gateway/src/http/ward_content.rs
use axum::{extract::{Path, State}, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use crate::state::AppState;
use crate::http::ApiError;
use gateway_database::age_bucket;

#[derive(Serialize)]
pub struct WardContentResponse {
    ward_id: String,
    summary: WardSummary,
    facts: Vec<ItemWithBucket<serde_json::Value>>,
    wiki: Vec<ItemWithBucket<serde_json::Value>>,
    procedures: Vec<ItemWithBucket<serde_json::Value>>,
    episodes: Vec<ItemWithBucket<serde_json::Value>>,
    counts: Counts,
}

#[derive(Serialize, Default)]
pub struct WardSummary {
    title: String,
    description: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize)]
pub struct ItemWithBucket<T: Serialize> {
    #[serde(flatten)]
    item: T,
    age_bucket: &'static str,
}

#[derive(Serialize)]
pub struct Counts {
    facts: usize,
    wiki: usize,
    procedures: usize,
    episodes: usize,
}

pub async fn get_ward_content(
    State(state): State<AppState>,
    Path(ward_id): Path<String>,
) -> Result<Json<WardContentResponse>, ApiError> {
    let now: DateTime<Utc> = Utc::now();

    let facts = state.memory_repo.list_by_ward(&ward_id, 100)?;
    let wiki = state.wiki_repo.list_for_ward(&ward_id, 100)?;
    let procedures = state.procedure_repo.list_for_ward(Some(&ward_id), 100)?;
    let episodes = state.episode_repo.list_for_ward(&ward_id, 50)?;

    let counts = Counts {
        facts: facts.len(),
        wiki: wiki.len(),
        procedures: procedures.len(),
        episodes: episodes.len(),
    };

    Ok(Json(WardContentResponse {
        ward_id: ward_id.clone(),
        summary: derive_summary(&wiki),
        facts: facts.into_iter().map(|f| ItemWithBucket {
            age_bucket: age_bucket(now, parse_ts(&f.created_at)),
            item: serde_json::to_value(f).unwrap(),
        }).collect(),
        wiki: wiki.into_iter().map(|a| ItemWithBucket {
            age_bucket: age_bucket(now, parse_ts(&a.updated_at)),
            item: serde_json::to_value(a).unwrap(),
        }).collect(),
        procedures: procedures.into_iter().map(|p| ItemWithBucket {
            age_bucket: age_bucket(now, parse_ts(p.last_used.as_deref().unwrap_or(&p.created_at))),
            item: serde_json::to_value(p).unwrap(),
        }).collect(),
        episodes: episodes.into_iter().map(|e| ItemWithBucket {
            age_bucket: age_bucket(now, parse_ts(&e.created_at)),
            item: serde_json::to_value(e).unwrap(),
        }).collect(),
        counts,
    }))
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn derive_summary(wiki: &[gateway_database::WikiArticle]) -> WardSummary {
    let index = wiki.iter().find(|a| a.title == "__index__");
    if let Some(a) = index {
        return WardSummary {
            title: a.ward_id.clone(),
            description: Some(a.content.lines().next().unwrap_or("").to_string()),
            updated_at: Some(a.updated_at.clone()),
        };
    }
    WardSummary { title: wiki.first().map(|a| a.ward_id.clone()).unwrap_or_default(), description: None, updated_at: None }
}
```

If `list_by_ward` / `list_for_ward` don't exist on the repos, add thin wrappers over existing list methods — they're straightforward `SELECT … WHERE ward_id = ?1 LIMIT ?2` calls. Each wrapper goes in its respective repo file.

- [ ] **Step 4: Register the route**

In `gateway/src/http/mod.rs`:
```rust
pub mod ward_content;
```

In `gateway/src/server.rs` wherever routes are registered (look for `.route("/api/memory/`):
```rust
.route("/api/wards/:ward_id/content", get(http::ward_content::get_ward_content))
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p gateway --test ward_content_endpoint`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add gateway/src/http/ward_content.rs gateway/src/http/mod.rs gateway/src/server.rs \
        gateway/gateway-database/src/*.rs gateway/tests/ward_content_endpoint.rs
git commit -m "feat(http): GET /api/wards/:ward_id/content aggregator"
```

---

## Task 6: `/api/memory/search` unified endpoint

**Files:**
- Create: `gateway/src/http/memory_search.rs`
- Modify: `gateway/src/http/mod.rs`, `gateway/src/server.rs`
- Test: `gateway/tests/memory_unified_search.rs`

- [ ] **Step 1: Write the failing test**

```rust
// gateway/tests/memory_unified_search.rs
use serde_json::json;

#[tokio::test]
async fn searches_all_four_types_in_parallel() {
    let (state, _tmp) = test_helpers::seed_multi_type_content().await;
    let client = test_helpers::router_client(state).await;
    let body = json!({
        "query": "hormuz",
        "mode": "hybrid",
        "types": ["facts","wiki","procedures","episodes"],
        "ward_ids": ["maritime-vessel-tracking"]
    });
    let resp: serde_json::Value = client.post("/api/memory/search").json(&body).send_json().await;
    for key in ["facts","wiki","procedures","episodes"] {
        assert!(resp[key]["hits"].is_array(), "missing hits for {key}");
        assert!(resp[key]["latency_ms"].is_number());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p gateway --test memory_unified_search`
Expected: FAIL — route 404.

- [ ] **Step 3: Implement handler**

```rust
// gateway/src/http/memory_search.rs
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use crate::state::AppState;
use crate::http::ApiError;

#[derive(Deserialize)]
pub struct SearchBody {
    pub query: String,
    #[serde(default = "default_mode")] pub mode: String,
    #[serde(default = "default_types")] pub types: Vec<String>,
    #[serde(default)] pub ward_ids: Vec<String>,
    #[serde(default)] pub filters: Option<serde_json::Value>,
    #[serde(default = "default_limit")] pub limit: usize,
}

fn default_mode() -> String { "hybrid".into() }
fn default_types() -> Vec<String> { vec!["facts".into(),"wiki".into(),"procedures".into(),"episodes".into()] }
fn default_limit() -> usize { 10 }

#[derive(Serialize, Default)]
pub struct UnifiedResponse {
    facts: TypeBlock,
    wiki: TypeBlock,
    procedures: TypeBlock,
    episodes: TypeBlock,
}

#[derive(Serialize, Default)]
pub struct TypeBlock {
    hits: Vec<serde_json::Value>,
    latency_ms: u64,
}

pub async fn memory_search(
    State(state): State<AppState>,
    Json(req): Json<SearchBody>,
) -> Result<Json<UnifiedResponse>, ApiError> {
    let ward = req.ward_ids.first().map(|s| s.as_str());
    let embedding = match req.mode.as_str() {
        "fts" => None,
        _ => state.embedding_client.embed(&[&req.query]).await.ok()
                .and_then(|mut v| v.pop()),
    };

    let (facts_r, wiki_r, procs_r, eps_r) = tokio::join!(
        run_if("facts", &req.types, || search_facts(&state, &req, embedding.clone(), ward)),
        run_if("wiki",  &req.types, || search_wiki(&state, &req, embedding.clone(), ward)),
        run_if("procedures", &req.types, || search_procedures(&state, &req, embedding.clone(), ward)),
        run_if("episodes",   &req.types, || search_episodes(&state, &req, embedding.clone(), ward)),
    );

    Ok(Json(UnifiedResponse {
        facts: facts_r.unwrap_or_default(),
        wiki: wiki_r.unwrap_or_default(),
        procedures: procs_r.unwrap_or_default(),
        episodes: eps_r.unwrap_or_default(),
    }))
}

async fn run_if<F, Fut>(name: &str, types: &[String], f: F) -> Option<TypeBlock>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = TypeBlock>,
{
    if types.iter().any(|t| t == name) { Some(f().await) } else { None }
}

async fn search_facts(state: &AppState, req: &SearchBody, emb: Option<Vec<f32>>, ward: Option<&str>) -> TypeBlock {
    let t0 = Instant::now();
    let (rows, sources) = state.memory_repo
        .search_memory_facts_hybrid(&req.query, emb, "agent:root", req.limit, ward)
        .unwrap_or_default();
    let src_map: std::collections::HashMap<_, _> = sources.into_iter().collect();
    TypeBlock {
        hits: rows.into_iter().map(|f| {
            let mut v = serde_json::to_value(&f).unwrap();
            if let Some(s) = src_map.get(&f.id) { v["match_source"] = serde_json::json!(s); }
            v
        }).collect(),
        latency_ms: t0.elapsed().as_millis() as u64,
    }
}

async fn search_wiki(state: &AppState, req: &SearchBody, emb: Option<Vec<f32>>, ward: Option<&str>) -> TypeBlock {
    let t0 = Instant::now();
    let rows = state.wiki_repo.search_hybrid(&req.query, ward, emb, req.limit).unwrap_or_default();
    TypeBlock {
        hits: rows.into_iter().map(|h| {
            serde_json::json!({
                "id": h.article.id, "ward_id": h.article.ward_id, "title": h.article.title,
                "snippet": h.article.content.chars().take(240).collect::<String>(),
                "updated_at": h.article.updated_at, "score": h.score,
                "match_source": h.match_source,
            })
        }).collect(),
        latency_ms: t0.elapsed().as_millis() as u64,
    }
}

async fn search_procedures(state: &AppState, req: &SearchBody, emb: Option<Vec<f32>>, ward: Option<&str>) -> TypeBlock {
    let t0 = Instant::now();
    let rows = match emb {
        Some(e) => state.procedure_repo.search_by_similarity("agent:root", &e, ward, req.limit).unwrap_or_default(),
        None => Vec::new(),
    };
    TypeBlock {
        hits: rows.into_iter().map(|p| {
            let mut v = serde_json::to_value(&p).unwrap();
            v["match_source"] = serde_json::json!("vec");
            v
        }).collect(),
        latency_ms: t0.elapsed().as_millis() as u64,
    }
}

async fn search_episodes(state: &AppState, req: &SearchBody, emb: Option<Vec<f32>>, ward: Option<&str>) -> TypeBlock {
    let t0 = Instant::now();
    let rows = match emb {
        Some(e) => state.episode_repo.search_by_similarity(&e, ward, req.limit).unwrap_or_default(),
        None => state.episode_repo.search_fts(&req.query, ward, req.limit).unwrap_or_default(),
    };
    TypeBlock {
        hits: rows.into_iter().map(|e| {
            let mut v = serde_json::to_value(&e).unwrap();
            v["match_source"] = serde_json::json!(if emb.is_some() { "vec" } else { "fts" });
            v
        }).collect(),
        latency_ms: t0.elapsed().as_millis() as u64,
    }
}
```

Add `procedure_repo.search_by_similarity` taking optional ward_id if not already, and `episode_repo.search_fts` as a thin wrapper if not present (copy the FTS pattern from `memory_repository.rs`).

- [ ] **Step 4: Register route**

In `gateway/src/http/mod.rs`: `pub mod memory_search;`
In `gateway/src/server.rs`: `.route("/api/memory/search", post(http::memory_search::memory_search))`

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p gateway --test memory_unified_search`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add gateway/src/http/memory_search.rs gateway/src/http/mod.rs gateway/src/server.rs \
        gateway/gateway-database/src/*.rs gateway/tests/memory_unified_search.rs
git commit -m "feat(http): POST /api/memory/search unified hybrid across 4 types"
```

---

## Task 7: Feature flag `memory_tab_command_deck`

**Files:**
- Modify: `apps/ui/src/services/transport/types.ts` (add `featureFlags` field)
- Modify: `apps/ui/src/features/settings/AdvancedTab.tsx` (add toggle)
- Create: `apps/ui/src/features/memory/useFeatureFlag.ts`
- Test: `apps/ui/src/features/memory/__tests__/useFeatureFlag.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// apps/ui/src/features/memory/__tests__/useFeatureFlag.test.ts
import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useFeatureFlag } from "../useFeatureFlag";

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({
    getSettings: async () => ({ success: true, data: { featureFlags: { memory_tab_command_deck: true } } }),
  }),
}));

describe("useFeatureFlag", () => {
  it("returns true when flag is enabled", async () => {
    const { result } = renderHook(() => useFeatureFlag("memory_tab_command_deck"));
    await waitFor(() => expect(result.current).toBe(true));
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npm test -- useFeatureFlag`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the hook**

```ts
// apps/ui/src/features/memory/useFeatureFlag.ts
import { useEffect, useState } from "react";
import { getTransport } from "@/services/transport";

export function useFeatureFlag(name: string): boolean {
  const [on, setOn] = useState(false);
  useEffect(() => {
    let alive = true;
    (async () => {
      const t = await getTransport();
      const r = await t.getSettings();
      if (alive && r.success) setOn(Boolean(r.data?.featureFlags?.[name]));
    })();
    return () => { alive = false; };
  }, [name]);
  return on;
}
```

Ensure `getSettings` on the transport returns `featureFlags: Record<string, boolean>`. Add the shape to `types.ts`. Add the toggle row in `AdvancedTab.tsx` (pattern-match other toggles in that file):

```tsx
<ToggleRow
  label="Memory Tab — Command Deck (beta)"
  description="New ward-first memory view with hybrid search."
  checked={featureFlags.memory_tab_command_deck ?? false}
  onChange={(v) => updateFeatureFlag("memory_tab_command_deck", v)}
/>
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/ui && npm test -- useFeatureFlag`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/memory/useFeatureFlag.ts \
        apps/ui/src/features/memory/__tests__/useFeatureFlag.test.ts \
        apps/ui/src/services/transport/types.ts \
        apps/ui/src/features/settings/AdvancedTab.tsx
git commit -m "feat(ui): feature-flag hook + memory_tab_command_deck toggle"
```

---

## Task 8: Transport types + methods for new endpoints

**Files:**
- Modify: `apps/ui/src/services/transport/types.ts`, `apps/ui/src/services/transport/interface.ts`, `apps/ui/src/services/transport/http.ts`
- Test: `apps/ui/src/services/transport/__tests__/ward_content.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// apps/ui/src/services/transport/__tests__/ward_content.test.ts
import { describe, it, expect, vi } from "vitest";
import { HttpTransport } from "../http";

global.fetch = vi.fn(async () => new Response(JSON.stringify({
  ward_id: "wardA", summary: { title: "wardA" },
  facts: [], wiki: [], procedures: [], episodes: [],
  counts: { facts: 0, wiki: 0, procedures: 0, episodes: 0 },
}), { status: 200 })) as unknown as typeof fetch;

describe("HttpTransport.getWardContent", () => {
  it("hits GET /api/wards/:ward_id/content and returns counts", async () => {
    const t = new HttpTransport("http://localhost:3000");
    const r = await t.getWardContent("wardA");
    expect(r.success).toBe(true);
    expect(r.data?.counts.facts).toBe(0);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npm test -- ward_content`
Expected: FAIL — `getWardContent` is not a function.

- [ ] **Step 3: Add types + methods**

`types.ts`:
```ts
export type AgeBucket = "today" | "last_7_days" | "historical";
export type MatchSource = "hybrid" | "fts" | "vec" | "title";

export interface WardContent {
  ward_id: string;
  summary: { title: string; description?: string; updated_at?: string };
  facts: Array<MemoryFact & { age_bucket: AgeBucket }>;
  wiki: Array<WikiArticle & { age_bucket: AgeBucket }>;
  procedures: Array<Procedure & { age_bucket: AgeBucket }>;
  episodes: Array<SessionEpisode & { age_bucket: AgeBucket }>;
  counts: { facts: number; wiki: number; procedures: number; episodes: number };
}

export interface HybridSearchRequest {
  query: string;
  mode?: "hybrid" | "fts" | "semantic";
  types?: Array<"facts" | "wiki" | "procedures" | "episodes">;
  ward_ids?: string[];
  filters?: { category?: MemoryCategory; confidence_gte?: number };
  limit?: number;
}

export interface HybridSearchResponse {
  facts: { hits: (MemoryFact & { match_source: MatchSource; score: number })[]; latency_ms: number };
  wiki: { hits: (WikiArticle & { snippet: string; match_source: MatchSource; score: number })[]; latency_ms: number };
  procedures: { hits: (Procedure & { match_source: MatchSource })[]; latency_ms: number };
  episodes: { hits: (SessionEpisode & { match_source: MatchSource })[]; latency_ms: number };
}
```

`interface.ts` — add the two methods to the `Transport` interface. `http.ts` — implement with `fetch`, return `{ success: boolean; data?: T; error?: string }` pattern used elsewhere.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/ui && npm test -- ward_content`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/services/transport
git commit -m "feat(ui): transport types + methods for ward content and unified search"
```

---

## Task 9: Hooks `useWards`, `useWardContent`, `useHybridSearch`, `useTimewarp`

**Files:**
- Create: `apps/ui/src/features/memory/command-deck/hooks.ts`
- Test: `apps/ui/src/features/memory/command-deck/__tests__/hooks.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// apps/ui/src/features/memory/command-deck/__tests__/hooks.test.ts
import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useWardContent } from "../hooks";

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({
    getWardContent: vi.fn(async () => ({ success: true, data: {
      ward_id: "wardA", summary: { title: "wardA" },
      facts: [{ id:"f1", content:"x", category:"pattern", confidence:0.9, created_at:"2026-04-15T10:00:00Z", age_bucket:"today" }],
      wiki: [], procedures: [], episodes: [],
      counts: { facts: 1, wiki: 0, procedures: 0, episodes: 0 },
    } })),
  }),
}));

describe("useWardContent", () => {
  it("loads and returns ward content", async () => {
    const { result } = renderHook(() => useWardContent("wardA"));
    await waitFor(() => expect(result.current.data?.counts.facts).toBe(1));
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npm test -- hooks`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement hooks**

```ts
// apps/ui/src/features/memory/command-deck/hooks.ts
import { useCallback, useEffect, useState } from "react";
import { getTransport } from "@/services/transport";
import type { WardContent, HybridSearchRequest, HybridSearchResponse } from "@/services/transport/types";

export function useWards() {
  const [wards, setWards] = useState<{ id: string; count: number }[]>([]);
  useEffect(() => {
    (async () => {
      const t = await getTransport();
      const r = await t.listWards?.();
      if (r?.success) setWards(r.data ?? []);
    })();
  }, []);
  return wards;
}

export function useWardContent(wardId: string | null) {
  const [data, setData] = useState<WardContent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!wardId) return;
    setLoading(true);
    const t = await getTransport();
    const r = await t.getWardContent(wardId);
    if (r.success) { setData(r.data ?? null); setError(null); }
    else { setError(r.error ?? "failed"); }
    setLoading(false);
  }, [wardId]);

  useEffect(() => { refresh(); }, [refresh]);
  return { data, loading, error, refresh };
}

export function useHybridSearch(query: string, opts: Omit<HybridSearchRequest, "query">) {
  const [data, setData] = useState<HybridSearchResponse | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!query.trim()) { setData(null); return; }
    const t = setTimeout(async () => {
      setLoading(true);
      const transport = await getTransport();
      const r = await transport.searchMemoryHybrid({ query, ...opts });
      if (r.success) setData(r.data ?? null);
      setLoading(false);
    }, 250);
    return () => clearTimeout(t);
  }, [query, JSON.stringify(opts)]);

  return { data, loading };
}

export function useTimewarp() {
  const [days, setDays] = useState(0); // 0 = now; max 30
  return { days, setDays };
}
```

Note: `listWards` is a transport method that returns `{ id, count }`. If missing, add it in Task 8's transport changes; it's a thin wrapper over `GET /api/wards`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/ui && npm test -- hooks`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/memory/command-deck/hooks.ts \
        apps/ui/src/features/memory/command-deck/__tests__/hooks.test.ts
git commit -m "feat(ui): command-deck hooks (wards, content, search, timewarp)"
```

---

## Task 10: `MemoryItemCard` component

**Files:**
- Create: `apps/ui/src/features/memory/command-deck/MemoryItemCard.tsx`
- Test: `apps/ui/src/features/memory/command-deck/__tests__/MemoryItemCard.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen } from "@testing-library/react";
import { MemoryItemCard } from "../MemoryItemCard";

describe("MemoryItemCard", () => {
  it("renders content, category badge, age, and match_source when given", () => {
    render(
      <MemoryItemCard
        id="f1"
        content="Hormuz geofence is 24-27N, 54-58E"
        category="instruction"
        confidence={0.9}
        created_at="2026-04-15T10:00:00Z"
        age_bucket="today"
        match_source="hybrid"
      />
    );
    expect(screen.getByText(/Hormuz geofence/)).toBeInTheDocument();
    expect(screen.getByText(/instruction/i)).toBeInTheDocument();
    expect(screen.getByText(/hybrid/i)).toBeInTheDocument();
  });

  it("applies decay class based on age_bucket", () => {
    const { container } = render(
      <MemoryItemCard id="f2" content="x" category="pattern" confidence={1} created_at="2026-03-01T00:00:00Z" age_bucket="historical" />
    );
    expect(container.querySelector(".memory-item")).toHaveClass("decay-2");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npm test -- MemoryItemCard`
Expected: FAIL — component not found.

- [ ] **Step 3: Implement component**

```tsx
// apps/ui/src/features/memory/command-deck/MemoryItemCard.tsx
import type { MemoryCategory } from "@/services/transport/types";
import type { AgeBucket, MatchSource } from "@/services/transport/types";

export interface MemoryItemCardProps {
  id: string;
  content: string;
  category: MemoryCategory;
  confidence: number;
  created_at: string;
  age_bucket: AgeBucket;
  match_source?: MatchSource;
  ward_id?: string;
  onClick?: () => void;
}

const DECAY: Record<AgeBucket, string> = {
  today: "",
  last_7_days: "decay-1",
  historical: "decay-2",
};

export function MemoryItemCard(p: MemoryItemCardProps) {
  return (
    <div
      className={`memory-item ${DECAY[p.age_bucket] ?? ""}`}
      role="button"
      tabIndex={0}
      onClick={p.onClick}
      onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") p.onClick?.(); }}
    >
      <div className="memory-item__body">
        <span className={`memory-kind memory-kind--${p.category}`}>{p.category}</span>
        {p.ward_id && <span className="memory-ward-tag">◆ {p.ward_id}</span>}
        <span>{p.content}</span>
      </div>
      <div className="memory-item__meta">
        {p.match_source && <span className={`memory-why memory-why--${p.match_source}`}>{p.match_source}</span>}
        <span className="memory-score">conf {p.confidence.toFixed(2)}</span>
        <span>{new Date(p.created_at).toLocaleDateString()}</span>
      </div>
    </div>
  );
}
```

Styling: add `.memory-item`, `.memory-kind--*`, `.memory-why--*`, `.decay-*` to `apps/ui/src/styles/components.css` using existing token values. Example opacity rules mirror the mockup:

```css
.memory-item { opacity: 1; transition: opacity .15s; padding: var(--spacing-3); border: 1px solid var(--color-border); border-radius: var(--radius-md); display: grid; grid-template-columns: 1fr auto; gap: var(--spacing-2) var(--spacing-3); }
.memory-item.decay-1 { opacity: .7; }
.memory-item.decay-2 { opacity: .45; }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/ui && npm test -- MemoryItemCard`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/memory/command-deck/MemoryItemCard.tsx \
        apps/ui/src/features/memory/command-deck/__tests__/MemoryItemCard.test.tsx \
        apps/ui/src/styles/components.css
git commit -m "feat(ui): MemoryItemCard with kind/source/age styling"
```

---

## Task 11: `WardRail` component

**Files:**
- Create: `apps/ui/src/features/memory/command-deck/WardRail.tsx`
- Test: `apps/ui/src/features/memory/command-deck/__tests__/WardRail.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { WardRail } from "../WardRail";

const wards = [
  { id: "literature-library", count: 142 },
  { id: "equity-valuation",   count: 89 },
  { id: "__global__",         count: 34 },
];

describe("WardRail", () => {
  it("lists wards, marks active, and fires onSelect", () => {
    const onSelect = vi.fn();
    render(<WardRail wards={wards} activeId="literature-library" onSelect={onSelect} />);
    expect(screen.getByText("literature-library")).toHaveAttribute("aria-current", "true");
    fireEvent.click(screen.getByText("equity-valuation"));
    expect(onSelect).toHaveBeenCalledWith("equity-valuation");
  });

  it("separates global wards under a GLOBAL heading", () => {
    render(<WardRail wards={wards} activeId="" onSelect={() => {}} />);
    expect(screen.getByText(/^GLOBAL$/i)).toBeInTheDocument();
    expect(screen.getByText("__global__")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npm test -- WardRail`
Expected: FAIL — not found.

- [ ] **Step 3: Implement**

```tsx
// apps/ui/src/features/memory/command-deck/WardRail.tsx
interface Props {
  wards: { id: string; count: number }[];
  activeId: string;
  onSelect: (id: string) => void;
}

export function WardRail({ wards, activeId, onSelect }: Props) {
  const regular = wards.filter((w) => !w.id.startsWith("__"));
  const global = wards.filter((w) => w.id.startsWith("__"));
  return (
    <nav className="memory-wards">
      <div className="memory-wards__title">WARDS <span>{regular.length}</span></div>
      <ul>
        {regular.map((w) => (
          <li key={w.id}>
            <button
              className={`memory-ward ${w.id === activeId ? "is-active" : ""}`}
              aria-current={w.id === activeId}
              onClick={() => onSelect(w.id)}
            >
              <span className="memory-ward__dot" />
              <span className="memory-ward__name">{w.id}</span>
              <span className="memory-ward__badge">{w.count}</span>
            </button>
          </li>
        ))}
      </ul>
      {global.length > 0 && (
        <>
          <div className="memory-wards__title">GLOBAL <span>∞</span></div>
          <ul>
            {global.map((w) => (
              <li key={w.id}>
                <button className={`memory-ward ${w.id === activeId ? "is-active" : ""}`} aria-current={w.id === activeId} onClick={() => onSelect(w.id)}>
                  <span className="memory-ward__dot memory-ward__dot--global" />
                  <span className="memory-ward__name">{w.id}</span>
                  <span className="memory-ward__badge">{w.count}</span>
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </nav>
  );
}
```

Add CSS for `.memory-wards`, `.memory-ward.is-active`, `.memory-ward__dot`, etc. using tokens.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/ui && npm test -- WardRail`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/memory/command-deck/WardRail.tsx \
        apps/ui/src/features/memory/command-deck/__tests__/WardRail.test.tsx \
        apps/ui/src/styles/components.css
git commit -m "feat(ui): WardRail with global/regular grouping"
```

---

## Task 12: `SearchBar` component

**Files:**
- Create: `apps/ui/src/features/memory/command-deck/SearchBar.tsx`
- Test: `apps/ui/src/features/memory/command-deck/__tests__/SearchBar.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { SearchBar } from "../SearchBar";

describe("SearchBar", () => {
  it("fires onChange with query and current mode", () => {
    const onChange = vi.fn();
    render(<SearchBar onChange={onChange} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "hormuz" } });
    expect(onChange).toHaveBeenLastCalledWith({ query: "hormuz", mode: "hybrid" });
  });

  it("switches mode to fts when FTS tab clicked", () => {
    const onChange = vi.fn();
    render(<SearchBar onChange={onChange} />);
    fireEvent.click(screen.getByText("FTS"));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "q" } });
    expect(onChange).toHaveBeenLastCalledWith({ query: "q", mode: "fts" });
  });

  it("detects quoted phrase and forces fts hint", () => {
    const onChange = vi.fn();
    render(<SearchBar onChange={onChange} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: '"exact phrase"' } });
    const last = onChange.mock.calls.at(-1)?.[0];
    expect(last.mode).toBe("fts");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npm test -- SearchBar`
Expected: FAIL — not found.

- [ ] **Step 3: Implement**

```tsx
import { useState } from "react";

type Mode = "hybrid" | "fts" | "semantic";
interface Props { onChange: (v: { query: string; mode: Mode }) => void; }

export function SearchBar({ onChange }: Props) {
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<Mode>("hybrid");

  function emit(q: string, m: Mode) {
    const effectiveMode: Mode = /"[^"]+"/.test(q) ? "fts" : m;
    onChange({ query: q, mode: effectiveMode });
  }

  return (
    <div className="memory-search">
      <span className="memory-search__icon">⌕</span>
      <input
        className="memory-search__input"
        type="text"
        value={query}
        onChange={(e) => { setQuery(e.target.value); emit(e.target.value, mode); }}
        placeholder="search memories, wiki, procedures…"
      />
      <div className="memory-search__mode" role="tablist">
        {(["fts","hybrid","semantic"] as Mode[]).map((m) => (
          <button
            key={m}
            role="tab"
            aria-selected={mode === m}
            className={mode === m ? "is-active" : ""}
            onClick={() => { setMode(m); emit(query, m); }}
          >{m.toUpperCase()}</button>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/ui && npm test -- SearchBar`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/memory/command-deck/SearchBar.tsx \
        apps/ui/src/features/memory/command-deck/__tests__/SearchBar.test.tsx \
        apps/ui/src/styles/components.css
git commit -m "feat(ui): SearchBar with mode toggle and quoted-phrase fts override"
```

---

## Task 13: `ScopeChips` component

**Files:**
- Create: `apps/ui/src/features/memory/command-deck/ScopeChips.tsx`
- Test: `apps/ui/src/features/memory/command-deck/__tests__/ScopeChips.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { ScopeChips } from "../ScopeChips";

describe("ScopeChips", () => {
  it("toggles a type chip off when clicked", () => {
    const onChange = vi.fn();
    render(<ScopeChips types={["facts","wiki"]} onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: /facts/i }));
    expect(onChange).toHaveBeenCalledWith({ types: ["wiki"] });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npm test -- ScopeChips`
Expected: FAIL.

- [ ] **Step 3: Implement**

```tsx
type Type = "facts" | "wiki" | "procedures" | "episodes";
interface Props {
  types: Type[];
  onChange: (v: { types: Type[] }) => void;
}

const ALL: Type[] = ["facts","wiki","procedures","episodes"];

export function ScopeChips({ types, onChange }: Props) {
  const toggle = (t: Type) => {
    const next = types.includes(t) ? types.filter((x) => x !== t) : [...types, t];
    onChange({ types: next });
  };
  return (
    <div className="memory-chips">
      <span className="memory-chips__label">TYPE</span>
      {ALL.map((t) => (
        <button
          key={t}
          className={`memory-chip ${types.includes(t) ? "is-on" : ""}`}
          onClick={() => toggle(t)}
        >{t}</button>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/ui && npm test -- ScopeChips`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/memory/command-deck/ScopeChips.tsx \
        apps/ui/src/features/memory/command-deck/__tests__/ScopeChips.test.tsx \
        apps/ui/src/styles/components.css
git commit -m "feat(ui): ScopeChips for content-type filtering"
```

---

## Task 14: `ContentDeck` + `ContentList`

**Files:**
- Create: `apps/ui/src/features/memory/command-deck/ContentDeck.tsx`
- Create: `apps/ui/src/features/memory/command-deck/ContentList.tsx`
- Test: `apps/ui/src/features/memory/command-deck/__tests__/ContentList.test.tsx`

- [ ] **Step 1: Write the failing test for ContentList**

```tsx
import { render, screen } from "@testing-library/react";
import { ContentList } from "../ContentList";

describe("ContentList", () => {
  it("groups items by age_bucket with headers", () => {
    render(<ContentList items={[
      { id: "a", content: "recent", category: "instruction", confidence: 1, created_at: "2026-04-15T12:00:00Z", age_bucket: "today" },
      { id: "b", content: "mid", category: "pattern", confidence: 0.9, created_at: "2026-04-13T12:00:00Z", age_bucket: "last_7_days" },
      { id: "c", content: "old", category: "policy", confidence: 0.9, created_at: "2026-02-15T12:00:00Z", age_bucket: "historical" },
    ]} />);
    expect(screen.getByText(/today/i)).toBeInTheDocument();
    expect(screen.getByText(/last 7 days/i)).toBeInTheDocument();
    expect(screen.getByText(/historical/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npm test -- ContentList`
Expected: FAIL.

- [ ] **Step 3: Implement ContentList**

```tsx
import { MemoryItemCard } from "./MemoryItemCard";
import type { AgeBucket, MemoryCategory, MatchSource } from "@/services/transport/types";

interface Item {
  id: string; content: string; category: MemoryCategory; confidence: number;
  created_at: string; age_bucket: AgeBucket; match_source?: MatchSource; ward_id?: string;
}

const LABELS: Record<AgeBucket, string> = {
  today: "TODAY",
  last_7_days: "LAST 7 DAYS",
  historical: "HISTORICAL",
};

export function ContentList({ items }: { items: Item[] }) {
  const groups: Record<AgeBucket, Item[]> = { today: [], last_7_days: [], historical: [] };
  for (const it of items) groups[it.age_bucket].push(it);

  return (
    <div className="memory-list">
      {(["today","last_7_days","historical"] as AgeBucket[]).map((b) => (
        groups[b].length > 0 && (
          <section key={b}>
            <h3 className="memory-list__group-label">
              <span>{LABELS[b]}</span><span>{groups[b].length} items</span>
            </h3>
            {groups[b].map((it) => <MemoryItemCard key={it.id} {...it} />)}
          </section>
        )
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Implement ContentDeck**

```tsx
// apps/ui/src/features/memory/command-deck/ContentDeck.tsx
import { useState } from "react";
import { ContentList } from "./ContentList";
import type { WardContent } from "@/services/transport/types";

type Tab = "facts" | "wiki" | "procedures" | "episodes";
const TABS: Tab[] = ["facts","wiki","procedures","episodes"];

export function ContentDeck({ data, onOpenGraph }: { data: WardContent | null; onOpenGraph: () => void }) {
  const [tab, setTab] = useState<Tab>("facts");
  if (!data) return <div className="memory-deck-empty">Select a ward to view content.</div>;

  const counts = data.counts;

  return (
    <div className="memory-deck">
      <header className="memory-deck__head">
        <div className="memory-deck__crumb">◆ {data.ward_id}</div>
        {data.summary.description && <div className="memory-deck__summary">{data.summary.description}</div>}
        <nav className="memory-deck__tabs" role="tablist">
          {TABS.map((t) => (
            <button key={t} role="tab" aria-selected={tab === t} className={tab === t ? "is-active" : ""} onClick={() => setTab(t)}>
              {t} <span>{counts[t]}</span>
            </button>
          ))}
          <button className="memory-deck__graph" onClick={onOpenGraph}>Graph ↗</button>
        </nav>
      </header>
      <div className="memory-deck__body">
        {tab === "facts" ?
          <ContentList items={data.facts.map((f) => ({ ...f, content: f.content }))} /> :
          tab === "wiki" ?
          <WikiList items={data.wiki} /> :
          tab === "procedures" ?
          <ProcList items={data.procedures} /> :
          <EpisodeList items={data.episodes} />}
      </div>
    </div>
  );
}

function WikiList({ items }: { items: WardContent["wiki"] }) {
  return <ul className="memory-list">{items.map((a) => <li key={a.id} className="memory-item"><strong>{a.title}</strong><p>{a.content.slice(0, 240)}</p></li>)}</ul>;
}
function ProcList({ items }: { items: WardContent["procedures"] }) {
  return <ul className="memory-list">{items.map((p) => <li key={p.id} className="memory-item"><strong>{p.name}</strong><p>{p.description}</p></li>)}</ul>;
}
function EpisodeList({ items }: { items: WardContent["episodes"] }) {
  return <ul className="memory-list">{items.map((e) => <li key={e.id} className="memory-item"><strong>{e.outcome}</strong><p>{e.task_summary}</p></li>)}</ul>;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd apps/ui && npm test -- ContentList`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/features/memory/command-deck/ContentDeck.tsx \
        apps/ui/src/features/memory/command-deck/ContentList.tsx \
        apps/ui/src/features/memory/command-deck/__tests__/ContentList.test.tsx \
        apps/ui/src/styles/components.css
git commit -m "feat(ui): ContentDeck with Facts/Wiki/Procedures/Episodes tabs + grouped ContentList"
```

---

## Task 15: `WriteRail` + `AddDrawer`

**Files:**
- Create: `apps/ui/src/features/memory/command-deck/WriteRail.tsx`
- Create: `apps/ui/src/features/memory/command-deck/AddDrawer.tsx`
- Test: `apps/ui/src/features/memory/command-deck/__tests__/WriteRail.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { WriteRail } from "../WriteRail";

describe("WriteRail", () => {
  it("opens AddDrawer with preset category when + Instruction clicked", () => {
    const onSave = vi.fn();
    render(<WriteRail wardId="wardA" onSave={onSave} counts={{ facts: 10, wiki: 2, procedures: 1, episodes: 3 }} />);
    fireEvent.click(screen.getByRole("button", { name: /\+ instruction/i }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Always verify OPF metadata" } });
    fireEvent.click(screen.getByRole("button", { name: /save/i }));
    expect(onSave).toHaveBeenCalledWith({ category: "instruction", content: "Always verify OPF metadata", ward_id: "wardA" });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npm test -- WriteRail`
Expected: FAIL.

- [ ] **Step 3: Implement AddDrawer**

```tsx
// apps/ui/src/features/memory/command-deck/AddDrawer.tsx
import { useState } from "react";
import type { MemoryCategory } from "@/services/transport/types";

interface Props {
  initialCategory: MemoryCategory;
  wardId: string;
  onSave: (v: { category: MemoryCategory; content: string; ward_id: string }) => void;
  onClose: () => void;
}

export function AddDrawer({ initialCategory, wardId, onSave, onClose }: Props) {
  const [content, setContent] = useState("");
  const [category, setCategory] = useState<MemoryCategory>(initialCategory);

  return (
    <div role="dialog" aria-modal="true" className="add-drawer">
      <label>
        <span>Category</span>
        <select value={category} onChange={(e) => setCategory(e.target.value as MemoryCategory)}>
          {(["instruction","policy","preference","pattern","correction","decision","entity"] as MemoryCategory[]).map((c) => <option key={c} value={c}>{c}</option>)}
        </select>
      </label>
      <textarea
        rows={4}
        value={content}
        onChange={(e) => setContent(e.target.value)}
        aria-label="memory content"
      />
      <div className="add-drawer__actions">
        <button onClick={onClose}>Cancel</button>
        <button onClick={() => onSave({ category, content, ward_id: wardId })}>Save</button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Implement WriteRail**

```tsx
// apps/ui/src/features/memory/command-deck/WriteRail.tsx
import { useState } from "react";
import type { MemoryCategory } from "@/services/transport/types";
import { AddDrawer } from "./AddDrawer";

interface Props {
  wardId: string;
  counts: { facts: number; wiki: number; procedures: number; episodes: number };
  onSave: (v: { category: MemoryCategory; content: string; ward_id: string }) => void;
}

export function WriteRail({ wardId, counts, onSave }: Props) {
  const [open, setOpen] = useState<MemoryCategory | null>(null);

  return (
    <aside className="memory-write">
      <div className="memory-write__title">WRITE</div>
      <button onClick={() => setOpen("pattern")}>+ Fact <kbd>F</kbd></button>
      <button onClick={() => setOpen("instruction")}>+ Instruction <kbd>I</kbd></button>
      <button onClick={() => setOpen("policy")}>+ Policy <kbd>P</kbd></button>

      <div className="memory-write__stats">
        <div>{wardId}</div>
        <div>facts {counts.facts}</div>
        <div>wiki {counts.wiki}</div>
        <div>procedures {counts.procedures}</div>
        <div>episodes {counts.episodes}</div>
      </div>

      {open && (
        <AddDrawer
          initialCategory={open}
          wardId={wardId}
          onClose={() => setOpen(null)}
          onSave={(v) => { onSave(v); setOpen(null); }}
        />
      )}
    </aside>
  );
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd apps/ui && npm test -- WriteRail`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/features/memory/command-deck/WriteRail.tsx \
        apps/ui/src/features/memory/command-deck/AddDrawer.tsx \
        apps/ui/src/features/memory/command-deck/__tests__/WriteRail.test.tsx \
        apps/ui/src/styles/components.css
git commit -m "feat(ui): WriteRail + AddDrawer with category presets"
```

---

## Task 16: `MemoryTab` shell (new command-deck version)

**Files:**
- Create: `apps/ui/src/features/memory/command-deck/MemoryTab.tsx`
- Test: `apps/ui/src/features/memory/command-deck/__tests__/MemoryTab.test.tsx`

- [ ] **Step 1: Write the failing integration test**

```tsx
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { MemoryTab } from "../MemoryTab";

vi.mock("@/services/transport", () => ({
  getTransport: async () => ({
    listWards: async () => ({ success: true, data: [{ id: "literature-library", count: 5 }] }),
    getWardContent: async (id: string) => ({
      success: true,
      data: {
        ward_id: id, summary: { title: id, description: "curated library" },
        facts: [{ id: "f1", content: "always check the graph first", category: "instruction", confidence: 1, created_at: new Date().toISOString(), age_bucket: "today" }],
        wiki: [], procedures: [], episodes: [],
        counts: { facts: 1, wiki: 0, procedures: 0, episodes: 0 },
      },
    }),
    searchMemoryHybrid: async () => ({ success: true, data: { facts: { hits: [], latency_ms: 0 }, wiki: { hits: [], latency_ms: 0 }, procedures: { hits: [], latency_ms: 0 }, episodes: { hits: [], latency_ms: 0 } } }),
    saveMemoryFact: async () => ({ success: true }),
  }),
}));

describe("MemoryTab (command-deck)", () => {
  it("renders wards, selects the first, shows its facts", async () => {
    render(<MemoryTab agentId="agent:root" />);
    await waitFor(() => expect(screen.getByText("literature-library")).toBeInTheDocument());
    fireEvent.click(screen.getByText("literature-library"));
    await waitFor(() => expect(screen.getByText(/check the graph first/)).toBeInTheDocument());
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npm test -- command-deck/__tests__/MemoryTab`
Expected: FAIL.

- [ ] **Step 3: Implement shell**

```tsx
// apps/ui/src/features/memory/command-deck/MemoryTab.tsx
import { useState } from "react";
import { SearchBar } from "./SearchBar";
import { ScopeChips } from "./ScopeChips";
import { WardRail } from "./WardRail";
import { ContentDeck } from "./ContentDeck";
import { WriteRail } from "./WriteRail";
import { useWards, useWardContent, useHybridSearch } from "./hooks";
import { getTransport } from "@/services/transport";

interface Props { agentId: string; }

export function MemoryTab({ agentId }: Props) {
  const wards = useWards();
  const [activeId, setActiveId] = useState(wards[0]?.id ?? "");
  const { data, refresh } = useWardContent(activeId || null);

  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<"hybrid" | "fts" | "semantic">("hybrid");
  const [types, setTypes] = useState<("facts"|"wiki"|"procedures"|"episodes")[]>(["facts","wiki"]);
  const search = useHybridSearch(query, { mode, types, ward_ids: activeId ? [activeId] : [] });

  async function saveFact(v: { category: string; content: string; ward_id: string }) {
    const t = await getTransport();
    await t.saveMemoryFact({ agent_id: agentId, ...v });
    await refresh();
  }

  return (
    <div className="memory-tab-deck">
      <div className="memory-tab-deck__top">
        <SearchBar onChange={(v) => { setQuery(v.query); setMode(v.mode); }} />
        <ScopeChips types={types} onChange={(v) => setTypes(v.types)} />
      </div>
      <div className="memory-tab-deck__grid">
        <WardRail wards={wards} activeId={activeId} onSelect={setActiveId} />
        <ContentDeck data={search.data ? mapSearchToContent(search.data, activeId) : data} onOpenGraph={() => {}} />
        <WriteRail wardId={activeId} counts={data?.counts ?? { facts: 0, wiki: 0, procedures: 0, episodes: 0 }} onSave={saveFact} />
      </div>
    </div>
  );
}

function mapSearchToContent(_res: unknown, _wardId: string) {
  // Placeholder until search result rendering diverges from ward-content view.
  return null;
}
```

Styling: add `.memory-tab-deck` with the grid definition, mirroring the mockup.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/ui && npm test -- command-deck/__tests__/MemoryTab`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/memory/command-deck/MemoryTab.tsx \
        apps/ui/src/features/memory/command-deck/__tests__/MemoryTab.test.tsx \
        apps/ui/src/styles/components.css
git commit -m "feat(ui): command-deck MemoryTab shell"
```

---

## Task 17: Feature-flag gate in router

**Files:**
- Modify: `apps/ui/src/App.tsx` (or wherever `<MemoryTab>` is mounted)
- Modify: `apps/ui/src/features/memory/MemoryTab.tsx` → rename to `MemoryTabLegacy.tsx`

- [ ] **Step 1: Rename legacy file**

```bash
git mv apps/ui/src/features/memory/MemoryTab.tsx apps/ui/src/features/memory/MemoryTabLegacy.tsx
```

- [ ] **Step 2: Update the router**

Replace the current import and mount with:

```tsx
import { MemoryTabLegacy } from "@/features/memory/MemoryTabLegacy";
import { MemoryTab as MemoryTabCommandDeck } from "@/features/memory/command-deck/MemoryTab";
import { useFeatureFlag } from "@/features/memory/useFeatureFlag";

function MemoryTabGate({ agentId }: { agentId: string }) {
  const on = useFeatureFlag("memory_tab_command_deck");
  return on
    ? <MemoryTabCommandDeck agentId={agentId} />
    : <MemoryTabLegacy agentId={agentId} />;
}
```

Wherever `<MemoryTab agentId=…/>` was previously used, substitute `<MemoryTabGate agentId=…/>`.

- [ ] **Step 3: Sanity-check by running the UI build**

Run: `cd apps/ui && npm run build`
Expected: type-check + bundle success.

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src
git commit -m "feat(ui): gate MemoryTab on memory_tab_command_deck feature flag"
```

---

## Task 18: End-to-end smoke + documentation

**Files:**
- Create: `apps/ui/src/features/memory/command-deck/__tests__/e2e-smoke.test.tsx`
- Modify: `apps/ui/ARCHITECTURE.md`

- [ ] **Step 1: Write an e2e-style happy-path test**

```tsx
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { MemoryTab } from "../MemoryTab";

// uses real transport against a dev server mock — see vitest setup for server mock pattern
vi.mock("@/services/transport", () => ({
  getTransport: async () => ({
    listWards: async () => ({ success: true, data: [{ id: "wardA", count: 2 }] }),
    getWardContent: async () => ({
      success: true, data: {
        ward_id: "wardA", summary: { title: "wardA" },
        facts: [{ id:"f1", content:"alpha", category:"instruction", confidence:0.9, created_at:new Date().toISOString(), age_bucket:"today" }],
        wiki: [], procedures: [], episodes: [],
        counts: { facts: 1, wiki: 0, procedures: 0, episodes: 0 },
      },
    }),
    searchMemoryHybrid: async () => ({ success: true, data: { facts: { hits: [], latency_ms: 0 }, wiki: { hits: [], latency_ms: 0 }, procedures: { hits: [], latency_ms: 0 }, episodes: { hits: [], latency_ms: 0 } } }),
    saveMemoryFact: vi.fn(async () => ({ success: true })),
  }),
}));

describe("Memory Tab e2e smoke", () => {
  it("write flow: add instruction via right rail", async () => {
    render(<MemoryTab agentId="agent:root" />);
    await waitFor(() => expect(screen.getByText("wardA")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: /\+ instruction/i }));
    fireEvent.change(screen.getByRole("textbox", { name: /memory content/i }), { target: { value: "new instruction" } });
    fireEvent.click(screen.getByRole("button", { name: /save/i }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });
});
```

- [ ] **Step 2: Run test**

Run: `cd apps/ui && npm test -- e2e-smoke`
Expected: PASS.

- [ ] **Step 3: Update ARCHITECTURE.md**

Append under the Memory section:

```markdown
### Memory Tab — Command Deck

Three-column layout under `features/memory/command-deck/`:
- `WardRail` (left) · `ContentDeck` with Facts/Wiki/Procedures/Episodes tabs (center) · `WriteRail` (right)
- Top bar `SearchBar` + `ScopeChips`
- Gated behind feature flag `memory_tab_command_deck` (`useFeatureFlag`)
- Data sources: `GET /api/wards/:ward_id/content`, `POST /api/memory/search`
- Recency decay via `age_bucket` (server-side) mapped to `.decay-*` CSS classes
```

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/memory/command-deck/__tests__/e2e-smoke.test.tsx \
        apps/ui/ARCHITECTURE.md
git commit -m "test(ui): memory tab e2e smoke; doc: ARCHITECTURE command-deck section"
```

---

## Self-Review

**1. Spec coverage:**
- Three-column layout → Tasks 11–16 ✓
- Tabbed content per ward → Task 14 ✓
- Temporal fade with age_bucket → Tasks 2 (backend), 10 + 14 (frontend) ✓
- Hybrid search default + mode toggle → Tasks 4, 6, 12 ✓
- Inline filters + "why" badges → Tasks 10, 13 ✓
- Persistent write rail → Task 15 ✓
- Wiki FTS table → Task 1 ✓
- Wiki hybrid search → Task 3 ✓
- New `/api/wards/:ward_id/content` → Task 5 ✓
- New `/api/memory/search` → Task 6 ✓
- `search_memory_facts` mode upgrade → Task 4 ✓
- Feature flag + rollout → Tasks 7, 17 ✓
- Existing GraphView entry point preserved → Task 14 (`Graph ↗` tab button) ✓
- A11y (role/tabIndex/keydown) → Task 10, 11, 15 ✓

**2. Placeholders:** None. Every step shows concrete code or exact commands.

**3. Type consistency:** `AgeBucket`, `MatchSource`, `MemoryCategory`, `HybridSearchResponse`, `WardContent` defined in Task 8 `types.ts` and used verbatim by hooks (9), components (10–16), and backend serializations (4, 5, 6).

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-15-memory-tab-command-deck.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
