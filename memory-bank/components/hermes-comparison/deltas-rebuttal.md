# Hermes Deltas — Honest Rebuttal

Counter-assessment of [`deltas.md`](./deltas.md) (OpenCode / GLM-5.1 critique).
Written after the procedure-as-callable branch (`feat/run-procedure-tool`,
19 commits) landed.

Current code-backed follow-up:
[`impact-analysis-2026-05-30.md`](./impact-analysis-2026-05-30.md).

The original report has real findings, but its framing is broken in ways
that matter. This doc reorders the priorities and corrects the misframings.

---

## The big one — #1 Self-Improving Skill Loop is wrong

The report says: *"Skills can be loaded, listed, and created via API, but
the agent has no tool to autonomously create skills mid-execution. There
is no curator. There is no skill usage telemetry. There is no skill
refinement cycle."*

**This was written before the run_procedure work landed.** What we have
in `procedures` is structurally stronger than Hermes's skills:

| Hermes skill | z-Bot procedure |
|---|---|
| Markdown file → injected as prompt context | Structured `Vec<PatternStep>` → dispatched by `RunProcedureTool` |
| Author by LLM writing markdown | Auto-mined by `PatternExtractor` from successful sessions |
| Curator archives based on heuristics | Decay engine + success/failure counters + dedup floor |
| No telemetry beyond load count | `success_count`, `failure_count`, `avg_duration_ms`, `last_used`, ward scoping, bi-temporal validity, embeddings, 3-tier middleware surfacing |
| Refinement = LLM rewrites the markdown | Refinement = sleep-cycle re-mining + counter-driven promotion + ConflictResolver |

The thing we *don't* have is a **`create_skill` tool that the agent calls
mid-execution**. That's a different paradigm (authoring) than the one we
picked (mining + dispatch). The report treats the absence of the
authoring tool as equivalent to "no self-improving loop" — that's the
misframing.

If you wanted to *add* autonomous authoring, it's a ~2-day commit.
But it's a tactical addition to an existing loop, not the missing
flagship feature the report claims.

**Verdict on #1: overstated. Real delta is much smaller — and the
architectural direction is arguably better.**

---

## #2 — 22 platform adapters is inflated 20×

The report counts each platform as a separate capability gap. They're
all built on one ABC in Hermes. The real gap is **one reference adapter
+ WebSocket transport** — maybe ~5 days of work to demonstrate the
pattern. After that, each new platform is template-replicated
engineering, not a new capability.

**Verdict on #2: real gap, but the effort estimate (4-6 weeks) inflates
by counting work that's mostly repetition.**

---

## #5 — Session search misses what's already there

*"Memory search covers distilled knowledge, not the original
conversations."*

True, but the distilled knowledge surface (facts + entities + procedures
+ beliefs) is arguably **better** than raw FTS5 on transcripts for "what
did we discuss last month" queries. FTS5 returns keyword matches; the
recall system returns semantically-relevant *summarized* knowledge.
Different signal, different value.

A raw `search_sessions` tool would be useful — but framing it as a delta
is misleading. The need is "search across past *content*" which the
recall system already does.

**Verdict on #5: real gap on raw transcript search; misleadingly named
as a recall gap.**

---

## #12 — Orchestrator + MoA conflates two things

The report says: *"orchestrator exists as framework code but is not
wired into the gateway."*

You have a fully-wired planner→builder→specialist orchestration. It runs
in every session of meaningful complexity. `intent_analysis` classifies,
`analyze_intent` returns `IntentAnalysis`, `format_intent_injection`
builds the root prompt, `delegate_to_agent` spawns sub-agents. **That IS
the orchestrator wiring.**

What's *actually* missing is **Mixture-of-Agents specifically** —
parallel LLM calls to multiple providers, aggregator synthesis. That's
a distinct feature, not the same thing as orchestration.

**Verdict on #12: orchestration claim is wrong; MoA-specifically is a
real gap.**

---

## What the report gets right

These are real, accurate gaps. None of them controversial:

- **#3 Browser automation** — true gap, ~2 weeks of work
- **#4 TTS / STT / image gen** — true gap (we have image-in, not image-out)
- **#6 Credential pooling** — true gap, already in backlog (FallbackLlmClient)
- **#7 Computer use** — true gap, niche
- **#8 IDE integration** — true gap, niche
- **#11 i18n** — true gap, low priority

---

## What the report gets wrong by omission — the actual P0

The report puts #9 (one-click install) and #10 (docs) at P2. **That's
backwards.** Here's the actual situation:

- z-Bot has more memory engineering than Hermes (beliefs, bi-temporal,
  hierarchy, MMR, query gate, 3-tier procedure recommendations)
- z-Bot has stronger procedure semantics than Hermes (the 19-commit
  branch we just landed)
- z-Bot has the daemon architecture, the planner→builder loop, the
  run_procedure dispatcher, the procedures dedupe, the graduated tier
  system

**And approximately zero new users can install it without a Rust
toolchain.**

The actual P0 problem isn't "does z-Bot have feature X" — it's "**can a
user discover, install, and reach first-success in under 30 minutes?**"
Hermes wins on that single dimension and it dwarfs every other
comparison. The report buries this at #9/#10 because it organized by
capability, not by adoption funnel.

If you fixed only #9 (installer) + #10 (getting-started doc), you'd
close more user-facing gap with Hermes than fixing #1-8 combined would.
Engineering quality without distribution is a research project, not a
product.

---

## Adjusted priority matrix

| Actual priority | Report's # | What it really is |
|---|---|---|
| P0 | #9 + #10 | Distribution: installer + docs. The only thing actually blocking adoption. |
| P1 | #2 (slimmed) | One reference connector adapter + WebSocket transport (~1 week, not 4-6) |
| P1 | #4 | Multimodal-out (TTS/STT/image gen) — voice is the growth path |
| P1 | #6 | Credential pooling — already in backlog as FallbackLlmClient |
| P2 | #3 | Browser automation — real but niche |
| P2 | #5 | Raw transcript search — useful, 2 days |
| P3 | #1 | `create_skill` tool — tactical add on top of existing stronger loop |
| P3 | #12 | MoA parallel ensemble — distinct from orchestration which already works |
| P3 | #7 + #8 + #11 | Computer use / IDE / i18n — niche or post-adoption |

---

## The actual question

The report frames everything as "what does z-Bot need to catch up to
Hermes." That's the wrong question. **The right question is: what does
z-Bot need to convert engineering quality into user value?**

Answer: a 5-minute install + a 10-minute first-success path. Everything
else is post-MVP.
