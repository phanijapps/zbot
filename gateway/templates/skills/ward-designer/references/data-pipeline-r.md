# data-pipeline — R

Use this reference for an R data-pipeline ward. Rare combination — Python (with pandas/pyarrow) and Go are far more common for production ETL. R pipelines are appropriate only when (a) the analysts downstream work in R and want the transforms co-located with their analysis, or (b) the domain has strong R-specific data-source tooling (e.g., epidemiology with `epiR`).

**When to use:** small-to-medium batch pipelines where analysts own the whole stack in R; survey-data cleaning; domain-specific R ecosystems.

**When NOT to use:** production-scale ETL, streaming, heavy concurrency, any pipeline that services non-R consumers directly.

---

## AGENTS.md

````markdown
# <Ward Name>

An R ward for batch data-pipeline work. Justify the R choice in `memory-bank/ward.md`.

## Scope

Narrow to R-native ecosystems or analyst-owned batch pipelines.

## Conventions

- Package layout: R package convention (`DESCRIPTION`, `NAMESPACE`, `R/`). Package name has no hyphens.
- Function naming: snake_case.
- Import syntax: `library(<package>)`.
- Tidyverse for transforms (`dplyr`, `tidyr`, `purrr`).
- Error handling: `stop()` with classed conditions. Wrap network calls in `retry_call()` helpers.
- Schema validation: `pointblank` package or manual `stopifnot` on expected columns.
- Tests: `testthat` with `tests/testthat/fixtures/`.

## Report staging

Standard `specs/<sub-domain>/` + `reports/<sub-domain>/` with `summary.md` + `manifest.json`.

Summaries rendered from `summary.Rmd` via `rmarkdown::render`.

## Sub-domain naming

Kebab-case (R package name has no hyphens; sub-domain slugs do).

## DOs

- Land raw extracts before transforming. Store under `raw/<source>/<YYYY-MM-DD>.rds` (binary preserves types) or `.csv` (inspectable).
- Transforms take tibbles in and return tibbles out — same library boundary throughout.
- Register every reusable function in `core_docs.md`.

## DON'Ts

- Do not use base `data.frame` — use `tibble`.
- Do not rely on `options()` state; pass parameters explicitly.
- Do not do live API calls in tests; use cached fixtures.
````

---

## memory-bank/ward.md

````markdown
# Ward: <ward-name>

## Purpose
Justify the R choice — analyst ownership, R-specific ecosystem, or downstream R consumers.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `<current-slug>` | <description> | — |
| `<future-slug-1>` | <description> | — |
| `<future-slug-2>` | <description> | — |

## Dependencies

- **CRAN:** `tidyverse`, `lubridate`, `pointblank` (schema validation), `rmarkdown`, `testthat`, `httr2`, `arrow` (for parquet if needed).

## Assumptions

- R >= 4.3.
- All timestamps UTC.
````

---

## memory-bank/structure.md

````markdown
# Structure — <ward-name> (R)

```
<ward-name>/
├── AGENTS.md
├── DESCRIPTION
├── NAMESPACE
├── memory-bank/
├── R/
│   ├── sources.R                       # (planned)
│   ├── transforms.R                    # (planned)
│   └── sinks.R                         # (planned)
├── raw/
├── processed/
├── tests/testthat/
├── specs/<sub-domain>/
└── reports/<sub-domain>/
    ├── summary.md                      # rendered from summary.Rmd
    ├── summary.Rmd
    ├── manifest.json
    └── <data files>
```

## Cross-cutting rules

- Every exported function takes and returns a tibble.
- Every source function caches to `raw/<source>/<YYYY-MM-DD>.rds`.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — <ward-name>

| Symbol | File | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |
````

---

## Language-specific notes

- **Parquet:** `arrow::write_parquet()` if the sink is parquet. Otherwise CSV or `.rds` is fine.
- **Scheduling:** R has no native cron equivalent; orchestration lives outside the ward (cron + Rscript, or a workflow tool).
- **If you find yourself needing high concurrency or streaming:** stop. Reconsider whether this should be a Python or Go ward.
- **This reference is terse on purpose.** Expand when real use justifies it.
