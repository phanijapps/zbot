# Fixtures

Each fixture bundle contains four files:

| file | purpose |
|---|---|
| `session.json` | metadata: session id, title, executions tree, artifacts |
| `llm-responses.jsonl` | one LLM request/response pair per line, keyed by (execution_id, iteration) |
| `tool-results.jsonl` | one tool invocation per line, keyed by (execution_id, tool_index) |
| `ws-events.jsonl` | the ServerMessage stream zerod emitted during the original recording |

## Adding a fixture

1. Run a real session against a live daemon + capture it:

       PYTHONPATH=. python3 e2e/scripts/record-fixture.py \
           --session-id sess-XXXX \
           --out e2e/fixtures/<name>

2. Copy the simple-qa spec templates and adjust the expected title + subagent count:

       cp e2e/playwright/ui-mode/simple-qa.ui.spec.ts \
          e2e/playwright/ui-mode/<name>.ui.spec.ts
       cp e2e/playwright/full-mode/simple-qa.full.spec.ts \
          e2e/playwright/full-mode/<name>.full.spec.ts

3. Commit the fixture and specs together.

## Synthetic fixtures

`e2e/fixtures/simple-qa/` is generated programmatically by
`e2e/fixtures/seed_synthetic.py`. It's a placeholder for early harness
work, not a real recording. Regenerate with:

    PYTHONPATH=. python3 e2e/fixtures/seed_synthetic.py
