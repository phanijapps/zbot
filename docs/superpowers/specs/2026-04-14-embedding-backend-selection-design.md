# Embedding Backend Selection — Design

**Date:** 2026-04-14
**Branch:** `feature/embedding-backend-selection`
**Status:** Design (awaiting user review)

## Goal

Let the user choose between two embedding backends — the daemon-internal `BGE-small-en-v1.5` (default) and an Ollama-hosted model from a curated list — via Settings → Advanced. Switching backends triggers an automatic reindex when dimensions differ. The internal embedding model is not loaded into daemon memory when Ollama is selected.

## Context

Current state: `gateway/src/state.rs:238-245` hardcodes `LocalEmbeddingClient::new()` which uses `EmbeddingModel::AllMiniLML6V2` (384d, MTEB ~50). The user wants quality flexibility — internal for speed, Ollama for higher MTEB scores — without manual reindex steps.

The trait `EmbeddingClient` and its two implementations (`LocalEmbeddingClient` for fastembed, `OpenAiEmbeddingClient` for HTTP-based providers) already exist. Ollama exposes OpenAI-compat `/v1/embeddings`, so the existing HTTP client speaks to it natively.

## Non-goals

- Spawning embeddings as a child process (Phase 2 idea, not in scope here)
- Multiple simultaneous backends or per-ward backend selection
- Custom Ollama models beyond a curated dropdown (defer to a future "advanced" mode)
- Free-text URL for non-Ollama OpenAI-compat embedding endpoints (defer)

## Acceptance criteria

- [ ] Settings → Advanced has an "Embeddings" panel with internal/Ollama toggle, URL field, and curated model dropdown
- [ ] Default fresh-install state is internal BGE-small (384d, ~130MB)
- [ ] Toggling to Ollama and saving triggers: validate model → pull if missing (with progress) → reindex if dim changed (with progress) → swap client atomically
- [ ] Switching back to internal triggers the same reindex flow
- [ ] When Ollama is the active backend, internal embedding model is NOT resident in daemon memory (Arc dropped, fastembed cleaned up)
- [ ] Daemon boot succeeds even if Ollama is unreachable; recall degrades to FTS-only with a UI banner
- [ ] sqlite-vec virtual tables (`memory_facts_index`, `kg_name_index`, `session_episodes_index`) are reindexed on dim change with daemon-side progress reporting
- [ ] On daemon crash mid-reindex, boot cleanup removes orphan `*__new` tables and the system can resume cleanly
- [ ] Ollama unreachable mid-session → embedding-based recall suspended, FTS continues; UI surfaces the degraded state
- [ ] `~/Documents/zbot/data/.embedding-state` marker file tracks indexed dim atomically (temp + rename)
- [ ] UI components have unit tests covering the toggle, model dropdown, and progress modal

## Architecture

### Configuration

`~/Documents/zbot/config/settings.json` gains an `embeddings` section:

```jsonc
{
  "embeddings": {
    "backend": "internal",       // "internal" | "ollama"
    "dimensions": 384,           // tracked for migration detection
    "ollama": {                  // present only when backend == "ollama"
      "base_url": "http://localhost:11434",
      "model": "mxbai-embed-large"
    }
  }
}
```

`~/Documents/zbot/data/.embedding-state` (one-line marker, atomic write):
```
backend=ollama model=mxbai-embed-large dim=1024 indexed_at=2026-04-14T13:00:00Z
```

Two locations because:
- `settings.json` = user intent (what they asked for)
- `.embedding-state` = system fact (what's actually live in indexes)

When they diverge, the daemon reconciles via reindex.

### EmbeddingService

New module: `gateway/gateway-services/src/embedding_service.rs`.

```rust
pub struct EmbeddingService {
    inner: Arc<RwLock<EmbeddingState>>,
    paths: SharedVaultPaths,
}

struct EmbeddingState {
    config: EmbeddingConfig,
    client: Arc<dyn EmbeddingClient>,
    dimensions: usize,
    indexed_dim: usize,
    needs_reindex: bool,
    health: Health,
}

pub enum Health {
    Ready,
    Reindexing { table: &'static str, current: usize, total: usize },
    Pulling { mb_done: u64, mb_total: u64 },
    OllamaUnreachable,
    ModelMissing,
    Misconfigured(String),
}

impl EmbeddingService {
    pub async fn from_config(paths: SharedVaultPaths) -> Result<Self, String>;
    pub fn client(&self) -> Arc<dyn EmbeddingClient>;          // hot-path, read lock
    pub fn health(&self) -> Health;
    pub async fn reconfigure(&self, new: EmbeddingConfig) -> Result<(), String>;
    pub async fn ensure_indexed(&self, db: &KnowledgeDatabase) -> Result<(), String>;
}
```

`state.rs` constructs one `EmbeddingService` at boot. All embedding consumers (memory_fact_store, distillation, recall) call `service.client()` instead of holding an `Arc` directly. The hot path is read-only; reconfigure briefly takes a write lock during atomic swap.

### Curated Ollama model list

Surfaced via `GET /api/embeddings/models` (UI doesn't hardcode):

| # | Ollama tag | Dim | Size | MTEB | Notes |
|---|---|---|---|---|---|
| 1 | `snowflake-arctic-embed:s` | 384 | 130MB | ~57 | Same dim as internal — no reindex |
| 2 | `nomic-embed-text` | 768 | 274MB | ~62 | General purpose |
| 3 | `mxbai-embed-large` | 1024 | 670MB | ~65 | Top MTEB; recommended |
| 4 | `bge-large` | 1024 | 670MB | ~64 | Alternative |
| 5 | `bge-m3` | 1024 | 1.2GB | ~63 | Multilingual + multi-purpose |
| 6 | `snowflake-arctic-embed` | 1024 | 670MB | ~63 | Recent strong entrant |

Dropdown displays as `model-tag (Ndim, sizeMB)`.

### HTTP API

| Endpoint | Method | Purpose |
|---|---|---|
| `/api/embeddings/health` | GET | Returns `{backend, model?, dim, status, indexed_count}` |
| `/api/embeddings/models` | GET | Returns curated dropdown list |
| `/api/embeddings/configure` | POST | Body = new `EmbeddingConfig`. Returns SSE stream of progress events ending with `Ready` or `Error`. |

Progress events on the SSE stream:
```
event: pulling     data: {"mb_done": 412, "mb_total": 670}
event: reindexing  data: {"table": "memory_facts_index", "current": 87, "total": 173}
event: ready       data: {"backend": "ollama", "model": "mxbai-embed-large", "dim": 1024}
event: error       data: {"reason": "...", "rollback": "..." }
```

## Components

### Lifecycle states

```
Boot:
  load settings.json  → config
  read .embedding-state → marker
  build client per config
  if Ollama: GET /api/tags → if down, set Health::OllamaUnreachable but continue
  if marker.dim ≠ config.dim → set needs_reindex=true
  if needs_reindex: ensure_indexed() → reindex pipeline → write new marker

reconfigure(new):
  validate new config
  if new.backend == ollama && model not in /api/tags:
      Health::Pulling
      stream POST /api/pull, update progress
  build new client
  unload old: drop old Arc → fastembed cleans ONNX runtime
  if dims differ: ensure_indexed() (block, show progress)
  swap client + write new marker
  Health::Ready
```

### Reindex pipeline

Three sqlite-vec tables, identical algorithm:

```
For each table (memory_facts_index, kg_name_index, session_episodes_index):

  BEGIN TRANSACTION
    CREATE VIRTUAL TABLE {name}__new USING vec0(embedding float[NEW_DIM])
  COMMIT

  -- Streamed loop, NOT inside the transaction
  SELECT id, <text_column> FROM {source}
    For each batch of 100:
      embed batch via service.client()
      INSERT INTO {name}__new(rowid, embedding) VALUES (?, ?), ...
      emit ProgressEvent::Reindexing{ table, current, total }

  BEGIN TRANSACTION
    DROP TABLE {name}
    ALTER TABLE {name}__new RENAME TO {name}
  COMMIT
```

Boot-time cleanup of orphan `*__new` tables (idempotent):
```sql
DROP TABLE IF EXISTS memory_facts_index__new;
DROP TABLE IF EXISTS kg_name_index__new;
DROP TABLE IF EXISTS session_episodes_index__new;
```

### Schema sites to parameterize

The hardcoded `384` in vec0 DDLs:
- `gateway/gateway-database/src/knowledge_schema.rs` — kg_name_index
- `gateway/gateway-database/src/memory_repository.rs` — memory_facts_index
- `gateway/gateway-database/src/episode_repository.rs` — session_episodes_index

Refactor each to accept the active dimension from `EmbeddingService::dimensions()`.

### UI

New panel under existing Settings → Advanced. Single `EmbeddingsCard` component with:
- Toggle: "Use internal embedding (default, 384d, ~130MB)"
- Conditional Ollama subform (URL input, model dropdown) when toggle is off
- Warning text when selected model dim ≠ current dim
- "Save & Switch" button → opens progress modal
- Progress modal subscribes to `/api/embeddings/configure` SSE stream and renders pulling/reindexing/ready/error states

Validation done at the UI:
- URL must parse as valid URL
- Model must be in dropdown (no free text)
- Submit disabled when current state matches new config

## Data flow (UI Save → backend swap)

```
User clicks Save & Switch
   ↓
UI: POST /api/embeddings/configure  with new EmbeddingConfig
   ↓
Daemon: EmbeddingService::reconfigure(new)
   ↓
   validate new config
   ↓
   if backend=ollama:
       GET {ollama}/api/tags
       if model missing → POST /api/pull (stream)
           emit Pulling events to SSE → UI progress
   ↓
   build new client, unload old (drop Arc)
   ↓
   if dim differs:
       ensure_indexed() runs reindex on each of 3 tables
           emit Reindexing events to SSE → UI progress
   ↓
   write .embedding-state marker (temp + rename)
   ↓
   atomic swap of Arc<EmbeddingClient>
   ↓
   emit Ready event → UI shows success modal
   ↓
UI: refresh page or in-place rehydrate Settings panel
```

## Error handling

| Failure | Behavior |
|---|---|
| Ollama unreachable at boot, ollama-selected | Boot succeeds; `Health::OllamaUnreachable`; UI banner; recall→FTS-only |
| Ollama unreachable mid-session | After 3 failures in 60s, suspend embedding-based recall; UI banner |
| Pull fails (network/disk) | Rollback config save; old backend stays live; UI shows error with Retry |
| Embedding call fails on single row during reindex | Log warning, skip; reindex completes with `{indexed: N, skipped: K}` summary |
| >50% rows fail in a batch | Abort reindex; old index stays; UI shows "embedding backend appears unhealthy" |
| Daemon crash mid-pull | Ollama supports resume natively; UI re-stream picks up where it left off |
| Daemon crash mid-reindex | Boot drops orphan `*__new`; marker unchanged; UI shows "Reindex required" prompt |
| User edits settings.json with invalid model | `Health::Misconfigured(reason)`; UI offers reset to default |
| Concurrent Save (double-click) | Per-process semaphore; second call waits, reapplies if config differs |

## Edge cases

| Case | Behavior |
|---|---|
| 384d Ollama (snowflake-arctic-embed:s) ↔ 384d internal | Swap client only; no reindex |
| Same model, just URL change | Just rebuild HTTP client; no reindex |
| Internal → Ollama with same dim | Just swap client |
| Internal → Ollama with different dim | Pull (if needed) → reindex |
| Pre-feature daemon installed (no `embeddings.*` in settings.json) | Defaults to internal/384d; no migration |
| `.embedding-state` missing | Treat as fresh — full reindex on first set |
| Vault has zero embeddings (fresh install) | Reindex completes in <1s; nothing to do |

## Non-functional requirements

| Category | Requirement |
|---|---|
| Hot-path latency (`client.embed(text)`) | Internal: P50 ≤50ms, P99 ≤100ms. Ollama: P50 ≤400ms, P99 ≤1s |
| Memory (internal active) | ≤200MB resident |
| Memory (internal unloaded) | ≤30MB residual within 5s of switch |
| Memory (Ollama backend) | 0 daemon-side residual |
| Reindex throughput (internal) | ≥20 embeddings/sec |
| Reindex throughput (Ollama) | ≥5 embeddings/sec |
| Marker durability | Atomic write via temp + rename |
| Settings durability | Atomic write via temp + rename |
| Backwards compat | Daemons pre-feature default to internal/384d; no migration job |
| UI tests | Unit tests for EmbeddingsCard (toggle, dropdown, validation), progress modal (pulling, reindexing, error states), SSE event handling |

## Test plan

**Backend:**
- Unit: EmbeddingService state transitions, marker read/write, config validation
- Unit: EmbeddingConfig parse + serialize roundtrip
- Unit: HealthCheck transitions across all states
- Mock Ollama HTTP server for pull-stream parsing
- Integration: boot → switch to Ollama → verify reindex completes → verify embeddings come back at new dim
- Integration: kill daemon mid-reindex → boot → verify orphan table cleanup
- Integration: corrupt `.embedding-state` → boot → verify clean recovery
- Property test: arbitrary (set internal, set Ollama-A, set Ollama-B, swap models) sequences leave system consistent

**UI:**
- Unit: EmbeddingsCard renders defaults
- Unit: toggle off → Ollama panel reveals
- Unit: model selection → warning text appears when dim differs
- Unit: Save button disabled when config unchanged
- Unit: progress modal renders Pulling events correctly
- Unit: progress modal renders Reindexing events with table name
- Unit: error event surfaces with Retry button
- Unit: SSE event handler routes to correct state slice

## Implementation files (touched / new)

| File | Change |
|---|---|
| `gateway/gateway-services/src/embedding_service.rs` | NEW — core service, lifecycle state machine |
| `gateway/gateway-services/src/lib.rs` | export EmbeddingService |
| `gateway/gateway-services/src/recall_config.rs` or `settings.rs` | New `EmbeddingConfig` shape parsed from settings.json |
| `gateway/src/state.rs:238` | Replace direct `LocalEmbeddingClient::new()` with `EmbeddingService::from_config(paths)` |
| `gateway/src/api/embeddings.rs` | NEW — HTTP handlers for `/api/embeddings/{health,models,configure}` |
| `gateway/src/server.rs` | Wire new routes |
| `gateway/gateway-database/src/knowledge_schema.rs` | Parameterize kg_name_index dim |
| `gateway/gateway-database/src/memory_repository.rs` | Parameterize memory_facts_index dim |
| `gateway/gateway-database/src/episode_repository.rs` | Parameterize session_episodes_index dim |
| `gateway/gateway-execution/src/sleep/embedding_reindex.rs` | NEW — reindex routine, called by EmbeddingService |
| `runtime/agent-runtime/src/llm/local_embedding.rs:35` | Default model now `EmbeddingModel::BGESmallENV15` |
| `apps/ui/src/features/settings/EmbeddingsCard.tsx` | NEW — UI component |
| `apps/ui/src/features/settings/EmbeddingsCard.test.tsx` | NEW — unit tests |
| `apps/ui/src/features/settings/EmbeddingProgressModal.tsx` | NEW — progress modal |
| `apps/ui/src/features/settings/EmbeddingProgressModal.test.tsx` | NEW — unit tests |
| `apps/ui/src/services/transport/http.ts` | Add `getEmbeddingsHealth`, `getEmbeddingsModels`, `configureEmbeddings(config, onProgress)` |

## Risks

- **sqlite-vec doesn't support live dim changes.** Reindex is the only path. Mitigated by progress reporting + boot-time cleanup of partial states.
- **Ollama reliability.** Loopback HTTP should be stable, but a busy host can produce 502s. Mitigated by health-check + degraded recall + clear UI signaling.
- **Pull time variability.** A 1.2GB model on a slow connection takes >5 minutes. Mitigated by Ollama's resume-on-retry + progress reporting.
- **User edits settings.json by hand.** Mitigated by validation + `Health::Misconfigured` state + UI reset-to-default option.
- **Backwards compat for old `memory_facts_index` rows.** Existing 384d embeddings are valid for internal/BGE-small (same dim). Switching to Ollama with same dim works without reindex but vectors are effectively garbage (different model space). Only same-model swaps avoid reindex; same-dim-different-model swaps still need reindex. To handle this: treat any backend or model change (not just dim) as requiring reindex. Update marker comparison logic accordingly.

## Implementation phasing

1. **Phase 1 — backend swap mechanics** (no UI yet)
   - EmbeddingService + state machine
   - Settings.json schema
   - Marker file
   - Reindex routine
   - Boot-time reconcile + orphan cleanup
   - HTTP API endpoints

2. **Phase 2 — UI**
   - EmbeddingsCard
   - Progress modal
   - SSE wiring + tests

3. **Phase 3 — Ollama integration polish**
   - Health-check loop
   - Pull-stream handling + Retry on failure
   - Degraded-state banner

Each phase ends green (cargo test --workspace, npm test for UI) before proceeding.

## Related work / dependencies

- Builds on PR `feature/ward-reuse-cleanup` (knowledge graph quality work) which lands first
- Backlog item #132 (Embedding upgrade: BGE-small drop-in + provider-driven Path C) — this design supersedes that and provides the full UI-driven flow
