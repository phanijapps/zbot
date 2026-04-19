# data-pipeline — Go

Use this reference for a Go ward that extracts from operational sources, transforms, and persists. Go's strengths: concurrency, binary deployment, low memory footprint, strong static typing. Favored for high-throughput or latency-sensitive pipelines.

**Example scenario:** An `ops-metrics` ward. Sub-domains: `service-latency` (p50/p95/p99 per service), `error-rate` (rolling error rate by deploy), `deploy-cadence` (releases per team per week).

---

## AGENTS.md

````markdown
# Ops Metrics

A Go ward that extracts operational telemetry, computes rolling SRE-style metrics, and persists to a timeseries store or parquet sink.

## Scope

**In scope**
- Pull telemetry from log aggregators (Loki, CloudWatch, Datadog)
- Compute percentile-based latency, error rate, throughput metrics
- Emit metrics to a timeseries store or parquet for BI
- Handle backfill and incremental windows

**Out of scope**
- Real-time alerting → use a dedicated alerting stack
- Distributed tracing UI → use tracing backends directly
- APM agent instrumentation → application-side concern

## Conventions

- **Module path:** `github.com/<org>/ops-metrics`. `go.mod` at ward root.
- **Package layout:** `cmd/<binary>/` for executables; `internal/<package>/` for shared code; `pkg/<package>/` for exported types (only if another ward consumes them).
- **Import syntax:** `github.com/<org>/ops-metrics/internal/<package>`.
- **File boundaries:** One file per source/transform/sink, as with Python — but also one test file per source file (`*_test.go`).
- **Error handling:** Return `error` explicitly. Wrap with `fmt.Errorf("context: %w", err)`. No panics in pipeline code.
- **Concurrency:** Use `errgroup.Group` for parallel source fetches. Cancel via `context.Context` on any error.
- **Data paths:** Raw extracts under `raw/<source>/<YYYY-MM-DD>/*.json.gz`. Processed under `processed/<metric>/<YYYY-MM-DD>/*.parquet`.
- **Tests:** `go test ./...`. Fixtures under `testdata/` (Go convention).

## Report staging (every sub-domain)

Parallel directories:

- `specs/<sub-domain>/` — spec + step files.
- `reports/<sub-domain>/` — dashboard-ready outputs.

Inside every `reports/<sub-domain>/`:

- **`summary.md`** — readout, written by a higher-level tool (Go produces data; the summary is markdown).
- **`manifest.json`** — artifact listing with the data window:
  ```json
  {
    "sub_domain": "service-latency",
    "produced_at": "2026-04-18",
    "data_window": {"from": "2026-04-11T00:00:00Z", "to": "2026-04-18T00:00:00Z"},
    "files": [
      {"path": "latency_p99.parquet", "purpose": "p99 latency per service per minute"},
      {"path": "latency_trend.png",    "purpose": "p95 trend chart across services"}
    ]
  }
  ```

## Sub-domain naming

Kebab-case. Examples: `service-latency`, `error-rate`, `deploy-cadence`.

## Cross-referencing sub-domains

```markdown
## Related sub-domains
- `../service-latency/summary.md` — source of the baseline latency distribution
```

## DOs

- Always pass `context.Context` as the first argument to any function that does I/O.
- Cancel on error via `errgroup`. Never let goroutines leak.
- Use `struct` types, not `map[string]interface{}`, for parsed telemetry records.
- Compile a single binary per sub-domain under `cmd/<sub-domain>/main.go` that orchestrates extract → transform → sink.

## DON'Ts

- Never use `panic()` for expected errors. Return them.
- Never discard `error` returns. Handle or wrap.
- Never fetch without a context timeout. Uncapped fetches hang pipelines.
- Never log-then-return silently. Return the error and let the caller decide.

## How to use this ward

Before new work:
1. Check `memory-bank/ward.md` sub-domains table.
2. Check `memory-bank/structure.md` for existing sources/transforms.
3. Check `memory-bank/core_docs.md` for registered packages.
````

---

## memory-bank/ward.md

````markdown
# Ward: ops-metrics

## Purpose
Compute SRE-style rolling metrics from operational telemetry and persist to timeseries sinks.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `service-latency` | p50/p95/p99 latency per service, per minute | — |
| `error-rate` | Rolling error rate tagged by deploy version | `deploy-cadence` |
| `deploy-cadence` | Deploys per team per week, merge lead time | — |

## Sub-domains this ward does NOT support

- Real-time alerting → dedicated alerting stacks
- Distributed tracing UI → tracing backends
- User-facing analytics → customer-analytics ward

## Key concepts

- **Percentile aggregation** — latency summarized as p50/p95/p99 per (service, minute).
- **Rolling window** — metrics computed over sliding N-minute or N-hour windows.
- **Backfill** — recomputing historic windows after schema change.

## Dependencies

- **Go modules:** `github.com/apache/arrow/go` (parquet), `github.com/prometheus/client_model`, `golang.org/x/sync/errgroup`, `github.com/stretchr/testify`.
- **External:** Loki / CloudWatch / Datadog APIs.
- **Upstream wards:** none.
- **Downstream wards:** BI dashboards consume the parquet outputs.

## Assumptions

- Go 1.22+ required.
- All timestamps UTC.
````

---

## memory-bank/structure.md

````markdown
# Structure — ops-metrics (Go)

```
ops-metrics/
├── AGENTS.md
├── go.mod                                  # (planned) module declaration
├── go.sum
├── memory-bank/
├── cmd/
│   ├── service-latency/                    # (planned) binary for this sub-domain
│   │   └── main.go
│   ├── error-rate/                         # (proposed)
│   │   └── main.go
│   └── deploy-cadence/                     # (proposed)
│       └── main.go
├── internal/
│   ├── sources/
│   │   ├── loki.go                         # (planned) Loki log fetch
│   │   ├── cloudwatch.go                   # (proposed)
│   │   └── datadog.go                      # (proposed)
│   ├── transforms/
│   │   ├── percentile.go                   # (planned) p50/p95/p99 compute
│   │   ├── rolling.go                      # (planned) sliding-window aggregator
│   │   └── deploy_join.go                  # (proposed) join metrics to deploys
│   └── sinks/
│       └── parquet.go                      # (planned) write parquet output
├── testdata/                               # Go test fixtures
├── raw/                                    # raw extracts (gitignored)
├── processed/                              # transformed (gitignored)
├── specs/
│   ├── service-latency/                    # (planned)
│   ├── error-rate/                         # (proposed)
│   └── deploy-cadence/                     # (proposed)
└── reports/
    ├── service-latency/
    │   ├── summary.md
    │   ├── manifest.json
    │   ├── latency_p99.parquet
    │   └── latency_trend.png
    ├── error-rate/
    └── deploy-cadence/
```

## Extension policy

- New source → file under `internal/sources/`, test alongside (`*_test.go`), register in `core_docs.md`.
- New transform → file under `internal/transforms/`, register.
- New sub-domain → new `cmd/<slug>/main.go` binary + `specs/<slug>/` + `reports/<slug>/`.

## Cross-cutting rules

- Every public function takes `context.Context` as first arg.
- Every `main.go` under `cmd/` follows pattern: parse flags → build sources/transforms/sinks → wire with `errgroup` → emit `manifest.json`.
- Errors wrap with `fmt.Errorf("source %q: %w", source, err)`.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — ops-metrics (Go)

| Symbol | Package | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |

## Registration rule

Every exported Go symbol registers. Example:

| `FetchLokiQuery` | `internal/sources/loki` | `(ctx context.Context, q string, from, to time.Time) ([]LogRecord, error)` | Stream Loki logs for a window | step_2 of service-latency |
````

---

## Language-specific notes

- **Build:** `go build ./cmd/<sub-domain>/` produces a single binary per sub-domain.
- **Test:** `go test ./...` at ward root. `go test -race` for concurrency checks.
- **Lint:** `golangci-lint run` at ward root with `.golangci.yml` config.
- **Format:** `gofmt -s` is the only style debate that's over.
- **Parquet:** `github.com/apache/arrow/go/v14/parquet` is the canonical writer; `xitongsys/parquet-go` is an alternative.
- **HTTP clients:** Standard library `net/http` with a `Client` that has a timeout. Every request includes `context.Context`.
- **Concurrency:** `errgroup.Group` + `context.Context` cancels on first error; use this for parallel source fetches.
- **Binaries vs. libraries:** Each sub-domain gets its own `cmd/<slug>/main.go`. Shared code under `internal/`. Do not reuse a single binary for many sub-domains — Go's static linking is cheap; binaries are disposable.
