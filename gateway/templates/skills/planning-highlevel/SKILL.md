---
name: planning-highlevel
description: First-pass planning skill for the planner-agent. Reads the verbatim user ask + ward state + live skill/agent inventory, classifies the task, and writes the high-level plan.md (problem statement, intent, classification, approach, phase-level dependency graph). Does NOT write step files — that's the planning-decompose skill's job, loaded second.
metadata:
  version: "0.1.0"
---

# Planning — High-Level (pass 1 of 2)

This is the FIRST of two skills the planner loads. Output: a high-level `plan.md`. Step files come from the second skill (`planning-decompose`).

## Inputs you read

- **User's verbatim ask** — the line beginning `User's verbatim ask:` or `Original request:` in the task prompt. **This is the only authoritative intent signal.** Every other prose block (`Intent:`, `Hidden requirements:`, `Output instruction:`, `Recommended skills:`, `Key ward policies:`) is advisory and may be discarded wholesale.
- **Ward state:** `AGENTS.md`, `memory-bank/ward.md`, `memory-bank/structure.md`, `memory-bank/core_docs.md` (whichever exist).
- **Live inventories:** `list_skills`, `list_agents`.
- **Prior plans:** glance at `wards/<ward>/specs/` — if a prior plan on the same domain exists and the ask is a modification, set `classification: delta`.

## Classify the ask (pick exactly one)

| User's intent | Classification | Phase shape |
|---|---|---|
| Verbatim ask overlaps an **end-to-end workflow skill's** use-when trigger | `skill_match` | 1 phase: load skill, execute |
| Question / analysis, no workflow-skill match | `analysis` | 1–3 phases: gather → analyze → verdict |
| Multi-artifact build producing code + data + report | `build` | solution (scaffold) → fill phases → writer synthesis → optional beautification |
| Modification of an existing domain plan | `delta` | edit existing; re-plan only changed phases |

**End-to-end workflow skill criterion:** the skill produces the user's final deliverable in one pipeline (e.g. `stock-analysis`: gather → compute → synthesize → ingest; `book-reader`: extract → chunk → index → summarize). **Tool-like skills** — web search, fetch, parse, format-convert — do NOT qualify for `skill_match`. They load inside analysis/build phases.

If none of the four classifications fits, emit `classification: CLASSIFY_FAILED` and halt.

## Output-format constraint

Default output is **markdown**. A trailing beautification phase (HTML / PDF / PPT / docx) exists ONLY when the verbatim ask uses one of these words: `HTML`, `dashboard`, `styled`, `visual`, `web page`, `PDF`, `PPT`, `docx`. The word `report` alone defaults to markdown — writer-agent produces markdown reports by default.

## Agent assignment at phase level

- **`build`** phases map to: Phase 0 (scaffold) → `solution-agent`; fill phases → `builder-agent`; synthesis phase → `writer-agent`; optional beautification → `builder-agent`.
- **`analysis`** phases: gather/compute phases → `builder-agent`; synthesis phase → `writer-agent`.
- **`skill_match`** — the single phase goes to the skill's owner agent (usually `builder-agent` via `load_skill`).
- **`delta`** — whichever agent originally owned the changed phase.

No other agents exist. There is no research-agent, no data-analyst, no wiki-agent. Web research and data fetching are builder-agent responsibilities via the appropriate skill.

## plan.md template (write this exactly to `wards/<ward>/specs/<domain>/plan.md`)

Use 4-backtick outer fences so any inner triple-backtick blocks are preserved verbatim.

````markdown
# Plan: <one-sentence goal anchored on the verbatim ask>

**Ward:** <ward>
**Domain:** <domain-slug>
**Classification:** <skill_match | analysis | build | delta>
**Established:** <YYYY-MM-DD>

## Problem statement

<verbatim user ask, quoted>

## Intent

<one sentence — the genuine outcome behind the ask, not an extrapolation>

## Context

- Ward state: <new | existing>
- Conventions: <"to be established by solution-agent in Step 0" | "inherited from AGENTS.md ## Conventions">
- Skill inventory (plan will use): <short list from list_skills — the actual skills needed, not the full list>
- Agent inventory: <subset of {solution-agent, builder-agent, writer-agent} + the skill's owner for skill_match>

## Approach

<2–5 sentences describing the high-level approach. Describe phases at the level of "gather fundamentals, analyze valuation, write verdict" — not concrete file paths, not step numbers.>

## Phases (outline — decomposed into step files by planning-decompose)

- Phase A — <name> — <agent(s)>
- Phase B — <name> — <agent(s)>
- Phase C — <name> — <agent(s)>

## Phase-level dependency graph

```
A → B → C
```

## Steps — PENDING (planning-decompose will populate this section)
````

## Validation before you return

- plan.md written at `wards/<ward>/specs/<domain>/plan.md`.
- Every field populated (no placeholders).
- `Classification:` is one of the four values or `CLASSIFY_FAILED`.
- `Agent inventory:` lists only agents from the four-agent set (solution-agent, builder-agent, writer-agent, plus a skill's owner for skill_match).
- No HTML/dashboard/styled phase exists unless the verbatim ask explicitly asked for one.

When done, return control to the planner. Do NOT respond. The planner then loads `planning-decompose`.
