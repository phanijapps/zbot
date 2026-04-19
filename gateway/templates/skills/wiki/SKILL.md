---
name: wiki
description: >
  Promote ward artifacts (books, articles, research, reports, images) into
  the Obsidian vault ward, placing each producer-folder under the right
  top-level vault folder. A pure cross-ward layout mover — no frontmatter
  rewriting, no wikilink conversion, no graph regeneration. Producer
  skills (book-reader, stock-analysis, news-research, …) emit
  vault-ready folders; this skill files them.
metadata:
  version: "0.3.0"
---

# Wiki

Lay out finished ward content into the Obsidian vault ward. Producer
skills already emit vault-ready folders — linked markdown, entity pages,
frontmatter. `wiki` only decides where each folder belongs in the vault
tree and copies it there.

## Use when

- The user asks to "publish", "promote", "move to vault", "file the
  ward", or "build the wiki"
- A plan's final-promotion step runs
- A ward has finished producing content and the user wants it parked in
  the vault for durable access

## Vault ward resolution

The vault is a **dedicated ward**, not a subdirectory of the source
ward. The name is configurable via
`settings.json → execution.wiki.wardName` (default `"wiki"`) and the
ward is pre-created at gateway startup.

Resolution order (stop at first hit):

1. **Marker scan** — call `ward(action="list")`. The vault ward's
   `AGENTS.md` starts with the marker line `<!-- obsidian-vault -->`.
   Pick that ward. This survives config renames.
2. **Default name** — if marker scan returns nothing, use the ward
   named `wiki`.
3. **Abort** — if neither resolves, report "no vault ward" and stop.
   Do not create a new ward; subagents are not permitted to create
   wards, and missing-at-runtime is a configuration error.

Record the resolved name once at the start of the run and reuse it
throughout.

## Canonical vault layout

```
<wiki-ward>/
├── 00_Inbox/              # unclassified items; never deleted
├── 10_Journal/{Daily,Weekly}/
├── 20_Projects/<project>/ # agent-produced final reports
├── 30_Library/
│   ├── Books/<slug>/      # from books/<slug>/ in the origin ward
│   └── Articles/<slug>/   # from articles/<slug>/
├── 40_Research/<archetype>/<subject>/<date-slug>/
├── 50_Resources/
├── 60_Archive/
├── 70_Assets/
│   ├── Knowledge_Graphs/  # reserved for DB exports; wiki does not write here
│   ├── Images/
│   └── Documents/
└── _zztemplates/          # untouched
```

## Cross-ward copy — no ward-switch required

Run the copy via `shell` with absolute paths. You are executing in the
origin ward; the vault ward sits alongside it under `wards/`. Build the
paths once:

```
SRC=$(pwd)                              # origin ward root
WARDS=$(dirname "$SRC")                 # wards root
DEST="$WARDS/<wiki-ward-name>"          # vault ward root
```

`<wiki-ward-name>` is the resolved name from the resolution step.

Do NOT switch wards with `ward(action="use")`. Switching changes the
working directory mid-run and loses the origin-ward reference.

## Workflow

### 1. Resolve the vault ward

Discover the vault ward name (marker scan → default `wiki`). Abort if
not found.

### 2. Enumerate candidates

Walk the origin ward root. Match each path against the routing table
in [`references/routing.md`](references/routing.md). Produce a list of
`{source_abs, dest_abs, type, action}` tuples where
`action ∈ {copy, update, skip}`:

- `copy` — destination does not exist yet
- `update` — destination exists and source hash differs (vault is stale)
- `skip` — destination exists and hashes match

### 3. Execute the plan

For each tuple, copy source → destination with `cp -a` (or equivalent)
preserving timestamps. Whole producer folders transfer as one unit —
the producer skill already wrote the frontmatter, wikilinks, and entity
pages; nothing in the content is rewritten at promotion.

### 4. Report

Emit a summary: counts by content type (copied / updated / skipped), the
resolved vault ward name, and any items that landed in `00_Inbox/` so
the user can sort them.

## Rules

1. **Cross-ward copy, never move.** The origin ward stays replayable.
2. **Never rewrite content.** No frontmatter edits, no wikilink
   rewriting, no markdown reformatting. Producer skills own their output
   shape; `wiki` moves bytes.
3. **Unknown structure → `00_Inbox/<relative_path>`.** Never guess a
   category.
4. **Idempotent.** Rerunning on an unchanged origin ward reports all
   `skip`s and writes nothing.
5. **Vault ward name resolved exactly once per run** — in Step 1. All
   downstream logic uses that resolved name. Config changes pick up on
   the next run automatically.
6. **No ward-switching.** Use shell with absolute paths.

## What this skill does NOT do

- Create new content
- Create wards (subagents can't; the wiki ward is bootstrapped at
  gateway startup)
- Ingest into the main knowledge graph (producer skills do that inline)
- Regenerate an aggregate `kg.json` (the SQLite main KG is the aggregate)
- Rewrite any `.md` or `.json` being moved
- Touch `_zztemplates/` or any origin ward's `memory-bank/`, `specs/`,
  `AGENTS.md`
- Delete from the origin ward
