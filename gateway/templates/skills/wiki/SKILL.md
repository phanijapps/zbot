---
name: wiki
description: >
  Promote ward artifacts (books, articles, research, reports, images) into
  an Obsidian vault at the ward root, placing each folder under the right
  top-level vault folder. A pure layout mover — no frontmatter rewriting,
  no wikilink conversion, no graph regeneration. Producer skills
  (book-reader, article-reader, research-reader) emit vault-ready
  folders; this skill only files them.
metadata:
  version: "0.2.0"
---

# Wiki

Lay out finished ward content into the Obsidian vault. Producer skills
(book-reader, article-reader, research-reader) already emit vault-ready
folders — linked markdown, entity pages, frontmatter. `wiki` only decides
where each folder belongs in the vault tree and copies it there.

## Use when

- The user asks to "publish", "promote", "move to vault", "file the
  ward", or "build the wiki"
- A ward has finished producing content and the user wants it parked in
  the vault for durable access

## Vault location (v1 + future hook)

Compute the vault path **once** at the start of the run: `$WARD/Vault/`,
where `$WARD` is the current active ward root. Create it on demand. Do
not seed folders that will stay empty.

This resolver is the single extension point. A future revision can swap
the source (config, env var, user setting) without touching anything
downstream. All routing logic takes the resolved path as an input.

## Canonical vault layout

```
Vault/
├── 00_Inbox/              # unclassified items land here, never deleted
├── 10_Journal/
├── 20_Projects/<project>/ # agent-produced final reports
├── 30_Library/
│   ├── Books/<slug>/      # from books/<slug>/
│   └── Articles/<slug>/   # from articles/<slug>/
├── 40_Research/<topic>/   # from research/<topic>/
├── 50_Resources/
├── 60_Archive/
├── 70_Assets/
│   ├── Knowledge_Graphs/  # reserved for future DB exports; wiki does not write here
│   ├── Images/
│   └── Documents/
└── _zztemplates/          # untouched
```

## Workflow

### 1. Resolve the vault path and scan the ward

Walk the ward root. For each directory/file, consult the routing table
in `references/routing.md` to produce a `{source, dest, type, action}`
tuple. `action ∈ {copy, update, skip}`:

- `copy` — destination does not exist yet
- `update` — destination exists but source hash differs
- `skip` — destination exists and hashes match

### 2. Execute the plan

For each tuple, copy the source into the destination — file, directory,
and all children, as-is. Producer skills already wrote the frontmatter,
the wikilinks, and the entity pages; nothing in the content is rewritten
at promotion.

### 3. Report

Emit a summary: counts by content type (copied / updated / skipped), the
vault path, and any items that landed in `00_Inbox/` so the user can
sort them.

## Rules

1. **Copy, never move.** The ward stays replayable.
2. **Never rewrite content.** No frontmatter edits, no wikilink
   rewriting, no markdown reformatting. Producer skills own their output
   shape; `wiki` moves bytes.
3. **Unknown structure → `00_Inbox/`.** Never guess a category.
4. **Idempotent.** Rerunning on an unchanged ward reports all `skip`s
   and writes nothing.
5. **Vault path resolution lives in exactly one place** — Step 1. This
   is the future configurability hook.

## What this skill does NOT do

- Create new content
- Ingest into the main knowledge graph (producer skills do that inline)
- Regenerate an aggregate `kg.json` (the SQLite main KG is the aggregate)
- Rewrite any `.md` or `.json` being moved
- Touch `_zztemplates/` or any `memory-bank/`, `specs/`, `AGENTS.md`
- Delete from the ward
