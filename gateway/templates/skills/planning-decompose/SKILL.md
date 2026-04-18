---
name: planning-decompose
description: Second-pass planning skill for the planner-agent. Reads the high-level plan.md written by planning-highlevel, decomposes its phases into fully-elicited per-step files at steps/stepN.md, and updates plan.md's Steps section with the final outline. Loaded by the planner AFTER planning-highlevel.
metadata:
  version: "0.1.0"
---

# Planning — Decomposition (pass 2 of 2)

This is the SECOND of two skills the planner loads. Input: the `plan.md` written by `planning-highlevel`. Output: one fully-elicited step file per step + an updated `plan.md` with the Steps section populated.

## Inputs you read

- `wards/<ward>/specs/<domain>/plan.md` — just written by the first pass. Read: Classification, Intent, Approach, Phases, Phase-level dependency graph.
- Ward state (same files the first pass read): `AGENTS.md`, `memory-bank/*.md`.

## Step-file naming (non-negotiable)

- **`skill_match`** → `steps/step1.md` only. No `step0.md`.
- **`analysis`** → `steps/step1.md`, `step2.md`, `step3.md`. No `step0.md`.
- **`build`** → `steps/step0.md` is always scaffold-only (owned by `solution-agent`), followed by `step1.md`+ (owned by `builder-agent` fill phases), `stepN.md` (owned by `writer-agent` synthesis), optional `stepN+1.md` (owned by `builder-agent` format-convert if the ask asked for HTML/PDF/PPT/docx).
- **`delta`** → edit existing step files in place; create new ones only when the phase is new.

The presence of `step0.md` means "solution-agent scaffolds first." Only `build` plans have one.

## Agent assignments — hard rules

| Step role | Agent | What they do |
|---|---|---|
| Step 0 (build only) | `solution-agent` | Architecture decisions, ward setup, AGENTS.md (Conventions + DOs/DONTs + how-to-use), memory-bank/*.md, directory tree, **shell files** (interface stubs, no implementation bodies). |
| Fill (build, analysis) | `builder-agent` | Load skills, implement primitives, fetch data, run scripts, produce output files. Emits `reuse_audit:` block on every coding step. |
| Synthesis (build, analysis) | `writer-agent` | Read prior output files (explicit paths from steps' `Output:` fields), produce a coherent markdown report with citations. Never runs data skills. |
| Beautification (optional, only if verbatim ask asks) | `builder-agent` | Loads a format-convert skill, reads writer's markdown, produces HTML/PDF/PPT/docx. |
| skill_match single step | Skill's owner agent | Usually `builder-agent` via `load_skill(skill='<name>')`. Skill produces the final artifact end-to-end. |

No other agents exist. There is no research-agent, no data-analyst, no wiki-agent.

## Step-file template (write each step file exactly like this)

Every step file MUST have these fields. Mark NA when genuinely inapplicable.

````markdown
# Step <N> — <title>

**Agent:** <solution-agent | builder-agent | writer-agent | <skill owner>>
**Skills:** <comma-separated from list_skills, or NA>
**Ward:** <ward>
**Domain:** <domain>

## Goal

<one paragraph: what this step achieves in the context of the plan's approach>

## Reuse audit

For `builder-agent` coding steps this MUST be a filled yaml block:

```yaml
reuse_audit:
  looking_for: [symbols this step will call]
  found:       [subset already in memory-bank/core_docs.md — will import via Conventions.import_syntax]
  missing:     [subset not yet registered — will implement and register]
  plan: <one-sentence import/implement sequence>
```

For `solution-agent` (Step 0), `writer-agent`, or `skill_match` steps: NA.

## Input

- <explicit file path> — <what it provides>
- ("none" if no file inputs)

## Output

- <explicit file path> — <format + schema + required fields>

## Implementation

Numbered concrete actions. Name the tool, the args, the target file. No prose substitute for a specific call.

For `solution-agent` Step 0: numbered directory-creation, shell-file-writing, doc-writing actions. No code execution.
For `builder-agent` fill: load the listed skills, call primitives via import_syntax, save outputs.
For `writer-agent`: read listed input files, synthesize, write markdown.
For `builder-agent` beautification: load the format-convert skill, read writer's markdown, emit the styled artifact.

## Acceptance (BDD)

```gherkin
Given <preconditions>
When <step runs>
Then <verifiable observations>
 And <more observations>
```

## Validation

Copy-pasteable shell commands that exit 0 iff the step succeeded.

- `solution-agent`: `ls` + `test -f` checks that every required shell file and doc exists.
- `builder-agent` coding: import check + output-file structural check.
- `writer-agent`: grep for required sections and citation count.
- `skill_match`: `ls` of the skill's output directory.

## Depends on

- Step <N>: <what this dependency provides>
- ("none" for independent steps)

## On failure

Optional. Non-obvious recovery/halt guidance only when the default "halt and report" isn't right.
````

## Reuse-first rules for builder-agent coding steps

Every fill step MUST carry a filled `reuse_audit:` block (yaml, not prose). Planner enforces this at decompose time.

- **Import, don't re-derive.** Symbols in `found` imported via Conventions' `import_syntax`.
- **Fix in place, don't fork.** If a primitive lacks a feature, builder edits it in `<module_root>/` (parameterize, extend). No near-copies.
- **Register, don't leak.** New primitives land in `<module_root>/` and get appended to `memory-bank/core_docs.md`. Never inside the task directory.
- **Task scripts are thin wrappers.** Files under `<domain>/code/` hardcode inputs, call primitives, save outputs. Zero reusable logic.

## Update plan.md's Steps section

After writing all step files, edit `plan.md` — replace `## Steps — PENDING (...)` with:

````markdown
## Steps

1. [Step 0](steps/step0.md) — <title> — solution-agent — <1-line goal>
2. [Step 1](steps/step1.md) — <title> — builder-agent — <1-line goal>
3. [Step 2](steps/step2.md) — <title> — builder-agent — <1-line goal>
...

## Step-level dependency graph

```
0
|
1, 2, 3  (parallel)
 \ | /
   4
   |
   5  (optional beautification)
```
````

## Validation before you return

- Every phase in plan.md's Phases section has at least one step file.
- Every step file exists at `wards/<ward>/specs/<domain>/steps/step<N>.md`.
- Every step file has all required fields filled (no placeholder text).
- Every `builder-agent` coding step's `Reuse audit` is a filled yaml block (not NA, not prose).
- Every `Agent:` field names only `solution-agent`, `builder-agent`, `writer-agent`, or a skill owner.
- `plan.md`'s Steps section is populated (not `PENDING`).
- No step places `<module_root>/` or `memory-bank/` inside a task directory.
- No HTML/PDF/PPT/docx beautification step exists unless the user's verbatim ask asked for one.

When done, return control to the planner. The planner responds with `Plan: wards/<ward>/specs/<domain>/plan.md (<N> steps)`.
