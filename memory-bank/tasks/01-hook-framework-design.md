# Task 01: External Hook Framework Design

## Objective
Design a hook framework where hooks are EXTERNAL services (Node.js, Python, Go, etc.) that plug into the gateway via HTTP APIs.

## Core Principle
**Hooks are external processes, not compiled into the gateway.**

The gateway provides:
1. HTTP endpoints for hook registration
2. Webhook callbacks for outbound responses
3. SSE/WebSocket streams for real-time events
4. Inbound endpoint for hooks to trigger agents

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     EXTERNAL HOOKS                          │
│  (Any language: Node.js, Python, Go, etc.)                  │
├─────────────────────────────────────────────────────────────┤
│  WhatsApp Hook     │  Telegram Hook    │  Email Hook        │
│  (Node.js)         │  (Python)         │  (Go)              │
└────────┬───────────┴────────┬──────────┴────────┬───────────┘
         │                    │                   │
         │ HTTP               │ HTTP              │ HTTP
         ▼                    ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│                      GATEWAY APIs                           │
├─────────────────────────────────────────────────────────────┤
│  POST /api/hooks              - Register hook               │
│  DELETE /api/hooks/{id}       - Unregister hook             │
│  GET /api/hooks               - List hooks                  │
│                                                             │
│  POST /api/hooks/{id}/invoke  - Hook triggers agent         │
│  GET /api/hooks/{id}/events   - SSE stream for responses    │
│                                                             │
│  (Gateway calls hook's callback_url for responses)          │
└─────────────────────────────────────────────────────────────┘
```

## Hook Registration Schema

```json
{
  "id": "whatsapp-prod",
  "name": "WhatsApp Production",
  "type": "whatsapp",
  "enabled": true,
  "config": {
    "callback_url": "http://localhost:3000/callback",
    "callback_auth": "Bearer xxx",
    "default_agent_id": "root",
    "timeout_ms": 30000
  },
  "metadata": {
    "phone_number_id": "123456789"
  }
}
```

## Flow: External Hook Triggers Agent

```
1. WhatsApp sends webhook to WhatsApp Hook (Node.js)
2. Hook parses message, extracts user phone + text
3. Hook calls: POST /api/hooks/whatsapp-prod/invoke
   {
     "source_id": "+1234567890",
     "channel_id": null,
     "message": "Hello agent",
     "metadata": { "message_id": "wamid.xxx" }
   }
4. Gateway creates/finds conversation for source_id
5. Gateway invokes agent with hook context
6. Agent executes, uses respond() tool
7. Gateway calls hook's callback_url:
   POST http://localhost:3000/callback
   {
     "hook_id": "whatsapp-prod",
     "source_id": "+1234567890",
     "message": "Hello! How can I help?",
     "conversation_id": "conv-uuid"
   }
8. WhatsApp Hook receives callback, sends via WhatsApp API
```

## Alternative: SSE Stream

If hook prefers real-time streaming instead of callbacks:

```
1. Hook subscribes: GET /api/hooks/whatsapp-prod/events (SSE)
2. Hook invokes: POST /api/hooks/whatsapp-prod/invoke
3. Gateway streams events to SSE connection:
   - event: agent_started
   - event: token (streaming)
   - event: respond (final response)
   - event: agent_completed
```

## Files to Create

| File | Purpose |
|------|---------|
| `src/hooks/mod.rs` | Hook types and storage |
| `src/hooks/config.rs` | Hook configuration structs |
| `src/hooks/service.rs` | HookService (CRUD, invoke, callback) |
| `src/http/hooks.rs` | HTTP endpoints |
| `hooks.json` | Persisted hook registrations |

## Gateway Responsibilities

1. **Store hook registrations** (hooks.json)
2. **Accept invocations** from hooks
3. **Track hook context** during execution
4. **Call callbacks** or stream events for responses
5. **Retry failed callbacks** with exponential backoff

## Hook (External Service) Responsibilities

1. **Register** itself with gateway on startup
2. **Handle platform webhooks** (WhatsApp, Telegram, etc.)
3. **Translate** platform format to gateway invoke format
4. **Receive callbacks** or subscribe to SSE
5. **Send responses** back to platform

## Key Design Decisions

1. **Callback vs SSE**: Support both. Callback is simpler, SSE enables streaming.
2. **Auth**: Hooks provide `callback_auth` header value for gateway to use.
3. **Conversation mapping**: Gateway maps `source_id` to conversations automatically.
4. **Multi-tenant**: Each hook has its own config, can use different agents.

## Next Tasks

- Task 02: Hook Configuration and Storage
- Task 03: Hook HTTP Endpoints
- Task 04: Hook Invocation Flow
- Task 05: Callback/SSE Response Routing
