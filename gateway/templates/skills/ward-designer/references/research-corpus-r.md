# research-corpus — R

Use this reference for an R ward that ingests and analyzes a document corpus. R is natural when the corpus work involves statistical text analysis — topic modeling (LDA, STM), sentiment, stylometry — or when authors already work in R.

**Example scenario:** A `humanities-corpus` ward for social-science text analysis. Sub-domains: `election-speech-sentiment` (sentiment over a speech corpus), `author-attribution` (stylometric attribution), `topic-evolution` (topic-model drift across a decade).

---

## AGENTS.md

````markdown
# Humanities Corpus

An R ward for statistical analysis of text corpora — topic modeling, sentiment, stylometry. Reports authored as R Markdown rendered to markdown.

## Scope

**In scope**
- Ingest plain-text or lightly-marked-up corpora
- Topic modeling (LDA, STM) and topic evolution
- Sentiment and emotion analysis
- Stylometric attribution and comparison
- Visualization of text-statistical results

**Out of scope**
- Heavy PDF extraction → Python ward
- Semantic search over embeddings → Python or Node ward
- Multi-language translation → upstream

## Conventions

- **Package layout:** R package (`DESCRIPTION`, `NAMESPACE`, `R/`). Package name `humanitiescorpus` (no hyphens).
- **Function naming:** snake_case.
- **Tidyverse + tidytext:** use `tidytext::unnest_tokens` as the canonical tokenizer; `dplyr` for all manipulation.
- **Import syntax:** `library(humanitiescorpus)`.
- **Document model:** tibble with columns `doc_id, author, year, text`. One row per document.
- **Passage model:** after `unnest_tokens(...)`, tibble gains `word` (or `sentence`, `paragraph`) with offsets preserved by line number.
- **Error handling:** `stop()` with `tryCatch()` at pipeline boundaries.
- **Tests:** `testthat` with fixtures in `tests/testthat/fixtures/`.

## Report staging

Parallel directories `specs/<sub-domain>/` and `reports/<sub-domain>/`. Required `summary.md` (rendered R Markdown) + `manifest.json`.

## Sub-domain naming

Kebab-case at the sub-domain level even though the R package itself has no hyphens.

## Cross-referencing sub-domains

Standard `## Related sub-domains` section in `summary.md`.

## DOs

- Use `tibble`, not `data.frame`.
- Preserve document-level metadata (`author`, `year`) through every transform so results can be stratified.
- Use `stm` for topic models with covariates, `topicmodels::LDA` for simpler cases.

## DON'Ts

- Do not use base `grep`/`regmatches` for tokenization — use `tidytext::unnest_tokens`.
- Do not lose passage offsets during tokenization.
- Do not compare across corpora with different preprocessing — normalize preprocessing first.

## How to use this ward

Before new work, check ward.md → structure.md → core_docs.md as usual.
````

---

## memory-bank/ward.md

````markdown
# Ward: humanities-corpus

## Purpose
Statistical analysis of text corpora — topic modeling, sentiment, stylometry — using R's tidytext + stm ecosystem.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `election-speech-sentiment` | Sentiment trajectory over a political speech corpus | — |
| `author-attribution` | Stylometric attribution of disputed authorship | — |
| `topic-evolution` | Structural topic model drift across a decade | — |

## Sub-domains this ward does NOT support

- Heavy PDF OCR → Python literature-library
- Semantic / embedding-based search → Python or Node ward

## Key concepts

- **DFM** — document-feature matrix (via `quanteda`).
- **STM** — Structural Topic Model with covariates.
- **Stylometry** — authorship attribution via word/ngram frequency patterns.
- **Zeta score** — Craig's Zeta, a standard stylometric distinctiveness measure.

## Dependencies

- **CRAN:** `tidyverse`, `tidytext`, `quanteda`, `stm`, `topicmodels`, `rmarkdown`, `testthat`.

## Assumptions

- R >= 4.3.
- English corpora by default; multilingual requires explicit preprocessing adjustments.
````

---

## memory-bank/structure.md

````markdown
# Structure — humanities-corpus

```
humanities-corpus/
├── AGENTS.md
├── DESCRIPTION
├── NAMESPACE
├── memory-bank/
├── R/
│   ├── ingest.R                        # (planned) load + clean text
│   ├── tokenize.R                      # (planned) tidytext tokenization
│   ├── topic_model_stm.R               # (planned) STM fitting
│   ├── sentiment.R                     # (proposed) sentiment lexicons + aggregation
│   └── stylometry.R                    # (proposed) Zeta + Burrow's Delta
├── corpus/
├── tests/testthat/
├── specs/
│   ├── election-speech-sentiment/      # (planned)
│   └── author-attribution/             # (proposed)
└── reports/
    ├── election-speech-sentiment/
    │   ├── summary.md                  # rendered from summary.Rmd
    │   ├── summary.Rmd
    │   ├── manifest.json
    │   ├── sentiment_timeseries.csv
    │   └── sentiment_plot.png
    └── author-attribution/
        └── ...
```

## Extension policy

- New analysis method → new file under `R/`, register in `core_docs.md`.
- New sub-domain → `specs/<slug>/` + `reports/<slug>/` with required files.

## Cross-cutting rules

- Every exported function returns a `tibble` with preserved metadata columns.
- Every sub-domain report's `summary.md` is produced from a co-located `summary.Rmd`.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — humanities-corpus

| Symbol | File | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |
````

---

## Language-specific notes

- **Build:** `R CMD build .` and `devtools::install()` for dev.
- **Test:** `devtools::test()`.
- **Render reports:** `rmarkdown::render("summary.Rmd", output_format = "md_document")`.
- **Tokenization:** `tidytext::unnest_tokens(data, word, text)` is the canonical starting point.
- **Topic models:** prefer `stm` over `topicmodels` when covariates matter (year, author, genre).
- **Stylometry:** `stylo` package for canonical ngram-frequency methods; `quanteda` for DFM machinery.
