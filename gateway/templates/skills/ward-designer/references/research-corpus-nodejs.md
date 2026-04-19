# research-corpus — Node.js (TypeScript)

Use this reference for a TypeScript ward that ingests, indexes, and queries a document corpus. Reasonable choice when embedding generation happens in Node (via `@huggingface/transformers`) or when the ward integrates with an existing Node stack.

**Example scenario:** A `docs-knowledge` ward for indexing internal engineering docs (markdown + HTML). Sub-domains: `api-reference-summary` (summarize API docs), `architecture-qa` (question-answering over architecture pages).

---

## AGENTS.md

````markdown
# Docs Knowledge

A TypeScript ward for ingesting engineering documentation, building a semantic index, and answering queries against it with passage-level citations.

## Scope

**In scope**
- Ingest markdown, HTML, and plain text into a searchable corpus
- Keyword and semantic retrieval with source passage pointers
- Summarization and Q&A over the corpus with inline citations

**Out of scope**
- PDF-heavy corpora → use a Python ward (better PDF tooling)
- Multi-language corpora → translation is upstream
- Image or figure extraction → image-corpus ward

## Conventions

- **Package layout:** npm workspace. `"type": "module"`.
- **Import syntax:** `import { <symbol> } from '@<org>/docs-knowledge/<module>'`.
- **File boundaries:** Module per ingestion format; module per retrieval strategy.
- **Document IDs:** `doc_id = slugify(title) + "_" + sha1(bytes).slice(0, 8)`.
- **Passage shape:** `{ doc_id: string; section: string; paragraph: number; text: string; char_span: [number, number] }`.
- **Error handling:** Typed errors extending `Error`. Use `neverthrow` or plain throws — pick one and be consistent.
- **Tests:** Vitest. Fixtures in `src/**/__fixtures__/`.

## Report staging

Parallel directories `specs/<sub-domain>/` and `reports/<sub-domain>/` with required `summary.md` + `manifest.json`.

## Sub-domain naming

Kebab-case. Examples: `api-reference-summary`, `architecture-qa`.

## Cross-referencing sub-domains

`summary.md` lists related sub-domains under `## Related sub-domains`.

## DOs

- Validate parsed metadata with `zod` before returning.
- Store parsed text under `corpus/<doc_id>/text.md` and metadata under `corpus/<doc_id>/meta.json`.
- Every summary or answer in a sub-domain report cites back to `doc_id:section:paragraph`.

## DON'Ts

- Do not re-parse a document without checking by `doc_id`.
- Do not produce an answer without inline citations.
- Do not drop section/paragraph offsets during ingest.
- Do not use `any` — use `unknown` and narrow.

## How to use this ward

Standard flow: check ward.md → structure.md → core_docs.md before writing new code.
````

---

## memory-bank/ward.md

````markdown
# Ward: docs-knowledge

## Purpose
Index internal engineering docs and answer questions against them with passage-level citations.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `api-reference-summary` | Summarize each API endpoint's contract | — |
| `architecture-qa` | Answer questions about system architecture with citations | — |
| `runbook-index` | Build a searchable index of incident runbooks | — |

## Sub-domains this ward does NOT support

- PDF-heavy corpora → Python literature-library
- Multi-language corpora → translation upstream

## Key concepts

- **doc_id** — stable slug + sha1 suffix.
- **Passage** — `(doc_id, section, paragraph)` with text + char span.
- **Citation** — string `doc_id:section:paragraph`.

## Dependencies

- **npm:** `unified`, `remark-parse`, `rehype-parse`, `@huggingface/transformers` (embeddings), `chromadb`, `zod`, `vitest`.
- **External:** none (local embeddings).

## Assumptions

- Node.js >= 20 (for `@huggingface/transformers`).
- English-only corpus in v1.
````

---

## memory-bank/structure.md

````markdown
# Structure — docs-knowledge

```
docs-knowledge/
├── AGENTS.md
├── package.json
├── tsconfig.json
├── memory-bank/
├── src/
│   ├── ingest/
│   │   ├── markdown.ts                 # (planned)
│   │   ├── html.ts                     # (proposed)
│   │   └── txt.ts                      # (proposed)
│   ├── corpus/
│   │   └── store.ts                    # (planned)
│   ├── retrieval/
│   │   ├── keyword.ts                  # (planned)
│   │   └── semantic.ts                 # (planned)
│   ├── summarize/
│   │   └── qa.ts                       # (proposed)
│   └── schemas/
│       └── passage.ts                  # (planned) zod schema
├── corpus/
├── index/
├── specs/
│   ├── api-reference-summary/          # (planned)
│   └── architecture-qa/                # (proposed)
└── reports/
    ├── api-reference-summary/
    └── architecture-qa/
```

## Extension policy

- New ingestion format → new file under `src/ingest/`.
- New retrieval strategy → under `src/retrieval/`.
- New sub-domain → paired `specs/<slug>/` + `reports/<slug>/`.

## Cross-cutting rules

- Every ingest module returns `AsyncIterable<Passage>`.
- Every retrieval module returns ranked `Passage[]` with a `score: number` field.
- Citations use format `doc_id:section:paragraph`.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — docs-knowledge

| Symbol | Module | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |
````

---

## Language-specific notes

- **Build:** `tsc -b`.
- **Test:** `vitest run`.
- **Embeddings:** `@huggingface/transformers` with a small model (`Xenova/all-MiniLM-L6-v2`) for dev-friendly local inference.
- **Vector store:** `chromadb` client (connects to a local or hosted Chroma) — the ward does NOT embed the Chroma server.
- **Markdown parsing:** `unified` + `remark-parse`. Preserve heading structure to derive passage sections.
- **HTML parsing:** `unified` + `rehype-parse` → `rehype-remark` to normalize to markdown first, then ingest.
- **Passage type:** Exported from `src/schemas/passage.ts` as a `zod` schema + inferred TypeScript type.
