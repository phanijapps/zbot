# Gateway

Network layer providing HTTP and WebSocket APIs for agent interaction. This is the integration point that composes runtime + services.

## Structure

```
gateway/
├── src/
│   ├── http/           # REST API routes
│   ├── websocket/      # WebSocket handler
│   ├── execution/      # Agent invocation + delegation
│   ├── database/       # SQLite persistence
│   ├── services/       # Agent, Provider, Skill services
│   ├── events/         # Event bus for streaming
│   └── hooks/          # Inbound triggers (webhooks, cron)
└── templates/          # System prompt templates
```

## Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| 18791 | HTTP | REST API + Static files |
| 18790 | WebSocket | Real-time streaming |

## HTTP Endpoints

### Agents
- `GET /api/agents` - List agents
- `POST /api/agents` - Create agent
- `GET /api/agents/:id` - Get agent
- `PUT /api/agents/:id` - Update agent
- `DELETE /api/agents/:id` - Delete agent

### Conversations
- `GET /api/conversations` - List conversations
- `POST /api/conversations` - Create conversation
- `GET /api/conversations/:id/messages` - Get messages

### Logs
- `GET /api/logs/sessions` - List execution sessions
- `GET /api/logs/sessions/:id` - Get session detail

### Other
- `GET /api/health` - Health check
- `GET /api/providers` - List LLM providers
- `GET /api/skills` - List skills
- `GET /api/tools` - List tools

## WebSocket Protocol

```typescript
// Client sends
{ type: "invoke", agent_id: string, conversation_id: string, message: string }
{ type: "stop", conversation_id: string }

// Server sends
{ type: "token", delta: string }
{ type: "tool_call", tool_name: string, args: object }
{ type: "tool_result", result: string }
{ type: "agent_completed", result: string }
```

## Responsibilities

- HTTP/WebSocket protocol handling
- Request routing and validation
- Composing runtime with services
- Static file serving (dashboard)
- Event broadcasting

## Dependencies

- `framework/zero-*` - Core abstractions
- `runtime/*` - Execution engine
- `services/*` - Data services
