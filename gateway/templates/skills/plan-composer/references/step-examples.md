# plan-composer — worked step examples

One example per common step shape. Load the relevant section when you're unsure how to structure a particular step.

All examples conform to the template in `SKILL.md`: Goal, Skills, Reusable inputs, Domain inputs, Reusable outputs, Domain outputs, Acceptance (BDD), Depends on, Parallel-safe with, Risk tier, optional Guardrail.

Every step pre-classifies each artifact into one of two buckets:

- **Reusable** — potentially usable by another sub-domain. Lands at the ward root (`<module_root>/`, `templates/`, `snippets/`, `shared-docs/`, `data/`). Every reusable output is registered in `memory-bank/core_docs.md`.
- **Domain** — only this sub-domain uses it. Lands under `<sub-domain>/` in `code/`, `data/`, or `reports/`.

Ward-designer translates these buckets into exact ward-relative paths via each step's `## Paths` table.

---

## ward-setup (always Step 1 for `build` plans)

A low-risk bootstrap step. Skill is always `ward-designer`. Produces (or verifies) the ward's four doctrine files; never produces source code.

`````markdown
# Step 1 — Ward setup

## Goal
Set up (or review) the `research` ward for sub-domain `neo4j-vs-alternatives`. Ensures AGENTS.md + memory-bank docs exist, and every subsequent step file carries a `## Paths` table that assigns exact ward-relative paths to its outputs.

## Skills
- ward-designer: the ward scaffolder

## Reusable inputs (import from ward root)
- none

## Domain inputs (prior-step outputs or context paths)
- context path `specs/neo4j-vs-alternatives/spec.md` — the spec ward-designer reads

## Reusable outputs (land under ward root; register in memory-bank/core_docs.md)
- `AGENTS.md` — ward operating guide (only if missing)
- `memory-bank/ward.md` — ward purpose + ≥ 3 sub-domains (only if missing)
- `memory-bank/structure.md` — directory tree with status markers (only if missing)
- `memory-bank/core_docs.md` — reusable-asset registry (only if missing)

## Domain outputs (land under <sub-domain>/)
- none

## Acceptance (BDD)

```gherkin
Given the ward may or may not already exist
When ward-designer runs
Then wards/<ward>/AGENTS.md exists with non-trivial content
 And wards/<ward>/memory-bank/ward.md, structure.md, core_docs.md all exist
 And every subsequent step file has a ## Paths table assigning exact ward-relative paths to each output
 And no .py / .js / .go / .ts / .rs source file was written by this step
```

## Depends on
- none

## Parallel-safe with
- none — must complete before any other step

## Risk tier
low
`````

---

## research-gather

A low-risk step that fetches information from an external source. Many of these run in parallel in a `research` plan. Typical output is a per-subject markdown dossier — domain-bucket.

`````markdown
# Step 2 — Fetch Neo4j Community Edition overview

## Goal
Collect current feature, pricing (including hosted options), license, and community-size data for Neo4j Community Edition. Feeds Step 5's comparison matrix.

## Skills
- duckduckgo-search: primary query
- light-panda-browser: fetch full-page content for top results

## Reusable inputs (import from ward root)
- none

## Domain inputs (prior-step outputs or context paths)
- none

## Reusable outputs (land under ward root; register in memory-bank/core_docs.md)
- none

## Domain outputs (land under <sub-domain>/)
- `neo4j-vs-alternatives/data/neo4j_profile.md` — markdown dossier with five sections (Features, License, Hosted options, Community, Sources). Every claim cited with a URL.

## Acceptance (BDD)

```gherkin
Given the step has not yet run
When the research subagent completes
Then neo4j-vs-alternatives/data/neo4j_profile.md exists
 And it contains all five required sections
 And at least 3 independent sources are cited inline with URLs
 And every factual claim is followed by a citation marker
```

## Depends on
- Step 1: ward-setup

## Parallel-safe with
- Step 3, Step 4, Step 5 (sibling candidate-fetch steps)

## Risk tier
low
`````

---

## research-synthesize

A medium-risk step that integrates prior outputs into a new artifact. Typically depends on all gather/analyze steps.

`````markdown
# Step 6 — Write recommendation memo

## Goal
Produce a recommendation tied to the context's workload (read-heavy, AWS, modest budget). Use the comparison matrix from Step 5 as evidence.

## Skills
- summarize_sources: evidence-based synthesis

## Reusable inputs (import from ward root)
- `shared-docs/citation_format.md` — inline citation style (if present — fall back to markdown footnotes otherwise)

## Domain inputs (prior-step outputs or context paths)
- step_5.output `neo4j-vs-alternatives/data/comparison_matrix.md`
- context path `workload_constraints` in spec.md

## Reusable outputs (land under ward root; register in memory-bank/core_docs.md)
- none

## Domain outputs (land under <sub-domain>/)
- `neo4j-vs-alternatives/reports/recommendation.md` — one paragraph naming the recommended database, followed by a bulleted rationale (≥ 3 bullets) tied to the workload constraints.

## Acceptance (BDD)

```gherkin
Given Step 5's comparison_matrix.md exists
  And the workload constraints are "read-heavy, AWS, modest budget"

When the synthesis subagent completes

Then neo4j-vs-alternatives/reports/recommendation.md exists
 And exactly one database is named as the recommendation
 And the rationale has at least 3 bullets
 And every bullet cites a row of the comparison matrix
 And all three constraints (read-heavy, AWS, modest-budget) are addressed explicitly
```

## Depends on
- Step 5: comparison matrix

## Parallel-safe with
- none — synthesis step

## Risk tier
medium
`````

---

## code-fill

A medium-risk step in a `build` plan. Produces both a reusable primitive AND a thin task-specific wrapper. This is the canonical bucket split that every code-fill step exhibits.

`````markdown
# Step 3 — Implement the currency-conversion primitive

## Goal
Create a pure conversion function in the ward's reusable module root, plus a thin task-specific CLI wrapper that calls it with the current sub-domain's rates file. Step 5's reporting consumes the primitive; this step exists because no convert() exists yet.

## Skills
- code_writer: clean-code conventions
- pytest: unit-test harness

## Reusable inputs (import from ward root)
- `<module_root>/rates_loader.load_rates(path: str) -> dict[str, Decimal]` — Step 2's primitive

## Domain inputs (prior-step outputs or context paths)
- step_1.output — scaffold (project tree exists)
- context path — rates source (named in spec's context)

## Reusable outputs (land under ward root; register in memory-bank/core_docs.md)
- `<module_root>/currency.py` — module exporting `convert(amount: Decimal, from_code: str, to_code: str, rates: dict) -> Decimal`. Pure function. Raises `UnknownCurrencyError` on unknown code.

## Domain outputs (land under <sub-domain>/)
- `<sub-domain>/code/convert_cli.py` — thin CLI wrapper: load rates via `rates_loader.load_rates`, call `currency.convert`, print result. Hardcodes the sub-domain's rates file path.
- `<sub-domain>/tests/test_convert_cli.py` — smoke test for the CLI wrapper

## Acceptance (BDD)

```gherkin
Given the ward's module_root is set per Conventions
  And rates_loader.load_rates is importable from a prior step

When the coding subagent completes

Then <module_root>/currency.py defines convert(amount, from_code, to_code, rates) -> Decimal
 And calling convert with same-currency returns the amount unchanged
 And calling convert with an unknown code raises UnknownCurrencyError
 And `pytest tests/currency/` exits 0
 And memory-bank/core_docs.md has an appended row registering currency.convert
 And <sub-domain>/code/convert_cli.py imports currency.convert (does not re-implement)
```

## Depends on
- Step 1: scaffold
- Step 2: rates-loader primitive

## Parallel-safe with
- Step 4 (independent primitive in the same fill phase)

## Risk tier
medium
`````

---

## browser-flow

A medium-risk step that drives a browser. Usually non-parallel because UI sessions do not multiplex well. Typically all outputs are domain-bucket (captures are per-target).

`````markdown
# Step 4 — Capture competitor pricing pages

## Goal
Visit each competitor's public pricing page and archive the rendered HTML plus a screenshot. Feeds Step 5's extraction.

## Skills
- light-panda-browser: render + capture

## Reusable inputs (import from ward root)
- none

## Domain inputs (prior-step outputs or context paths)
- step_3.output `<sub-domain>/data/competitor_urls.json`

## Reusable outputs (land under ward root; register in memory-bank/core_docs.md)
- none

## Domain outputs (land under <sub-domain>/)
- `<sub-domain>/data/captures/<slug>/page.html` — rendered HTML, one per URL
- `<sub-domain>/data/captures/<slug>/page.png` — screenshot, one per URL

## Acceptance (BDD)

```gherkin
Given competitor_urls.json exists with ≥ 1 URL

When the browser subagent completes

Then every URL in competitor_urls.json has a <slug>/page.html AND a <slug>/page.png
 And no capture file is empty (> 1 KB each)
 And the capture index lists every slug with timestamp
```

## Depends on
- Step 3: URL list

## Parallel-safe with
- none — browser session is serial

## Risk tier
medium
`````

---

## high-risk-write

A step that modifies external state. Always `high` tier. MUST include a Guardrail.

`````markdown
# Step 7 — Publish the competitive-analysis report to the shared drive

## Goal
Upload the finalized report to the team drive at the path declared in the spec's context. External write with low reversibility.

## Skills
- drive_upload: the drive client

## Reusable inputs (import from ward root)
- none

## Domain inputs (prior-step outputs or context paths)
- step_6.output `<sub-domain>/reports/report.md`
- context path `target_drive_path` (from spec.md's context section)

## Reusable outputs (land under ward root; register in memory-bank/core_docs.md)
- none

## Domain outputs (land under <sub-domain>/)
- `<sub-domain>/data/upload_receipt.json` — `{file_id, url, uploaded_at, source_sha256}`

## Acceptance (BDD)

```gherkin
Given the final report.md exists locally
  And target_drive_path is set in context

When the upload subagent completes

Then <sub-domain>/data/upload_receipt.json exists
 And it contains file_id, url, uploaded_at, and source_sha256
 And the remote file's SHA-256 matches source_sha256
 And upload_receipt.url returns 200 on a follow-up GET
```

## Depends on
- Step 6: final report

## Parallel-safe with
- none — external write

## Risk tier
high

## Guardrail
If the drive API returns any non-2xx status, halt and report the status plus response body. Do NOT retry the upload silently — retries could create duplicates on the shared drive. If the SHA-256 check fails after upload, halt and flag the mismatch for human review. Do NOT proceed to downstream steps until the receipt is verified.
`````

---

## writing-synthesize

A step that produces a long-form written artifact. Usually medium (reversible) unless it publishes externally. Occasionally emits a reusable section template if the same shape will be used across sub-domains.

`````markdown
# Step 5 — Write the integrated competitive analysis report

## Goal
Produce a ~3000–4000-word markdown report integrating each competitor profile with cross-competitor takeaways. Final deliverable before optional format conversion.

## Skills
- long_form_writer: narrative-structured writing

## Reusable inputs (import from ward root)
- `shared-docs/report_sections.md` — canonical section order for analysis reports (if present)

## Domain inputs (prior-step outputs or context paths)
- step_4.output (per-competitor profile markdowns under `<sub-domain>/data/`)
- context path `audience_and_tone` (from spec.md)

## Reusable outputs (land under ward root; register in memory-bank/core_docs.md)
- `shared-docs/competitive_report_template.md` — IF this is the first such report in the ward; defines the section skeleton future reports reuse. Otherwise none.

## Domain outputs (land under <sub-domain>/)
- `<sub-domain>/reports/report.md` — 3000–4500 words. Sections: Executive summary, one section per competitor, Cross-competitor takeaways, Strategic implications.
- `<sub-domain>/summary.md` — one-paragraph memo + verdict
- `<sub-domain>/manifest.json` — artifact listing

## Acceptance (BDD)

```gherkin
Given per-competitor profiles exist from Step 4
  And audience_and_tone is defined

When the writer subagent completes

Then <sub-domain>/reports/report.md exists
 And it contains all six required sections
 And its word count is between 3000 and 4500
 And every factual claim has an inline citation
 And <sub-domain>/summary.md carries a clear verdict
 And <sub-domain>/manifest.json lists every file produced by this step
```

## Depends on
- Step 4: per-competitor profiles

## Parallel-safe with
- none — synthesis

## Risk tier
medium
`````

---

## format-convert (beautification)

A medium-risk step emitted ONLY when the verbatim prompt used an explicit format word (HTML, PDF, PPT, docx, slides). Consumes the writer's markdown; produces the styled artifact.

`````markdown
# Step 8 — Convert report to HTML

## Goal
Take the writer's markdown report and render it as a styled single-file HTML using the ward's reusable HTML template. Only runs because the user explicitly asked for an HTML report.

## Skills
- html-report: templated styled HTML renderer

## Reusable inputs (import from ward root)
- `templates/html_report.html.jinja` — the ward's shared HTML template (if present; if missing, step also emits it as a reusable output)

## Domain inputs (prior-step outputs or context paths)
- step_5.output `<sub-domain>/reports/report.md`
- step_5.output `<sub-domain>/summary.md`

## Reusable outputs (land under ward root; register in memory-bank/core_docs.md)
- `templates/html_report.html.jinja` — IF this ward has not yet produced an HTML report (first-use seeds the template). Otherwise none.

## Domain outputs (land under <sub-domain>/)
- `<sub-domain>/reports/report.html` — rendered HTML, single file, embedded CSS

## Acceptance (BDD)

```gherkin
Given <sub-domain>/reports/report.md exists
  And the user's verbatim prompt contained the word "HTML"

When the format-convert subagent completes

Then <sub-domain>/reports/report.html exists
 And it parses as valid HTML
 And its rendered sections match the markdown report's section count
 And the HTML references no external resources (self-contained)
 And the file size is under 2 MB
```

## Depends on
- Step 5: report.md

## Parallel-safe with
- none — single-output converter

## Risk tier
medium
`````

---

## Bucket-classification cheatsheet

When unsure which bucket an artifact belongs in:

| Artifact shape | Bucket | Lands at |
|---|---|---|
| Function / class / constant / module callable from another sub-domain | reusable | `<module_root>/<module>.<ext>` |
| Template file (Jinja, Handlebars, Mustache) for an output shape | reusable | `templates/<name>.<ext>` |
| Code snippet reused verbatim across primitives | reusable | `snippets/<name>.<ext>` |
| Markdown fragment (citation style, disclaimer, boilerplate) | reusable | `shared-docs/<name>.md` |
| Shared reference dataset used by multiple sub-domains | reusable | `data/<name>.<ext>` |
| Script hardcoding this sub-domain's inputs / tickers / slugs | domain | `<sub-domain>/code/<name>.<ext>` |
| Computed data (JSON, CSV, serialized state) for this sub-domain only | domain | `<sub-domain>/data/<name>.<ext>` |
| Per-URL capture (HTML, screenshot, archive) | domain | `<sub-domain>/data/captures/<slug>/` |
| Markdown memo / report / verdict for this sub-domain | domain | `<sub-domain>/reports/<name>.md` |
| Final styled deliverable (HTML / PDF / PPT / docx) | domain | `<sub-domain>/reports/<name>.<ext>` |
| Transactional record of an external write (upload receipt, commit SHA, post ID) | domain | `<sub-domain>/data/<name>.json` |
| Summary + manifest required per ward convention | domain | `<sub-domain>/summary.md` + `<sub-domain>/manifest.json` |

**Default when ambiguous: reusable.** A ward that hoards assets per sub-domain can't grow.
