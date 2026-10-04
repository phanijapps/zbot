# Spec: File-configured external hooks

- **Status:** Shipped
- **Owner:** @videogamer
- **Plan:** [`plan.md`](plan.md)
- **Constrained by:** [Engine hook contract](../engine-hook-framework/spec.md) for existing zbot semantics; [RFC-0021](../../rfc/0021-conversation-first-desktop-agent.md) for trusted definitions; Rig 0.43.0 migration
- **Brief:** docs/product/briefs/rig-hooks-session-graph.md
- **Discovery:** none
- **Contract:** [hooks.schema.json](../../../contracts/jsonschema/hooks.schema.json), [hook-event.schema.json](../../../contracts/jsonschema/hook-event.schema.json), [hook-response.schema.json](../../../contracts/jsonschema/hook-response.schema.json), [session-details.yaml](../../../contracts/openapi/session-details.yaml)
- **Shape:** mixed
- **Mode:** full (dependency, interface, security, or user-facing structural change)

> **Spec contract:** Objective, Boundaries, Testing Strategy, and Acceptance Criteria define delivery. The implementation matches this contract or changes it in the same reviewed work.

## Objective

The operator edits <vault>/config/hooks.json to attach ordered external commands to defined gateway/Rig lifecycle events. Python scripts, Node .mjs files and compiled programs receive a bounded JSON event and return a validated response. Matching root/delegated runs invoke the commands and expose durable, sanitized activity. Missing/disabled hooks leave existing execution unchanged.

## Boundaries

### Always do

- Resolve the configured vault through VaultPaths; keep the empty starter file and editor schema usable.
- Snapshot validated configuration at ingress and share its revision across child executions; apply schema defaults and reject duplicate IDs.
- Keep hook effects subordinate to tool authorization, existing before-tool veto and protected runtime controls.

### Ask first

- Add hook configuration/Test controls to Settings or enable per-token process spawning.
- Add a remote hook service, interpreter installation, ward/project hook trust, or new host-privilege/sandbox policy.

### Never do

- Add a second executor or general plugin abstraction; interpret argv using an implicit shell.
- Register a hook from retrieved content, model output, skills or project discovery.
- Persist raw stdin, stdout, stderr, tool arguments/results or executable paths in session activity.

## Testing Strategy

TDD validates config defaults, unique IDs, event response legality, deterministic order and error policy. Real subprocess integration fixtures prove Python, Node and native executable invocation, literal argv, cwd, stdin/EOF, timeout, output bounds and cleanup without touching the live vault. Gateway/runtime integration checks cover Chat/Research, root/delegated, resume and stream/non-stream event counts. HTTP/UI tests and manual rendered review prove bounded hook activity survives reload and remains accessible.

## Acceptance Criteria

- [x] **AC1 — File loading: absent config means no configured hooks; version-1 config validates against hooks.schema.json and applies declared defaults. Invalid syntax/schema, duplicate IDs or oversized config prevents that new invocation with a bounded diagnostic, without replacing its prior configuration or executing a command.**
- [x] **AC2 — Snapshot: each accepted root invocation owns one configuration snapshot; children inherit it. File edits affect the next root invocation and neither alter an in-flight run nor restart it. Disabled hooks spawn zero processes.**
- [x] **AC3 — Event cardinality: session_start fires once at creation of a session on accepted root ingress; user_prompt once per accepted root message; run_start/run_end once per root or child execution attempt; before_model/after_model once per provider attempt; before_tool/after_tool once per admitted tool dispatch/result; invalid_tool_call once per rejected model call. Child jobs do not manufacture root user-prompt or session events; opening/reloading a session is read-only.**
- [x] **AC4 — Command invocation: command is executable plus literal arguments. v1 cwd is the vault. Ward/project-relative program execution is rejected rather than implicitly trusted. A JSON event is written to stdin, stdin closes, and output follows the response schema. Python, Node mjs and native fixtures observe their expected arguments/payload.**
- [x] **AC5 — Protocol: at exit zero, empty stdout means continue; nonempty stdout is exactly one bounded response JSON object. stderr is diagnostics, not instructions. Nonzero exit, malformed/unsupported output or size overrun follows on_failure. Explicit block is honored only at declared pre-action events. Context additions are accepted only at session_start, user_prompt, run_start and before_model; no response grants tool access or rewrites model, arguments or authorization.**
- [x] **AC6 — Composition: matching commands run sequentially in file order within an event. A block stops later configurable hooks for that event and prevents the pending operation. Existing protected checks still apply; continue or a later hook cannot undo a protected veto. Concurrency across independent tool calls does not share mutable hook-event state.**
- [x] **AC7 — Bounds and cleanup: config ≤256 KiB, stdin ≤64 KiB, stdout/stderr each ≤32 KiB and accumulated context ≤8 KiB UTF-8 per invocation. Timeout covers spawn, stdin, wait and drain. Timeout/cancellation terminates and reaps child processes and descendants under the supported host strategy; no retry repeats a side-effecting hook automatically.**
- [x] **AC8 — Environment and trust: processes receive a documented minimal environment and literal paths; provider/MCP credentials and the ambient environment are not forwarded. Operator-installed programs run with the daemon user’s host rights; hooks are not a sandbox or authorization boundary. Config/scripts are operator-owned, never sourced from an API/model-generated registration path; the Program trust rule below is tested before any spawn. A single recursive credential-redaction projection filters arguments before serialization; nested keys/header/env containers and registered provider/MCP secret sentinels are covered by negative tests. Unprojectable arguments are omitted with an explicit flag, never sent raw.**
- [x] **AC9 — Durable activity: event/run/session/agent/hook IDs, status, duration and exit code are stored as bounded metadata; running, completed, blocked, failed, timeout, cancelled and skipped states have distinct labels. Reloaded session details reproduce those labels without raw payloads, script diagnostics, paths or credentials.**
- [x] **AC10 — No hooks settings UI: the operator guide and validated file examples are sufficient to configure hooks. Session Activity uses existing tokens and keyboard-accessible disclosure at desktop and narrow widths.**
- [x] **AC11 — Actual wiring: integration tests prove events reach the configured programs on root/delegated Chat/Research and both model delivery modes. A merely parsed file or registered callback is not acceptance evidence.**

## Assumptions

- Technical: VaultPaths/config layout and the version-1 configuration schema exist; actual callback registration is not yet implemented (runtime/agent-primitives/src/vault_paths.rs; contracts/jsonschema/hooks.schema.json).
- Technical: Rig 0.43.0 hook source has run lifecycle, completion, dispatch and outcome callbacks; synchronous model selection is intentionally excluded (versioned source linked in the brief).
- Product: external Python/Node/Go-compatible commands and file-only management are user-confirmed on 2026-10-03.
- Process: JSON Schema and existing REST projection are the interface types; no API-contract authoring skill is installed, so direct authored contracts receive schema validation and secure-design review.

## Protocol rules

The event/response schemas define the complete wire surface. Ingress has invocation/session identity while run/turn/tool IDs remain null until authoritative IDs exist. Model events include provider/model identity and outcome category, without instructions or completion content. Tool events include the allowed tool name and pre-call arguments after the credential projection below; post-call events contain status only. Payloads exceeding the byte limit fail before spawn rather than silently truncate.

Configured context is attributed hook data added through existing context assembly below protected instructions, never a system prompt or automatic memory write. The aggregate byte limit is enforced across hooks and repeated model attempts. Block responses cannot also add context. Response reason remains private to the invocation and is never a UI label/log field. Runtime validation rejects event-illegal actions/context even when the standalone response schema validates.

Run-end is observation-only, including cancellation. After cancelling/reaping the active command, settlement invokes run-end with a separate bounded cleanup deadline, capped at five seconds total across its configured observers. It does not inherit the already-cancelled token; shutdown still aborts/reaps cleanup. Exhausted cleanup budget records remaining observers as skipped. Existing user Stop settles the agent immediately; observer cleanup cannot restart it.

Operator commands intentionally have the daemon user’s filesystem/network rights. Minimal environment removes implicit credential forwarding, not access to files already readable by that user. This contract introduces no security claim against a separately authorized unrestricted host shell. Automatic hook registration from agents/skills/remote inputs is unavailable; ordinary model-writable project files are not a configuration discovery source.

Credential projection is one shared helper, reused if a matching sanctioned helper exists. Normalize JSON key names to lowercase ASCII alphanumerics. Omit a whole subtree when its key contains token, password, secret, credential, authorization, cookie, apikey, accesskey or privatekey; omit header/headers/env/environment containers so header name/value arrays cannot bypass key filtering. Visit arrays and objects recursively. Replace registered provider/MCP secret-value substrings in remaining string values using the existing secret accessor, without persisting the matching values. Never read secret files or ambient environment to build this matcher. If projection/validation fails, send arguments={} and arguments_redacted=true; any removal/replacement also sets this flag. The executed tool still receives its original admitted arguments. This rule minimizes known credential exposure; it does not promise detection of all private prose in the user’s intentionally forwarded prompt.

Program trust for v1 is deliberately narrow: the operator installs/reviews programs and scripts outside ward/project roots, preferably <vault>/config/hooks/. Every canonical executable and configured script file must be owned by the daemon user or the OS administrator and not group/world writable (or equivalent restrictive host ACL); symlink resolution occurs before checking location/ownership. Bare executables resolve only through the permitted PATH and receive the same checks. Relative program/script arguments resolve from the vault. Any resolved command program/script path inside a ward/project root is rejected before spawn; cwd:ward is schema-invalid. Python/Node fixtures use operator-installed script files, not scripts discovered in a repository. Interpreter invocation in v1 is a file-form command (python SCRIPT [args] or node SCRIPT [args]); inline eval/module loaders are rejected rather than inferring dynamically executed files. Other language runtimes require the same provable file-form mapping; compiled programs need only executable validation. Tests cover project script paths, ward cwd, symlink escapes and writable files. This validates operator-installed entry points, not every dependency an operator program may deliberately load; it grants no sandbox guarantee. Project-supplied hooks remain deferred to a separately reviewed trust/change-invalidation contract under RFC-0021.
