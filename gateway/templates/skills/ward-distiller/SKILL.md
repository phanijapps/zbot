---
name: ward-distiller
description: >
  Scan a ward for knowledge-graph JSON files and insert their contents into
  the graph via the ingest tool. Use when the user wants to distill a ward,
  populate the graph from pre-extracted JSON, index a batch import, or sync
  on-disk knowledge artifacts into `kg_entities` / `kg_relationships`.
---

# Ward Distiller

Find graph-shaped JSON in the current ward and hand it to the `ingest` tool. Nothing else. The extraction already happened somewhere else — this skill just gets the data into `kg_entities` and `kg_relationships`.

## Tool discipline — non-negotiable

You MUST call the `ingest` tool with `{entities: [...], relationships: [...]}` for each graph-shaped file found.

You MUST NOT call `memory.save_fact` as a shortcut. Saving entities as memory facts bypasses the graph entirely — `graph_query` will not find them, the knowledge graph stays empty, and the next session cannot traverse relationships. If you find yourself typing `memory(action="save_fact", ...)` in this skill, STOP — you are using the wrong tool. The correct tool is `ingest`.

### Correct call shape

```
ingest(
  source_id="<relative/path/to/file.kg.json>",
  entities=<the file's entities array verbatim>,
  relationships=<the file's relationships array verbatim>
)
```

Do not reshape the arrays. Do not summarize. Do not convert entities to facts. Pass them through.

## Use when

- The user asks to distill, ingest, index, or sync a ward's on-disk knowledge into the graph
- A preprocessor produced JSON files the agent should pick up
- The user wants to re-run ingestion after editing a graph JSON by hand

Do not use when the ward has only prose. Use `book-reader` or a domain reader instead and let the normal ingestion path extract entities.

## Core contract

For each graph-shaped JSON file found, one `ingest` call carrying whatever `entities` and/or `relationships` arrays the file contains. That is the whole job.

Create no new files. Do not rewrite the source JSON. Do not invent entities the file did not list.

## What counts as a graph-shaped JSON

A file is in scope if it is valid JSON AND has at least one of:

- a top-level `entities` array whose items carry `id`, `name`, `type`
- a top-level `relationships` array whose items carry `type`, `from`, `to`

Filename hints that usually indicate in-scope files (non-exhaustive):

- `*.kg.json`
- anything under a `knowledge-graph/` or `graph/` directory
- files named `entities.json`, `relationships.json`, `nodes.json`, `edges.json`, `catalog*.json`

A file without those arrays is out of scope even if it lives next to one
that's in scope.

## Workflow

1. Resolve the ward root (the current ward if one is active, else the ward
   the user named).
2. Find every JSON file under the ward root that matches the criteria above.
3. For each file, read it and call `ingest` with its `entities` array and/or
   `relationships` array, passing the filename as `source_id` for provenance.
   Send whatever is present — if the file has only entities, pass an empty
   relationships array (or omit it); same the other way around.
4. Collect per-file counts returned by `ingest`.
5. Report a summary: files scanned, files in scope, entities upserted,
   relationships upserted, files skipped with the reason.

One `ingest` call per file is fine. Batching multiple files into a single
call is also fine when it makes the merge order clearer (e.g., all files
in the same subdirectory describe one topic).

## Rules

1. Only ingest files that pass the shape check. Silently skip the rest.
2. If a file is malformed JSON, skip it and include the path in the skipped
   list with reason `"invalid json"`.
3. If an entity lacks `id`, `name`, or `type`, or a relationship lacks
   `type`, `from`, or `to`, skip that item (not the whole file) and note it.
4. Preserve the ids in the file verbatim. Do not rewrite slugs.
5. Properties on entities and relationships pass through unchanged. The
   graph merges them on repeat ingests (keys union, arrays concatenate),
   so re-running the skill on the same ward is idempotent.
6. Never modify the source JSON.
7. If the user asks what is in the ward without asking to ingest, do not
   call `ingest`; list the files and their shape instead.

## Provenance

Pass the relative file path as `source_id` (e.g., `"catalog/people.kg.json"`)
so the graph retains a pointer back to where the data came from. The agent
does not need to add extra provenance fields — if the file's entities or
relationships already carry `properties.evidence`, that is preserved
verbatim through the merge.

## Retrieval behavior

This skill does not read from the graph. For queries about what has been
ingested, defer to the graph-query tool and return its results directly.