---
name: ward-designer
description: Run by builder-agent at the ward-setup step of a `build` plan. Two phases. Phase 1 scaffolds the ward only when missing (if AGENTS.md + memory-bank already exist, they are left untouched). Phase 2 is the load-bearing work — walks every later step file and writes a Paths table that assigns the exact ward-relative path for each output the step will produce. Subsequent subagents cannot improvise placements; the Paths table binds. Reusability is a path decision made here at planning time, not improvised at execution time.
metadata:
  version: "3.0.0"
---

# ward-designer

**Reusability fails when subagents choose paths at execution time.** A subagent writing Python naturally drops the file next to the script it's working on — which is `<sub-domain>/code/`, not `<module_root>/`. The AGENTS.md text saying "reusable primitives live in `core/`" gets read and ignored. The only way to bind placement is to **pre-assign the path in the step file**. That is this skill's core mission.

Runs in two phases inside a single invocation. Both mandatory.

## Phase 1 — scaffold (cheap, idempotent, preserves existing doctrine)

Read `wards/<ward>/specs/<sub_domain>/spec.md`. Derive `module_root` from the spec's language:

| language | module_root |
|---|---|
| python | `core/` |
| nodejs | `src/lib/` |
| go | `pkg/` |
| r | `R/` |
| perl | `lib/` |
| rust | `src/` |

Check for pre-existing ward doctrine:

- `wards/<ward>/AGENTS.md`
- `wards/<ward>/memory-bank/ward.md`
- `wards/<ward>/memory-bank/structure.md`
- `wards/<ward>/memory-bank/core_docs.md`

**If the file exists and has non-trivial content, REUSE it. Do not rewrite. Do not augment. The executing-ward's own AGENTS.md is authoritative.** Only emit a replacement if the file is missing or empty.

When emitting replacements, use the minimal seeds below. They are deliberately terse — a ward grows its doctrine through use, not through template bloat.

### Seed — `AGENTS.md`

````markdown
# <Ward Name Title-Case>

A `<language>` ward for `<one-sentence purpose inferred from spec intent>`.

## Conventions
- Reusable primitives live under `<module_root>/`. Never under `<sub-domain>/`.
- Sub-domain artifacts (scripts, computed data, reports) live under `<sub-domain>/{code,data,reports}/`.
- Every reusable primitive is registered in `memory-bank/core_docs.md` when created.
- Import syntax: `<example, e.g. from core.<module> import <symbol>>`.
- One public function per primitive file. Take tickers / keys / domain values as arguments — never hardcode.
- No `_v2` files. Fix the original.

## Sub-domain staging
- `specs/<sub-domain>/` — spec + step files.
- `<sub-domain>/` — outputs: `code/`, `data/`, `reports/`, plus `summary.md` + `manifest.json`.
````

### Seed — `memory-bank/ward.md`

````markdown
# Ward: <name>

<one-paragraph purpose>

## Sub-domains
| Slug | Description | Status |
|---|---|---|
| `<current>` | <current deliverable> | in progress |
| `<plausible-future-1>` | <future ask> | proposed |
| `<plausible-future-2>` | <future ask> | proposed |

## Key concepts
- <term> — <one-line definition>
````

### Seed — `memory-bank/structure.md`

````markdown
# Structure — <ward name>

```
<ward>/
├── AGENTS.md
├── memory-bank/{ward.md, structure.md, core_docs.md}
├── <module_root>/              # reusable code primitives
├── templates/                  # reusable templates
├── snippets/                   # reusable code snippets
├── shared-docs/                # reusable markdown fragments
├── data/                       # shared / reference data
├── <sub-domain>/
│   ├── code/                   # sub-domain scripts (thin wrappers)
│   ├── data/                   # sub-domain computed data
│   ├── reports/                # sub-domain deliverables
│   ├── summary.md
│   └── manifest.json
└── specs/<sub-domain>/
```

Status markers: `(exists)` / `(planned)` / `(proposed)`.
````

### Seed — `memory-bank/core_docs.md`

````markdown
# Core docs — <ward name>

Reusable-asset registry. Append a row whenever a new reusable primitive is created.

| Symbol | Module | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |

Templates / snippets / shared-docs get their own tables when first added.
````

### Directory bootstrap

Emit a shell block the orchestrator executes to create empty directories and artifact stubs:

````
=== SHELL: directory-bootstrap ===
mkdir -p wards/<ward>/<module_root> wards/<ward>/templates wards/<ward>/snippets wards/<ward>/shared-docs wards/<ward>/data
mkdir -p wards/<ward>/<sub_domain>/{code,data,reports}
test -f wards/<ward>/<sub_domain>/summary.md || echo "# <sub_domain>" > wards/<ward>/<sub_domain>/summary.md
test -f wards/<ward>/<sub_domain>/manifest.json || echo '{"sub_domain":"<sub_domain>","produced_at":"<YYYY-MM-DD>","files":[]}' > wards/<ward>/<sub_domain>/manifest.json
````

## Phase 2 — step path assignment (the load-bearing work)

List `specs/<sub_domain>/step_*.md`. Identify the current ward-setup step (the one whose `## Suggested skill` is `ward-designer`). Skip it.

For every other step, REWRITE the step file to include a `## Paths` table. Preserve everything plan-composer already wrote (Goal, Inputs, Outputs, Acceptance, Depends on, etc.) — the Paths table is an ADDITION, not a replacement.

Emit each rewritten step as `=== FILE: specs/<sub_domain>/step_<N>.md ===`.

### The Paths table — the only section that binds execution

````markdown
## Paths (assigned by ward-designer — do not deviate)

| Artifact | Bucket | Exact ward-relative path | Register in core_docs.md? |
|---|---|---|---|
| `<filename>` | reusable | `<module_root>/<subpath>` | yes |
| `<filename>` | reusable | `templates/<name>.<ext>` | yes |
| `<filename>` | domain | `<sub_domain>/code/<subpath>` | no |
| `<filename>` | domain | `<sub_domain>/data/<subpath>` | no |
| `<filename>` | domain | `<sub_domain>/reports/<subpath>` | no |

The executing subagent MUST write each artifact at the listed path. No alternative locations, no "I'll just put it here instead." Every row marked "Register: yes" adds one line to `memory-bank/core_docs.md` during step execution.
````

### Placement rules — the decision ward-designer makes

Walk each output in the step's `Outputs` section. For each:

- Function / class / constant / module potentially usable by another sub-domain → **reusable**, at `<module_root>/`.
- Template / schema / reusable snippet / shared markdown fragment → **reusable**, at `templates/` | `snippets/` | `shared-docs/`.
- Shared reference dataset (used across sub-domains) → **reusable**, at `data/` at ward root.
- Script that hardcodes this sub-domain's inputs → **domain**, at `<sub_domain>/code/`.
- Computed data for this sub-domain only → **domain**, at `<sub_domain>/data/`.
- Report / plot / table for this sub-domain only → **domain**, at `<sub_domain>/reports/`.

**Default when ambiguous: reusable.** A ward that hoards assets per sub-domain is not reusable by definition.

## What to return

Emit all blocks in one response. The orchestrator prepends `wards/<ward>/` to all FILE paths and executes the SHELL block.

````
=== FILE: AGENTS.md ===              (only if currently missing/empty)
=== FILE: memory-bank/ward.md ===    (only if missing/empty)
=== FILE: memory-bank/structure.md ===   (only if missing/empty)
=== FILE: memory-bank/core_docs.md ===   (only if missing/empty)
=== SHELL: directory-bootstrap ===
=== FILE: specs/<sub_domain>/step_2.md ===   (Paths table added)
=== FILE: specs/<sub_domain>/step_3.md ===   (Paths table added)
=== FILE: specs/<sub_domain>/step_N.md ===   (Paths table added)
````

This skill does not write files directly, does not create directories directly, does not respond to the user.

## Halt clauses

- Classification is not `build` (and not `delta` of a prior `build`) → emit `=== FILE: halt.md ===` with one-line reason.
- `spec.md` missing → halt.
- Only the ward-setup step exists in the plan → emit `=== FILE: memory-bank/phase2-skipped.md ===` noting single-step plan.

## Self-critique gate

Before returning, verify:

- Existing AGENTS.md / memory-bank files were NOT rewritten. Only missing files got seeded.
- Every non-ward-setup step has a `## Paths` table.
- Every Paths-table row has an explicit ward-relative path — no placeholders, no `<fill-in>`, no "TBD."
- Ambiguous artifacts defaulted to the reusable bucket.
- No source code files (`.py`, `.js`, `.go`, `.ts`, `.rs`, etc.) were emitted by this skill itself. Directories are empty.
- Every row with "Register: yes" can be found in the step's Outputs — no invented artifacts.
