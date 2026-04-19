# Routing — ward structure → vault destination

The classifier is purely structural and whole-folder. `wiki` moves
complete producer-skill output folders into their matching vault
locations. It does NOT descend into a folder to reclassify children —
if the producer wrote `books/<slug>/_index.md` + `chunks/` + `entities/`,
the whole folder goes to `Vault/30_Library/Books/<slug>/` as a unit.

## Rule table

| Ward source pattern              | Content type     | Vault destination                         | Notes                           |
|----------------------------------|------------------|-------------------------------------------|---------------------------------|
| `books/<slug>/`                  | book folder      | `30_Library/Books/<slug>/`                | whole folder, contents as-is    |
| `articles/<slug>/`               | article folder   | `30_Library/Articles/<slug>/`             | whole folder, contents as-is    |
| `research/<archetype>/`          | research folder  | `40_Research/<archetype>/`                | whole tree. Producers nest `<archetype>/<subject>/<date-slug>/` per `_shared/research_archetype.md` |
| `reports/<project>/`             | project folder   | `20_Projects/<project>/`                  | whole folder, contents as-is    |
| loose `**/*.pdf`                 | document         | `70_Assets/Documents/<ward>__<basename>`  | loose files only                |
| loose `**/*.{png,jpg,jpeg,svg,gif,webp}` | image    | `70_Assets/Images/<ward>__<basename>`     | loose files only                |
| anything else at ward root       | unknown          | `00_Inbox/<relative_path>`                | path preserved                  |

"Loose" means the file is not inside one of the producer-folder patterns
above. Images inside a book folder are carried along with the folder —
they are part of the book's output.

## Classifier order

First match wins. Most specific pattern first:

1. The four producer-folder patterns (`books/`, `articles/`, `research/`,
   `reports/`).
2. Loose PDFs / images at ward root or in ad-hoc subfolders not matching
   #1.
3. Fallback: `00_Inbox/`.

## Never-promote list

These paths are ward infrastructure, not content. `wiki` skips them
entirely:

- `specs/` — execution plans
- `memory-bank/` — agent working notes
- `AGENTS.md`, `CLAUDE.md`, `README.md` at ward root
- Dotfiles / dotfolders (`.git/`, `.DS_Store`, …)
- Files over 50 MB — route to `00_Inbox/` with a warning instead of
  promoting

## Idempotence — hash-compare before writing

For every planned copy (whole folder or single file), compute the content
hash and compare against the destination (if any):

- `copy` — destination missing
- `update` — source differs from destination, source wins
- `skip` — already up to date

For whole folders, the skill walks leaves and compares per-file, so
partial updates don't reclassify the whole folder as `update`.
