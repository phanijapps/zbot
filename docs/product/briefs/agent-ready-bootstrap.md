# Brief: Agent-ready skills and subagents on first setup

- **Slug:** `agent-ready-bootstrap`
- **Received:** 2026-10-03
- **Owner:** AgentZero maintainer
- **Status:** Draft
- **Shape:** A (outcome brief; no story list)
- **Initiative:** First Desktop Agent (`ini-002`)

## Outcome

On first setup, a new zbot instance has the supported skills and subagents from
[agent-ready-repo](https://github.com/eugenelim/agent-ready-repo) available through
zbot's normal discovery, skill loading, and delegation paths. Users do not need
to translate another agent's configuration or install the catalogue manually.
Missing prerequisites and unsupported capabilities are reported clearly.

The user confirmed first-instance setup, all supported packs, this slug, and
registration under First Desktop Agent. This brief defines product behavior;
an RFC is needed only if later design changes a standing project rule.

## Appetite

Open; no time or effort budget committed. Keep the implementation simple and
reuse existing seeding, skill services, delegation, and Rig execution.

## Rabbit holes

- Do not introduce unnecessary complexity: reuse the current implementation
  paths before proposing another installation or execution framework.

## Scope / Non-goals

**In scope:**

- Review all released, non-example packs at a pinned upstream revision. Account
  for every skill and subagent as supported, blocked by a named prerequisite,
  or unsupported with a reason. Exclude examples and experimental pilot packs.
- Adapt canonical `.apm/skills` assets into bundled zbot `skills/<name>/SKILL.md`
  directories, retaining required references, scripts, and relative links.
- Adapt canonical subagents into zbot agent configuration and instructions.
  Use the instance's configured provider/model and existing Rig execution;
  upstream aliases such as `opus` must not become provider model IDs.
- Seed the supported assets during first setup using existing bundled templates.
  Make them discoverable by the intent catalogue, skill loader, and delegator.
  Load skill content when needed rather than attaching all bodies to each prompt.
- Map tool names, delegation, context handling, project paths, and workflow
  capabilities explicitly. Preserve reviewer permission restrictions through
  enforceable zbot controls; frontmatter or prose alone is insufficient.
- Report missing tools, services, credentials, helper scripts, and workspace
  scaffolding. A blocked dependency must not silently become an executable skill.
  Account for dependencies between packs and assets.
- Preserve existing skills and agents, including name collisions and user edits.
  Setup retries must expose or recover partial installation without overwriting
  those edits. Routine restarts must not reinstall deleted defaults.
- Retain upstream revision, asset provenance, and applicable license notices.

**Non-goals for this proposal:**

- A general plugin marketplace, package manager, or new agent execution engine.
- Automatic upstream upgrades, external account connection, credential creation,
  or automatic installation of third-party binaries during setup.
- Retroactive installation into existing instances, or seeding on every agent
  or ward creation. Existing-instance opt-in can be considered separately.

## Success measures

- A clean instance exposes every supported asset from the compatibility inventory
  after first setup, with valid references and agent skill assignments.
- Representative research and coding requests discover and load the expected
  skills, delegate to the adapted specialists, and complete through Rig using
  the configured provider/model. Verification distinguishes a working workflow
  from successful parsing or a copied file.
- Missing prerequisites produce an actionable reason, without unnecessary retry
  loops, false success, or unsupported agent/tool calls.
- Setup replay, interrupted setup, existing-name collisions, user edits, and
  restart after user deletion preserve the promised first-setup behavior.
- Prompts contain only necessary catalogue metadata and selected skill content;
  the full imported catalogue is not injected into every session.

## Risks and constraints

- Upstream baseline reviewed: `afcc1eeb8b9111bb44d90d37fb37ee468a93c260`.
  Its canonical non-example inventory contains 23 packs, 130 skill entrypoints,
  and 17 subagent declarations. These are candidates, not a claim of support.
  Revalidate against the pinned tree before implementation.
- [Core](https://github.com/eugenelim/agent-ready-repo/blob/afcc1eeb8b9111bb44d90d37fb37ee468a93c260/packs/core/pack.toml)
  is repository-scoped. Its skills can be available instance-wide while their
  execution still requires appropriate project context. First setup must not
  invent a ward or write project scaffolding into the user's home directory.
- Several workflows rely on helper scripts, hooks, human approval pauses,
  delegation semantics, and external tools. Copying Markdown does not provide
  those contracts. Unsupported requirements must be named in the inventory.
- Imported agent declarations use tool names and model aliases that differ from
  zbot. Model translation must preserve user configuration; permission mapping
  must preserve actual access restrictions.
- Bundled assets must work without an upstream network fetch on first setup.
  User-facing prerequisite reporting should reuse an existing surface where
  possible. The exact surface remains a design decision.

## Existing implementation evidence

- `gateway/src/state/seeding.rs`: `seed_defaults` seeds agents from bundled
  `default_agents.json`, then skills, then preloads the skill cache.
- `seed_default_skills` currently copies every embedded `skills/` asset only
  when the vault skills directory is empty. One unrelated file or a partial
  failed copy makes that check insufficient for reliable completion.
- `gateway/gateway-services/src/agents.rs`: `seed_default_agents` creates agent
  configurations using the default provider/model, skips existing agents, and
  has limited template-managed instruction refresh. Its repeated-startup
  behavior needs checking against first-setup and user-deletion requirements.
- `gateway/gateway-services/src/skills.rs`: `SkillFrontmatter` already accepts
  name and description. Extra upstream permission metadata is not an enforced
  zbot permission policy merely because the skill parses successfully.
- `gateway/gateway-execution/src/invoke/builder.rs` collects skill summaries;
  `runtime/agent-tools/src/tools/execution/skills.rs` implements `load_skill`.
  Reuse these discovery and loading paths.

## Hook adaptation proposal

Reuse zbot's existing `EngineHook` / `HookSet` and its Rig adapter. Add the
minimum gateway lifecycle wiring needed for session and user-prompt events;
do not route every event through a before-model-call hook.

| Upstream behavior | Proposed zbot trigger | Effect |
| --- | --- | --- |
| `session-start.py` | Defined session lifecycle events, scoped to the active ward | Surface pending adaptation notices. Do not automatically replay project knowledge as standing instructions. |
| `work-loop-check.py` | Each accepted root user prompt, before task execution | Add the short work-loop reminder once for that prompt, without rerunning it on every tool/model turn. |
| `pre-pr.py` | Explicit pre-PR workflow step; optional project Git pre-push hook | Run project gates and expose failures and skips distinctly. A pre-push hook covers pushes, not every PR creation path. |

The pinned catalogue has two wired lifecycle hooks and one pre-PR helper, not
three interchangeable runtime events. Its session hook can be called on start,
resume, clear, compaction, and fork in other hosts. Inventory and verify zbot's
actual equivalent events; list unsupported events instead of claiming parity.

`before_tool` already blocks calls through `ToolDecision::Block`, and
`after_tool` handles completed results. Reuse these for an actual tool gate
when its action is identifiable. Prompt reminders guide workflow; they do not
enforce approval or prevent publication by themselves. Hard publication gates
need coverage of the supported publication paths, not substring matching of
arbitrary shell commands.

Hook commands run as configured processes with explicit arguments, active-ward
working directory, timeout, and output limits. Only reviewed configured hook
output intended as a reminder enters prompt context; arbitrary script stdout
and project knowledge remain results/evidence. Configured blocking gates must
report failure, timeout, and missing mandatory dependencies without presenting
them as a passed check. Optional notices may report a skip.

The pre-PR helper searches other hosts' skill roots, so its adapted version must
resolve zbot's installed work-loop scripts explicitly. Python 3.11+ and project
scaffolding are prerequisites where retained Python scripts need them.

Verification must show hook execution and observable effects for Chat and
Research, root versus delegated work, a subsequent user prompt, resume, and a
blocked tool action. It must also prove that model/tool turns do not multiply
per-prompt hook execution and that upstream skip behavior is not called a pass.

## Open decisions

- Effort budget and the supported/blocked/unsupported compatibility inventory.
- Enforcement mapping for upstream subagent permissions, nested delegation,
  human approval pauses, and hook-dependent behavior.
- Placement of project-scoped supporting assets and prerequisite status, using
  current ward and configuration conventions wherever possible.

## Proposed shippable cut

Not decomposed. Confirm the compatibility inventory and the first-setup contract
before proposing implementation slices or materializing specs.

## Spec map

<!-- Specs are added after a proposed cut is confirmed. -->

| Spec | Status |
| --- | --- |
