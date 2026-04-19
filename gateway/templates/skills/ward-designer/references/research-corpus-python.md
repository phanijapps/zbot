# research-corpus — Python

Use this reference for a Python ward that ingests documents, indexes them, retrieves passages, and produces summaries or answers. Representative domains: literature library, legal corpus, research papers, documentation knowledge.

**Example scenario:** A `literature-library` ward. Sub-domains: `smith-2024-summary` (summarize one paper), `agentic-systems-topic-map` (cross-corpus topic analysis), `popper-quotes` (author-attribution extraction). Shared primitives for ingestion, indexing, and retrieval.

---

## AGENTS.md

````markdown
# Literature Library

A Python ward for ingesting, indexing, and querying a document corpus — PDFs, EPUBs, text, HTML. Planners arriving here want to read, search, summarize, or extract from documents.

## Scope

**In scope**
- Ingest documents (PDF, EPUB, TXT, HTML) into a searchable corpus
- Retrieve passages by keyword, semantic similarity, or author/topic filters
- Summarize individual documents or topic clusters
- Extract quotes, citations, and key arguments with source pointers

**Out of scope**
- Authoring new documents → use `long-form-writing`
- Image / figure extraction as standalone artifacts → use `image-corpus`
- Translation → use `translation` with this ward's passages as input
- Audio / video transcription → use `media-transcription`

## Conventions

- **Package layout:** `literature_library/` installable package. `pip install -e .` for dev.
- **Import syntax:** `from literature_library.<module> import <symbol>`.
- **File boundaries:** One module per ingestion format (`ingest/pdf.py`, `ingest/epub.py`); one per retrieval strategy (`retrieval/keyword.py`, `retrieval/semantic.py`).
- **Document IDs:** `doc_id = f"{slugify(title)}_{sha1(raw_bytes)[:8]}"`. Stable across re-ingests.
- **Passage shape:** Every passage is a dict/dataclass with `doc_id, page, paragraph, text, char_span`.
- **Storage:** Parsed text under `corpus/<doc_id>/text.md`. Metadata under `corpus/<doc_id>/meta.json`. Embeddings under `index/<index_name>/`.
- **Tests:** `pytest`. Fixtures in `tests/fixtures/` — tiny representative PDFs/EPUBs.

## Report staging (every sub-domain)

Every sub-domain has parallel directories:

- `specs/<sub-domain>/` — spec + step files.
- `reports/<sub-domain>/` — output artifacts.

Inside every `reports/<sub-domain>/`:

- **`summary.md`** — findings, summary memo, or extracted content. Every claim cites back to `doc_id:page:paragraph`.
- **`manifest.json`** — artifact listing, including which corpus documents were consulted:
  ```json
  {
    "sub_domain": "agentic-systems-topic-map",
    "produced_at": "2026-04-18",
    "corpus_docs_consulted": ["smith_2024_a7f3c2d9", "jones_2023_9e1b5d4f"],
    "files": [
      {"path": "topic_map.json",       "purpose": "Topic clusters with doc_ids per cluster"},
      {"path": "topic_map.png",        "purpose": "Visual topic map"},
      {"path": "representative_quotes.md", "purpose": "One quote per topic with source"}
    ]
  }
  ```

## Sub-domain naming

Kebab-case. Examples: `smith-2024-summary`, `agentic-systems-topic-map`, `popper-quotes`.

## Cross-referencing sub-domains

A sub-domain can cite another — e.g., a topic map built on top of prior per-paper summaries:

    ## Related sub-domains
    - `../smith-2024-summary/summary.md` — one of the summaries fed into this topic map
    - `../jones-2023-summary/summary.md`

## DOs

- Preserve page and paragraph offsets for every passage — citations depend on them.
- Cache parsed text under `corpus/<doc_id>/text.md` on first ingest. Re-ingesting the same doc_id is a no-op.
- Every summary or finding in a sub-domain report cites back to `doc_id:page:paragraph`.
- Use `sha1(raw_bytes)[:8]` for doc_id uniqueness across title collisions.

## DON'Ts

- Do not re-parse a document without first checking by `doc_id`.
- Do not produce a summary without inline citations.
- Do not drop passage offsets when ingesting — they are the grounding for every later claim.
- Do not store large binaries in the corpus dir beyond the parsed text and metadata — originals live outside the ward.

## How to use this ward

Before new work:
1. Check `memory-bank/ward.md` sub-domains table.
2. Check `memory-bank/structure.md` for existing ingest/retrieval modules.
3. Check `memory-bank/core_docs.md` for registered primitives.
````

---

## memory-bank/ward.md

````markdown
# Ward: literature-library

## Purpose
Ingest, index, and query a document corpus. Produce grounded summaries, topic maps, quote extractions — all with inline citations back to source passages.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `smith-2024-summary` | Summarize one paper, extract main argument + citations | — |
| `agentic-systems-topic-map` | Build topic clusters across the corpus on "agentic systems" | Per-paper summaries |
| `popper-quotes` | Extract every quote attributed to Karl Popper, with source | — |

## Sub-domains this ward does NOT support

- Machine translation → use `translation`
- New prose generation → use `long-form-writing`
- Figure / table extraction as standalone → use `image-corpus`

## Key concepts

- **doc_id** — stable identifier: `slugify(title) + "_" + sha1(bytes)[:8]`.
- **Passage** — a `(doc_id, page, paragraph, text, char_span)` tuple.
- **Citation** — `doc_id:page:paragraph` pointer string used in summaries.
- **Embedding index** — vector index over passages; rebuilt on corpus change.

## Dependencies

- **PyPI:** `pypdf`, `ebooklib`, `beautifulsoup4`, `sentence-transformers`, `chromadb` (or `faiss-cpu`), `pytest`.
- **External:** none (local embedding model).
- **Upstream wards:** none.
- **Downstream wards:** `research-dashboard` if it exists.

## Assumptions

- Documents are in English unless otherwise specified.
- Corpus fits on a single machine's disk (no distributed storage in v1).
````

---

## memory-bank/structure.md

````markdown
# Structure — literature-library

```
literature-library/
├── AGENTS.md
├── pyproject.toml
├── memory-bank/
├── literature_library/
│   ├── __init__.py
│   ├── ingest/
│   │   ├── __init__.py
│   │   ├── pdf.py                     # (planned) PDF → passages
│   │   ├── epub.py                    # (proposed)
│   │   ├── txt.py                     # (proposed)
│   │   └── html.py                    # (proposed)
│   ├── corpus/
│   │   └── store.py                   # (planned) doc_id → corpus/<id>/ persistence
│   ├── retrieval/
│   │   ├── keyword.py                 # (planned) keyword + regex search
│   │   └── semantic.py                # (planned) embedding-based search
│   ├── summarize/
│   │   ├── single_doc.py              # (planned) summarize one doc_id
│   │   └── topic_cluster.py           # (proposed) topic map across docs
│   └── extract/
│       └── quotes.py                  # (proposed) author-attributed quote extraction
├── corpus/                            # parsed documents (gitignored)
│   └── <doc_id>/
│       ├── text.md
│       └── meta.json
├── index/                             # vector index (gitignored)
├── tests/
├── specs/
│   ├── smith-2024-summary/            # (planned)
│   ├── agentic-systems-topic-map/     # (proposed)
│   └── popper-quotes/                 # (proposed)
└── reports/
    ├── smith-2024-summary/
    │   ├── summary.md                 # the paper summary
    │   └── manifest.json
    ├── agentic-systems-topic-map/
    │   ├── summary.md                 # topic map narrative
    │   ├── manifest.json
    │   ├── topic_map.json
    │   └── topic_map.png
    └── popper-quotes/
        ├── summary.md
        ├── manifest.json
        └── quotes.csv
```

## Extension policy

- New ingestion format → new file in `literature_library/ingest/`, register in `core_docs.md`.
- New retrieval strategy → new file in `literature_library/retrieval/`, register.
- New sub-domain → `specs/<slug>/` + `reports/<slug>/` with required files.

## Cross-cutting rules

- Every ingest module returns `Iterator[Passage]` where Passage has fields `doc_id, page, paragraph, text, char_span`.
- Every retrieval module returns a ranked `List[Passage]` with a `score` field.
- Citations in any summary use format `doc_id:page:paragraph`.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — literature-library

| Symbol | Module | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |

## Registration rule

Every reusable function appended here. Example:

| `ingest_pdf` | `literature_library.ingest.pdf` | `(path: Path) -> Iterator[Passage]` | Parse PDF into passages with page+para offsets | step_2 of smith-2024-summary |
````

---

## Language-specific notes

- **Embeddings:** `sentence-transformers` with a small local model (`all-MiniLM-L6-v2`) unless scale demands otherwise.
- **Vector store:** `chromadb` (dev-friendly, file-backed) or `faiss-cpu` (scale-friendly). Pick per sub-domain size.
- **PDF parsing:** `pypdf` for structure; fall back to `pdfplumber` for complex layouts.
- **Build:** `pip install -e .` from `pyproject.toml`.
- **Test:** `pytest`. Fixtures: tiny representative PDFs (~10 KB each) in `tests/fixtures/`.
- **Passage type:** dataclass or `pydantic.BaseModel`. Prefer `pydantic` if validation matters.
