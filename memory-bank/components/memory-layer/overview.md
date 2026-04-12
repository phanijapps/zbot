# Memory Layer — z-Bot's Cognitive System

## Purpose

The memory layer stores, retrieves, and applies knowledge across sessions so agents learn from experience, avoid past mistakes, and reuse existing work instead of rediscovering it.

Originally a passive fact store, the layer now includes **six cognitive capabilities** (Phases 1–6) that transform z-Bot from a stateless executor into an agent that accumulates knowledge, remembers procedures, and queries its own history.

## Design Principles

1. **Every recalled fact saves tokens** — a fact recalled is a fact the agent doesn't rediscover via tool calls
2. **Corrections > strategies > domain** — priority ordering ensures rules are followed first
3. **Accuracy over volume** — 10 verified facts beat 100 hallucinated ones
4. **Ward-scoped + global** — facts belong to a ward or apply everywhere
5. **Epistemic honesty** — archival (historical) facts never decay; volatile state does
6. **Provenance end-to-end** — every entity and relationship traces to its source episode
7. **Write everywhere, read smart** — distillation, ward artifacts, tool results, and agents all feed memory; recall surfaces only what's relevant

## Six Capability Layers

z-Bot's memory is not a single system but a stack of cooperating layers, each with distinct responsibilities:

| # | Layer | Role | Files |
|---|-------|------|-------|
| 0 | **Base memory** | Facts, embeddings, episodes, distillation | `memory_repository.rs`, `recall.rs`, `distillation.rs` |
| 1 | **Knowledge graph** | Entities, relationships, typed ontology, causal edges | `services/knowledge-graph/`, `graph_query.rs` |
| 2 | **Working memory** | Live per-iteration context scratchpad | `invoke/working_memory.rs` |
| 3 | **Ward wiki** | Karpathy-style compiled knowledge per ward | `ward_wiki.rs`, `wiki_repository.rs` |
| 4 | **Procedural memory** | Reusable multi-step action sequences | `procedure_repository.rs` + distillation extraction |
| 5 | **Micro-recall** | Targeted lookups at decision points | `invoke/micro_recall.rs` |
| 6 | **KG evolution** | Episodes, ward artifact indexer, expanded ontology, entity resolver, epistemic classes, multi-view queries, real-time extraction | `kg_episode_repository.rs`, `ward_artifact_indexer.rs`, `resolver.rs`, `tool_result_extractor.rs` |

Each layer has its own doc:
- [`cognitive-layers.md`](cognitive-layers.md) — Layers 2–5 (working memory, wiki, procedures, micro-recall)
- [`knowledge-graph.md`](knowledge-graph.md) — Layer 1 + Layer 6 (the full graph architecture)
- [`data-model.md`](data-model.md) — Every table, every column, schema version history
- [`backlog.md`](backlog.md) — Planned future work

## When Each Layer Runs

| Phase | Layers Active | Trigger |
|-------|---------------|---------|
| **Session start** | 0, 1, 3 | `recall_with_graph()` — facts + wiki articles + graph context injected as system message |
| **Intent analysis** | 0, 4 | `recall_for_intent()` + procedure recall — memory context and proven procedures added to intent prompt |
| **Subagent spawn** | 0, 1 | `recall_for_delegation_with_graph()` — corrections + graph-enriched context for child agent |
| **Each iteration** | 2 | Working memory renders as system message, updated after every tool result |
| **Per tool result** | 5, 6 | Micro-recall triggers (error, ward entry, delegation, entity mention) + real-time tool extraction |
| **Mid-session (every N turns)** | 0 | Mid-session recall hook refreshes working memory |
| **Session end** | 0, 1, 3, 4, 6 | Distillation extracts facts/entities/relationships/procedures; ward wiki compiles; ward artifact indexer scans structured files |

## Architecture (Current State, Post-Phase 6)

```
┌──────────────────────── INGESTION LAYER ─────────────────────────┐
│                                                                   │
│  Distillation LLM   ─┐                                            │
│  Ward Artifact Index ─┤                                           │
│  Tool Result Extract ─┼─► Episodes ─► Resolver ─► Storage         │
│  Session Transcript ─┤                                            │
│  User Corrections   ─┘                                            │
│                                                                   │
└──────────────────────────┬────────────────────────────────────────┘
                           │
           ┌───────────────┼───────────────┐
           │               │               │
┌──────────▼──────┐ ┌─────▼──────────┐ ┌──▼──────────────────┐
│  memory_facts   │ │  kg_entities   │ │ ward_wiki_articles  │
│  + embeddings   │ │ kg_relationships│ │ + procedures        │
│  + temporal cols│ │ + causal edges │ │ + kg_episodes       │
│  + epistemic    │ │ + bitemporal   │ │                     │
└──────────┬──────┘ └────────┬───────┘ └──────────┬──────────┘
           │                 │                    │
           └────────┬────────┴─────────┬──────────┘
                    │                  │
     ┌──────────────▼──────┐  ┌────────▼───────────────┐
     │   RECALL SERVICE     │  │   QUERY SERVICE         │
     │  class-aware scoring │  │  MAGMA views:           │
     │  wiki-first          │  │  - semantic             │
     │  procedure matching  │  │  - temporal             │
     │  graph enrichment    │  │  - entity (connections) │
     │                      │  │  - hybrid (reranked)    │
     └──────────────┬───────┘  └────────┬────────────────┘
                    │                   │
                    └─────────┬─────────┘
                              │
                    ┌─────────▼────────┐
                    │   AGENT CONTEXT  │
                    │  System message  │
                    │  Working memory  │
                    │  Graph tool      │
                    │  Memory tool     │
                    └──────────────────┘
```

## Epistemic Classes (Phase 6c)

Every fact, entity, and relationship carries an `epistemic_class` that determines its lifecycle:

| Class | Behavior | Examples |
|-------|----------|----------|
| `archival` | **Never decays with age.** Mild 0.3x penalty only if explicitly corrected. | "Gandhi born 1869", "Savarkar president 1937–38", PDF contents |
| `current` | Decays sharply (0.1x) when superseded. | "AAPL = $523", "INR/USD = 85.2", "current president" |
| `convention` | Stable, no temporal decay. Replaced only on explicit policy change. | "Use plotly not matplotlib", user preferences |
| `procedural` | No temporal decay. Evolves via success/failure counts. | "stock_analysis_report procedure, 87% success" |

This is the user's core insight baked into the system: **historical archives are forever**. They may be corrected later but never become irrelevant.

## Storage Summary

| Store | Location | What |
|-------|----------|------|
| `memory_facts` | `conversations.db` | Facts with embeddings, epistemic class, provenance, temporal columns |
| `embedding_cache` | `conversations.db` | SHA-256 → embedding vector cache |
| `session_episodes` | `conversations.db` | Session outcomes with strategy and learnings |
| `recall_log` | `conversations.db` | Which facts were recalled per session (predictive recall) |
| `distillation_runs` | `conversations.db` | Extraction run tracking |
| `ward_wiki_articles` | `conversations.db` | Compiled per-ward knowledge (Phase 3) |
| `procedures` | `conversations.db` | Reusable multi-step action sequences (Phase 4) |
| `kg_episodes` | `conversations.db` | Provenance records for every extraction (Phase 6a) |
| `kg_entities` | `knowledge_graph.db` | Typed entities with aliases, epistemic class, bitemporal validity |
| `kg_relationships` | `knowledge_graph.db` | Directional edges with bitemporal `valid_at`/`invalidated_at` |
| `kg_causal_edges` | `knowledge_graph.db` | Causal relationships (causes/prevents/requires/enables) |
| `ward.md`, `core_docs.md`, `structure.md` | `wards/{id}/memory-bank/` | Curated ward markdown artifacts |

Full schema details in [`data-model.md`](data-model.md).

## Recall Scoring Pipeline (Current State)

```
base_score = (0.7 × vector_cosine) + (0.3 × BM25_score)
    × category_weight        (correction: 1.5, strategy: 1.4, user: 1.3, instruction: 1.2, domain: 1.0)
    × ward_affinity          (1.3x if fact matches current ward)
    × mention_boost          (1.0 + log2(mention_count))
    × contradiction_penalty  (0.7x if contradicted)
    × predictive_boost       (1.3x if recalled in similar past sessions)
    × class_aware_penalty    (Phase 6c — applies only to superseded facts)
        - archival: 0.3x if corrected, else no penalty
        - current: 0.1x if superseded
        - convention: no penalty
        - procedural: no penalty
```

## User-Managed Memory (Policies, Instructions, About Me)

Three types of user-managed entries, all stored as regular memory facts with reserved key prefixes:

| Type | Category | Confidence | Weight | Purpose |
|------|----------|------------|--------|---------|
| **Policy** | `correction` | 1.0 | 1.5x | Hard rules agents MUST follow |
| **Instruction** | `instruction` | 0.9 | 1.2x | Soft preferences that guide behavior |
| **About Me** | `user` | 0.95 | 1.3x | Personal context for personalization |

All user-created entries are **pinned** (protected from distillation overwrite). Keys starting with `policy.`, `instruction.`, or `user.profile` are reserved — distillation skips them.

### Default Seeded Policies (from `gateway/templates/default_policies.json`)

8 policies + 2 instructions + 1 about-me seeded on first run:
- Research first (never rely on training data)
- Code modularity (files < 3KB, import existing)
- Web research tools (use skills, not shell curl)
- Atomic delegation (one file per step)
- Ward first (enter ward, read docs)
- Planner discovers agents (call list_agents)
- Update docs after code (core_docs.md mandatory)
- Full delegation spec (include goal, input, output, acceptance)
- Output format preference (interactive HTML, Tailwind)
- Documentation quality (SDK-level core_docs.md)
- Default about-me ("I am a private person, just call me Mr Z.")

## Research Foundation

The cognitive capabilities are grounded in recent AI agent memory research:

| Paper | Contribution to z-Bot | Implementation |
|-------|----------------------|----------------|
| **Graphiti/Zep** (arXiv:2501.13956) | Episode-based ingestion, bitemporal edges, entity resolution, hybrid search | `kg_episodes`, temporal columns, `EntityResolver` |
| **MAGMA** (arXiv:2601.03236) | Multi-view queries (semantic/temporal/causal/entity) | `GraphView` enum + `search_entities_view` |
| **A-MEM** (NeurIPS 2025) | Zettelkasten self-organization, dynamic linking | `EntityResolver::merge_alias` + alias accumulation |
| **Karpathy's LLM Wiki** | Compile knowledge once, don't re-derive per query | Ward wiki (Phase 3) + Ward artifact indexer (6a) |
| **MemGPT/Letta** | Virtual context, two-tier memory | Working memory (Phase 2) — live scratchpad + recall for deep context |
| **CIDOC CRM / Wikidata** | Epistemic status distinction (archival vs current) | `epistemic_class` column on facts, entities, relationships |

## Key Files

| File | Purpose |
|------|---------|
| `gateway/gateway-database/src/memory_repository.rs` | MemoryFact CRUD + hybrid search |
| `gateway/gateway-database/src/episode_repository.rs` | Session episodes |
| `gateway/gateway-database/src/recall_log_repository.rs` | Predictive recall tracking |
| `gateway/gateway-database/src/wiki_repository.rs` | Ward wiki articles (Phase 3) |
| `gateway/gateway-database/src/procedure_repository.rs` | Procedures (Phase 4) |
| `gateway/gateway-database/src/kg_episode_repository.rs` | Episodes for KG provenance (Phase 6a) |
| `gateway/gateway-execution/src/recall.rs` | MemoryRecall service, class-aware scoring |
| `gateway/gateway-execution/src/distillation.rs` | LLM-based fact/entity/procedure extraction |
| `gateway/gateway-execution/src/ward_wiki.rs` | Karpathy-style wiki compilation |
| `gateway/gateway-execution/src/ward_artifact_indexer.rs` | JSON collection → graph entities (Phase 6a) |
| `gateway/gateway-execution/src/tool_result_extractor.rs` | Real-time tool output → graph (Phase 6d) |
| `gateway/gateway-execution/src/invoke/working_memory.rs` | Live context scratchpad (Phase 2) |
| `gateway/gateway-execution/src/invoke/working_memory_middleware.rs` | Entity extraction from tool results |
| `gateway/gateway-execution/src/invoke/micro_recall.rs` | Decision-point lookups (Phase 5) |
| `gateway/gateway-execution/src/ward_sync.rs` | ward.md generation |
| `gateway/gateway-services/src/recall_config.rs` | RecallConfig with JSON overrides |
| `services/knowledge-graph/src/storage.rs` | Graph entity/relationship CRUD |
| `services/knowledge-graph/src/resolver.rs` | Entity resolution cascade (Phase 6b) |
| `services/knowledge-graph/src/service.rs` | GraphService with multi-view queries (Phase 6d) |
| `services/knowledge-graph/src/causal.rs` | Causal edges (Phase 1) |
| `runtime/agent-tools/src/tools/memory.rs` | Agent-facing memory tool |
| `runtime/agent-tools/src/tools/graph_query.rs` | Agent-facing graph query tool (Phase 1 + 6d) |
