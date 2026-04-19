# analysis-workspace — Python

Use this reference for a Python ward that pulls external data, computes derived metrics, and produces a written verdict. Representative domains: stock analysis, startup due-diligence, macro research, competitor landscape.

**Example scenario:** A `financial-analysis` ward serving multiple sub-domains: `googl-valuation` (DCF + comparable-company), `aapl-options-chain` (open interest + implied volatility analysis), `sector-semiconductor-readout` (top-10 ranking). Shared primitives for data fetching, valuation models, and options math.

---

## AGENTS.md

````markdown
# Financial Analysis

A Python ward for valuation, scenario modeling, options analytics, and comparative ranking of publicly-traded equities. Planners arriving here produce a written verdict (memo, readout, ranking) backed by pulled financial data and computed metrics.

## Scope

**In scope**
- Single-ticker valuation (DCF, comparable-company, sum-of-parts)
- Multi-ticker comparison and ranking
- Options-chain analytics tied to an underlying ticker
- Scenario / sensitivity analysis over fundamental drivers

**Out of scope**
- Portfolio optimization across many tickers → use `portfolio-analytics`
- Personal-finance planning → use `personal-finance`
- Fixed-income / bond analytics → use `fixed-income`
- Machine-learning price prediction → use `quant-ml`

## Conventions

- **Package layout:** `financial_analysis/` is the installable package (editable install via `pip install -e .`). Sub-packages match module directories.
- **Import syntax:** `from financial_analysis.<module> import <symbol>`. Never `from .<module>` inside notebooks.
- **File boundaries:** One file per primitive or per external data source. `fundamentals.py` is separate from `options.py`; both are separate from `valuation_dcf.py`.
- **Error handling:** Network failures retry up to 3 times with exponential backoff (0.5s, 1s, 2s) before raising `DataSourceError`. Log ticker + source on each retry.
- **Data paths:** Raw API responses cache to `cache/<source>/<ticker>/<YYYYMMDD>.json`. Computed metrics live in function return values, not files.
- **Dates:** Every returned record includes `as_of_date` (ISO-8601 string). Never rely on implicit "now".
- **Types:** Return values use `decimal.Decimal` for money, `datetime.date` for dates, `pandas.DataFrame` only at the edges (function boundaries), never internally.
- **Tests:** `pytest` with fixtures in `tests/fixtures/<source>/*.json`. Never hit live APIs from tests.

## Report staging (every sub-domain)

Every sub-domain routed through this ward has parallel directories:

- `specs/<sub-domain>/` — spec + step files from spec-builder + plan-composer.
- `reports/<sub-domain>/` — output artifacts from step execution.

Inside every `reports/<sub-domain>/`:

- **`summary.md`** — the human-readable memo / readout / ranking. Content adapts to the sub-domain's deliverable.
- **`manifest.json`** — artifact listing. Schema:
  ```json
  {
    "sub_domain": "googl-valuation",
    "produced_at": "2026-04-18",
    "files": [
      {"path": "dcf_model.csv",     "purpose": "DCF output rows — cashflow, discount rate, terminal value per year"},
      {"path": "peers.csv",         "purpose": "Peer set used for comparable-company valuation"},
      {"path": "sensitivity.png",   "purpose": "WACC × terminal-growth sensitivity heatmap"}
    ]
  }
  ```

All other files under `reports/<sub-domain>/` are emergent from step outputs.

## Sub-domain naming

Kebab-case: `^[a-z0-9]+(-[a-z0-9]+)*$`. Examples: `googl-valuation`, `aapl-options-chain`, `q3-semiconductor-readout`.

## Cross-referencing sub-domains

If a sub-domain cites another, list it in `summary.md` under `## Related sub-domains`:

    ## Related sub-domains
    - `../aapl-valuation/summary.md` — baseline fundamentals used in this chain analysis

## DOs

- Cache every external API response to `cache/` before parsing — retries should hit cache, not the network.
- Expose one public function per primitive file, typically named `compute(<explicit args>) -> <typed return>`.
- Register every primitive in `memory-bank/core_docs.md` the moment it exists.
- Use `Decimal` (not `float`) for money. `float` drift over multi-step calculations is a silent valuation bug.

## DON'Ts

- Do not hard-code ticker symbols outside `specs/<sub-domain>/`. Primitives take ticker as an argument.
- Do not pull data silently from production APIs inside tests — use `cache/` fixtures.
- Do not produce a verdict from a primitive. Verdicts are synthesis steps only.
- Do not bypass the cache. Every data-source client reads cache first, writes cache second, never both skipped.

## How to use this ward

For a new ask in this domain:

1. Does `memory-bank/ward.md`'s Sub-domains table match this ask?
2. Does `memory-bank/structure.md` show primitives that already do what we need?
3. Does `memory-bank/core_docs.md` register functions we can import?

If yes to all three: add a new `specs/<sub-domain>/` + `reports/<sub-domain>/` pair, extend primitives in place. If no to any: ask the orchestrator whether to extend the ward or create a new one.
````

---

## memory-bank/ward.md

````markdown
# Ward: financial-analysis

## Purpose
Produce defensible written analyses of public equities — valuations, comparisons, scenario models, options analytics — backed by pulled financial data and computed metrics.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `googl-valuation` | DCF + comparable-company valuation for Alphabet | — |
| `aapl-options-chain` | Open-interest and implied-volatility analysis of Apple's options chain | `aapl-valuation` (if it exists) |
| `sector-semiconductor-readout` | Top-10 US semiconductor names ranked on growth-adjusted valuation | — |

## Sub-domains this ward does NOT support

- Portfolio construction / asset allocation → use `portfolio-analytics`
- Personal financial planning → use `personal-finance`
- Bond / fixed-income analysis → use `fixed-income`
- ML-based price prediction → use `quant-ml`

## Key concepts

- **DCF** — Discounted cash flow valuation.
- **Comparable-company** — Multiples-based valuation using a peer set.
- **As-of date** — Every record carries the date its data was pulled.
- **Open interest** — Number of options contracts held by market participants at a strike.
- **IV surface** — Implied volatility by strike × expiration.

## Dependencies

- **External:** yfinance (price/fundamentals), financialmodelingprep (extended financials), cboe (options chains).
- **Upstream wards:** none.
- **Downstream wards:** `portfolio-analytics` may consume primitives from this ward.

## Assumptions

- Default is US equities unless the ask specifies otherwise.
- "Current" fundamentals means the latest reported quarter; older data requires explicit request.
````

---

## memory-bank/structure.md

````markdown
# Structure — financial-analysis

File layout. Status markers: `(exists)`, `(planned)` (current spec), `(proposed)` (future sub-domain likely).

```
financial-analysis/
├── AGENTS.md
├── pyproject.toml                          # (planned) package metadata + deps
├── memory-bank/
│   ├── ward.md
│   ├── structure.md
│   └── core_docs.md
├── financial_analysis/                     # installable package
│   ├── __init__.py
│   ├── data_sources/
│   │   ├── __init__.py
│   │   ├── yfinance_client.py              # (planned) wraps yfinance + cache + retry
│   │   ├── fmp_client.py                   # (proposed) financialmodelingprep wrapper
│   │   └── cboe_client.py                  # (proposed) options chain fetch
│   ├── fundamentals/
│   │   ├── __init__.py
│   │   ├── income_statement.py             # (planned) parse + normalize income stmt
│   │   ├── balance_sheet.py                # (proposed)
│   │   └── cash_flow.py                    # (proposed)
│   ├── valuation/
│   │   ├── __init__.py
│   │   ├── dcf.py                          # (planned) DCF primitive
│   │   └── multiples.py                    # (proposed) comparable-company
│   └── options/
│       ├── __init__.py
│       └── chain.py                        # (proposed) options-chain analytics
├── cache/                                  # raw API responses (gitignored)
├── tests/
│   ├── data_sources/
│   ├── fundamentals/
│   ├── valuation/
│   ├── options/
│   └── fixtures/
├── specs/
│   ├── googl-valuation/                    # (planned)
│   │   ├── spec.md
│   │   └── step_*.md
│   ├── aapl-options-chain/                 # (proposed)
│   │   ├── spec.md
│   │   └── step_*.md
│   └── sector-semiconductor-readout/       # (proposed)
│       ├── spec.md
│       └── step_*.md
└── reports/
    ├── googl-valuation/                    # (planned)
    │   ├── summary.md                      # valuation memo
    │   ├── manifest.json
    │   ├── dcf_model.csv
    │   ├── peers.csv
    │   └── sensitivity.png
    ├── aapl-options-chain/                 # (proposed)
    │   ├── summary.md
    │   ├── manifest.json
    │   └── ...
    └── sector-semiconductor-readout/       # (proposed)
        ├── summary.md
        └── manifest.json
```

## Extension policy

- **New primitives** land under `financial_analysis/<module>/`. Every new file gets a one-line responsibility here AND a row in `core_docs.md`.
- **New sub-domain** creates both `specs/<slug>/` and `reports/<slug>/` with the required `summary.md` + `manifest.json`. Both appear here with `(planned)` before step execution.
- **Primitives are shared** across sub-domains; reports are not. A primitive used only by one sub-domain belongs in that sub-domain's step outputs, not at the ward root.

## Cross-cutting rules

- Every public function in `financial_analysis.*` returns a typed value and takes explicit arguments.
- Every data-source client caches to `cache/<source>/<ticker>/<YYYYMMDD>.json` before parsing.
- Valuation functions use `decimal.Decimal` for money, never `float`.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — financial-analysis

Registered primitives available for import. Every new primitive a builder-agent creates must be appended here.

| Symbol | Module | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |

## Registration rule

When a builder-agent creates a reusable function, class, or constant, append a row. The orchestrator's reuse audit reads this file; primitives that aren't registered won't be reused.

Example of a fully-registered row (for reference):

| `fetch_income_statement` | `financial_analysis.data_sources.yfinance_client` | `(ticker: str, as_of: date) -> pd.DataFrame` | Pull income statement with caching | step_2 of googl-valuation |
````

---

## Language-specific notes

- **Package install:** `pip install -e .` for development. `pyproject.toml` at ward root declares `financial_analysis` as the package.
- **Build tool:** `hatchling` or `setuptools` — ward's choice, stated in `pyproject.toml`.
- **Test runner:** `pytest`. Fixtures in `tests/fixtures/<source>/`.
- **Type checker:** `mypy` optional but recommended. Return annotations on every public function.
- **Linter:** `ruff` at the ward root.
- **Cache format:** JSON. Raw responses serialized with `json.dumps(..., default=str, indent=2)` so dates round-trip cleanly.
- **Money type:** `decimal.Decimal` in internal math. Convert to `float` only at display boundaries.
