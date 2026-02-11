## Target Behavior

* Worker connects once (WS/gRPC stream/SSE) and **subscribes**.
* AgentZero **pushes** outbound messages instantly to the right worker.
* If the worker disconnects:

  * AgentZero writes outbound to **outbox (SQLite)** as `pending`
  * When worker reconnects, AgentZero **replays** pending items (or worker requests replay once).

So it’s event-driven, but still reliable.

---

## Architecture

```
+--------------------------+           +----------------------------------------+
|  Channel Worker (Slack)  |<--WS/SSE-->|        AgentZero Bridge Gateway        |
|  Channel Worker (Teams)  |  (push)    |  - inbound endpoint                    |
|  Channel Worker (Email)  |           |  - push channel (subscriptions)        |
+------------+-------------+           |  - outbox (sqlite) for durability      |
             |                         +------------------+---------------------+
             | inbound webhook/poll/stream               |
             v                                           v
      channel-native events                      Agent runtime + memory
             |
             +-- POST /bridge/inbound  ------------------------>
                        (envelope + normalized)
```

---

## Protocol: Worker ↔ AgentZero (Push Channel)

### Option A (recommended): **WebSocket**

Simple, bidirectional, great for desktops/servers.

**Worker opens WS:**
`GET /bridge/ws?adapter_id=slack-worker-1`

First message from worker:

```json
{
  "type": "hello",
  "adapter_id": "slack-worker-1",
  "capabilities": ["outbound.message", "outbound.file"],
  "accepts": {
    "intent_kinds": ["message.v1"],
    "native_types": ["slack.blocks.v2"]
  },
  "resume": { "last_acked_outbox_id": "..." }
}
```

AgentZero replies:

```json
{ "type": "hello_ack", "server_time": "...", "heartbeat_seconds": 20 }
```

**Outbound push from AgentZero → worker**

```json
{
  "type": "outbox_item",
  "outbox_id": "uuid",
  "destination": { "address": { "...": "..." } },
  "correlation": { "channel": "slack", "conversation_id": "...", "in_reply_to": "..." },
  "intent": { "kind": "message.v1", "text": "..." },
  "native": null
}
```

**Worker ACK**

```json
{ "type": "ack", "outbox_id": "uuid" }
```

If send fails:

```json
{ "type": "fail", "outbox_id": "uuid", "error": "rate_limited", "retry_after_seconds": 30 }
```

**Heartbeat**
Either side can ping:

```json
{ "type": "ping" }  -> { "type": "pong" }
```

### Option B: **gRPC streaming**

Same semantics, more typing, great if you already use gRPC heavily.

### Option C: **SSE + POST ack**

SSE is one-way server→worker; worker still POSTs acks.

---

## Core Reliability Mechanism (still needed): SQLite Outbox

Even with push, keep outbox:

1. Agent produces reply → write outbox row `pending`
2. If worker is connected:

   * push immediately
   * on ACK → mark `sent`
3. If worker is not connected:

   * keep `pending`
   * on reconnect → replay pending items

This prevents “agent finishes but can’t respond”.

### Replay strategy

On worker `hello` include `resume.last_acked_outbox_id` (or last_acked_timestamp).
AgentZero sends any `pending` (and optionally `leased`) items after that.

---

## Inbound Flow (still HTTP/event-driven)

Worker receives channel event, posts to AgentZero:

`POST /bridge/inbound`

Body = InboundEnvelope (same as before):

* envelope + `normalized.text`
* `reply_to` includes `adapter_id` and channel-specific `address`

AgentZero:

* dedupe by `(channel, conversation_id, message_id)`
* map `(channel, conversation_id)` to `session_id` (create if missing)
* append “turn” to session
* agent runs
* agent emits outbound reply → outbox → push

---

## Routing: How AgentZero knows which worker to push to

Use `reply_to.adapter_id` from inbound message.

* In group chat, that adapter_id will be the worker that received it.
* All future replies in that conversation should default to the last seen `reply_to` (store per session).

Add a small table:
**session_routes**

* `session_id`
* `default_adapter_id`
* `default_address_json`
* `updated_at`

So even if agent triggers an outbound later (timer/tool), it knows where to send.

---

## Minimal APIs (push-based)

You only need:

1. `POST /bridge/inbound`
2. `GET /bridge/ws` (WebSocket)
3. (optional) `POST /bridge/outbox/ack` and `/fail` **if not using WS for ack**

No polling endpoint needed.

---

## State Machine for Outbox Items

`pending -> inflight -> sent`

* `pending`: stored, not yet delivered
* `inflight`: pushed, waiting for ack (set `lease_until` to avoid stuck inflight)
* `sent`: acked

If `lease_until` expires without ack:

* revert to `pending` and retry (or mark failed after N attempts)

This solves “worker died after receiving but before ack”.

---

## What to Tell Your Coding Agent to Build (Implementation Tasks)

### Phase 1: Foundation

* [ ] Define JSON structs for `InboundEnvelope`, `OutboxItem`, WS messages
* [ ] SQLite migrations: `inbound_dedup`, `bridge_sessions`, `session_routes`, `outbox`, `workers`
* [ ] Implement `/bridge/inbound`:

  * dedupe
  * create/find session
  * update session_routes with reply_to
  * enqueue agent turn

### Phase 2: Push Channel

* [ ] Implement WS server `/bridge/ws`
* [ ] Track connected workers: `adapter_id -> connection`
* [ ] On `hello`, record worker in DB and memory; run replay:

  * find `pending` outbox for adapter_id; push them
* [ ] Implement ack/fail handlers:

  * ack => mark sent
  * fail => increment attempts, set next_attempt_at

### Phase 3: Agent → Outbox → Push

* [ ] Add a “BridgeEmitter” in agent runtime:

  * `emit_reply(session_id, text, attachments, native?)`
  * writes outbox row
  * tries push if worker connected
* [ ] Ensure it uses `session_routes` to determine destination if agent didn’t specify

### Phase 4: Worker SDK (thin)

* [ ] Provide a tiny reference client (Rust/Go/Node):

  * connect WS, send hello, handle outbox_item, send ack/fail, ping
  * helper to POST /bridge/inbound
* [ ] Build first worker: Signal or Slack to validate

### Phase 5: Deprecation Path

* [ ] Keep old connectors unchanged
* [ ] Add a config switch:

  * “use bridge v2” for selected channels
* [ ] Once stable, migrate channels and remove old connectors

---

## Answer to the “push-only” concern

Push-only without outbox will fail the moment a worker is offline.
So the winning combo is:

✅ **Push for realtime**
✅ **Outbox for durability**
✅ **Replay on reconnect**

It still feels event-driven and instant.
