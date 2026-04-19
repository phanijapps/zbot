# Output file shapes

This file is a quick pointer map. The authoritative format spec for every
`.md` file the skill writes lives in
[../../_shared/obsidian_conventions.md](../../_shared/obsidian_conventions.md).

A book folder produced by this skill contains only three file kinds:

| Path                              | Kind         | Shape defined in                      |
|-----------------------------------|--------------|---------------------------------------|
| `books/<slug>/_index.md`          | book MOC     | `_shared/obsidian_conventions.md` → `_index.md` body |
| `books/<slug>/chunks/ch-NN.md`    | chapter      | `_shared/obsidian_conventions.md` → chunk body       |
| `books/<slug>/entities/*.md`      | entity page  | `_shared/obsidian_conventions.md` → entity page body |

No `book.json`. No `book.kg.json`. No per-section graph files. Obsidian
derives the graph from wikilinks + frontmatter at render time.

## Main-graph ingest payload

One call at the end of a successful run. Exactly one `book:<slug>` entity
plus one entity per cross-source real-world entity mentioned in the book
(author, real places, real organizations, public works, named concepts).
See "Main-graph ingest" in `../SKILL.md` for the rule and an example
payload.
