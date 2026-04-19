# analysis-workspace — Node.js (TypeScript)

Use this reference for a TypeScript analysis ward. Less common than Python or R for financial / research analytics, but a good fit when the ward is tightly integrated with a web front-end or when type-safety matters across many named record shapes.

**Example scenario:** A `saas-metrics-analysis` ward that pulls data from internal APIs and renders comparative readouts. Sub-domains: `arr-quarterly-readout` (quarterly ARR comparison), `retention-cohort-breakdown` (cohort retention analysis).

---

## AGENTS.md

````markdown
# SaaS Metrics Analysis

A TypeScript ward for SaaS metrics comparison, cohort analysis, and ARR readouts. Planners produce written comparative reports backed by typed fetched data and computed metrics.

## Scope

**In scope**
- ARR / MRR / net-retention computation from subscription data
- Cohort retention and churn analysis
- Comparative quarterly readouts across product lines

**Out of scope**
- Portfolio / investment analytics → equity analysis ward
- User-facing live dashboards → front-end ward
- Machine learning forecasting → quant-ml

## Conventions

- **Package layout:** npm workspaces. `package.json` at ward root with `"type": "module"`.
- **Import syntax:** `import { <symbol> } from '@<org>/saas-metrics-analysis/<module>'`. Path aliases via `tsconfig.json` `paths`.
- **File boundaries:** One file per primitive. `.ts` files in `src/<module>/`. Tests in `src/<module>/<file>.test.ts` (Vitest convention).
- **Error handling:** Throw typed errors (`class DataSourceError extends Error`). Catch at orchestration boundaries. Network retries via a `retry()` helper up to 3 attempts, exponential backoff.
- **Dates:** Always `Date` or ISO-8601 strings, never `number`. `date-fns` for arithmetic.
- **Money:** `string` representation to avoid float drift; convert to `number` only at display boundaries.
- **Tests:** Vitest. Fixtures in `src/**/__fixtures__/`.

## Report staging (every sub-domain)

Parallel directories:

- `specs/<sub-domain>/` — spec + step files.
- `reports/<sub-domain>/` — outputs.

Inside every `reports/<sub-domain>/`:

- **`summary.md`** — the readout memo.
- **`manifest.json`** — artifact listing.

## Sub-domain naming

Kebab-case.

## Cross-referencing sub-domains

`summary.md` lists related sub-domains under `## Related sub-domains`.

## DOs

- Use `zod` schemas to validate every API response at the boundary.
- Cache external responses as JSON under `cache/`.
- Prefer named exports; avoid default exports.

## DON'Ts

- Do not use `any`. Use `unknown` and narrow.
- Do not store money as `number` in internal math.
- Do not bundle for production — this ward is an analytics workspace, not a web app.

## How to use this ward

Before new work: check ward.md → structure.md → core_docs.md as usual.
````

---

## memory-bank/ward.md

````markdown
# Ward: saas-metrics-analysis

## Purpose
Produce typed, defensible SaaS-metrics readouts — ARR, retention, cohort analysis.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `arr-quarterly-readout` | Quarterly ARR comparison across product lines | — |
| `retention-cohort-breakdown` | Cohort retention analysis with breakdowns by plan tier | — |
| `upsell-opportunity-ranking` | Rank accounts by upsell potential | — |

## Sub-domains this ward does NOT support

- Investment analysis → equity-analysis ward
- Live user-facing dashboards → front-end ward

## Key concepts

- **ARR** — Annual Recurring Revenue normalized to an annual figure.
- **Net retention** — Revenue retention including expansion, excluding new sales.
- **Cohort** — Customers acquired in the same period.

## Dependencies

- **npm:** `zod`, `date-fns`, `vitest`, `typescript`.
- **External APIs:** subscription store, product DB.

## Assumptions

- TypeScript strict mode on.
- Node.js >= 20.
````

---

## memory-bank/structure.md

````markdown
# Structure — saas-metrics-analysis

```
saas-metrics-analysis/
├── AGENTS.md
├── package.json
├── tsconfig.json
├── vitest.config.ts
├── memory-bank/
├── src/
│   ├── sources/
│   │   ├── subscription_db.ts          # (planned)
│   │   └── product_events.ts           # (proposed)
│   ├── metrics/
│   │   ├── arr.ts                      # (planned) ARR compute
│   │   ├── retention.ts                # (planned) cohort retention
│   │   └── upsell_score.ts             # (proposed)
│   ├── schemas/
│   │   └── subscription.ts             # (planned) zod schemas
│   └── utils/
│       ├── cache.ts
│       └── retry.ts
├── cache/
├── specs/
│   ├── arr-quarterly-readout/          # (planned)
│   └── retention-cohort-breakdown/     # (proposed)
└── reports/
    ├── arr-quarterly-readout/
    │   ├── summary.md
    │   ├── manifest.json
    │   └── arr_by_product.csv
    └── retention-cohort-breakdown/
        └── ...
```

## Extension policy

- New source → `src/sources/<name>.ts` with a `zod` schema in `src/schemas/`.
- New metric → `src/metrics/<name>.ts`, register.
- New sub-domain → `specs/<slug>/` + `reports/<slug>/`.

## Cross-cutting rules

- Every source module validates its response with a `zod` schema before returning.
- Every metric function returns a typed record array, not `any[]`.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — saas-metrics-analysis

| Symbol | Module | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |
````

---

## Language-specific notes

- **Build:** `tsc -b` for type-check; `tsx` for ad-hoc execution.
- **Test:** `vitest run`. Co-located `*.test.ts` files.
- **Lint:** `eslint` with `@typescript-eslint` strict config.
- **Strict mode:** `"strict": true` in `tsconfig.json`. Also enable `noUncheckedIndexedAccess`.
- **Cache format:** JSON. Use `JSON.stringify(obj, null, 2)` for writes.
- **Module resolution:** `"module": "NodeNext"` in `tsconfig.json` for native ESM.
