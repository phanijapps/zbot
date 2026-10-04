# RFC-0021: Conversation-First Desktop Agent

- **Status:** Accepted
- **Author:** zbot maintainers
- **Approver:** zbot maintainer
- **Date opened:** 2026-09-26
- **Date closed:** 2026-09-27
- **Decision weight:** heavy
- **Related:** [desktop-agent desk research](../../zbot-desktop-agent-experience-survey.md), [product context](../product/product-context.md), [RFC-0004 (release installers)](0004-github-release-installer-and-packaging.md), [RFC-0007 (ward-specific Vault in Research)](0007-ward-specific-vault-in-research.md), [RFC-0011 (memory-engine cutover)](0011-engram-memory-engine-cutover.md)

## Reviewer brief

- **Decision:** Make a conversation-first desktop application the primary zbot experience while retaining its distinct Chat and Research execution paths, Memory, Observatory, and ward file exploration.
- **Recommended outcome:** Accept the experience contract and staged delivery below; require evidence gates before selecting a desktop shell or claiming Codex hook-file compatibility.
- **Change if accepted:**
  - Bring fragmented route-level session surfaces into a shared shell: top-center Chat/Research switch, readable conversation, and optional Activity/Sources/Files panel.
  - Keep Memory, Observatory, and the Research ward explorer directly reachable, and make memory used in a turn inspectable without filling the default conversation with internals.
  - Deliver a desktop host and a user-configurable lifecycle-hook subsystem through separately verified implementation slices.
- **Affected surface:** React UI, session/event presentation, daemon lifecycle and packaging, runtime hook boundaries, and local action permissions.
- **Stakes:** Costly but reversible. Current web routes and data contracts remain available during migration; untrusted executable hooks create a security boundary.
- **Review focus:** Whether simplification preserves control and provenance, whether memory and graph workflows remain first-class, and whether hook/desktop authority is bounded.
- **Not in scope:** Replacing the Research harness, merging Chat and Research runtime modes, changing memory storage ownership under RFC-0011, redesigning Observatory's graph algorithms, or choosing Tauri/Electron in this RFC.

## The ask

**Recommendation:** Approve one session-centered product shell, not one merged agent pipeline. A user chooses Chat or Research at the top of the conversation; both modes share navigation and review surfaces while continuing to use their current execution semantics. Memory and Observatory remain named destinations. A thin desktop host and lifecycle hooks follow only after their contracts and trust checks pass.

Zbot's product promise is long-running, goal-oriented work with cross-session learning, yet its current React application spreads Chat, Research, Memory, Observatory, and other capabilities across broad route-level navigation. Research is already the default route; the problem is fragmented session surfaces, not a dashboard landing page. Memory is the local store of durable facts and learned beliefs that users can inspect and correct; Observatory is the explorable graph of entities and relationships built from that knowledge. The user-approved concept simplifies the main experience without removing those capabilities. The [desk research](../../zbot-desktop-agent-experience-survey.md) and [current UI routes](../../apps/ui/src/App.tsx) establish the baseline; they do not prove the new shell works in use.

| ID | Question | Recommendation | Why | Decide by | Reviewer action |
| --- | --- | --- | --- | --- | --- |
| D1 | What is the primary interaction model? | Shared conversation/session shell with a top-center Chat/Research switch and contextual details. | One place to start, resume, inspect, and review while preserving two runtime paths. | RFC sign-off | Accept or amend the UI contract. |
| D2 | How do Memory and Observatory fit? | Direct sidebar destinations plus turn-level memory provenance; keep Observatory a full graph workspace. | They are product capabilities, not settings or extra tabs competing with Activity/Sources/Files. | RFC sign-off | Accept or amend discoverability and provenance rules. |
| D3 | What is the desktop and hook commitment? | Thin desktop host around the daemon; a distinct Codex-like lifecycle-hook layer with opt-in trust controls and measured compatibility. | Reuses the existing runtime and avoids treating internal callbacks or inbound gateway hooks as a user hook API. | Before shell/hook specs are approved | Accept the boundary; do not infer a framework or blanket compatibility promise. |

## Problem & goals

The current UI has separate `/chat`, `/research`, `/memory`, and `/observatory` routes and broad primary navigation. Chat is for a quick exchange; Research is for a longer task that may plan, delegate, use tools, and produce a reviewable result. Research already carries turns, tool activity, artifacts, a ward file explorer, and session controls; Chat has a leaner path and artifacts. Memory exposes facts, beliefs (derived claims), and contradictions (conflicting claims); Observatory exposes the knowledge graph. These are useful capabilities, but the route structure makes them feel like separate products rather than one agent with different depths of work. [Routes](../../apps/ui/src/App.tsx), [Research page](../../apps/ui/src/features/research-v2/ResearchPage.tsx), [Chat](../../apps/ui/src/features/chat-v2/QuickChat.tsx), [Memory](../../apps/ui/src/features/memory/command-deck/MemoryTab.tsx), [Observatory](../../apps/ui/src/features/observatory/ObservatoryPage.tsx).

Goals:

- Start and resume Chat or Research from one calm, keyboard-accessible desktop surface.
- Preserve access to Research's ward file explorer alongside session artifacts; a ward is a persistent project workspace, not just a collection of files produced by one session.
- Preserve Research's planning, delegation, artifact, and cancellation behavior and Chat's quick path; no implicit mode switch during an active turn.
- Make progress, action requests, sources, generated files, and memory use inspectable and truthful to persisted events.
- Keep Memory editable/inspectable and Observatory explorable, directly reachable from the main shell.
- Reuse zbot's local daemon, provider choice, ward context, and existing persistence contracts rather than building a second agent engine.
- Expose lifecycle hooks with explicit event timing, trust, permissions, limits, and a conformance matrix before compatibility claims.

Non-goals:

- No rewrite of Memory's backing engine or Observatory's graph layout; [RFC-0011](0011-engram-memory-engine-cutover.md) owns memory-engine parity and preserves zbot's UI/read-model contracts.
- No removal of current web/CLI access during desktop introduction.
- No automatic elevation of a hook script's privileges, and no assumption that visual activity is a security audit log.
- No copying ChatGPT's branding or exact interface.

## Proposal

### D1 — One session shell, two execution modes

Use a slim left rail for New chat, Search, recent sessions, Projects, Memory, Observatory, and Settings. A **ward** is zbot's persistent project workspace; the UI may call it a project where that is clearer. Put the Chat/Research switch at the top center of the conversation. Show the current mode and ward/project context on every session; selecting an existing session restores its recorded mode. Switching mode while viewing an idle existing session starts a new session in the selected mode and leaves the original session unchanged. The switch is unavailable during an active turn until the turn completes or the user stops it; a running session is never silently converted.

The center column is the task and answer. A concise status row expands into the execution trace. The optional right panel has exactly three primary tabs: **Activity**, **Sources**, and **Files**. Activity includes plan, tool, delegation, hook, memory-recall, permission, and error events only when the underlying session exposes them. Sources are server-resolved records explicitly cited in the answer or emitted by a structured source-use event; a merely visited page stays in Activity. Files are server-resolved session artifacts, not a general filesystem browser. In Research, the session's project/ward control opens the ward-scoped explorer alongside the transcript, with file search and slide-out preview; Projects also offers full-page access. Browse, search, preview, and ward-scoping parity must pass before the old Research layout is retired. This preserves the intent of [RFC-0007](0007-ward-specific-vault-in-research.md) without crowding the three-tab panel. The panel never treats model or hook text as an openable path: local files require artifact IDs resolved under approved roots, with canonical-path and symlink checks; external source links allow only safe web schemes. Missing provenance yields an honest empty state, not fabricated citations or inferred tool steps.

Both modes continue to use their existing runtime semantics: Chat is the lean path, Research the planning/delegation path. The shared shell is a presentation and navigation boundary, not a new monolithic session engine. Existing routes remain during migration until session recovery, streaming, cancellation, and artifact parity pass. [Execution modes](../../gateway/gateway-execution/src/config.rs), [Research session hook](../../apps/ui/src/features/research-v2/useResearchSession.ts), [Chat session hook](../../apps/ui/src/features/chat-v2/useQuickChat.ts).

### D2 — Memory and Observatory are first-class but contextual

Memory remains a named sidebar destination for searching, inspecting, correcting, and managing durable knowledge, including the existing facts/beliefs/contradictions views. A turn's Activity can disclose that memory was recalled or written, with a path to inspect a record where the backend can provide an authorized stable reference. Activity defaults to redacted, non-secret labels; revealing full content requires deliberate inspection under the record's access rules. Session export or sharing must explicitly choose whether memory provenance is included. The UI must not imply that a memory affected an answer unless that provenance is recorded. Future memory controls should use existing zbot ownership and authorization rules.

Observatory remains a named, full-page graph workspace. A session can deep-link to relevant entities or relationships only when it has a stable graph identifier; otherwise the sidebar opens the general Observatory. This avoids squeezing the graph into the three-tab details panel or reducing it to decorative statistics. The desktop shell must preserve the existing Observatory data and interaction contract while its navigation changes. [Memory UI](../../apps/ui/src/features/memory/command-deck/MemoryTab.tsx), [Observatory UI](../../apps/ui/src/features/observatory/ObservatoryPage.tsx), [RFC-0011](0011-engram-memory-engine-cutover.md).

### D3 — Thin desktop host and lifecycle hooks

The desktop host owns window lifecycle, daemon startup/reconnect/shutdown, local transport, update-channel integration, and OS permissions. This RFC does not require automatic self-update; [RFC-0004](0004-github-release-installer-and-packaging.md) explicitly leaves it and signing/notarization to follow-on release work. The daemon remains the authority for execution, stores, and providers. Desktop transport defaults to loopback-only access; any remote binding is a separate, explicit configuration. Before release, require per-user authentication or an unguessable host-issued token, Origin checks for browser and WebSocket clients, coverage of the worker bridge endpoint, and negative tests for unauthorized cross-origin and LAN access. Choose the host framework only after a small cross-platform spike measures startup/reconnect, package size, signing/update path, local endpoint protection, and failure recovery. Keep browser UI and CLI as supported clients while the host is introduced. [Architecture](../architecture/architecture.md).

Add a **lifecycle-hook subsystem** distinct from `EngineHook` (internal runtime callbacks) and `gateway-hooks` (inbound trigger/response routing). The first contract should cover session start/end, prompt submission, before/after tool, stop/interrupt, and permission-request points (moments when daemon policy requires the user's approval) where zbot can prove correct ordering and cancellation. Hook definitions are opt-in. The local user must review and trust a project-supplied hook before its command runs; changing its definition invalidates that trust. Hooks run with a sanitized, allowlisted environment and scoped working directory: provider keys, daemon credentials, and vault secrets are absent by default; any secret access needs an explicit brokered grant. Each event specifies matcher rules (which events/tools trigger it), JSON input/output, timeout, output cap, ordering, error behavior, and whether it may deny or amend (rewrite) an action. A hook cannot grant authority the user or daemon policy did not already grant. After any amendment, the daemon revalidates the final action immediately before its side effect and asks the user again if the prior approval no longer covers it. Activity records original and final action metadata and the hook outcome without leaking secrets or raw sensitive payloads by default. [Engine hooks](../../runtime/agent-runtime/src/engine/hooks.rs), [gateway hooks](../../gateway/gateway-hooks/src/lib.rs), [Codex hook reference](https://learn.chatgpt.com/docs/hooks).

“Codex-like” is a behavior target, not a blanket claim that Codex configuration files or scripts run unchanged. A follow-on compatibility matrix must test representative event/matcher/input/output cases and explicitly list exceptions before the UI or docs claim compatibility. Codex itself documents local-tool coverage exceptions, so hook outcomes are not a substitute for the daemon's permission boundary. [Codex hook reference](https://learn.chatgpt.com/docs/hooks).

## Options considered

The D1 UI axis is how much of the product shares the first screen. **Do nothing** preserves mature pages but leaves the fragmented entry experience. **Put every capability into one conversation pane** removes navigation but overloads the reading surface and weakens graph/memory workflows. **Use a shared conversation shell with direct deep destinations** is recommended because it simplifies ordinary work without flattening specialist views.

The D2 knowledge-navigation axis is where Memory and Observatory live. **Leave them only as current route-level destinations** preserves function but misses the new shell's discoverability and per-turn provenance. **Collapse them into the Activity/Sources/Files panel** keeps one window but makes correction and graph exploration cramped. **Keep direct sidebar destinations with contextual memory provenance** is recommended; Observatory remains a full workspace and the right panel stays focused on the current session.

The desktop axis is who owns agent execution. **Keep web-only** avoids packaging but does not deliver the requested desktop agent. **Rewrite execution inside the desktop process** duplicates authority and persistence. **Use a thin host around `zbotd`** is recommended; the specific host technology remains an empirical choice.

The hook axis is where user scripts attach. **Do nothing** leaves only internal callbacks and cannot meet the requested lifecycle behavior. **Expose `EngineHook` or `gateway-hooks` directly** confuses internal extension points with a stable, trusted user API. **Add a separate user-facing lifecycle layer over proven runtime events** is recommended, with narrowly advertised compatibility.

## Risks & what would make this wrong

- **False simplification:** If common Research controls become hard to find, completion or review may slow. Retain a one-action path to Activity and compare representative tasks against the current UI before retiring routes.
- **Mode confusion:** If a mode switch changes a live task unexpectedly, users may lose work. Pin mode per session; define any transition separately and test resume/reload behavior.
- **Provenance fiction:** Sources, memory, and hook rows can look authoritative while being inferred. Render only backed records; expose unknown/empty states and verify them against persisted session data.
- **Memory/graph/ward regression:** Navigation redesign could strand Memory corrections, Observatory graph interactions, or Research's ward explorer. Preserve their routes and contract tests through migration; require ward explorer parity before retirement of the old Research layout. RFC-0011 remains the memory-engine authority.
- **Desktop privilege expansion:** A host or local endpoint could widen access to files, commands, or credentials. Require threat-model review and negative tests of process ownership, default loopback binding, authentication, browser/WS origin checks, bridge access, OS permissions, and update trust before shipping.
- **Hook command execution:** Project-controlled hook files or model-influenced arguments can execute unsafe commands or exfiltrate secrets. Require explicit trust, sanitized environment, scoped execution, no privilege escalation, bounded time/output, redacted diagnostics, and independent daemon policy enforcement. Failures at a permission boundary must not silently allow the blocked action.
- **Compatibility overclaim:** Similar event names can mask different timing or result semantics. Publish a versioned conformance table and mark unsupported combinations rather than promising drop-in Codex parity.

## Evidence & prior art

- The [desk-research survey](../../zbot-desktop-agent-experience-survey.md) records the current code baseline, alternative UI patterns, and unresolved host/hook questions. Its competitor findings are directional, not outcome evidence.
- [ChatGPT Work and Codex documentation](https://help.openai.com/en/articles/20001275-chatgpt-work-and-codex) describes a top-of-page Chat/Work switch and shared Recents in its desktop app. This supports the interaction pattern, not a claim that zbot should copy its implementation.
- [Claude Code desktop documentation](https://code.claude.com/docs/en/desktop) describes task controls and permission modes; it supports making authority and progress visible, not a claim of effectiveness for zbot users.
- [Codex hooks documentation](https://learn.chatgpt.com/docs/hooks) specifies event inputs, decisions, trust and tool-coverage limits. It is the reference for the requested behavior and the reason compatibility needs a conformance test.
- The [zbot product context](../product/product-context.md) makes autonomous goals, local-first data, provider choice, and cross-session memory core principles. This RFC changes their presentation, not their ownership.

## Experiment / validation

1. Have at least five representative users run the same short Chat task, multi-source Research task, and ward-backed task (work using a persistent project workspace) in the current UI and a clickable shared-shell prototype. Record time to start/resume, whether users can state the active mode and task status, whether they can find the result/source/file, and whether they can open Memory, Observatory, and the ward explorer. Before replacing the default route, require every participant to find Stop, the current mode, the result, and the ward explorer for a ward task without coaching; require at least four of five to find Memory and Observatory without coaching; investigate any slower median start/resume time before approval.
2. Replay persisted sessions with streaming, cancellation, reload, artifacts, and memory recall. Require parity of final answer, task status, artifacts, and backed provenance before switching the default route. Negative-test forged source/file paths, absolute paths, symlink escapes, unauthorized memory references, and unsafe link schemes. Treat missing source/memory identifiers as an explicit gap, not a UI-only fix.
3. Spike at least two viable desktop-host approaches against one test matrix: daemon cold start, reconnect after crash, clean shutdown, default loopback binding, unauthorized LAN and cross-origin requests (HTTP, WebSocket, and worker bridge), installer/update/signing path, and package footprint. Record the choice in an ADR after results, not by preference alone.
4. Build hook fixtures for representative allow/deny/rewrite, timeout, crash, malformed output, delegated-agent, and interruption cases. Require every denial to prevent the protected action, every timeout/error to follow its declared policy, no hook to bypass daemon permissions, and a rewritten privileged action to receive a fresh policy and consent check. Verify provider keys and daemon/vault secrets are absent from the hook environment, input, output, and diagnostics by default; then publish only the compatible subset.

## Open questions

1. **Which desktop host and first supported OS set?** Default: preserve the web client and choose the smallest host meeting the spike matrix. Owner: desktop implementer and zbot maintainer. Decide by: desktop packaging spec approval.
2. **How much Codex hook-file/schema compatibility is worth supporting?** Default: behaviorally equivalent lifecycle events with a tested subset; no drop-in claim. Owner: runtime/security maintainers. Decide by: hook spec approval.
3. **What source and memory identifiers are durable enough for per-turn provenance?** Default: show only existing stable references; omit a row when unavailable. Owner: gateway and UI maintainers. Decide by: session-view spec approval.

## Follow-on artifacts

- A session-shell spec with mode, recents, Activity/Sources/Files, Memory/Observatory navigation, ward explorer parity, accessibility, and reload/cancellation acceptance tests.
- A desktop-host spike and ADR, then a packaging/lifecycle spec with security review.
- A hook-event inventory, trust-model and conformance spec; keep the execution-policy boundary independent of hooks.
- A provenance contract spec only if current session/source/memory event records are insufficient for the proposed UI.

## Errata

### 2026-09-27 — Ward explorer visibility and future administration

The accepted proposal's visible Research ward-explorer affordance and its discoverability/parity gate are superseded for the new session shell. Keep the existing explorer implementation and legacy direct-route access for compatibility and rollback, but do not show an explorer link or panel in the new shell. Its absence from the new navigation is intentional, not a missing feature.

The shell is also the foundation for future in-app agent creation, settings, customization, and lifecycle-hook management. Those administration flows require separate contracts and are not required to ship the first session-shell slice. This clarification does not authorize new hook execution behavior or alter the trust boundaries above.
