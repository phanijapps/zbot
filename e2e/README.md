# e2e — Mock harness for /research-v2

See `docs/superpowers/specs/2026-04-20-e2e-mock-harness-design.md` for the design.

Two modes:
- **UI mode** — Playwright against a mock gateway server. Fast, UI-only.
- **Full mode** — Playwright against a real zerod binary, with mock-llm
  replacing the provider and tool-replay replacing tool execution.

Run:

    # Mode UI
    cd e2e && ./scripts/boot-ui-mode.sh simple-qa &
    cd playwright && npx playwright test ui-mode/

    # Mode Full
    cd e2e && ./scripts/boot-full-mode.sh simple-qa &
    cd playwright && npx playwright test full-mode/

See `fixtures/README.md` for how to add a new scenario.
