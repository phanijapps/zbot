# How AgentZero Remembers Things

## The big idea

A chat transcript is not memory. It's a transcript — it captures what happened, not what was learned. AgentZero's memory system works differently: every correction the user gives, every principle the agent infers, every session it completes gets written down as a small **fragment** in a growing notebook. Over time the system gets smarter at writing the right fragments, connecting related ones, retiring contradicting ones, and surfacing the most relevant ones at the start of a new session.

Think of it as the difference between a daily log and a reference book. The log captures everything in sequence; the reference book captures what actually matters, distilled. Both exist, but when the agent starts a new session, it opens the reference book — not the log.

---

## What's already built

### Phase 1 — The foundation: session handoff and better recall

**The problem:** Every new session started cold. The agent had no memory of what was said last week, no record of corrections the user had given, no sense of what the user was actively working on. Telling it something once meant telling it again next time.

**What it does now:** When a session ends, the system runs an LLM over the conversation and writes a compact summary — a "handoff" fragment — to the memory store. When the next session opens, that fragment is read back and injected as a `## Last Session` block at the top of the agent's context. Alongside it, any active corrections (facts tagged `correction`) are always injected directly, and active goals are pulled in too. A second recall pass then searches for anything in the notebook that's relevant to what the handoff summary mentions — giving the agent a small targeted "context from last session" bundle before it says a word.

Recall itself was also fixed here. Previously, every fact came back with a fake relevance score of 0.5 because the underlying search query's real score was being ignored. Now real similarity scores are used, and any result below 0.3 is dropped as noise.

**How it helps:** Corrections stick across sessions. The agent knows what you were working on. "Didn't I tell you this last week?" becomes rarer.

*Sleep component: `HandoffWriter` (runs at session end). Interval: instantaneous — no configuration needed.*

---

### Phase 2 — Pattern abstraction: "aha, that's actually a principle"

**The problem:** Imagine giving five corrections over a month, all variations of the same idea: use sentence case in commit titles, don't capitalize after a dash, keep the subject line under 72 characters. Each becomes a separate fragment. Five separate fragments all compete for the same recall slot. The notebook gets noisier, not smarter, as the user gives more feedback.

**What it does now:** During each sleep cycle (by default, once a day), a component called the `CorrectionsAbstractor` wakes up. It fetches all the `correction` fragments for an agent. If there are three or more, it asks an LLM whether they share a common theme. If the LLM finds one with sufficient confidence, it writes a single `schema` fragment capturing the distilled principle — something like `schema.corrections.a3f8b2c0 → "Always use sentence case in commit titles and keep subject lines under 72 characters"`. Schema fragments rank higher than raw corrections in recall (weight 1.6 vs 1.5), so the agent sees the distilled rule, not the noisy pile that produced it.

**How it helps:** The notebook gets smarter the more feedback you give. Ten corrections become one authoritative principle. The agent doesn't need to see all ten to behave correctly.

*Sleep component: `CorrectionsAbstractor`. Configurable via `settings.json` → `execution.memory.correctionsAbstractorIntervalHours` (default: 24).*

---

### Phase 3 — Conflict resolution: "wait, two of these disagree"

**The problem:** The notebook grows over months. At some point it contains two schema fragments that flat-out contradict each other. "Always use rebase to merge feature branches" and "Never rebase — always use merge commits." Both get surfaced at session start. The agent gets confused and the user gets inconsistent behavior.

**What it does now:** During each sleep cycle, a component called the `ConflictResolver` scans all schema fragments for an agent. For each pair that are semantically similar (cosine similarity ≥ 0.85 — meaning they're likely about the same topic), it asks an LLM whether they actually contradict. If the LLM says yes with sufficient confidence, the system picks a winner: the fragment with higher confidence score wins; if they're tied, the more recently updated one wins. The loser is marked with a `superseded_by` pointer to the winner and disappears from recall. It isn't deleted — just retired. The audit trail records why.

**How it helps:** The notebook self-cleans. Stale or wrong principles get retired automatically rather than accumulating as invisible noise. The agent behaves consistently even as its knowledge base has evolved.

*Sleep component: `ConflictResolver`. Configurable via `settings.json` → `execution.memory.conflictResolverIntervalHours` (default: 24).*

---

## What's still remaining

### Phase 4 — Belief network (not started)

Today, confidence and evidence live on individual fragments. A fragment knows its own confidence score, but the broader knowledge graph — the agent's model of *people*, *projects*, and *concepts* — doesn't know how sure it is about any of those entities.

The belief network phase moves confidence up to the knowledge graph itself. A person node, a project node, a relationship — all would carry an uncertainty score that decays when contradictions arrive and strengthens when consistent evidence accumulates. The agent could then reason not just about "what do I know" but "how sure am I about what I know."

This is a structural change, not a new feature bolted on. It requires a schema migration on the knowledge-graph tables and rewrites to how the graph is read and updated. It's the right next step after the fragment-level cleanup is stable.

---

## Coming next: the memory-crate extraction

The memory subsystem currently lives spread across several files inside the gateway. There's a tracking document at `memory-bank/future-state/2026-05-13-memory-crate-extraction-tracking.md` that catalogs every component, every setting, and every wiring point added across Phases 1–3. Its purpose is to give a future extraction into a standalone `zero-memory` crate a clean starting inventory — so that refactor can be mechanical rather than exploratory.

No code has moved yet. The tracking doc is just a map.

---

## One-page recap

**What's working:**
- Session handoff: the agent knows what you were working on last time
- Always-inject corrections: past feedback is surfaced every session, not just on lucky recall hits
- Min-score filtering: recall no longer returns everything at the same fake relevance
- Pattern abstraction: repeated corrections are distilled into schema principles overnight
- Conflict resolution: contradicting schema principles are detected and retired automatically

**What's next:**
- Belief network (Phase 4): confidence on knowledge-graph nodes, not just individual fragments
- `zero-memory` crate extraction: isolating the subsystem for easier testing and reuse

**How each phase makes the agent better:**
- Phase 1: the agent no longer starts every session cold — it carries forward what you told it
- Phase 2: the more feedback you give, the smarter the notebook gets, not just bigger
- Phase 3: the notebook stays consistent over time without manual curation
- Phase 4 (future): the agent will know *how confident* to be, not just *what* it believes
