# research-corpus — Go

Use this reference for a Go research-corpus ward. Rare combination — Python dominates corpus ingestion and indexing because of `pypdf`, `sentence-transformers`, and vector-store clients. Go is appropriate only when (a) corpus ingestion is a high-throughput service, or (b) the ward integrates into an existing Go infrastructure.

**When to use:** high-volume corpus ingest (millions of docs/day), search embedded in a Go service.

**When NOT to use:** small-to-medium corpora, embedding-heavy work, any ward where domain experts write the code.

---

## AGENTS.md

````markdown
# <Ward Name>

A Go ward for corpus ingestion and indexing. Justify the Go choice in `memory-bank/ward.md`.

## Scope

Narrow to high-throughput ingest or search embedded in a Go service.

## Conventions

- Module path: `github.com/<org>/<ward-name>`.
- Package layout: `cmd/` for binaries, `internal/` for shared code.
- Passage shape: `type Passage struct { DocID string; Page, Paragraph int; Text string; CharSpan [2]int }`.
- Error handling: explicit `error` returns with `fmt.Errorf("...: %w", err)`.
- Tests: `go test ./...` with `testdata/` fixtures.

## Report staging

Standard `specs/<sub-domain>/` + `reports/<sub-domain>/` with `summary.md` + `manifest.json`.

The Go ward produces indexed corpus + query results as data. Summaries come from a downstream tool.

## Sub-domain naming

Kebab-case.

## DOs

- Use `context.Context` on every I/O call.
- Stream documents rather than loading into memory when possible (`io.Reader` pipelines).
- Every passage preserves `DocID, Page, Paragraph, CharSpan`.

## DON'Ts

- Do not embed a vector store — connect to one as a service (Chroma, Qdrant).
- Do not write narrative summaries in Go; emit data, let downstream narrate.
- Do not load a whole corpus into memory — stream.
````

---

## memory-bank/ward.md

````markdown
# Ward: <ward-name>

## Purpose
Justify the Go choice — throughput, embedding in a service, or both.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `<current-slug>` | <description> | — |
| `<future-slug-1>` | <description> | — |
| `<future-slug-2>` | <description> | — |

## Dependencies

- **Go modules:** `github.com/pdfcpu/pdfcpu` (PDF), `golang.org/x/sync/errgroup`. Vector store via HTTP client (Chroma, Qdrant).

## Assumptions

- Go 1.22+.
- Corpus originals stored externally; this ward indexes references, not full copies.
````

---

## memory-bank/structure.md

````markdown
# Structure — <ward-name> (Go)

```
<ward-name>/
├── AGENTS.md
├── go.mod
├── memory-bank/
├── cmd/
│   ├── ingest/main.go                  # (planned) streaming ingest binary
│   └── query/main.go                   # (planned) retrieval binary
├── internal/
│   ├── ingest/
│   │   ├── pdf.go
│   │   ├── txt.go
│   │   └── passage.go                  # Passage struct
│   ├── store/
│   │   └── vector_client.go            # HTTP client to vector store
│   └── retrieval/
│       └── semantic.go
├── testdata/
├── specs/<sub-domain>/
└── reports/<sub-domain>/
```

## Cross-cutting rules

- Every ingest function returns `<-chan Passage` or streams via callback.
- Every retrieval returns `[]Passage` ranked by score.
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

- **PDF:** `pdfcpu` is the most maintained pure-Go PDF library; `pdf` is lighter but limited.
- **Vector store:** do NOT embed Chroma or Qdrant in Go. Run them as services; use their HTTP/gRPC clients.
- **Embeddings:** Go has no native high-quality embedding runtime. Call an embedding service (Ollama, a Python microservice) over HTTP.
- **If you find yourself fighting missing libraries:** stop. Prototype in Python and reconsider whether this ward should actually be Go.
- **This reference is terse on purpose.** Expand when real use reveals what's missing.
