# External hooks

Attach reviewed local programs to agent lifecycle events through
`<vault>/config/hooks.json`. The default vault is `~/Documents/zbot`.
Startup installs an empty version-1 starter and `hooks.schema.json` only when
those files do not exist. Existing files, including operator edits, are preserved.
An empty, missing, or entirely disabled configuration executes no hook programs.
There is no Settings editor.

## Install a hook

Create a directory outside Git repositories, wards, and configured project roots.
`<vault>/config/hooks/` is the recommended location. Programs and script files
must belong to the daemon user or root and must not be group/world writable.
Executables also need execute permission. Symlinks resolve before these checks;
a link into a project or writable file is rejected. These programs run with the
daemon user's host rights: this is not a sandbox. Review their dependencies and
effects before enabling them.

For a Python observer, save this as `config/hooks/observe.py`:

```python
import json
import sys

event = json.load(sys.stdin)
print(json.dumps({"version": 1, "action": "continue"}))
```

Then edit `config/hooks.json`:

```json
{
  "$schema": "./hooks.schema.json",
  "version": 1,
  "hooks": [
    {
      "id": "python-observer",
      "event": "run_end",
      "command": ["python3", "config/hooks/observe.py"]
    }
  ]
}
```

Set the configuration and script permissions to `0600`, for example with
`chmod 600 config/hooks.json config/hooks/observe.py` from the vault directory.
The interpreter must already be installed in the permitted PATH or supplied by
an absolute path. Relative program and script paths resolve from the vault.

Start a new Chat or Research message. Expand the observer's row in the session's
**Activity** panel to inspect its event, agent, run, duration, and exit code.
Reloading the session reads recorded activity and does not execute hooks.

## Node and compiled Go

Save a Node observer as `config/hooks/observe.mjs`:

```javascript
let input = "";
for await (const chunk of process.stdin) input += chunk;
const event = JSON.parse(input);
process.stdout.write(JSON.stringify({ version: 1, action: "continue" }));
```

Use `"command": ["node", "config/hooks/observe.mjs"]` and script permissions
`0600`. Python and Node use script-file forms. Inline evaluation, module loaders,
package launchers, and runtime options before the script are rejected.

For Go, compile the program before registering it. Save this as an operator-owned
`observe.go` outside a project root:

```go
package main

import (
    "encoding/json"
    "os"
)

func main() {
    var event map[string]any
    if json.NewDecoder(os.Stdin).Decode(&event) != nil {
        os.Exit(1)
    }
    if json.NewEncoder(os.Stdout).Encode(map[string]any{
        "version": 1, "action": "continue",
    }) != nil {
        os.Exit(1)
    }
}
```

Build with `go build -o observe-go observe.go`, install the binary under
`config/hooks/`, and set its permissions to `0700`. Use
`"command": ["config/hooks/observe-go"]`. Other compiled executables use the same
form. `go run`, shell command strings, and implicit builds are not supported.
Each command element is a literal argument; there is no implicit shell expansion.

## Events and responses

| Event | When it runs | Block | Context |
| --- | --- | --- | --- |
| `session_start` | First accepted root message, including a pre-created empty Chat | Yes | Yes |
| `user_prompt` | Each accepted root message | Yes | Yes |
| `run_start` | Each actual root, child, or continuation attempt | Yes | Yes |
| `run_end` | Each attempt's settlement, including Stop | No | No |
| `before_model` | Each actual provider attempt, including retries | Yes | Yes |
| `after_model` | Each provider attempt's outcome | No | No |
| `before_tool` | Admitted tool dispatch, after protected policy | Yes | No |
| `after_tool` | Admitted tool outcome | No | No |
| `invalid_tool_call` | Rejected model tool call | Yes | No |

Programs receive one bounded version-1 JSON event on stdin, followed by EOF.
Use the [event schema](../../../contracts/jsonschema/hook-event.schema.json)
for the exact envelope. IDs correlate the invocation and actual runtime attempt;
run/turn IDs are null before the runtime establishes them. Tool arguments have
credential fields removed and registered credential values scrubbed; failed
projection supplies an empty object and an explicit flag. Original admitted tool
arguments remain unchanged.

Exit zero with **zero stdout bytes** to continue, or emit exactly one JSON object:

```json
{"version": 1, "action": "continue"}
```

A permitted pre-action can return `{"version":1,"action":"block"}`. An optional
`reason` is private and never becomes a log, Activity label, or chat/tool response.
A permitted context event can return:

```json
{"version": 1, "action": "continue", "context": "Operator-provided context."}
```

Context is attributed untrusted data below protected instructions. It cannot
grant tool access or rewrite provider settings, instructions, or tool arguments.
A block response cannot also supply context. Unknown fields, explicit null optional
fields, code fences, multiple objects, whitespace-only stdout, invalid UTF-8, and
event-illegal responses fail validation. Stderr carries diagnostics, never
instructions, and is not persisted or exposed through Activity.

## Configuration and limits

The [configuration schema](../../../contracts/jsonschema/hooks.schema.json) and
[response schema](../../../contracts/jsonschema/hook-response.schema.json) are
the exact contracts. Each hook has a unique lowercase ID and one event.
Hooks execute sequentially in file order; a block skips subsequent hooks for that
event. Protected policy always applies first. Defaults are `enabled: true`,
`cwd: "vault"`, `timeout_ms: 5000`, and `on_failure: "continue"`.
`on_failure: "block"` is permitted only for block-capable pre-actions. Stop always
cancels execution, even when failure policy is continue.

| Limit | Maximum |
| --- | ---: |
| Configuration | 256 KiB / 100 hooks |
| Command | 64 elements / 4096 characters per element |
| Event stdin | 64 KiB |
| Stdout and stderr | 32 KiB each |
| Aggregate context per invocation | 8 KiB UTF-8 |
| Configured timeout | 300,000 ms |

The timeout covers the full process operation, including stdin and concurrent
output draining. Commands are never automatically retried. Stop and future
cancellation terminate the command process group; direct children are reaped.
Settlement observers have a separate **total five-second** cleanup budget;
observers beyond that budget are skipped. Terminal observers cannot block or add
context. Stop settles the agent immediately while bounded cleanup runs.

The environment contains only `PATH=/usr/local/bin:/usr/bin:/bin` and
`LANG=C.UTF-8`. HOME, ambient environment variables, and provider/MCP credentials
are not forwarded. Explicitly provision any resources the reviewed program needs.

## Host support, edits, and recovery

Execution is currently verified only on Linux with systemd as PID 1. Unsupported
or unverified hosts reject configured commands before spawning them. Windows,
macOS, and containers without the verified reaper are not supported for execution;
empty/disabled hooks still leave normal bot execution available.
Ordinary descendants in the command's group are terminated and reaped by host
init. Programs must not daemonize into another session/process group; this is not
containment against adversarial programs.

A validated snapshot belongs to one accepted root invocation. Its children and
continuations retain that revision. Editing the file affects the next new root
message. Read-only creation, hydration, and reload execute no hook programs.
Invalid configuration prevents the new invocation before commands execute.

Accepted-message claims prevent automatically replaying ingress hooks after a
restart. A persisted root resume requires the same configuration revision and
restores consumed context bytes; hook context text remains ephemeral. Changed
revision or unavailable child ownership requires a new explicit root message.
Claims do not promise exactly-once external effects across a process/storage crash.

Activity stores only bounded IDs, event, fixed status, duration, and exit code.
Running, completed, blocked, failed, timed-out, cancelled, and skipped outcomes
are distinct. Startup marks interrupted Running rows cancelled without replaying
commands. Raw prompts, context, arguments, results, paths, diagnostics, and
private reasons are excluded. Fetch errors preserve previously loaded rows and
offer Retry; server truncation is stated explicitly.

The file ownership checks do not protect hook files from a tool or process
already authorized to run an unrestricted host shell as the daemon user. There
is no model-facing hook registration API or automatic project discovery.
