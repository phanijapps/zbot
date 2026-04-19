# analysis-workspace — R

Use this reference for an R ward that pulls external data, computes derived metrics, and produces a written verdict. R is particularly strong for financial time-series, econometric modeling, and domain-expert-authored analytics where the writeup (via R Markdown) is co-generated with the computation.

**Example scenario:** A `financial-analysis` ward using R. Sub-domains: `googl-valuation` (DCF + comparable-company via `tidyverse`), `sector-semiconductor-readout` (dplyr-based ranking). Reports authored as R Markdown rendered to markdown.

---

## AGENTS.md

````markdown
# Financial Analysis (R)

An R ward for valuation, comparative analysis, and scenario modeling of public equities. Reports are produced as R Markdown documents rendered to markdown for the orchestrator.

## Scope

**In scope**
- Single-ticker valuation (DCF, comparables)
- Multi-ticker comparison and ranking
- Time-series analysis and econometric modeling
- Scenario / sensitivity analysis

**Out of scope**
- Options-chain analytics → see a Python/Go ward (R options tooling is weaker)
- Portfolio optimization → use `portfolio-analytics`
- Real-time pricing → use a lower-latency stack

## Conventions

- **Package layout:** R package convention (`DESCRIPTION`, `NAMESPACE`, `R/`). Package name `financialanalysis` (R packages avoid hyphens).
- **Function naming:** snake_case. `fetch_income_statement()`, `compute_dcf()`.
- **Import syntax:** `library(financialanalysis)` at script top; internal references use `financialanalysis::<fn>`.
- **Error handling:** `stop()` with classed conditions (`withCallingHandlers`). Every network call wrapped in `retry_call(fn, max_attempts = 3)`.
- **Data paths:** Raw responses cache to `cache/<source>/<ticker>/<YYYYMMDD>.rds` (binary) or `.json`. Computed metrics live in function return values (typically `tibble`s).
- **Dates:** `lubridate::ymd()` for parsing. Every returned record includes `as_of_date`.
- **Money:** R has no `Decimal` type; use `double` with explicit rounding at display boundaries. Document tolerance.
- **Tests:** `testthat` with fixtures in `tests/testthat/fixtures/<source>/*.rds`. Never hit live APIs in tests.

## Report staging (every sub-domain)

Every sub-domain has parallel directories:

- `specs/<sub-domain>/` — spec + step files.
- `reports/<sub-domain>/` — output artifacts.

Inside every `reports/<sub-domain>/`:

- **`summary.md`** — rendered R Markdown. The `.Rmd` source may live alongside.
- **`manifest.json`** — artifact listing.

## Sub-domain naming

Kebab-case: `^[a-z0-9]+(-[a-z0-9]+)*$`. Note: R package names cannot contain hyphens, so the ward package name is `financialanalysis` (no hyphen) but sub-domain slugs ARE kebab-case.

## Cross-referencing sub-domains

`summary.md` lists related sub-domains under `## Related sub-domains` with relative paths.

## DOs

- Cache every external API response to `cache/` as `.rds` or `.json` before parsing.
- Wrap reports in R Markdown; render to markdown via `rmarkdown::render(..., output_format = "md_document")`.
- Return `tibble`s (not base `data.frame`) from primitives. Better printing, better handling of list-columns.
- Use `lubridate` for every date operation.

## DON'Ts

- Do not return `data.frame` with `stringsAsFactors = TRUE`. Use `tibble`.
- Do not hard-code ticker symbols outside `specs/<sub-domain>/`.
- Do not pull data in tests — use `cache/` fixtures.
- Do not use `T` / `F` for booleans. Use `TRUE` / `FALSE`.

## How to use this ward

Before new work:
1. Check `memory-bank/ward.md` sub-domains table.
2. Check `memory-bank/structure.md` for existing primitives.
3. Check `memory-bank/core_docs.md` for registered functions.
````

---

## memory-bank/ward.md

````markdown
# Ward: financial-analysis

## Purpose
Produce defensible written analyses of public equities using R's tidyverse ecosystem. Reports authored as R Markdown.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `googl-valuation` | DCF + comparable-company valuation, R Markdown report | — |
| `sector-semiconductor-readout` | Ranked readout across 10 semiconductor names | — |
| `macro-rate-scenario` | Scenario analysis over interest-rate paths | — |

## Sub-domains this ward does NOT support

- Options-chain analytics → use a Python or Go ward
- Portfolio optimization → use `portfolio-analytics`
- Real-time pricing → lower-latency stacks

## Key concepts

- **tidyverse** — dplyr, tidyr, ggplot2, purrr, lubridate — the canonical R workflow stack.
- **tibble** — tidyverse-aware data frame with consistent printing and list-column support.
- **R Markdown** — literate document combining prose, code, output, and plots.

## Dependencies

- **CRAN:** `tidyverse`, `lubridate`, `tidyquant` (market data), `rmarkdown`, `knitr`, `testthat`, `httr2`.
- **External APIs:** Yahoo Finance (via `tidyquant`), FRED (via `tidyquant::tq_get`).
- **Upstream wards:** none.
- **Downstream wards:** `portfolio-analytics` if it exists.

## Assumptions

- R >= 4.3 required for `|>` native pipe in primitive code.
- Reports render on the ward's local R environment; the orchestrator does not run R.
````

---

## memory-bank/structure.md

````markdown
# Structure — financial-analysis (R)

```
financial-analysis/
├── AGENTS.md
├── DESCRIPTION                         # (planned) R package metadata
├── NAMESPACE                           # (planned) exports
├── memory-bank/
│   ├── ward.md
│   ├── structure.md
│   └── core_docs.md
├── R/
│   ├── data_sources.R                  # (planned) wraps tidyquant with cache + retry
│   ├── fundamentals.R                  # (planned) parse income stmt / balance sheet
│   ├── valuation_dcf.R                 # (planned) DCF primitive
│   ├── valuation_multiples.R           # (proposed) comparable-company
│   └── utils_cache.R                   # (planned) cache read/write helpers
├── cache/                              # raw responses (gitignored)
├── tests/
│   └── testthat/
│       ├── test-fundamentals.R
│       ├── test-valuation.R
│       └── fixtures/
├── specs/
│   ├── googl-valuation/                # (planned)
│   └── sector-semiconductor-readout/   # (proposed)
└── reports/
    ├── googl-valuation/                # (planned)
    │   ├── summary.md                  # rendered from summary.Rmd
    │   ├── summary.Rmd                 # R Markdown source
    │   ├── manifest.json
    │   ├── dcf_model.csv
    │   └── sensitivity.png
    └── sector-semiconductor-readout/
        └── ...
```

## Extension policy

- New primitives land under `R/`. Every new file registers in `core_docs.md`.
- New sub-domain creates both `specs/<slug>/` and `reports/<slug>/` with required `summary.md` + `manifest.json`.
- R Markdown source (`summary.Rmd`) may live next to rendered `summary.md` for reproducibility.

## Cross-cutting rules

- Every exported primitive takes explicit arguments and returns a `tibble`.
- Every data-source call caches to `cache/<source>/<ticker>/<YYYYMMDD>.rds`.
- Use `|>` (native pipe), not `%>%` (magrittr), in new code.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — financial-analysis (R)

| Symbol | File | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |

## Registration rule

Every reusable function appended here as: symbol, source file, call signature, purpose, step that added it.

Example row:

| `fetch_income_statement` | `R/data_sources.R` | `fetch_income_statement(ticker, as_of)` | Pull income stmt with caching | step_2 of googl-valuation |
````

---

## Language-specific notes

- **Build:** `R CMD build .` produces a source tarball; `devtools::install()` for local dev.
- **Test:** `devtools::test()` or `testthat::test_dir("tests/testthat")`.
- **Lint:** `lintr::lint_package()`.
- **Render reports:** `rmarkdown::render("summary.Rmd", output_format = "md_document")`.
- **Cache format:** `.rds` (binary, preserves types) preferred over `.json` for cached tibbles. JSON only for human inspection of raw responses.
- **Money:** R has no `Decimal`. Use `double` and round only at display. Document tolerance explicitly in the DCF primitive.
- **Package name no-hyphen rule:** Sub-domain slugs use kebab-case (`googl-valuation`); R package name uses no hyphens (`financialanalysis`). This is an R language constraint, not a ward convention violation.
