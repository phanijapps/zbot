# data-pipeline — Python

Use this reference for a Python ward that extracts from operational sources, transforms, and persists to warehouses or dashboards. Representative domains: customer analytics, ops metrics, billing pipelines, event ingest.

**Example scenario:** A `customer-analytics` ward. Sub-domains: `churn-dashboard` (weekly churn rate by cohort), `ltv-computation` (revenue-LTV model), `activation-funnel` (signup-to-first-value funnel). Shared extract/transform primitives; each sub-domain produces its own dashboard-ready parquet under `reports/<sub-domain>/`.

---

## AGENTS.md

````markdown
# Customer Analytics

A Python ward for fetching customer-event data from operational stores, transforming it, and persisting to dashboard-ready tables or files.

## Scope

**In scope**
- Pull events from Stripe / app databases / CRM / product analytics
- Compute derived metrics (LTV, retention cohorts, churn rate, funnel conversion)
- Persist to parquet or warehouse sinks
- Produce dashboard-ready views and materialized summaries

**Out of scope**
- Live user-facing real-time analytics → use a streaming pipeline ward
- Marketing-attribution modeling → use `attribution-modeling`
- Experiment design / A/B analysis → use `experimentation`

## Conventions

- **Package layout:** `customer_analytics/` installable package.
- **Import syntax:** `from customer_analytics.<module> import <symbol>`.
- **File boundaries:** One file per source system (`sources/stripe.py`), one per derived metric (`transforms/ltv.py`), one per sink (`sinks/parquet_writer.py`).
- **Idempotency:** Every pipeline step must be safely re-runnable with the same inputs producing the same outputs. Reruns overwrite, never append blindly.
- **Partitioning:** Outputs partition by `as_of_date` (daily grain unless the ask requires finer).
- **Schemas:** YAML schema specs in `schemas/` — one per source and sink table.
- **Error handling:** Fail fast on schema drift. Raise `SchemaError` when a source column is missing. Retries on network errors (`httpx.HTTPStatusError`) up to 3 with backoff.
- **Data paths:** Raw extracts land in `raw/<source>/<YYYY-MM-DD>/`. Transforms write to `processed/<metric>/<YYYY-MM-DD>/`. Sub-domain reports stage in `reports/<sub-domain>/`.
- **Tests:** `pytest`. Fixtures use small-scale sample data in `tests/fixtures/`.

## Report staging (every sub-domain)

Parallel directories:

- `specs/<sub-domain>/` — spec + step files.
- `reports/<sub-domain>/` — dashboard-ready outputs.

Inside every `reports/<sub-domain>/`:

- **`summary.md`** — readout memo: headline numbers, method note, data window, key caveats.
- **`manifest.json`** — artifact listing, including the input-data window:
  ```json
  {
    "sub_domain": "churn-dashboard",
    "produced_at": "2026-04-18",
    "data_window": {"from": "2026-01-01", "to": "2026-04-18"},
    "files": [
      {"path": "churn_by_cohort.parquet", "purpose": "Weekly churn rate per cohort"},
      {"path": "churn_trend.png",          "purpose": "Trend chart of overall churn"}
    ]
  }
  ```

## Sub-domain naming

Kebab-case. Examples: `churn-dashboard`, `ltv-computation`, `activation-funnel`.

## Cross-referencing sub-domains

Example cross-ref in `ltv-computation/summary.md`:

    ## Related sub-domains
    - `../churn-dashboard/summary.md` — cohort definitions used in LTV computation

## DOs

- Land raw extracts verbatim before transforming. One file per (source, date).
- Transform functions are pure: same input → same output. No hidden state.
- Every transform reads its schema from `schemas/` and fails on drift.
- Every sub-domain report's `manifest.json` records the data window explicitly.

## DON'Ts

- Never mutate raw extracts in place. Raw stays raw; transforms write new files.
- Never infer primary keys — sources must supply them; if missing, raise.
- Never `append` to a sink without a key to dedupe on. Prefer `overwrite` with partition keys.
- Never run transforms against live production reads — go through the raw extract layer.

## How to use this ward

Before new work:
1. Check `memory-bank/ward.md` sub-domains table.
2. Check `memory-bank/structure.md` for existing sources/transforms/sinks.
3. Check `memory-bank/core_docs.md` for registered transforms.
````

---

## memory-bank/ward.md

````markdown
# Ward: customer-analytics

## Purpose
Batch-extract customer-event data, compute derived metrics, and produce dashboard-ready outputs.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `churn-dashboard` | Weekly churn rate by cohort | — |
| `ltv-computation` | Customer lifetime value model | `churn-dashboard` |
| `activation-funnel` | Signup → first-value funnel conversion | — |

## Sub-domains this ward does NOT support

- Real-time user-facing analytics → streaming pipeline
- Marketing-attribution modeling → `attribution-modeling`
- A/B experiment analysis → `experimentation`

## Key concepts

- **Cohort** — users grouped by acquisition week.
- **Churn** — drop-off rate between consecutive cohort observations.
- **LTV** — cumulative revenue per cohort over time.
- **Schema drift** — a source column vanishing, renaming, or changing type.

## Dependencies

- **PyPI:** `pandas`, `pyarrow`, `httpx`, `stripe`, `pytest`.
- **External:** Stripe API, app-database (read replica), CRM API.
- **Upstream wards:** none.
- **Downstream wards:** BI / dashboard tools consume the parquet outputs.

## Assumptions

- Data grain is daily unless a sub-domain explicitly asks for finer.
- All dates in UTC.
````

---

## memory-bank/structure.md

````markdown
# Structure — customer-analytics

```
customer-analytics/
├── AGENTS.md
├── pyproject.toml
├── memory-bank/
├── customer_analytics/
│   ├── __init__.py
│   ├── sources/
│   │   ├── stripe.py                  # (planned) Stripe events extractor
│   │   ├── app_db.py                  # (planned) app-database extractor
│   │   └── crm.py                     # (proposed)
│   ├── transforms/
│   │   ├── churn.py                   # (planned) weekly churn rate per cohort
│   │   ├── ltv.py                     # (proposed)
│   │   └── funnel.py                  # (proposed)
│   ├── sinks/
│   │   ├── parquet_writer.py          # (planned)
│   │   └── warehouse_loader.py        # (proposed)
│   └── common/
│       ├── schema_validate.py         # (planned)
│       └── retry.py                   # (planned)
├── schemas/                           # YAML per table
│   ├── stripe_events.yaml             # (planned)
│   └── app_users.yaml                 # (planned)
├── raw/                               # raw extracts (gitignored)
├── processed/                         # transformed outputs (gitignored)
├── tests/
├── specs/
│   ├── churn-dashboard/               # (planned)
│   ├── ltv-computation/               # (proposed)
│   └── activation-funnel/             # (proposed)
└── reports/
    ├── churn-dashboard/
    │   ├── summary.md
    │   ├── manifest.json
    │   ├── churn_by_cohort.parquet
    │   └── churn_trend.png
    ├── ltv-computation/
    │   └── ...
    └── activation-funnel/
        └── ...
```

## Extension policy

- New source → new file under `sources/`, schema spec under `schemas/`, register in `core_docs.md`.
- New transform → new file under `transforms/`, register.
- New sub-domain → `specs/<slug>/` + `reports/<slug>/` with required files.

## Cross-cutting rules

- Every source returns a DataFrame partitioned by `as_of_date`.
- Every transform is idempotent: `f(inputs) -> outputs` deterministic.
- Every sink takes `(df, partition_key, mode='overwrite')` and writes to a partition path.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — customer-analytics

| Symbol | Module | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |

## Registration rule

Every reusable extractor, transform, or sink registers here:

| `extract_stripe_events` | `customer_analytics.sources.stripe` | `(from_date: date, to_date: date) -> pd.DataFrame` | Pull Stripe events into raw DataFrame | step_2 of churn-dashboard |
````

---

## Language-specific notes

- **Runtime:** Python 3.11+. `pyarrow` for parquet; `pandas` for in-memory transforms.
- **Scale escape hatch:** If a transform exceeds single-machine memory, switch to `polars` or `duckdb` without changing primitive signatures. The `pandas.DataFrame` return type is a public interface to preserve.
- **Build:** `pip install -e .`.
- **Test:** `pytest` with `pytest-fixtures` for the schema-drift test harness.
- **Secrets:** API keys from environment variables (`STRIPE_API_KEY`), never committed.
- **Orchestration:** This ward's primitives can be driven by Airflow / Prefect / a plain cron — the ward itself doesn't know the orchestrator.
