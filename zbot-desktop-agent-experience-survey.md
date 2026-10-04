# Zbot desktop agent experience — initial desk research

> Discipline: applied (practitioner-pattern survey)

Date: 2026-09-26  
Status: initial desk research, updated with user direction; product hypotheses, not an implementation specification.

## Question and scope

How should zbot preserve its Research harness and Chat mode while becoming a desktop agent with a simpler ChatGPT-like UI and Codex-like lifecycle hooks?

This pass examines official product documentation, one peer-reviewed human–AI interaction study, zbot's indexed code graph, and targeted source files. It does not assume that visual similarity alone is the goal. It treats ChatGPT's Chat/Work split and Claude Code's task sessions as references for a coherent desktop agent experience.

## Confirmed direction

The user wants a desktop agent, a much simpler ChatGPT-like main UI, and hooks that work like Codex's lifecycle hooks. Research and Chat both stay. The desktop app is a product requirement; the shell technology and the exact compatibility level for Codex hook files remain design questions.

## Look and feel direction to prototype

**Design intent:** a calm conversation workspace that feels approachable when idle and stays readable during a long agent run. The main screen should show the person's task and zbot's answer first. Status, tool calls, agent trees, and diagnostics remain available through progressive disclosure. This follows the [official OpenAI documentation's distinction](https://learn.chatgpt.com/docs/use-chatgpt) between ChatGPT's low-technical-detail presentation and Codex's developer views; the proposed visual treatment is our inference.

```text
┌──────────────────────┬────────────────────────────────────────────┐
│ zbot       New chat   │  Research ▾             Project/ward  ··· │
│ Search               │                                            │
│ Recent               │       Conversation or welcome state       │
│  • Session title     │       Clear answer and source links       │
│  • Session title     │       ▸ Activity · 3 steps · 1 decision   │
│ Projects             │                                            │
│  • Project name      │   ┌────────────────────────────────────┐   │
│                      │   │ Ask zbot…                         │   │
│ Settings             │   │ Attach  ·  Chat / Research  ·  Send │   │
└──────────────────────┴───┴────────────────────────────────────┴───┘
```

- **Navigation:** a slim left rail for New, Search, Recents, and Projects, with Memory and Observatory directly reachable below the session list. Chat and Research switch at the top center of the conversation, with each session labeled by mode. Agent administration, Vault, integrations, and detailed logs move into settings or contextual panels. A running task or required decision gets a clear indicator in the session list.
- **Canvas:** one centered reading column, roughly 720–800 px wide, with generous margins. User prompts get a subtle tinted container; assistant answers read as page content. Tables, code, and citations stay legible without putting every paragraph in a card. Research outputs get a report view with sources and files beside the conversation when opened.
- **Palette and type:** start with soft neutral light and dark themes, high-contrast text, quiet borders, and one restrained zbot accent. The earlier [warm editorial ADR](docs/adr/decisions.md) records a copper accent and the existing font preference; the current [theme](apps/ui/src/styles/theme.css) instead defaults to deep charcoal with cyan. A prototype should test the warmer neutral direction before changing global tokens. Keep the current sans-serif family rather than introducing decorative display type.
- **Activity:** while working, show a short human-readable status line in the conversation. Expand it for the plan, tool calls, memory use, subagents, hook outcomes, and errors. Keep Activity, Sources, and Files as the right-panel tabs. Use color primarily for state and action, not decoration. A permission request appears as a focused decision card beside the relevant step.
- **Desktop behavior:** compact window sizes keep the composer and session title visible; the sidebar can collapse. Keyboard access, clear focus states, reduced motion, and readable zoom are part of the visual baseline. A companion quick-entry window is a later experiment after the main window works.

This is a prototype direction, not a request to copy ChatGPT's branding or an accepted palette. The key test is whether users can start a task, understand its state, and inspect its result without learning zbot's internal component names.

## Findings

1. **A conversation can remain the entry point while the depth of work changes.** ChatGPT desktop separates Chat (questions and conversation), Work (research and artifacts), and Codex (software tasks); Chat and Work share a Recents list and can start inside Projects. Claude Desktop similarly separates Chat, Cowork, and Code. Zbot already has a Chat/Research distinction, but its navigation presents them as separate top-level destinations. The opportunity is a shared conversation/session home with an explicit work mode and mode-specific controls. This is a product inference, not a recommendation to merge the two execution pipelines. [moderate]  
   Sources: [ChatGPT Work and Codex](https://help.openai.com/en/articles/20001275-chatgpt-work-and-codex), [ChatGPT desktop migration](https://help.openai.com/en/articles/20001276-moving-to-the-new-chatgpt-desktop-app), [Claude Code Desktop](https://code.claude.com/docs/en/desktop), [zbot routes and navigation](apps/ui/src/App.tsx).  
   Downgrade: indirectness — the products demonstrate the pattern, but do not prove it fits zbot's users.

2. **Long tasks need visible control points, not only a stream of activity.** ChatGPT Deep Research offers an editable plan, live progress, interruption, source control, and a cited report. Claude Code Desktop exposes permission mode, interruption and steering, task/subagent activity, and diff review. Zbot's Research UI already records turns, timelines, subagents, a plan path, artifacts, and a Stop action, but the evidence reviewed so far does not show an equivalent user-facing plan approval or risk-based action approval flow. A redesign should test whether the user can tell what zbot intends, what it is doing, what changed, and what requires a decision. [moderate]  
   Sources: [ChatGPT Deep Research](https://help.openai.com/en/articles/10500283-deep-research-in-chatgpt), [Claude Code Desktop](https://code.claude.com/docs/en/desktop), [human–AI interaction guidelines](https://www.microsoft.com/en-us/research/publication/guidelines-for-human-ai-interaction/), [zbot Research state](apps/ui/src/features/research-v2/types.ts), [zbot Research page](apps/ui/src/features/research-v2/ResearchPage.tsx).  
   Downgrade: indirectness — this is a design criterion inferred from product examples and HCI guidance; the approval-flow gap needs a full backend audit.

3. **The same agent engine can support multiple surfaces.** Claude Code describes one gather-context → act → verify loop across CLI, desktop, IDE, and other interfaces; its desktop UI runs the underlying CLI engine. This supports keeping zbot's existing Research and Chat semantics in the gateway while redesigning the presentation and control layer. [low]  
   Sources: [How Claude Code works](https://code.claude.com/docs/en/how-claude-code-works), [Claude Code Desktop](https://code.claude.com/docs/en/desktop), [zbot execution modes](gateway/gateway-execution/src/config.rs).  
   Downgrade: single source; indirectness — the two external pages are from one vendor and do not establish zbot's migration cost.

4. **A completed result should remain distinct from execution detail.** Claude Code Desktop offers collapsed and verbose transcript views, plus dedicated panes for diff, browser, terminal, files, plans, and tasks. ChatGPT's Work and Deep Research put the deliverable and its sources in a reviewable report. Zbot's Research UI currently interleaves turn blocks, subagents, surfaces, a context inspector, and artifact controls; Quick Chat uses compact activity chips and artifact cards. A common session model could expose a readable answer by default while retaining the detailed execution trace on demand. [moderate]  
   Sources: [Claude Code Desktop](https://code.claude.com/docs/en/desktop), [ChatGPT Deep Research](https://help.openai.com/en/articles/10500283-deep-research-in-chatgpt), [human–AI interaction guidelines](https://www.microsoft.com/en-us/research/publication/guidelines-for-human-ai-interaction/), [zbot Research page](apps/ui/src/features/research-v2/ResearchPage.tsx), [zbot Quick Chat](apps/ui/src/features/chat-v2/QuickChat.tsx).  
   Downgrade: indirectness — a user study is needed to establish which detail level zbot users prefer.

5. **Desktop context should be explicit and inspectable.** ChatGPT's macOS app shows which active apps and selections are included with a prompt, and its new desktop browser lets users inspect the same page the agent uses. Claude Code Desktop makes the project folder, execution environment, model, and permission mode visible before starting a session. For zbot, the parallel question is whether the user can see the current ward, attached files, selected sources, model, and tool authority before sending a task. [moderate]  
   Sources: [ChatGPT Work with Apps](https://help.openai.com/en/articles/10119604-work-with-apps-on-macos), [ChatGPT desktop browser](https://help.openai.com/en/articles/20001277-using-the-built-in-browser-in-the-chatgpt-desktop-app), [Claude Code Desktop](https://code.claude.com/docs/en/desktop), [human–AI interaction guidelines](https://www.microsoft.com/en-us/research/publication/guidelines-for-human-ai-interaction/), [zbot Research page](apps/ui/src/features/research-v2/ResearchPage.tsx).  
   Downgrade: indirectness — the sources establish controls, not their effectiveness for zbot.

6. **Codex hooks are a configured lifecycle contract, not merely callbacks inside the engine.** Official OpenAI Docs describe `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, compaction, subagent, Stop, Interrupt, and SessionEnd events. Hooks use event matchers, structured JSON on stdin, structured results, and trust review for non-managed definitions. Codex currently runs command and MCP tool handlers; tool coverage has documented exceptions. Zbot's `EngineHook` exposes `before_tool`, `after_tool`, context transformation, and recall, but has no equivalent user-facing lifecycle configuration in the inspected path. [low]  
   Sources: [Official OpenAI Docs: Hooks](https://learn.chatgpt.com/docs/hooks), [zbot engine hooks](runtime/agent-runtime/src/engine/hooks.rs), [zbot architecture](docs/architecture/architecture.md).  
   Downgrade: single source — Codex semantics are documented by one vendor; zbot's full event coverage still needs an inventory.

7. **A desktop shell can reuse zbot's React UI and Rust daemon, but packaging and authority boundaries must be designed together.** Zbot currently ships a React/Vite web dashboard and daemon/CLI, with no desktop app package found in the inspected `apps/` tree. Tauri documents Vite frontends, bundled external binaries, and capabilities for native access, making it a plausible candidate for a thin desktop shell around `zbotd`; Electron is another candidate but brings a bundled Chromium/Node runtime and its own renderer isolation requirements. This is a candidate comparison, not a framework decision. [low]  
   Sources: [Tauri Vite integration](https://v2.tauri.app/start/frontend/vite/), [Tauri sidecars](https://v2.tauri.app/develop/sidecar/), [Tauri capabilities](https://v2.tauri.app/security/capabilities/), [Electron security model](https://www.electronjs.org/docs/latest/tutorial/security), [zbot app structure](docs/architecture/architecture.md), [zbot package](package.json).  
   Downgrade: indirectness — no zbot desktop prototype or packaging measurement exists yet.

## Zbot baseline from Engram and source inspection

| Area | Current evidence | Design implication to test |
| --- | --- | --- |
| Modes | `SessionMode::Chat` runs a lean path; `SessionMode::Research` runs intent analysis, planning, delegation, and ward transitions. Memory runs in both. [Source](gateway/gateway-execution/src/config.rs). | Keep both runtime paths and make the user-facing choice understandable. |
| UI shell | A React web dashboard has separate `/chat` and `/research` routes, with Research as the default route and ten primary navigation items. [Source](apps/ui/src/App.tsx). | Test a session-first shell with tools/settings secondary to conversation. |
| Research | Research already has turn histories, subagent timelines, a context inspector, a ward vault, artifacts, and Stop. [Source](apps/ui/src/features/research-v2/ResearchPage.tsx). | Preserve these capabilities; reorganize their presentation around task state and review. |
| Chat | Quick Chat already has a compact conversation, inline activity chips, artifacts, and Stop. [Source](apps/ui/src/features/chat-v2/QuickChat.tsx). | Preserve its low-friction path; test whether sessions should be browseable beside Research sessions. |
| Transport | Both UI hooks call `executeAgent`; Research sends `deep`, while Chat sends its chat mode. Both subscribe to events and use `cancelSession`. [Chat](apps/ui/src/features/chat-v2/useQuickChat.ts); [Research](apps/ui/src/features/research-v2/useResearchSession.ts). | Investigate a shared session/event presentation layer before changing gateway semantics. |
| Hook names | `EngineHook` is an internal runtime extension point. `gateway-hooks` is an inbound-trigger/response-routing abstraction for CLI, cron, and connectors. [Runtime](runtime/agent-runtime/src/engine/hooks.rs); [Gateway](gateway/gateway-hooks/src/lib.rs). | Introduce a clearly named lifecycle-hook subsystem; do not conflate it with response routing. |
| Desktop delivery | `apps/` contains daemon, CLI, and React UI; the release RFC covers binary installers and explicitly leaves OS signing for later. [Architecture](docs/architecture/architecture.md); [Release RFC](docs/rfc/0004-github-release-installer-and-packaging.md). | Plan desktop app lifecycle, daemon ownership, packaging, signing, and updates as one deliverable. |

The product context calls zbot a goal-oriented local-first agent and emphasizes autonomous execution, cross-session memory, multiple providers, and persistent wards. Those are constraints for evaluating an interface change, not evidence that the current interface works well. [Source](docs/product/product-context.md).

## Initial product hypotheses to validate

- **H1 — Session-first shell:** One session list contains Chat and Research, with clear mode labels, status, project/ward context, and a single place to resume work. Test whether users can find and resume a task faster than in the current route-based navigation.
- **H2 — Progressive execution detail:** Default view shows the user request, concise progress, decisions, and result; an expandable activity view shows tools, subagents, logs, and provenance. Test comprehension and trust against the current Research page.
- **H3 — Explicit control boundary:** A session exposes task scope and action authority before execution, and an interrupt/steer/approval path during execution. First audit what controls the gateway already supports; then prototype only the missing interactions.
- **H4 — Reviewable outputs:** Research reports, citations, generated files, and code changes should be reachable from the turn that produced them and from a session-level output area. Test whether this reduces time to verify a result.
- **H5 — Quiet main UI without losing knowledge:** Default desktop window has a compact session list, one conversation canvas, a top-center mode control, a composer, and an optional Activity/Sources/Files panel. Keep Memory and Observatory as discoverable sidebar destinations, and reveal memory used by a turn in its activity/provenance. Move agent administration and integrations into secondary surfaces. Test whether first-time users can start and resume Chat or Research without understanding the internal architecture, while experienced users can inspect and correct durable knowledge.
- **H6 — Codex-like hook layer:** Start with the events zbot can reliably expose end to end (session start/end, prompt submit, before/after tool, stop/interrupt), a declared configuration file, and trust review before command execution. Map additional Codex events only when their timing and output semantics can be honored. Test existing Codex-style hook scripts against a conformance matrix rather than promising full compatibility from similar names.

These are hypotheses, not accepted requirements. None requires removing the research harness, chat mode, local-first data model, or provider choice.

## Known unknowns

- **Known-unknown:** Which specific frustrations make zbot feel fundamentally wrong today: visual density, latency, autonomy, reliability, session recovery, or action safety? Close with user walkthroughs and a task-based audit of current zbot sessions.
- **Known-unknown:** Which desktop shell should own `zbotd` and the React UI, and which OS integrations belong in the first release? Close with a small Tauri/Electron or native-shell spike covering startup, shutdown, local transport, installers, and signing.
- **Known-unknown:** Does “hooks work like Codex” require file-format and script compatibility, or behaviorally equivalent lifecycle events? Close with a hook compatibility matrix and a few representative scripts.
- **Known-unknown:** What approval and steering controls already exist end to end across gateway, runtime, and UI? Close with a focused event/API inventory and live-session tests.
- **Known-unknown:** Which Research and Chat behaviors are contractual for current users? Close with session fixtures, usage evidence, and interviews before redesigning their paths.
- **Unknowable from desk research:** Whether these patterns improve zbot users' task completion or trust. That requires prototypes and observed use; competitor documentation cannot establish it.

## Next research pass

1. Observe three representative zbot tasks: a short chat, a multi-source research request, and a code/ward task. Record entry friction, mode choice, state visibility, intervention, result review, and resume behavior.
2. Inventory gateway events, pause/resume/cancel/approval semantics, durable session records, and the two UI reducers. Distinguish presentation gaps from engine gaps.
3. Produce two low-fidelity interaction concepts: a unified session shell and a task workspace with Chat/Research as modes. Evaluate both against the findings and product constraints above.
4. Prototype a thin desktop shell that launches and reconnects to the daemon, then close the shell choice with measured startup, package size, OS support, and security boundaries.
5. Specify lifecycle hook event timing, configuration precedence, trust, input/output JSON, blocking behavior, timeouts, and observability against the official Codex contract.

## Source limits

Official product documentation describes intended behavior and may change quickly. It does not provide independent outcome data or internal architecture for either competitor. The HCI study supplies general interaction guidance, but predates current agent products. Zbot observations here are static code inspection; no usability sessions or live UI walkthroughs were performed.
