# ward-designer references — lookup

Pick the reference file that matches your ward's **shape** × **language** combination.

## Shapes

- **`analysis-workspace`** — pulls external data, computes derived metrics, produces a written verdict. Examples: stock-analysis, startup-due-diligence, competitor-landscape, policy-analysis.
- **`research-corpus`** — ingests documents, indexes them, retrieves passages, produces summaries or answers. Examples: literature-library, legal-corpus, research-papers, codebase-knowledge.
- **`data-pipeline`** — fetches data from operational sources, transforms, persists to a warehouse or dashboard-friendly format. Examples: customer-analytics, ops-metrics, billing-pipeline, event-ingest.

If the domain does not match any of the three, pick the closest shape and adapt. Do not copy the reference verbatim.

## Languages

- **`python`** — usually the default for data, research, and analysis work. Broadest library coverage.
- **`nodejs`** — TypeScript/JavaScript on Node. Common for services and event-driven work.
- **`go`** — common for data-pipelines, services, and performance-sensitive extract/transform work.
- **`r`** — common for statistical analysis and research corpora where domain experts write the code.

## File lookup

| Shape \ Language | python | nodejs | go | r |
|---|---|---|---|---|
| analysis-workspace | [analysis-workspace-python.md](analysis-workspace-python.md) | [analysis-workspace-nodejs.md](analysis-workspace-nodejs.md) | [analysis-workspace-go.md](analysis-workspace-go.md) | [analysis-workspace-r.md](analysis-workspace-r.md) |
| research-corpus | [research-corpus-python.md](research-corpus-python.md) | [research-corpus-nodejs.md](research-corpus-nodejs.md) | [research-corpus-go.md](research-corpus-go.md) | [research-corpus-r.md](research-corpus-r.md) |
| data-pipeline | [data-pipeline-python.md](data-pipeline-python.md) | [data-pipeline-nodejs.md](data-pipeline-nodejs.md) | [data-pipeline-go.md](data-pipeline-go.md) | [data-pipeline-r.md](data-pipeline-r.md) |

## Heat map

Not every combination is equally common in practice. The reference files reflect this — popular combinations have richer content; rare combinations are terse but complete.

- **Rich:** `analysis-workspace-python`, `analysis-workspace-r`, `research-corpus-python`, `data-pipeline-python`, `data-pipeline-go`.
- **Standard:** `analysis-workspace-nodejs`, `research-corpus-nodejs`, `research-corpus-r`, `data-pipeline-nodejs`.
- **Terse (rare combos):** `analysis-workspace-go`, `research-corpus-go`, `data-pipeline-r`.

If you find yourself frequently consulting a terse file, that's a signal to expand it.
