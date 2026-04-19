# analysis-workspace — Go

Use this reference for a Go analysis ward. Uncommon combination — Python and R dominate analysis work because of their ecosystems. Go is appropriate only when (a) the analysis is latency-sensitive and embedded in a service, or (b) the reports feed a Go-native downstream system. For most analysis tasks, prefer Python or R.

**When to use:** ticker-by-ticker metric computation embedded in a trading service; real-time market-hours analytics; heavy-concurrency screening over thousands of tickers.

**When NOT to use:** narrative reports, exploratory analysis, anything needing rich plotting.

---

## AGENTS.md

````markdown
# <Ward Name>

A Go ward for analysis work. Justify the choice of Go in `memory-bank/ward.md` — if the reason is not latency or service embedding, reconsider Python.

## Scope

**In scope** — narrow to latency-sensitive or service-embedded analysis.
**Out of scope** — narrative reports, exploratory work, rich plotting.

## Conventions

- Module path: `github.com/<org>/<ward-name>`.
- Package layout: `cmd/<binary>/` for executables, `internal/<package>/` for shared code.
- Error handling: explicit `error` returns, wrapped via `fmt.Errorf("context: %w", err)`.
- Concurrency: `errgroup.Group` + `context.Context`.
- Money: `github.com/shopspring/decimal` — no floats in internal math.
- Tests: `go test ./...`, fixtures in `testdata/`.

## Report staging

Standard `specs/<sub-domain>/` + `reports/<sub-domain>/` with `summary.md` + `manifest.json`.

Summary narrative is usually produced by a higher-level tool; Go's role is to produce data files that feed the summary.

## Sub-domain naming

Kebab-case.

## DOs

- Always pass `context.Context`.
- Use `decimal.Decimal` for money.
- Keep binaries per sub-domain under `cmd/<slug>/main.go`.

## DON'Ts

- Do not use floats for money.
- Do not panic in pipeline code.
- Do not write narrative reports in Go — produce data, let another tool narrate.
````

---

## memory-bank/ward.md

````markdown
# Ward: <ward-name>

## Purpose
Justify the Go choice here (latency, embedding, concurrency).

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `<current-slug>` | <description> | — |
| `<future-slug-1>` | <description> | — |
| `<future-slug-2>` | <description> | — |

## Sub-domains this ward does NOT support

- Narrative reports → use a Python or R analysis ward

## Dependencies

- **Go modules:** `github.com/shopspring/decimal`, `golang.org/x/sync/errgroup`, `github.com/stretchr/testify`.

## Assumptions

- Go 1.22+.
````

---

## memory-bank/structure.md

````markdown
# Structure — <ward-name> (Go)

```
<ward-name>/
├── AGENTS.md
├── go.mod
├── go.sum
├── memory-bank/
├── cmd/
│   └── <sub-domain>/main.go           # (planned) binary per sub-domain
├── internal/
│   ├── sources/
│   └── compute/
├── testdata/
├── specs/<sub-domain>/
└── reports/<sub-domain>/
    ├── summary.md
    ├── manifest.json
    └── <data files>
```

## Cross-cutting rules

- Every function takes `context.Context`.
- Errors wrapped with source context.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — <ward-name>

| Symbol | Package | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |
````

---

## Language-specific notes

- **Build:** `go build ./cmd/...`.
- **Test:** `go test ./...`; `-race` for concurrency checks.
- **If you find yourself writing narrative in Go:** stop. Emit the data files, let a downstream tool (Python + Jinja, or a markdown-rendering service) produce the narrative.
- **This reference is terse on purpose.** If you are designing this combination frequently, expand this file with domain-specific conventions.
