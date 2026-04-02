# Ward Scaffolding: Skill-Driven Reusable Apps

## Problem

Wards today are bare directories with a generic AGENTS.md. Each session recreates structure from scratch, nothing is reusable, and similar runs take long to execute. There's no spec-first workflow, no core module indexing, and no structured way to build on previous work.

## Vision

Wards are **reusable apps that grow over time**. Skills drive their structure. Agents work like coding agents — spec first, implement, build on what exists. The runtime handles mechanical scaffolding; the agent handles creative work.

## Design

### 1. Skill `ward_setup` Frontmatter

Skills gain an optional `ward_setup` section in SKILL.md frontmatter:

```yaml
---
name: financial-analysis
description: Stock, options, and market analysis
ward_setup:
  directories:
    - core/
    - output/
    - specs/
    - specs/archive/
    - memory-bank/
  language_skills:
    - python
  spec_guidance: |
    Financial analysis specs must cover:
    - Data sources (API, scraping, manual) with rate limits
    - Calculation methodology with formulas
    - Output format (HTML report, CSV, charts)
    - Dependencies on core modules
    - Error handling for market data gaps
  agents_md:
    purpose: "Reusable financial analysis workspace — stocks, options, ETFs, market research"
    conventions:
      - "All reusable code in core/, task-specific in subdirectories"
      - "Output files (reports, charts) in output/"
      - "Max 100 lines per file, one concern per module"
---
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `directories` | `string[]` | Directories to create on first ward creation |
| `language_skills` | `string[]` | Referenced language skills (informational — tells agents which runtime conventions to follow) |
| `spec_guidance` | `string` | Domain-specific hints injected into the agent's context when writing specs |
| `agents_md.purpose` | `string` | Seed purpose for AGENTS.md |
| `agents_md.conventions` | `string[]` | Coding conventions for this domain |

### 2. Runtime Scaffolding Middleware

**File:** `gateway/gateway-execution/src/middleware/ward_scaffold.rs`

A new middleware that runs when the ward tool returns `__ward_changed__: true` with `action: "created"`.

**Trigger:** Post-ward-creation, before agent proceeds with execution.

**What it does (new ward):**

1. Reads recommended skills from intent analysis result (already stored in execution state as `intent_analysis`)
2. For each skill with `ward_setup`, parses the frontmatter
3. Creates directories listed in `ward_setup.directories`
4. Generates AGENTS.md from `ward_setup.agents_md` merged with ward name + creation date
5. Reads language config from `~/Documents/zbot/config/wards/{language}.yaml` for conventions

**What it does (re-entry into existing ward):**

1. No scaffolding — directories already exist
2. Runs core module indexer (see Section 5)
3. Updates AGENTS.md `## Core Modules` section with current index
4. Knowledge graph recall runs as it does today via `recall_ward_facts`

**Pipeline position:**

```
Intent Analysis → Agent enters ward → Ward Tool returns → ward_scaffold middleware → Agent proceeds
```

### 3. Execution Graph: Spec-First Node

Intent analysis already builds execution graphs for complex tasks. The LLM prompt is updated to enforce that **the first node in any complex graph is a spec-writing node**.

**Updated intent analysis prompt addition:**

```
When approach is "graph" and the task involves writing code:
- The FIRST node must be a spec-writing node (id: "specs", agent: "root")
- This node reads the ward's AGENTS.md, existing core modules, and the user's ask
- It produces detailed specs in specs/<domain-specific>/*.md
- Subsequent nodes implement against those specs
- Do NOT put implementation work in the specs node
```

**The agent writes specs during execution** — not the runtime, not intent analysis. The graph just ensures the spec step happens first.

**Spec quality is driven by:**

1. The skill's `spec_guidance` (domain hints) — injected into agent context
2. The ward rule injection (see Section 4) — universal "write thorough specs" guidance
3. The agent's own judgment based on complexity

**For simple approach:** The agent decides during the session if specs are needed. If writing new functionality in a ward → spec first. If extending existing with a small change → just code.

### 4. Ward Rule Injection

`format_intent_injection()` in `intent_analysis.rs` is extended to always include:

```markdown
**Ward Rule:** ALL code must be written inside a ward. If you need to write code:
1. Enter the recommended ward (or create if new)
2. Read AGENTS.md to understand what exists in core/
3. Check if existing core/ modules already solve your need — reuse, don't recreate
4. If new functionality: write a spec in specs/<domain>/<name>.md first, then implement
5. After implementing: archive spec to specs/archive/, update AGENTS.md with new core modules

**Spec Lifecycle:**
- Active specs live in specs/
- After implementing a spec, archive it to specs/archive/
- Archived specs are searchable via knowledge graph for future context

**Spec Quality:**
Write specs detailed enough that a different agent can implement them without asking questions:
- Purpose: what this does and why
- Inputs/Outputs: exact data structures, types, formats
- Dependencies: which core/ modules to import, external packages needed
- Implementation detail: algorithm, data flow, error cases
- Integration: how this connects to other specs in this run
```

When a skill has `spec_guidance`, it's appended:

```markdown
**Domain Spec Guidance:**
{spec_guidance from skill}
```

This applies to both `simple` and `graph` approaches. The graph automates the ordering; the injection ensures the agent follows it regardless.

### 5. AGENTS.md Core Module Auto-Indexing

**File:** Extended in `gateway/gateway-execution/src/ward_sync.rs`

After each session that modifies `core/`, a post-execution hook scans core modules and updates the `## Core Modules` section in AGENTS.md.

**How it works:**

1. Reads language config from `~/Documents/zbot/config/wards/{language}.yaml`
2. Scans `core/` for files matching `file_extensions`
3. Extracts function/class signatures using `signature_patterns` regex
4. Extracts docstrings using `docstring_pattern` regex
5. Writes indexed signatures into AGENTS.md under `## Core Modules`

**Example output in AGENTS.md:**

```markdown
## Core Modules
- `core/data_fetcher.py` — fetch_ohlcv(ticker, period) → DataFrame, fetch_fundamentals(ticker) → dict
- `core/chart_builder.py` — build_candlestick(df, title) → saves PNG, build_indicator_chart(df, indicators) → saves PNG
- `core/report_generator.py` — generate_html_report(sections, output_path) → HTML file
```

**When it runs:** During distillation (post-session), same timing as `generate_ward_knowledge_file`.

**Graceful degradation:** If no language config exists for the files found in `core/`, indexing is skipped.

### 6. Language Pattern Configs

**Location:** `~/Documents/zbot/config/wards/`

User-editable YAML files that define how to parse each language's signatures for core module indexing.

**`python.yaml` (ships as default):**

```yaml
language: python
file_extensions: [".py"]
signature_patterns:
  function: "^def\\s+(\\w+)\\s*\\((.*)\\)"
  class: "^class\\s+(\\w+)"
docstring_pattern: '^\s*"""(.+?)"""'
conventions:
  - "Import from core/: `from core.<module> import <function>`"
  - "Use shared .venv at wards root"
  - "Max 100 lines per file, one concern per module"
  - "Use apply_patch for all file operations"
```

**`r.yaml` (example, user-created):**

```yaml
language: r
file_extensions: [".R", ".r"]
signature_patterns:
  function: "^(\\w+)\\s*<-\\s*function\\s*\\((.*)\\)"
docstring_pattern: "^#'\\s*(.+)"
conventions:
  - "Source from core/: source('core/<module>.R')"
```

**`node.yaml` (example, user-created):**

```yaml
language: node
file_extensions: [".js", ".ts"]
signature_patterns:
  function: "^export\\s+(async\\s+)?function\\s+(\\w+)\\s*\\((.*)\\)"
  class: "^export\\s+class\\s+(\\w+)"
docstring_pattern: "^\\s*\\*\\s+(.+)"
conventions:
  - "Import from core/: `import { fn } from './core/<module>.js'`"
  - "Use package.json at ward root for dependencies"
```

**Schema:** The runtime reads these at scan time. New languages are added by dropping a YAML file — no Rust code changes needed.

### 7. Spec Archival & Knowledge Graph

**Archival:** Agent responsibility. After implementing a spec, the agent moves it:

```
specs/spy/core-data.md → specs/archive/spy/core-data.md
```

**Knowledge graph integration:** The existing distillation process picks up archived specs during post-session processing. Key decisions, patterns, and domain knowledge from specs are indexed into the knowledge graph. Future sessions recall this via `recall_ward_facts` on ward re-entry.

No new knowledge graph infrastructure needed — the existing distillation + recall pipeline handles this.

## Ward Lifecycle Summary

```
First Run:
  Intent Analysis → recommends ward + skills
  → Agent creates ward → Runtime scaffolds (dirs, AGENTS.md from skill ward_setup)
  → [Graph: spec node first] Agent writes specs → Agent implements → Agent archives specs
  → Post-session: core module indexer updates AGENTS.md, distillation indexes to knowledge graph

Subsequent Runs:
  Intent Analysis → recommends existing ward + skills
  → Agent enters ward → Runtime re-indexes core modules in AGENTS.md
  → Agent reads AGENTS.md (knows what exists) + knowledge graph recall (patterns, decisions)
  → Agent reuses core/ modules, extends with new functionality
  → Spec → implement → archive cycle continues
  → Ward grows as a reusable app
```

## Files Changed

| File | Change |
|------|--------|
| `gateway/gateway-execution/src/middleware/ward_scaffold.rs` | **New** — scaffolding middleware |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Update prompt to enforce spec-first node in graphs; add ward rule + spec guidance to `format_intent_injection()` |
| `gateway/gateway-execution/src/ward_sync.rs` | Extend with core module indexer, read language YAML configs |
| `runtime/agent-tools/src/tools/ward.rs` | Slim down AGENTS.md generation (middleware handles it now) |
| `gateway/gateway-execution/src/middleware/mod.rs` | Register new ward_scaffold middleware |
| `gateway/gateway-execution/src/runner.rs` | Wire ward_scaffold into execution pipeline |
| `~/Documents/zbot/config/wards/python.yaml` | **New** — default Python language config |
| Skill SKILL.md files | Add optional `ward_setup` frontmatter (user-created skills) |

## Out of Scope

- Domain-specific skills (financial-analysis, math-tutor, etc.) — user creates these
- New language configs beyond `python.yaml` — user creates as needed
- Changes to the knowledge graph schema
- Ward migration for existing wards (they continue working as-is)
