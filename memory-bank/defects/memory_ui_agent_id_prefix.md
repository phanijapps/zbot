# [FIXED 2026-05-03] Defect — Memory UI sends `agent:root` but DB stores `root`

## Symptom

The memory page at `http://localhost:3000/memory` cannot delete (or otherwise
mutate) memory facts. Every DELETE returns:

```
HTTP/1.1 403 Forbidden
{"error":"Fact does not belong to this agent"}
```

Reproduced live on 2026-04-27 against a daemon with 50 real memory facts —
all DELETEs from the UI fail with the same 403.

## Reproduction

1. Run the daemon and UI (`http://localhost:3000`).
2. Open `/memory`, pick any fact, click delete.
3. The DELETE request goes to `/api/memory/agent%3Aroot/facts/<fact_id>` (i.e.
   URL-decoded path: `agent_id = "agent:root"`).
4. Response: 403 with `"Fact does not belong to this agent"`.

## Root cause

`apps/ui/src/features/memory/MemoryPage.tsx:11` hardcodes the agent id with
an `agent:` prefix:

```tsx
return on ? <MemoryTabCommandDeck agentId="agent:root" /> : <WebMemoryPanel />;
```

The actual `agent_id` stored on every memory fact in the DB is the bare
string `"root"` (verified by `curl http://127.0.0.1:18791/api/memory` —
all 50 facts have `"agent_id": "root"`). The DELETE handler at
`gateway/src/http/memory.rs:347` correctly enforces ownership:

```rust
match fact {
    Some(f) if f.agent_id == agent_id => { /* delete */ }
    Some(_) => 403 Forbidden — "Fact does not belong to this agent",
    None => 404,
}
```

Because the UI passes `"agent:root"` and the fact has `"root"`, the equality
check fails and the request is rejected. The same prefix would break any
other path-bound mutation (POST, PATCH) the UI sends through `/api/memory/:agent_id/...`.

## Why CI didn't catch this

The UI tests use the same broken value as the production code:

- `apps/ui/src/features/memory/command-deck/MemoryTab.test.tsx:95,104,108,113,123`
- `apps/ui/src/features/memory/command-deck/__tests__/MemoryTab.test.tsx:47`
- `apps/ui/src/features/memory/MemoryFactCard.test.tsx:13`

Each renders `<MemoryTab agentId="agent:root" />` and asserts the API mock was
called with `"agent:root"` — confirming the UI sends the wrong id consistently
rather than catching that the wrong id is sent.

There is no integration test that hits the daemon's real DELETE handler
with the UI's agent_id, which is the gap that allowed this to ship.

## Verification

Direct daemon DELETE with the correct id succeeds:

```bash
$ curl -s -w "\nHTTP %{http_code}\n" -X DELETE \
    "http://127.0.0.1:18791/api/memory/root/facts/fact-88971f13-77d0-4304-80fa-3920ade8bda8"

HTTP 204
```

## Fix

1. **`apps/ui/src/features/memory/MemoryPage.tsx:11`** — change
   `agentId="agent:root"` to `agentId="root"`.
2. **Test fixtures** — update the six test files listed above to use
   `agentId="root"`. While there, switch the assertions from "API called with
   `agent:root`" to "API called with `root`" so future drift is caught.
3. **Add a contract test** — exercise the real DELETE handler against a
   fact created via the UI's expected flow, asserting HTTP 204 (not asserting
   "the API was called with whatever string the UI happened to pass").

## Scope

UI-only fix. The daemon-side memory API is correct as-is. Should ship on
its own branch (e.g. `fix/ui-memory-agent-id`), not bundled with unrelated
work.

Unrelated to and predates Phase 1 of the persistence-readiness work
(`feature/phase1-kg-store-extraction`).

## Discovered

2026-04-27 — surfaced while smoke-testing memory operations during Phase 1
verification.
