# Hook Framework & Agent Delegation - Complete Overview

## Vision

A production-grade agent system with:
1. **Unified Hook System**: Single abstraction for all triggers (CLI, Web, Cron, WhatsApp, Telegram, Email, etc.)
2. **External Hook Support**: Polyglot adapters that connect via HTTP APIs (not compiled into gateway)
3. **Smart Response Routing**: Single `respond` tool that auto-routes to originating channel
4. **Agent Delegation**: Fire-and-forget to subagents with callback completion

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           INBOUND TRIGGERS                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  BUILT-IN (Part of Gateway)         EXTERNAL (Separate Services)       │
│  ┌─────┐  ┌─────┐  ┌──────┐        ┌──────────┐ ┌──────────┐           │
│  │ CLI │  │ Web │  │ Cron │        │ WhatsApp │ │ Telegram │  ...      │
│  └──┬──┘  └──┬──┘  └──┬───┘        │ (Node)   │ │ (Python) │           │
│     │        │        │            └────┬─────┘ └────┬─────┘           │
│     │        │        │                 │            │                  │
│     │   Direct call   │      POST /api/hooks/{id}/invoke               │
│     └────────┼────────┘                 └────────────┘                  │
│              │                               │                          │
│              ▼                               ▼                          │
├─────────────────────────────────────────────────────────────────────────┤
│                              GATEWAY                                    │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                        HookContext                                │  │
│  │  { hook_type, source_id, channel_id, callback_url, metadata }     │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                               │                                         │
│                               ▼                                         │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                     ExecutionRunner                               │  │
│  │  • Invokes agent with HookContext                                 │  │
│  │  • Passes context to tools (respond, delegate)                    │  │
│  │  • Manages delegation callbacks                                   │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                               │                                         │
│              ┌────────────────┼────────────────┐                        │
│              ▼                ▼                ▼                        │
│        ┌──────────┐    ┌────────────┐   ┌──────────────┐               │
│        │ respond  │    │ delegate   │   │ other tools  │               │
│        │   tool   │    │   tool     │   │              │               │
│        └────┬─────┘    └─────┬──────┘   └──────────────┘               │
│             │                │                                          │
│             ▼                ▼                                          │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                        HookRouter                                 │  │
│  │  Routes response to correct destination based on HookContext      │  │
│  └───────────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────────┤
│                          OUTBOUND RESPONSES                             │
│                                                                         │
│  BUILT-IN:                        EXTERNAL:                             │
│  • CLI  → stdout                  • HTTP callback to hook's URL         │
│  • Web  → WebSocket event         • With retry and auth                 │
│  • Cron → log only                                                      │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Task Breakdown

| Task | Title | Description | Dependencies |
|------|-------|-------------|--------------|
| 01 | Hook Context & Types | Foundation: HookContext, HookType, BuiltinHookType | - |
| 02 | Built-in Web Hook | WebSocket response routing via EventBus | 01 |
| 03 | Built-in Cron Hook | Scheduled execution with logging | 01 |
| 04 | External Hook Registration | CRUD API for external hooks | 01 |
| 05 | External Hook Invocation | API for hooks to trigger agents | 04 |
| 06 | Callback Response Routing | HTTP callbacks to external hooks | 05 |
| 07 | Respond Tool | Universal response tool using HookRouter | 02, 03, 06 |
| 08 | Agent Delegation | Fire-and-forget subagent pattern | 07 |

---

## Key Design Decisions

### 1. Built-in vs External Hooks

| Aspect | Built-in (CLI, Web, Cron) | External (WhatsApp, Telegram, etc.) |
|--------|---------------------------|-------------------------------------|
| Location | Part of gateway binary | Separate process |
| Language | Rust | Any (Node, Python, Go, etc.) |
| Communication | Direct function call | HTTP APIs |
| Response | Direct (stdout, WebSocket, log) | HTTP callback |
| Why | Core functionality, tight integration | Platform-specific, can vary independently |

### 2. HookContext Design

- **Travels with execution**: Every agent invocation carries HookContext
- **Immutable during execution**: Set at invocation, read by tools
- **Contains routing info**: hook_type, callback_url, auth
- **Enables `respond` tool**: Agent doesn't need to know destination

### 3. External Hook Protocol

**Registration:**
```
POST /api/hooks
{
  "id": "whatsapp-prod",
  "callback_url": "http://my-service:3000/callback",
  "callback_auth": "Bearer secret"
}
```

**Invocation:**
```
POST /api/hooks/{id}/invoke
{
  "source_id": "+1234567890",
  "message": "Hello agent"
}
→ 202 Accepted { "conversation_id": "..." }
```

**Callback (from gateway):**
```
POST {callback_url}
Authorization: {callback_auth}
{
  "type": "respond",
  "message": "Hello human!",
  "source_id": "+1234567890"
}
```

### 4. Agent Delegation

- **Fire-and-forget**: Parent delegates, doesn't block
- **Callback on completion**: Subagent result sent as message to parent
- **Task-scoped context**: Parent chooses what context to share
- **Original hook preserved**: Subagent's `respond` goes to user, not parent

---

## File Structure After Implementation

```
application/gateway/src/
├── hooks/
│   ├── mod.rs              # Module exports
│   ├── context.rs          # HookContext struct
│   ├── types.rs            # HookType, BuiltinHookType
│   ├── router.rs           # HookRouter for response routing
│   ├── builtin/
│   │   ├── mod.rs
│   │   ├── web.rs          # WebSocket responses
│   │   └── cron.rs         # Log-only responses
│   └── external/
│       ├── mod.rs
│       ├── config.rs       # ExternalHookConfig
│       ├── service.rs      # CRUD operations
│       ├── invocation.rs   # InvokeRequest/Response
│       └── mapper.rs       # ConversationMapper
├── delegation/
│   ├── mod.rs
│   ├── context.rs          # DelegationContext
│   └── handler.rs          # Callback handling
├── http/
│   ├── hooks.rs            # Hook CRUD + invoke endpoints
│   └── cron.rs             # Cron job endpoints
└── services/
    └── cron.rs             # CronService

application/agent-runtime/src/tools/
├── respond.rs              # Respond tool
└── delegate.rs             # Delegate tool

Config files:
├── hooks.json              # External hook registrations
└── cron.json               # Cron job configurations
```

---

## Verification Checklist

### Task 01: Hook Context
- [ ] `HookContext::builtin()` creates context for CLI/Web/Cron
- [ ] `HookContext::external()` creates context with callback_url
- [ ] Serialization roundtrip works

### Task 02: Web Hook
- [ ] WebSocket invoke creates HookContext
- [ ] Respond event published to EventBus
- [ ] Client receives response via WebSocket

### Task 03: Cron Hook
- [ ] Jobs saved to cron.json
- [ ] Jobs execute on schedule
- [ ] Responses logged (not sent anywhere)

### Task 04: External Hook Registration
- [ ] POST /api/hooks creates hook
- [ ] callback_auth not exposed in GET responses
- [ ] Duplicate ID returns 409

### Task 05: External Hook Invocation
- [ ] POST /api/hooks/{id}/invoke triggers agent
- [ ] Conversation mapped from source_id
- [ ] Returns 202 with conversation_id

### Task 06: Callback Routing
- [ ] respond tool triggers callback for external hooks
- [ ] Callback includes Authorization header
- [ ] Retry on failure (3 attempts)

### Task 07: Respond Tool
- [ ] Works for Web (WebSocket)
- [ ] Works for CLI (stdout)
- [ ] Works for External (callback)
- [ ] Returns error if no HookContext

### Task 08: Agent Delegation
- [ ] delegate_to_agent tool works
- [ ] Only allowed to defined subagents
- [ ] Callback sent to parent on completion
- [ ] Subagent respond goes to user (original hook)

---

## Each Task is Self-Contained

Every task file in `memory-bank/tasks/` contains:
1. **Context**: What we're building and why
2. **Specifications**: BDD-style Given/When/Then scenarios
3. **Implementation**: Complete code with file paths
4. **Verification**: Unit tests and API tests
5. **Dependencies**: What must be complete first
6. **Outputs**: Files created/modified

Tasks can be executed independently after clearing context.
