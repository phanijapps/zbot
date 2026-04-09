# Memory Layer — Backlog

## Planned Features

### P1: Memory Policies UI
**Status:** Spec written at `docs/superpowers/specs/2026-04-05-memory-policies-backlog.md`

Add a "Policies" tab to the Memory page. Users create/edit/toggle policy facts via UI instead of SQL. Policies are memory facts with category=correction, confidence=1.0, global scope. No new tables — thin API wrapper over upsert_memory_fact.

### P1: Direct Knowledge Graph Query Tool
**Status:** Deferred — requires gateway-execution dependency in agent-tools

Agents can't query the knowledge graph directly. The graph is surfaced indirectly via system-level recall_with_graph() and ward-entry recall. A direct `graph_query` action on the MemoryTool would let agents ask "what files exist for PTON?" and get structured entity/relationship results.

**Blocker:** MemoryTool lives in agent-tools crate. GraphService lives in gateway-execution. Adding the dependency creates a crate coupling issue. Options: (a) pass GraphService as a trait object into MemoryTool, (b) add a graph action that queries via the fact store.

### P2: Memory Pruning / Garbage Collection
**Status:** Not started

Active cleanup of low-value facts:
- Archive facts with confidence < 0.1 after 30 days
- Merge near-duplicate facts (keep highest confidence)
- Prune contradicted facts older than 90 days
- Cap total facts per ward at 500 (archive oldest)

Temporal decay exists in RecallConfig but no active pruning runs. The DB grows without bound.

### P2: Cross-Ward Memory Synthesis
**Status:** Not started

After N sessions, synthesize patterns across wards:
- "code-agent consistently takes 150-250s per task"
- "planner works best when it calls list_agents first"
- "research-agent + light-panda-browser is more reliable than shell curl"

These cross-ward insights become global strategy facts.

### P3: Memory Dashboard in UI
**Status:** Not started

Visual dashboard showing:
- Fact count over time (growth rate)
- Category distribution (corrections vs domain vs pattern)
- Episode outcomes (success/partial/failed trend)
- Most-recalled facts (from recall_log)
- Knowledge graph visualization (entities + relationships)

### P3: Embedding Model Upgrade Path
**Status:** Not started

Current: fastembed all-MiniLM-L6-v2 (384d, local ONNX). Good enough for <10K facts.
Future: Option to use provider's embedding model (e.g., text-embedding-3-small from OpenAI) for better semantic quality. Would require re-embedding all existing facts on model switch.

## Completed (2026-04-05)

- ✅ FTS5 query sanitization (OR-joined terms)
- ✅ System-level recall on first message (recall_with_graph)
- ✅ Intent analysis + memory (recall_for_intent)
- ✅ Subagent priming (recall_for_delegation)
- ✅ Subagent tools: WardTool, MemoryTool, GrepTool
- ✅ Graph relationship dedup (unique index)
- ✅ Entity normalization (file basename matching)
- ✅ Fact verification in distillation (grounding against tool outputs)
- ✅ Fact content dedup in distillation (60% word overlap)
- ✅ Failed episode warnings in recall
- ✅ Curated ward.md (deduped, capped ≤1KB)
- ✅ core_docs.md scans ALL code files (not just core/)
- ✅ Code-agent "read core_docs first" instruction
- ✅ write_file size warning > 5KB
- ✅ Stream decode fallback (non-streaming retry)
- ✅ Z.AI rate limit detection (codes 1234/1302/1303)
- ✅ Predictive recall (already in recall_with_graph)
- ✅ Mid-session recall hook wired
- ✅ Policy injection via memory facts
