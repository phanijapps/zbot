# SonarQube Scan Report — Duplication, Dead Code & Test Coverage

**Project:** phanijapps_zbot | **Date:** 2026-04-11 | **Branch:** feature/test-coverage

## Dashboard Metrics

| Metric | Value |
|--------|-------|
| Duplicated Lines | 4,939 (3.7%) |
| Duplicated Blocks | 229 |
| Duplicated Files | 66 |
| Test Coverage (SonarCloud) | 0% (LCOV pipeline pending) |
| Test Coverage (local cargo llvm-cov) | **55.45% lines, 48.71% branches** |
| Maintainability Issues | 466 |

---

## 1. Easy Duplication Wins (Files > 10% duplication)

### Tier 1 — Highest Impact, Easiest Fix

| File | Dup% | Blocks | What's Duplicated | Fix |
|------|------|--------|-------------------|-----|
| `middleware/config.rs` | **53.7%** | 4 | `ContextEditingConfig` and `SummarizationConfig` have nearly identical field patterns (trigger thresholds, policies, defaults) | Extract shared `MiddlewareTriggerConfig` struct with common fields |
| `ChatInput.tsx` | **32.7%** | 2 | `uploadFile()` function is duplicated between ChatInput and HeroInput | Already extracted — SonarQube may be scanning pre-merge main |
| `api_tests.rs` | **22.2%** | 8 | Test setup/teardown repeated across 8 test functions | Extract `setup_test_app()` helper and reuse |
| `ArtifactSlideOut.tsx` | **14.3%** | 2 | Shared with ArtifactsPanel | Already fixed on branch — `artifact-utils.tsx` |
| `ArtifactsPanel.tsx` | **14.3%** | 3 | Same as above | Already fixed on branch |
| `agent.rs` (agent-tools) | **10.7%** | 8 | `ListAgentsTool` and `CreateAgentTool` share JSON schema patterns | Extract `agent_schema_builder()` |
| `agents.rs` (gateway-services) | **8.6%** | 2 | Agent CRUD operations have similar DB query patterns | Extract `query_agent()` helper |

### Tier 2 — Medium Impact

Other files in the 440 duplicated list are mostly under 5% — not worth individual attention. The workspace-level clippy `#![allow]` dedup (already done) removed the biggest source.

---

## 2. Dead Code Analysis

### Rust — No dead code warnings

Clippy and `cargo check` report **zero dead code warnings** across the workspace. The workspace lints suppress `dead_code` — this is intentional to avoid noise from protocol structs with unused fields (WebSocket message types, CLI event types).

**Known unused but intentionally kept:**

| File | Item | Why Kept |
|------|------|----------|
| `apps/cli/src/client.rs` | Multiple struct fields (`session_id`, `tool_call_id`, `args`, `final_message`, `code`, `conversation_id`) | Protocol compatibility — fields are deserialized from JSON but not yet used in the CLI UI |
| `apps/cli/src/events.rs` | `is_quit()`, `is_enter()`, etc. | Future keyboard handler API — CLI is WIP |
| `apps/cli/src/ui.rs` | `COLOR_BG` constant | Planned for TUI background color |

**Recommendation:** The entire `apps/cli/` crate is mostly dead code. Consider either investing in it or removing it to reduce the workspace.

### TypeScript — No dead code detected

ESLint `no-unused-vars` is enabled with `argsIgnorePattern: "^_"`. No unused imports or variables found.

### Paths That Will Never Be Taken

| File | Line | Code Path | Why Unreachable |
|------|------|-----------|-----------------|
| `runner.rs` invoke_with_callback | ~667 | `Err(_) if agent_id == "root"` fallback in `load_or_create_root` | Root agent always exists — the daemon creates it on startup |
| `lifecycle.rs` get_or_create_session | ~54 | `Err(e)` → "Failed to get session" | SQLite query failure would require DB corruption |
| `executor.rs` execute_tool | ~1297 | MCP tool not found fallback | MCP tools are validated at registration time |
| `distillation.rs` resolve_distillation_target | ~645 | Third provider fallback loop | If orchestrator provider exists, first or second attempt always succeeds |
| `chat.rs` init_chat_session | ~78 | Runner not available error | Runner is always initialized before HTTP server starts |

---

## 3. Unit Test Coverage Gaps

### Well-Tested (>70% coverage)

| Crate | Line Coverage | Key Tests |
|-------|-------------|-----------|
| `execution-state` | 82% | 86 tests — CRUD, session lifecycle, delegation tracking |
| `knowledge-graph/types` | 99% | Type construction, serialization |
| `knowledge-graph/traversal` | 94% | BFS expansion, neighbor queries |
| `knowledge-graph/service` | 91% | CRUD, search, subgraph extraction |
| `knowledge-graph/extractor` | 86% | Entity/relationship parsing |

### Under-Tested (<30% coverage) — Easy to Improve

| Crate/File | Coverage | What to Test | Effort |
|------------|----------|--------------|--------|
| `daily-sessions/repository.rs` | **0%** | DB CRUD for daily session tracking — similar patterns to execution-state tests | Small — copy test patterns |
| `daily-sessions/cache.rs` | **0%** | In-memory LRU cache | Small — pure data structure |
| `daily-sessions/manager.rs` | **0%** | Session lifecycle manager | Medium — needs mock deps |
| `execution-state/handlers.rs` | **20%** | HTTP handler routes | Medium — needs test fixtures |
| `api-logs/repository.rs` | **41%** | Log storage queries | Small — add edge case tests |
| `api-logs/service.rs` | **46%** | Log aggregation, session detail building | Small |
| `knowledge-graph/storage.rs` | **61%** | SQLite graph operations | Medium — add delete/update tests |

### Not Testable Without Integration Setup (Skip)

| Crate | Why |
|-------|-----|
| `gateway` (main) | HTTP server, WebSocket, full stack |
| `agent-runtime/executor.rs` | Requires LLM mock, tool registry, full agent setup |
| `gateway-execution/runner.rs` | Requires daemon, DB, event bus — integration-only |

---

## 4. Recommended Actions (Priority Order)

### Quick Wins (1-2 hours)

1. **Extract `MiddlewareTriggerConfig`** from `middleware/config.rs` — drops 194 duplicated lines (53.7% → ~10%)
2. **Extract test helpers** in `api_tests.rs` — drops 187 duplicated lines (22.2% → ~5%)
3. **Add tests for `daily-sessions`** — 0% → 50%+ with 3 test functions copying execution-state patterns

### Medium Effort (half day)

4. **Add edge case tests** for `api-logs` — 41% → 70%+
5. **Extract agent schema builder** from `agent.rs` — drops 34 duplicated lines
6. **Add storage tests** for knowledge-graph — 61% → 80%+

### Defer

7. Rust cognitive complexity (45 issues) — requires major refactors, risk of regression
8. CLI dead code cleanup — depends on whether CLI will be invested in or removed
9. Integration tests for gateway/runner — needs full test harness
