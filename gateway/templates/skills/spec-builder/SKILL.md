---
name: spec-builder
description: First-pass planning skill for a Manus-like planner agent. Given a verbatim user ask plus advisory intent, context, state, and orchestrator-suggested skills, produces a high-level spec — problem statement, classification (skill_match | research | build | delta), skill-fit and tool-fit cascade, assumptions, success criteria, approach, phase graph, and pre-mortem. Does NOT decompose into steps — plan-composer does that second. Use when the planner agent receives a new task. Covers code, research, writing, browser flows, analysis, data work, and any general-purpose task.
metadata:
  version: "1.0.0"
---

# spec-builder — first-pass planning

This is the first of two skills the planner agent loads for every new task. Output: a single markdown spec. The second skill (`plan-composer`) decomposes it into step files.

## What this skill returns

Return exactly one text blob to the planner agent using this delimiter format:

```
=== FILE: spec.md ===
<full spec markdown>
```

The planner agent splits on `=== FILE: <name> ===` and writes the content to the appropriate path. This skill does **not** write files and does **not** respond to the user.

## Inputs you read

The planner passes these into context before invoking this skill:

- **`prompt`** — the user's verbatim ask. **The only authoritative intent signal.** Every other input is advisory.
- **`intent`** — one-sentence distilled goal from the orchestrator.
- **`context`** — facts about the session/project that may constrain the approach.
- **`state`** — current execution state. May include prior specs if this is a follow-up.
- **`suggested_skills`** — orchestrator's recommendations for which skills to use in steps.

Not passed as input — already available in the planner's system prompt:

- **Full skill catalog** — you can reference any skill by name.
- **Raw tool inventory** — tools available to the planner and sub-agents.

## Verbatim-prompt primacy

On any conflict between `prompt` and the other inputs, trust `prompt`. Note the conflict under `## Assumptions` in the output spec. Do not silently reconcile.

## Similarity check — delta gate (run first)

Before classifying, check whether this ask is a continuation of a prior spec:

1. Look in `state` for prior specs.
2. For each prior spec, compare its `## Problem statement` to the current `prompt`.
3. If they describe the same goal, same domain, and the same artifact shape — qualitative judgment ≥ 70% — classify as `delta` and treat that prior spec as the baseline to update.
4. If similarity is in the 60–80% gray zone, still prefer `delta`. It is cheaper to update than to replan, and the delta path re-evaluates every section anyway.

When classification is `delta`:

- Load the baseline spec's sections.
- Replace **Problem statement** with the new verbatim `prompt`.
- Re-evaluate **Intent, Context summary, Assumptions, Success criteria, Approach, Phases, Phase dependency graph, Pre-mortem**. Keep unchanged sections verbatim; rewrite only what the new ask shifts.
- Keep prior phase IDs stable. Only add or remove phases that actually change.
- Leave `## Steps — PENDING`. `plan-composer` re-decomposes only the changed phases.
- Name the baseline spec (path or id) under `## Context summary`.

## Classify the ask

Pick exactly one classification (and emit no others):

| Classification | Trigger |
|---|---|
| `skill_match` | One skill (from `suggested_skills` or the planner's catalog) covers the whole ask end-to-end. |
| `research` | Gather → analyze → synthesize. Includes Q&A, investigation, comparative analysis. |
| `build` | Produce a compound artifact — code, document, data, media, report. |
| `delta` | Similarity ≥ 70% to a prior spec in `state` (set by the delta gate above). |

**Failure modes (not normal classifications):**

- If none of the four fits: emit `Classification: CLASSIFY_FAILED` with a one-paragraph reason and halt. Do not guess.
- If the ask genuinely spans multiple independent projects: emit `Classification: decomposition` using the template below. The orchestrator will re-invoke the planner on each sub-project.

### Decomposition template

Use when the ask spans multiple independent projects:

````
=== FILE: spec.md ===
# Decomposition: <one-sentence observation of why this ask needs multiple specs>

**Classification:** decomposition

## Problem statement
> <verbatim prompt>

## Recommendation
This ask spans <N> independent projects that each warrant their own spec. Run spec-builder separately for each sub-project below.

## Sub-projects
1. **<slug>** — <one-sentence goal> — suggested order: <first | parallel with N | after N>
2. **<slug>** — ...
````

No phases, no steps, no pre-mortem for a decomposition spec.

## Skill-fit / tool-fit cascade

For every capability the plan needs, walk this cascade in order:

1. Does a skill in `suggested_skills` cover it? → list it under `## Suggested skills (from orchestrator)`.
2. If not, does a skill in the planner's full catalog cover it? → list it under `## Additional skills required`.
3. If not, does a raw tool in the planner's system prompt cover it? → list it under `## Raw tools required`.
4. If nothing covers it → list it under `## Capability gaps` with a fallback: `degrade`, `skip`, or `ask user`.

**Do not pretend gaps away.** A spec with acknowledged gaps is more useful than one that hallucinates coverage.

If any capability gap makes the plan non-executable, also emit `Classification: blocked` as an override. The orchestrator sees that the spec exists but should not be dispatched.

## Output-format constraint

Default output format is markdown. A beautification phase (HTML, PDF, PPT, docx, slides) exists **only** if the verbatim `prompt` explicitly uses one of those words. The word "report" alone defaults to markdown.

## Spec template

For `skill_match`, `research`, `build`, and `delta` classifications, produce exactly this structure inside the `=== FILE: spec.md ===` section:

````markdown
# Spec: <one-sentence goal anchored on the verbatim ask>

**Established:** <YYYY-MM-DD>
**Classification:** <skill_match | research | build | delta | CLASSIFY_FAILED | blocked | decomposition>

## Inputs snapshot
- prompt: provided
- intent: <provided | missing>
- context: <provided | missing>
- state: <provided | missing>
- suggested_skills: <provided | missing>

## Problem statement
> <verbatim prompt, quoted>

## Intent
<one sentence — the genuine outcome behind the ask, not an extrapolation>

## Context summary
- <3–7 bullets of load-bearing facts from the context input>
- ("none — no context provided" if missing)

## Suggested skills (from orchestrator)
- <skill_name>: <where it fits in the approach>

## Additional skills required
- <skill_name from planner catalog>: <why needed beyond suggested set>
- ("none" if suggested set suffices)

## Raw tools required
- <tool_name>: <what step needs it>
- ("none — prefer skills" if fully skill-covered)

## Capability gaps
- <need> — fallback: <degrade | skip | ask user>
- ("none" if fully covered)

## Assumptions
- <implicit assumption made explicit>
- <conflict with intent/context, resolved in favor of prompt, if any>

## Success criteria
- <observable, verifiable outcome>
- ...

## Approach
<2–5 sentences. Phase-level only. No tool call names, no file paths.>

## Phases
- Phase WS — Ward Setup — **documentation-only phase**. Builder-agent loads `ward-designer` skill and emits ONLY ward-doctrine files: `AGENTS.md`, `memory-bank/ward.md`, `memory-bank/structure.md`, `memory-bank/core_docs.md`. Phase WS MUST NOT produce source code (no `.py`, `.js`, `.go`, `.ts`, `.rs`, etc.) — primitive implementation belongs in later phases. If the ward already has a populated `AGENTS.md` + `memory-bank/ward.md` (REVIEW mode), Phase WS emits `memory-bank/review.md` plus optional small updates to the doctrine files.
- Phase A — <name> — <one-line purpose>
- Phase B — <name> — ...

## Phase dependency graph

```
WS ──▶ A ──▶ B ──▶ C
       ╰──▶ D (parallel with C)
```

## Pre-mortem
Top failure modes this plan must guard against:
1. <mode> — mitigation: <how plan handles it>
2. <mode> — mitigation: <...>

## Steps — PENDING (plan-composer fills this)
````

## Self-critique gate

Before returning, check the spec against this list. Revise and re-check until every item passes:

- Every success criterion maps to at least one phase.
- Every phase contributes to at least one success criterion.
- No capability gap silently dropped — every gap has a fallback.
- Pre-mortem references failure modes the approach actually handles, not generic platitudes.
- If `Classification: blocked`, `Capability gaps` is non-empty.
- If `Classification: delta`, the baseline spec is named under `## Context summary`.
- `## Inputs snapshot` accurately reflects which inputs were provided.
- The verbatim prompt is quoted exactly as given — do not paraphrase.

## Graceful degradation

If `intent`, `context`, `state`, or `suggested_skills` are missing or empty, proceed using `prompt` alone. Do not halt. Note which inputs were missing under `## Inputs snapshot`.

## Return control

Emit the spec inside one `=== FILE: spec.md ===` block and return. Do not write files. Do not respond to the user. The planner agent will read your output, serialize it, and then invoke `plan-composer` if the classification is execution-worthy (`skill_match`, `research`, `build`, `delta`).

## References

- `references/spec-examples.md` contains one worked example per classification. Read the relevant section if you are unsure of the template shape for a given classification — for example, *"If you are composing a `research` spec and unsure what a clean example looks like, read `references/spec-examples.md#research` before drafting."*
