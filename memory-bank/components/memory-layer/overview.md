# Memory Layer — The Brain

## Purpose

The memory layer is z-Bot's cognitive system. It stores, retrieves, and applies knowledge across sessions so agents learn from experience, avoid past mistakes, and reuse existing work instead of rediscovering it.

## Design Principles

1. **Every recalled fact saves tokens** — a fact recalled is a fact the agent doesn't rediscover via tool calls
2. **Corrections > strategies > domain** — priority ordering ensures rules are followed first
3. **Accuracy over volume** — 10 verified facts beat 100 hallucinated ones
4. **Ward-scoped + global** — facts belong to a ward or apply everywhere
5. **Write everywhere, read smart** — distillation writes facts/entities/episodes after every session; recall surfaces only what's relevant

## When It Runs

| Phase | What Happens | Trigger |
|-------|-------------|---------|
| **Session start** | `recall_with_graph()` injects recalled facts as system message | First message in `invoke_with_callback()` |
| **Intent analysis** | `recall_for_intent()` enriches the intent prompt with memory context | Before intent LLM call |
| **Ward entry** | Ward-scoped facts loaded when agent enters a ward | `WardTool` execution |
| **Subagent spawn** | `recall_for_delegation()` primes subagent with corrections, skills, context | `spawn_delegated_agent()` |
| **Mid-session** | `RecallHook` injects new facts every N turns | Executor loop (configurable) |
| **Session end** | Distillation extracts facts, entities, relationships, episodes | Fire-and-forget after completion |
| **Post-distillation** | ward.md and core_docs.md auto-regenerated | `auto_update_memory_bank()` |

## Architecture

```
                    ┌─────────────────────────────────┐
                    │         USER MESSAGE              │
                    └──────────┬──────────────────────┘
                               │
                    ┌──────────▼──────────────────────┐
                    │     SYSTEM-LEVEL RECALL           │
                    │  recall_with_graph()              │
                    │  → FTS5 (OR-joined terms)         │
                    │  → Vector cosine similarity       │
                    │  → Episode similarity search      │
                    │  → Graph entity expansion         │
                    │  → Predictive boost               │
                    │  → Priority scoring               │
                    │  ↓ Injected as system message     │
                    └──────────┬──────────────────────┘
                               │
                    ┌──────────▼──────────────────────┐
                    │     INTENT ANALYSIS + MEMORY      │
                    │  recall_for_intent()              │
                    │  → Corrections, strategies        │
                    │  → Graph entities                 │
                    │  → Similar past episodes          │
                    │  ↓ Injected into intent prompt    │
                    └──────────┬──────────────────────┘
                               │
                    ┌──────────▼──────────────────────┐
                    │     AGENT EXECUTION               │
                    │  WardTool → ward-entry recall     │
                    │  MemoryTool → agent self-recall   │
                    │  GrepTool → code discovery        │
                    └──────────┬──────────────────────┘
                               │
                    ┌──────────▼──────────────────────┐
                    │     SESSION DISTILLATION          │
                    │  LLM extracts from transcript:    │
                    │  → Facts (verified against tools) │
                    │  → Entities (normalized names)    │
                    │  → Relationships (deduped)        │
                    │  → Episodes (with outcome)        │
                    │  ↓ Stored in SQLite + KG          │
                    └──────────┬──────────────────────┘
                               │
                    ┌──────────▼──────────────────────┐
                    │     WARD KNOWLEDGE SYNC           │
                    │  ward.md — curated rules (≤1KB)   │
                    │  core_docs.md — all code sigs     │
                    │  structure.md — directory tree     │
                    └─────────────────────────────────┘
```

## Storage

| Store | Location | What |
|-------|----------|------|
| `memory_facts` | `conversations.db` | 238+ facts with embeddings, categories, confidence, ward scoping |
| `embedding_cache` | `conversations.db` | SHA-256 hash → embedding vector cache |
| `session_episodes` | `conversations.db` | Session outcomes with strategy, learnings, embeddings |
| `recall_log` | `conversations.db` | Which facts were recalled per session (for predictive recall) |
| `distillation_runs` | `conversations.db` | Tracking extraction runs (success/failed/skipped) |
| `memory_facts_archive` | `conversations.db` | Pruned/archived facts |
| `kg_entities` | `knowledge_graph.db` | Entities (persons, files, projects, tools) with mention counts |
| `kg_relationships` | `knowledge_graph.db` | Entity relationships (created, part_of, related_to) |
| `ward.md` | `wards/{id}/memory-bank/` | Curated rules: max 5 corrections, 3 strategies, 2 warnings |
| `core_docs.md` | `wards/{id}/memory-bank/` | Code inventory: all files with function signatures |
| `structure.md` | `wards/{id}/memory-bank/` | Directory tree with file purposes |

## Recall Scoring Pipeline

```
base_score = (0.7 × vector_cosine) + (0.3 × BM25_score)
    × category_weight    (correction: 1.5, strategy: 1.4, user: 1.3, instruction: 1.2, domain: 1.0)
    × ward_affinity      (1.3x if fact matches current ward)
    × temporal_decay      (exponential decay with per-category half-lives)
    × mention_boost       (1.0 + log2(mention_count))
    × contradiction_penalty (0.7x if contradicted)
    × predictive_boost    (1.3x if recalled in similar past sessions)
```

## Recall Output Format

```markdown
## Rules (from past corrections — ALWAYS follow these)
- NEVER rely on LLM training data for factual content...
- Always use duckduckgo-search skill for web research...

### Warnings (past failures — avoid these approaches)
- FAILED: Built inflation app without research — no real data.

### Preferences & Instructions
- User prefers visual, ADHD-friendly content with 3-minute sections

### Past Experiences
- SPY analysis (2026-04-05): SUCCESS — planner→code→data-analyst, 300K tokens

### Domain Knowledge
- [domain] Portfolio: PTON, NVDA, TSLA, AAPL, SPY at 20% each

### Related Entities
- PTON (organization) — analyzed by data-analyst, code-agent
```

## Policies

High-priority corrections injected as memory facts with:
- `category: "correction"` (1.5x weight in recall)
- `confidence: 1.0`
- `ward_id: "__global__"` (applies everywhere)
- `mention_count: 5` (high boost)

Policies surface at the top of every recall. Currently managed via direct SQL; UI planned (see backlog).

## Key Files

| File | Purpose |
|------|---------|
| `gateway/gateway-database/src/memory_repository.rs` | MemoryFact CRUD, FTS5 search, vector search, hybrid search |
| `gateway/gateway-database/src/episode_repository.rs` | SessionEpisode storage, similarity search |
| `gateway/gateway-database/src/recall_log_repository.rs` | Tracks which facts recalled per session |
| `gateway/gateway-execution/src/recall.rs` | MemoryRecall service: recall(), recall_with_graph(), recall_for_intent(), recall_for_delegation() |
| `gateway/gateway-execution/src/distillation.rs` | SessionDistiller: LLM fact/entity/episode extraction with verification |
| `gateway/gateway-execution/src/ward_sync.rs` | Generates curated ward.md from facts (deduped, capped) |
| `gateway/gateway-execution/src/runner.rs` | auto_update_memory_bank(): generates core_docs.md + structure.md |
| `gateway/gateway-services/src/recall_config.rs` | RecallConfig with deep-merge JSON overrides |
| `gateway/gateway-database/src/memory_fact_store.rs` | MemoryFactStore trait impl with contradiction detection |
| `services/knowledge-graph/src/storage.rs` | GraphStorage: entities, relationships, dedup, normalization |
| `runtime/agent-tools/src/tools/memory.rs` | MemoryTool: agent-facing save_fact/recall actions |
