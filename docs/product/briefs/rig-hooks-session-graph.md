# Brief: Rig upgrade, configurable hooks, and desktop experience completion

- **Slug:** `rig-hooks-session-graph`
- **Received:** 2026-10-03
- **Owner:** @videogamer
- **Status:** Ready
- **Shape:** A (outcome brief; no story list)
- **Initiative:** First Desktop Agent (`ini-002`)
- **Priority:** Before [agent-ready bootstrap](agent-ready-bootstrap.md)

## Outcome

Zbot runs on Rig 0.43.0, executes the hooks users configure in a vault file, and
provides a finished conversation and graph exploration experience. Hooks can
invoke external programs, including Python scripts, Node `.mjs` files, and Go
executables. Users can see what executed and why. The graph exposes knowledge
beyond the current 500-relationship fetch limit while retaining deliberate,
consistent UX styling.

## Appetite

Open; the user confirmed leaving the effort budget uncommitted. Deliver this
before agent-ready bootstrap. Continue the existing session-shell work rather
than replacing or duplicating its accepted contract.

## Rabbit holes

- Keep the solution simple: reuse Rig, the existing vault/config layout, runtime
  hooks, session details, and graph APIs before introducing another framework.

## Scope / Non-goals

**In scope:**

### Rig 0.43.0 migration

- Upgrade to the requested Rig 0.43.0 release, including the appropriate facade
  or component crates, features, and lockfile. Verify the published dependency
  contract; a version-number edit alone is insufficient.
- Migrate the current model, tool, hook, MCP, streaming, and run-state adapters
  to the versioned API while preserving the existing sole Rig execution path.
- Preserve the working Ollama/z.ai intent behavior, configured provider/model,
  output-token budgets, terminal answers, cancellation, and session reload.
  No successful behavior may silently fall back to another agent engine.

### File-configured external hooks

- Load user-owned hooks from `<vault>/config/hooks.json`, defaulting to
  `~/Documents/zbot/config/hooks.json`. Publish a versioned JSON Schema alongside
  it for editor validation. Use an ordered hook list with stable ID, enabled
  state, trigger event, command argv, vault working directory, timeout, and
  failure behavior. File configuration must reach actual execution.
- Support language-neutral process invocation, for example executable `python`
  with a script argument, `node` with a `.mjs` argument, or a compiled Go binary.
  Do not require users to write Rust adapters or use one scripting language.
- Define a bounded, versioned JSON event payload on stdin and a validated output
  contract. Distinguish diagnostics from context additions and control actions.
  An exit code, malformed response, or arbitrary stdout must not accidentally
  become an approval or privileged instruction.
- Define supported triggers against the published 0.43.0 hook API: run start
  and settlement, completion boundaries, tool dispatch/results, and invalid
  tool calls where supported. Define session/user-prompt events separately at
  the gateway. A session, user message, run, and model turn are different events.
- Matching hooks must fire on root and delegated Rig execution, including Chat,
  Research, streaming, and non-streaming paths as applicable. An invocation
  must carry stable event/run/session/agent identity for traceability.
- Expose supported triggers honestly. External processes cannot be mapped to
  synchronous, nonblocking callbacks by pretending they are asynchronous.
  High-frequency streaming triggers must be explicit opt-in, not a default
  process spawn for every token.
- Specify hook composition and allowed responses per event using 0.43.0
  semantics. Configured hooks cannot bypass existing tool authorization or
  protected runtime controls. A post-action failure cannot undo an executed tool.
- Execute configured programs with explicit argv, without implicit shell
  interpolation. Define bounded runtime/output, cancellation and process cleanup,
  environment exposure, and working-directory behavior. Loading a skill or
  opening a project must not silently register and execute its scripts as hooks.
- Disabled/nonmatching hooks never run. Invalid configuration is actionable.
  Required gates fail visibly on timeout or invalid output; optional notices
  may continue with a recorded failure/skip. Explain when edits take effect,
  preferably on the next run, without changing an in-flight run unpredictably.

**File contract and operator experience:** The schema is
[`contracts/jsonschema/hooks.schema.json`](../../../contracts/jsonschema/hooks.schema.json).
The editable file is `<vault>/config/hooks.json`; an editor schema copy lives at
`<vault>/config/hooks.schema.json`. Custom vault locations use the same relative
paths, without a hardcoded home-directory dependency. Disabled Python, Node,
and compiled executable examples are in
[`examples/hooks.example.json`](examples/hooks.example.json).

The top-level format is `version: 1` plus an ordered `hooks` array. Each item
requires `id`, `event`, and `command` (executable plus literal arguments).
Defaults are `enabled: true`, `cwd: vault`, `timeout_ms: 5000`, and
`on_failure: continue`. The loader applies these defaults; JSON Schema alone
does not apply them. IDs must be unique. Saving/editing the file does not run
commands. Hook configuration is snapshotted for each new run; syntax/schema
errors prevent starting that run with a diagnostic rather than quietly running
without the configured hooks. A missing file means no configured hooks.

Proposed external event names are `session_start`, `user_prompt`, `run_start`,
`run_end`, `before_model`, `after_model`, `before_tool`, `after_tool`, and
`invalid_tool_call`. These are zbot aliases to be mapped to the versioned Rig
and gateway lifecycle, not assertions that the current runtime supports them.
Session/prompt events apply to actual root ingress; run/model/tool events apply
to matching root and delegated executions. Array order determines registration.
`on_failure: block` is allowed only at a pre-action boundary; after-model,
after-tool, and run-end failures are observable and cannot undo completed work.
Start with all tool calls at each configured tool event; tool filters can be
considered later without making this first schema harder to edit.

The reviewed v1 contract uses `cwd: vault`. Programs/scripts are operator-managed
and validated outside ward/project roots; ward/project execution is deferred
until its explicit trust/change-invalidation contract exists. Bare command names
resolve through the permitted PATH; relative program/script paths resolve from
the vault. There is no
implicit shell, tilde, environment-variable, or template expansion. Environment,
stdin payload, output protocol, cancellation, and output-size limits remain part
of the implementation specification. Session Activity reports hook outcome,
duration, and bounded diagnostics while preserving credential masking.

A schema-valid empty `hooks.json` and adjacent schema were created in the
current vault for editing. This creates no registered runtime hook and executes
no scripts. Hook management, forms, and a Test button in Settings are deferred.

### Finish the session experience

- Finish the existing [desktop-session-shell contract](../../specs/desktop-session-shell/spec.md)
  and its approved visual direction. Reconcile remaining acceptance criteria,
  implementation, and retained QA evidence; avoid a competing shell spec.
- Replace decorative/placeholder Workspace tabs with usable Activity, Sources,
  and Files for the current conversation, including default Quick Chat where
  session identity can be proven. Where identity/details are unavailable, explain
  that state rather than showing invented records or inactive tabs as controls.
- Preserve New chat, recent-session restoration, Chat/Research mode, streaming,
  scoped Stop, history, sources, and file access across navigation and reload.
  Hook execution feedback must fit this existing session surface.
- Finish hierarchy, message/Markdown/code readability, composer and attachment
  controls, agent card contrast, panel scrolling, and responsive drawers. Keep
  the conversation dominant and use shared typography, spacing, and color tokens.
- Complete existing Agents, Settings, and Integrations presentation in the same
  shell. Hook-management controls are deferred; keep session execution feedback
  within the existing Activity surface.
- Verify populated/empty, active/completed/stopped/error, reconnect/reload,
  desktop/narrow, keyboard/focus, and long-content states against the contract.

### Graph completeness, interaction, and styling

- Remove the one-page 200-entity/500-relationship ceiling as an exploration
  limit. Support complete scoped exploration with bounded progressive loading
  or a suitable overview/detail design; do not merely raise a constant.
- Repair API pagination and count semantics where needed. Display loaded versus
  available counts and explicitly label partial/filtered/aggregated views.
  Both cross-agent and per-agent modes must work beyond the first page.
- Preserve relationship endpoints as data loads, stable identities, filters,
  search, selection, detail panels, zoom/pan, fit-to-view, and refresh behavior.
  Search must reach data beyond the initially loaded subset. Handle changing
  datasets, failed pages, empty results, and cancellation without duplicate edges.
- Evaluate [cosmos.gl](https://github.com/cosmosgl/graph) as the user's performance
  and interaction reference. Adopt it if measurement justifies it; the brief
  does not preselect a renderer or treat renderer replacement as a data fix.
- Style the graph as part of zbot: restrained background, legible focused labels,
  clear relationship emphasis, coherent entity colors/legend, compact controls,
  and a readable selected-entity panel. Dense graphs must remain navigable;
  rendering all labels at once is not a usability goal.
- Provide keyboard-accessible search and inspection, an accessible alternative
  to canvas-only selection, reduced-motion behavior, and an explicit fallback
  if required GPU capabilities are unavailable.

**Non-goals:**

- Hook configuration UI, hook editor, and hook Test action in Settings for now.
- Agent-ready pack installation in this brief; that remains a separate Draft.
- A replacement execution engine, general plugin marketplace, arbitrary remote
  script hosting, or automatic interpreter installation.
- Unbounded graph downloads or claims that every dataset fits in browser memory.
- Replacing the accepted session design or rewriting existing unrelated work.

## Success measures

- The resolved Rig dependency is 0.43.0 and relevant runtime checks pass for
  streaming/non-streaming Chat/Research, delegation, MCP, terminal responses,
  cancellation/reload, and working Ollama/z.ai intent analysis.
- A hook declared in the vault configuration executes at its matching runtime event; disabled
  and nonmatching controls prove nonexecution. Python, Node `.mjs`, and compiled
  executable fixtures all receive the documented payload and return observable
  outcomes. Restart preserves the user-owned file; edits take effect on the next run.
- Tests prove ordering, per-event execution counts, root/delegated scope,
  cancellation cleanup, timeout, invalid output, missing executable, and bounded
  failure feedback. No event registration grants additional tool permissions.
- File examples validate against the schema, invalid configuration is actionable,
  and session hook details have distinct running/error/completed states. Visual
  review covers desktop and narrow layouts, long content, and active work.
- A graph fixture exceeding 500 relationships can be explored beyond the first
  page, with matching endpoints and truthful loaded/total counts. A later-page
  entity is searchable and inspectable; per-agent and all-agent views both pass.
- A reproducible graph benchmark covers at least the observed scale of roughly
  16,700 entities and 4,700 relationships. Agree numeric latency, responsiveness,
  and memory budgets during specification; compare current and candidate
  renderers and record browser/hardware and GPU fallback behavior.

## Risks and constraints

- Rig 0.43.0 has breaking crate, runtime, provider, and hook changes. The current
  concept page uses signatures that differ from the published release. Use it
  as a conceptual reference and the versioned source as the implementation oracle.
- External hooks execute with host privileges and can have side effects. Use
  the existing vault ownership and tool authorization boundaries, keep command configuration
  operator-owned, and minimize payload/environment exposure. Hook policy is not
  a substitute for authorization inside tools/services.
- Existing session-shell work is Implementing and has user edits. Preserve those
  changes and reconcile the existing plan before starting implementation.
- API completeness and renderer scalability are separate obligations. A fast
  GPU view of the same truncated data does not satisfy graph completeness.

## Evidence and references

- `runtime/agent-runtime/Cargo.toml:49` and `Cargo.lock`: current Rig Git pin
  `6b1991bfb246411dd75839c8611e801a2309d33c`, version 0.39.0.
- [Rig hooks concepts](https://rig.rs/docs/concepts/hooks),
  [0.43.0 change log](https://github.com/0xPlaygrounds/rig/blob/654567eb64274fca00cab86cdd32c86b9913769e/crates/rig-core/CHANGELOG.md),
  and [versioned hook source](https://github.com/0xPlaygrounds/rig/blob/654567eb64274fca00cab86cdd32c86b9913769e/crates/rig-agent/src/agent/hook.rs).
  Published 0.43.0 crate metadata points to commit
  `654567eb64274fca00cab86cdd32c86b9913769e`. The public facade is the `rig` package;
  the current dependency layout must be migrated rather than assumed compatible.
- `runtime/agent-runtime/src/engine/hooks.rs` and
  `runtime/agent-runtime/src/rig_adapter/tool_hook.rs`: reusable hook composition
  and actual before/after-tool integration in the current Rig path.
- `gateway/gateway-services/src/settings.rs`: settings persist across restarts;
  the current `AppSettings` has no configurable lifecycle-hook collection.
- `http://localhost:3000/session` inspected read-only on 2026-10-03 at 1440×1000
  and 390×844. No page exception or mobile document-width overflow was observed
  in that default state. This is not evidence for untested live/reload states.
  `SessionShell.tsx` renders the outer Workspace tabs without selection handlers
  and always displays its selection hint, despite visible default Chat history.
- `apps/ui/src/features/observatory/graph-hooks.ts`: fetches 200 entities and
  500 relationships once, for per-agent and all-agent graph views.
  `gateway/src/http/graph.rs`: cross-agent endpoints currently report page length
  as `total` and do not pass an offset to cross-agent listing.
- Read-only `/api/graph/stats` returned 16,693 entities and 4,728 relationships
  during investigation. These totals are a scale observation, not a test fixture.
- [cosmos.gl source and README](https://github.com/cosmosgl/graph) supplies the
  GPU rendering reference. Renderer selection and zbot styling remain design work.

## Open decisions

The effort budget remains open. Exact migration symbols and renderer choice are
implementation evidence items. Hook protocol, event scope, environment/trust
rules, Activity fields, graph paging/projection and proposed performance budgets
are defined in the linked Draft specs. Their approval remains separate from this
Ready brief. Project-supplied hooks and ward cwd are deferred; this delivery uses
operator-installed programs from the vault configuration.

## Proposed shippable cut

The user approved this brief on 2026-10-03. Four Draft spec/plan pairs define
its delivery contracts. Rig migration precedes external-hook implementation;
session completion and graph exploration can proceed independently. The session
companion audits/closes the existing shell contract without replacing or
resetting its approved active plan. Agent-ready bootstrap follows this delivery.

Payload schemas and graph paging/performance proposals are now specified in the
linked Drafts. Spec/plan approval is the implementation gate; no runtime change
is implied by brief approval or by publishing these documents.

## Spec map

| Spec | Contract | Status |
| --- | --- | --- |
| `rig-043-migration` | [Rig 0.43.0 migration](../../specs/rig-043-migration/spec.md) | — |
| `external-hooks` | [File-configured external hooks](../../specs/external-hooks/spec.md) | — |
| `desktop-session-completion` | [Desktop session completion](../../specs/desktop-session-completion/spec.md) | — |
| `observatory-graph-completeness` | [Graph exploration completeness](../../specs/observatory-graph-completeness/spec.md) | — |
