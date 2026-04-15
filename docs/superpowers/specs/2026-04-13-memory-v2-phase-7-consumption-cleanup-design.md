# Memory v2 Phase 7 — Consumption Cleanup Design

**Date:** 2026-04-13
**Branch target:** `feature/memory-v2-phase-7` (forked from `feature/memory-v2-phase-6` after Phase 6 merges)
**Status:** Design

## Problem

Phases 1–6 built a sophisticated memory + knowledge graph: v22 schema, sleep-time compaction, episodes, pairwise entity merging, cross-session synthesis, procedural patterns, unified RRF recall. The **production side** — how that knowledge actually reaches agents during inference — has decayed into three legacy paths, a shadow call, and a policy contradiction. The graph is built well and consumed poorly.

Exploration confirmed:
- `recall_unified` (the Phase 5 smart path) is only called as a **shadow log** in `delegation/spawn.rs:334-348`; legacy `recall_for_delegation_with_graph` drives real injection
- **Session-start recall is gated on `!is_fast_mode()`**, which skips memory injection for the entire chat-mode UX path
- Subagents have the memory tool + shard **conditionally stripped** (`invoke/setup.rs:383-389`), making knowledge one-way
- `chat_protocol.md` tells agents "Do NOT use `memory(action=recall)` at the start of every turn" — written assuming chat = no memory. That assumption is wrong.
- Three public recall methods (`recall_with_graph`, `recall_for_intent`, `recall_for_delegation_with_graph`) each with its own enrichment quirks

## Mode clarification (precondition for the spec)

"Fast mode" has been conflating two unrelated concerns: *skip memory* and *skip pipeline*. These are split cleanly here.

| | Research mode | Chat mode |
|---|---|---|
| Memory injection (session start, full recall_unified pool) | ✅ | ✅ |
| Micro-recall (tool errors, entity mentions, ward entry) | ✅ | ✅ |
| Agent memory tool + `memory_learning.md` shard | ✅ | ✅ |
| Skill discovery / loading | ✅ | ✅ |
| Intent analysis | ✅ | ❌ |
| Planning phase | ✅ | ❌ |
| Delegation / subagents | ✅ | ❌ |
| Ward transitions / SDLC flow | ✅ | ❌ |
| Long-running iteration (max_iterations > 1) | ✅ | ❌ (single response) |

**Chat mode = memory + skills + direct response.** Research mode = memory + skills + full cognitive pipeline.

## Goal

One recall path. Memory available in both modes. Subagents fully enabled as memory readers *and* writers. No silent bypasses. No contradicting prompt guidance.

## Non-goals

- No schema changes (v22 stays)
- No new sleep-time jobs (Phase 6 owns those)
- No UI work (mode selection UI handled separately once backend is clean)

---

## Component 7a — Unified recall promotion

**Kill list (all deleted, not deprecated):**
1. `RecallService::recall_with_graph` (runner.rs session start)
2. `RecallService::recall_for_intent` (intent analysis)
3. `RecallService::recall_for_delegation_with_graph` (spawn.rs)
4. FTS-only fallback branch in runner.rs (the `if graph_recall_err { plain_fts_recall(...) }` path)
5. Shadow `recall_unified` call at `delegation/spawn.rs:334-348` (promote to real; delete the "Shadow call" log)

**Replacement:** every consumer calls `RecallService::recall_unified(RecallRequest { .. })` where `RecallRequest` is:

```rust
pub struct RecallRequest<'a> {
    pub query: &'a str,                       // user message | task description | error text | entity mention
    pub agent_id: Option<&'a str>,            // filter scope
    pub ward: Option<&'a str>,                // filter scope
    pub categories: &'a [FactCategory],       // empty = all
    pub top_k: usize,                         // default 8, session-start uses 12, micro-recall uses 3
    pub include_episodes: bool,               // default true
}
```

**Callsite adapters (thin, one per trigger):**
- `runner::recall_for_session_start(user_msg)` → top_k=12, categories=[], include_episodes=true
- `spawn::recall_for_delegation(task_desc, agent_id, ward)` → top_k=8, categories=[correction, pattern, domain, strategy]
- `micro_recall::recall_for_trigger(trigger)` → top_k=3, filtered by trigger type
- Intent analysis recall is deleted outright — intent analysis doesn't need a separate recall call; it operates on whatever the session-start injection surfaced.

**Net:** ~600 lines deleted (three methods + their enrichment helpers + fallback branch + shadow call), ~150 added (unified request type + 3 adapters).

---

## Component 7b — Mode split in runner

**File:** `gateway/gateway-execution/src/runner.rs`

**Current:** single `run()` function with `is_fast_mode()` checks scattered through it. Fast mode skips recall, skips intent analysis, skips planning, collapses to one iteration.

**New:**

```rust
pub enum SessionMode { Research, Chat }

impl Runner {
    pub async fn run(&self, mode: SessionMode, ...) -> Result<...> {
        // --- Shared prelude (runs for BOTH modes) ---
        let recall = self.recall_for_session_start(user_msg).await?;
        if !recall.is_empty() {
            history.push(ChatMessage::system(format_recalled_context(&recall)));
        }
        self.load_skills_for(agent).await?;

        match mode {
            SessionMode::Chat => self.run_chat(history, ...).await,
            SessionMode::Research => self.run_research(history, ...).await,
        }
    }

    async fn run_chat(&self, history: Vec<ChatMessage>, ...) -> Result<...> {
        // Single LLM call with tool loop bounded to max 3 tool iterations
        // No intent analysis, no planning, no delegation, no wards
    }

    async fn run_research(&self, history: Vec<ChatMessage>, ...) -> Result<...> {
        // Full pipeline: intent → planning → execution with wards + delegation
        // Existing research path, minus the recall calls (now in prelude)
    }
}
```

**Renames:**
- `is_fast_mode()` → **deleted**. Replaced with `mode: SessionMode` carried through the call stack.
- `FastModeConfig` → `ChatModeConfig` where the name still makes sense; otherwise drop.

**Mode selection:** `InvokeRequest` already carries a field for this (currently `fast_mode: bool`) — rename to `mode: SessionMode` with serde default = Chat.

---

## Component 7c — Subagent memory enablement

**File:** `gateway/gateway-execution/src/invoke/setup.rs:383-389`

**Current:**
```rust
let memory_shard = if instructions.contains("# RULES") {
    String::new() // Delegated subagent — no memory tool, no shard needed
} else {
    load_shard("memory_learning.md")?
};
```

**New:** unconditional load. Subagents get the full memory shard and the `memory` tool (save_fact, recall, get, set). They receive the delegation-time recall injection AND can drill further or save new facts learned during their task.

Tool registration in `runtime/agent-tools/src/registry.rs` (or wherever subagent tools are wired) must include `memory` and `graph_query` for delegated agents.

**Rationale:** one-way knowledge is the biggest utilization leak. A subagent that finds a correction or pattern should persist it. The sleep-time compactor will deduplicate and merge later.

---

## Component 7d — Prompt reconciliation

**Files:**
- `gateway/templates/shards/memory_learning.md`
- `gateway/templates/shards/chat_protocol.md`
- `gateway/templates/shards/first_turn_protocol.md`

**Policy (single, authoritative):**
> Memory is **automatically injected** at session start and at delegation time. The agent does **not** need to call `memory(action=recall)` at the start of every turn — that recall already happened. Use `memory(action=recall)` for **targeted drilling** when: (a) the injected context didn't cover a specific entity you now care about, (b) you hit an error and want to check for past corrections on that error, (c) you're about to make a decision that feels familiar and want to check past strategies. Use `memory(action=save_fact)` whenever you learn something the future-you would want to know.

**Changes:**
- `chat_protocol.md:16` — delete "Do NOT use memory(action='recall') at the start of every turn." (Replaced by policy above.)
- `memory_learning.md` — replace "before any task / after entering a ward / after delegation" blanket instruction with the targeted-drilling policy. Keep the save-fact guidance as-is.
- `first_turn_protocol.md:16` — delete the "call memory(action=recall) as first action" line. First-turn recall is now automatic.

---

## Component 7e — Failure surfacing

**File:** `gateway/gateway-execution/src/runner.rs` (shared prelude)

When `recall_for_session_start` returns an error (not empty — errored), inject a system message:

```
[Memory retrieval failed: {reason}. You can still call memory(action=recall, query=...) manually if you need past context for this task.]
```

Silent failure is the worst case: the agent assumes no memory exists when in fact retrieval broke.

---

## Component 7f — JSON parser finalization

Phase 6 T1 factored `json_shape::parse_llm_json` but left `distillation.rs` on its own tolerant multi-step parser. Revisit:

- If the tolerant chain is still justified (Vec<ExtractedFact> fallback is load-bearing), leave it alone and add a module-level comment explaining why.
- If the fallback is dead (no production traffic relies on the Vec shape), migrate to `parse_llm_json` and delete `parse_distillation_from_value`.

Decision deferred to implementation time with evidence from git log + grep of the response shapes actually produced by prompts.

---

## File structure

```
gateway/gateway-execution/src/
├── recall/
│   ├── mod.rs               [modified] delete 3 legacy methods, keep recall_unified + RecallRequest
│   └── unified.rs           [modified] accept RecallRequest, return ranked items + episodes
├── runner.rs                [rewritten] SessionMode enum, shared prelude, run_chat / run_research split
├── invoke/
│   ├── setup.rs             [modified] remove conditional memory shard strip
│   └── micro_recall.rs      [modified] call recall_unified via adapter
├── delegation/
│   └── spawn.rs             [modified] delete shadow call + legacy injection; call recall_unified adapter
└── ingest/
    └── distillation.rs      [conditionally modified] see 7f

gateway/templates/shards/
├── memory_learning.md       [rewritten] targeted-drilling policy
├── chat_protocol.md         [modified] delete recall-discouragement line
└── first_turn_protocol.md   [modified] delete automatic-recall instruction

runtime/agent-tools/src/
└── registry.rs              [modified] ensure memory + graph_query registered for subagents

gateway/src/
├── api/invoke.rs            [modified] InvokeRequest.mode: SessionMode (was fast_mode: bool)
└── state.rs                 [modified] thread SessionMode through
```

## Tasks

### T1 — RecallRequest + unified promotion

- Create `RecallRequest` struct in `recall/mod.rs`
- Extend `recall_unified` to accept `RecallRequest`
- Write 3 thin adapters: `recall_for_session_start`, `recall_for_delegation`, `recall_for_trigger`
- Integration test: each adapter produces non-empty results against seeded DB
- **Do not delete** legacy methods yet — that's T2 so the branch stays green while adapters are validated

### T2 — Delete legacy recall paths

- Delete `recall_with_graph`, `recall_for_intent`, `recall_for_delegation_with_graph`
- Delete FTS-only fallback branch in runner.rs
- Delete shadow call + log at `spawn.rs:334-348`
- Update 3 callsites: runner.rs session start, spawn.rs delegation, intent analysis (delete the intent recall call outright — intent works from shared prelude)
- `cargo check --workspace` + `cargo test --workspace` must pass

### T3 — SessionMode split

- Add `SessionMode` enum to `runner.rs`
- Rename `InvokeRequest.fast_mode: bool` → `InvokeRequest.mode: SessionMode` (serde default = Chat)
- Split `run()` into shared prelude + `run_chat()` + `run_research()`
- Delete `is_fast_mode()` and all its call sites
- Integration tests: chat mode injects memory; research mode injects memory + runs intent analysis; neither skips prelude

### T4 — Subagent memory enablement

- Remove conditional in `invoke/setup.rs:383-389`: memory shard loads unconditionally
- Verify `memory` + `graph_query` tools registered for subagents in `runtime/agent-tools/src/registry.rs`
- Integration test: delegated subagent calls `memory(action=save_fact)` and fact appears in DB
- Integration test: delegated subagent calls `memory(action=recall)` with task-specific query and gets results

### T5 — Prompt reconciliation

- Rewrite `memory_learning.md` per policy in §7d
- Delete recall-discouragement line in `chat_protocol.md:16`
- Delete automatic-recall instruction in `first_turn_protocol.md:16`
- Manual review: run a chat session and a research session, confirm no contradictory guidance surfaces in rendered prompts

### T6 — Failure surfacing

- Wrap `recall_for_session_start` call in runner.rs with error branch that injects the failure system message
- Unit test: mock recall service returning error, assert failure message is in history
- Unit test: mock recall service returning empty, assert no message is injected (empty is not failure)

### T7 — Distillation parser decision

- Grep git log + production logs (if any) for what response shapes distillation prompts actually produce
- Either migrate to `parse_llm_json` (and delete `parse_distillation_from_value`) or add a rationale comment and leave it
- Document the decision in the commit message

### T8 — Validation + push

- `cargo fmt --all --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --workspace`
- Full manual smoke: chat mode session, research mode session, delegation in research mode, verify memory injection in all three via logs
- Push branch, open PR

## Success criteria

- Exactly **one** recall function is called by production code (`recall_unified` via its 3 adapters)
- Chat mode sessions show memory injection in logs (currently show nothing)
- Subagents can save_fact during their task and the fact persists
- Zero references to `is_fast_mode`, `recall_with_graph`, `recall_for_intent`, `recall_for_delegation_with_graph` in the workspace
- Prompt shards no longer contradict each other on recall policy
- Net code delta: approximately **-500 lines** (600 deleted, 150 added, accounting for tests)

## Risks

- **Latency in chat mode**: adding recall_unified to the chat prelude adds ~100-300ms per session start. Mitigation: cap top_k at 8 for chat, enforce tight embedding timeout (already in place), accept the tradeoff — this is the whole point.
- **Subagent fact pollution**: enabling save_fact in subagents means more raw facts entering the graph. Mitigation: Phase 4's sleep-time compactor already handles dedup; Phase 6's pairwise verifier handles merges. This is exactly the workload the compaction pipeline was built for.
- **Prompt regression**: rewriting shards can shift agent behavior unpredictably. Mitigation: manual smoke in T8, keep the old shards in git history if rollback needed.

## Out of scope (explicitly)

- Mode selection UI (backend clean first)
- New memory categories
- New sleep-time jobs
- Performance tuning of recall_unified beyond what's needed to keep chat-mode latency reasonable
