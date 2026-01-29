# Timeline Zero: Server-Based Agent Architecture

## Executive Summary

This plan outlines the evolution of AgentZero from a desktop-first Tauri application to a server-based architecture with a web dashboard, inspired by [moltbot](https://github.com/moltbot/moltbot). The goal is to decouple the agent runtime from the UI, enabling headless CLI operation while providing a rich dashboard experience.

---

## Architecture Comparison

### Current AgentZero Architecture
```
┌─────────────────────────────────────────────┐
│                 TAURI APP                    │
├─────────────────────────────────────────────┤
│  ┌──────────────┐    ┌──────────────────┐  │
│  │    React     │◄──►│  Tauri Bridge    │  │
│  │   Frontend   │IPC │  (Rust Backend)  │  │
│  └──────────────┘    └──────────────────┘  │
│                             │               │
│                      ┌──────▼──────┐       │
│                      │ Agent       │       │
│                      │ Runtime     │       │
│                      │ (zero-*)    │       │
│                      └─────────────┘       │
└─────────────────────────────────────────────┘
```

### Moltbot Architecture (Inspiration)
```
┌─────────────────────────────────────────────────────────────┐
│                         GATEWAY                              │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌──────────────┐  │
│  │WebSocket│  │  HTTP   │  │   CLI   │  │   Channels   │  │
│  │ Server  │  │ Server  │  │         │  │(WA/TG/Slack) │  │
│  └────┬────┘  └────┬────┘  └────┬────┘  └──────┬───────┘  │
│       └────────────┴────────────┴───────────────┘          │
│                          │                                  │
│                   ┌──────▼──────┐                          │
│                   │  Pi Agent   │                          │
│                   │   Runtime   │                          │
│                   └─────────────┘                          │
└─────────────────────────────────────────────────────────────┘
         ▲                                    ▲
         │ WebSocket                          │ WebSocket
┌────────┴────────┐                  ┌────────┴────────┐
│   Web Dashboard │                  │  Canvas/A2UI    │
│   (React)       │                  │                 │
└─────────────────┘                  └─────────────────┘
```

### Proposed Timeline Zero Architecture
```
┌─────────────────────────────────────────────────────────────┐
│                    ZERO DAEMON (Rust)                        │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────┐  │
│  │                    GATEWAY                            │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌────────┐  │  │
│  │  │WebSocket│  │  HTTP   │  │  gRPC   │  │ Events │  │  │
│  │  │ :18790  │  │ :18791  │  │ :18792  │  │  Bus   │  │  │
│  │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬───┘  │  │
│  │       └────────────┴────────────┴────────────┘      │  │
│  └──────────────────────────────────────────────────────┘  │
│                            │                                │
│  ┌─────────────────────────▼────────────────────────────┐  │
│  │                   AGENT RUNTIME                       │  │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────────────┐ │  │
│  │  │ Sessions  │  │   Tools   │  │  MCP Integration  │ │  │
│  │  │ (SQLite)  │  │   Layer   │  │                   │ │  │
│  │  └───────────┘  └───────────┘  └───────────────────┘ │  │
│  └──────────────────────────────────────────────────────┘  │
│                            │                                │
│  ┌─────────────────────────▼────────────────────────────┐  │
│  │                   CHANNELS (FUTURE)                   │  │
│  │  ┌───────┐  ┌──────────┐  ┌───────┐  ┌───────────┐  │  │
│  │  │WebChat│  │ Telegram │  │ Slack │  │  Discord  │  │  │
│  │  └───────┘  └──────────┘  └───────┘  └───────────┘  │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
         ▲              ▲                    ▲
         │              │                    │
┌────────┴────┐  ┌──────┴──────┐  ┌─────────┴─────────┐
│ zero CLI    │  │Web Dashboard│  │  Tauri App        │
│             │  │  (React)    │  │  (lightweight UI) │
└─────────────┘  └─────────────┘  └───────────────────┘
```

---

## Key Concepts from Moltbot

### 1. Gateway as Control Plane
Moltbot uses a WebSocket gateway at `ws://127.0.0.1:18789` as the central control plane. All clients (CLI, web, channels) connect through this single endpoint.

**Adoption Strategy:**
- Create a standalone `zero-gateway` binary
- Support WebSocket + HTTP endpoints
- Use JSON-RPC for commands, streaming for events

### 2. Pi Agent Runtime
Moltbot's embedded agent runner handles:
- Context window management and compaction
- Tool call sanitization
- Subagent spawning
- Abort/cancellation

**Adoption Strategy:**
- Our `zero-agent` + `executor_v2` already implements most of this
- Add context compaction (already in plan)
- Expose via gateway instead of Tauri IPC

### 3. A2UI (Agent-to-User Interface)
Agents can generate UI through declarative JSON. Components are rendered client-side.

**Adoption Strategy:**
- Define a `ZeroUI` JSON schema for agent-driven UI
- React components that render ZeroUI specs
- Allow agents to create dashboards, forms, tables dynamically

### 4. Multi-Channel Support
Moltbot supports 13+ messaging platforms through unified channel abstraction.

**Adoption Strategy (Future):**
- Define `Channel` trait in Rust
- Start with WebChat (already have)
- Add Telegram, Slack, Discord integrations later

### 5. Daemon/Service Architecture
Moltbot runs as a background service on macOS (launchd), Linux (systemd), Windows (Task Scheduler).

**Adoption Strategy:**
- Create `zero daemon start|stop|status` commands
- Platform-specific service installers
- Auto-restart on crash

---

## Implementation Phases

### Phase 1: Gateway Extraction
**Goal:** Separate agent runtime from Tauri into standalone daemon

**Files to Create:**
```
crates/
├── zero-gateway/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── server.rs        # WebSocket + HTTP server
│       ├── router.rs        # Request routing
│       ├── session_manager.rs
│       └── events.rs        # Event broadcasting

application/
├── zero-daemon/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs          # Daemon entry point
```

**Key Changes:**
1. Extract `executor_v2.rs` logic into `zero-gateway`
2. Replace Tauri IPC with WebSocket messages
3. Add HTTP API for REST-style operations
4. Keep Tauri app as a thin client connecting via WebSocket

**API Design:**
```
WebSocket ws://localhost:18790/
├── connect              → session token
├── agent.invoke         → stream events
├── agent.stop           → stop execution
├── agent.list           → list agents
├── tool.call            → call tool directly
├── mcp.list             → list MCP servers
└── mcp.tools            → list MCP tools

HTTP http://localhost:18791/
├── GET  /api/agents           → list agents
├── GET  /api/agents/:id       → get agent
├── POST /api/agents/:id/invoke → invoke agent (returns WebSocket ticket)
├── GET  /api/sessions         → list sessions
├── GET  /api/sessions/:id     → get session
└── GET  /api/health           → health check
```

### Phase 2: CLI Interface
**Goal:** Full-featured CLI for headless operation

**Files to Create:**
```
application/
├── zero-cli/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── daemon.rs     # daemon start|stop|status
│       │   ├── agent.rs      # agent list|invoke|stop
│       │   ├── chat.rs       # interactive chat mode
│       │   ├── mcp.rs        # mcp list|enable|disable
│       │   └── vault.rs      # vault list|create|switch
│       └── tui/
│           ├── mod.rs
│           └── chat.rs       # Terminal chat UI (ratatui)
```

**Commands:**
```bash
# Daemon management
zero daemon start         # Start daemon in background
zero daemon stop          # Stop daemon
zero daemon status        # Check if daemon is running
zero daemon logs          # Tail daemon logs

# Agent operations
zero agent list           # List all agents
zero agent invoke <name>  # Invoke agent (streams to stdout)
zero agent chat <name>    # Interactive chat mode
zero agent stop <id>      # Stop running agent

# MCP servers
zero mcp list             # List MCP servers
zero mcp enable <id>      # Enable MCP server
zero mcp disable <id>     # Disable MCP server
zero mcp tools <id>       # List tools from MCP server

# Vault management
zero vault list           # List vaults
zero vault create <name>  # Create new vault
zero vault switch <name>  # Switch active vault

# Development
zero dev                  # Start daemon + web dashboard
```

### Phase 3: Web Dashboard
**Goal:** Standalone web dashboard for visual control

**Approach Options:**

**Option A: Reuse AgentZero React Components**
- Keep existing React components
- Replace Tauri IPC calls with WebSocket/HTTP
- Build with Vite for standalone web app
- Serve from daemon on port 18793

**Option B: Fresh React App (Moltbot-style)**
- New lightweight React app
- Focus on dashboard features:
  - Agent status overview
  - Session history
  - Real-time event streams
  - MCP server management
- Consider using their A2UI patterns

**Recommended: Option A with progressive enhancement**
```
src/
├── core/
├── shared/
├── features/
│   ├── agent-channels/     # ✅ Reuse (replace IPC)
│   ├── workflow-ide/       # ✅ Reuse (replace IPC)
│   ├── agents/             # ✅ Reuse (replace IPC)
│   ├── mcp/                # ✅ Reuse (replace IPC)
│   └── dashboard/          # 🆕 New - overview page
├── services/
│   ├── gateway.ts          # 🆕 WebSocket client
│   ├── api.ts              # 🆕 HTTP client
│   └── *.ts                # Refactor to use gateway/api
└── App.tsx                 # Conditional Tauri/Web mode
```

**Service Layer Refactor:**
```typescript
// services/gateway.ts
export class GatewayClient {
  private ws: WebSocket;

  async connect(url = 'ws://localhost:18790') { ... }
  async invoke(agentId: string, message: string): AsyncGenerator<StreamEvent> { ... }
  async stop(sessionId: string): Promise<void> { ... }

  // Event subscriptions
  onAgentEvent(callback: (event: AgentEvent) => void): Unsubscribe { ... }
}

// services/api.ts
export const api = {
  agents: {
    list: () => fetch('/api/agents').then(r => r.json()),
    get: (id: string) => fetch(`/api/agents/${id}`).then(r => r.json()),
    // ...
  },
  sessions: { ... },
  mcps: { ... },
};
```

### Phase 4: Tauri as Thin Client
**Goal:** Keep Tauri app as optional desktop wrapper

**Changes:**
1. Tauri app connects to daemon via WebSocket (same as web dashboard)
2. Add daemon auto-start on app launch
3. Keep Tauri-specific features:
   - System tray icon
   - Native notifications
   - File dialogs
   - Keyboard shortcuts

**New Tauri Commands:**
```rust
#[tauri::command]
async fn ensure_daemon_running() -> Result<bool, String> {
    // Check if daemon is running, start if not
}

#[tauri::command]
async fn get_daemon_url() -> String {
    "ws://localhost:18790".to_string()
}
```

### Phase 5: Agent-Driven UI (A2UI Inspired)
**Goal:** Allow agents to generate dynamic UI components

**ZeroUI Schema:**
```typescript
interface ZeroUISpec {
  version: "1.0";
  components: ZeroUIComponent[];
}

interface ZeroUIComponent {
  id: string;
  type: "text" | "card" | "table" | "form" | "chart" | "button" | "input";
  props: Record<string, unknown>;
  children?: string[];  // Child component IDs
  bindings?: {
    data?: string;      // State path
    onClick?: string;   // Action name
  };
}

// Example: Agent generates a task dashboard
{
  "version": "1.0",
  "components": [
    {
      "id": "header",
      "type": "text",
      "props": { "variant": "h1", "content": "Project Tasks" }
    },
    {
      "id": "task-table",
      "type": "table",
      "props": {
        "columns": ["Task", "Status", "Assignee"],
      },
      "bindings": { "data": "tasks" }
    }
  ]
}
```

**React Renderer:**
```typescript
function ZeroUIRenderer({ spec, state, onAction }) {
  return (
    <div>
      {spec.components.map(component => (
        <ZeroUIComponent
          key={component.id}
          component={component}
          state={state}
          onAction={onAction}
        />
      ))}
    </div>
  );
}
```

---

## Migration Path

### Backwards Compatibility
- Tauri app continues to work standalone (daemon embedded)
- Existing vault structure unchanged
- SQLite database format unchanged

### Gradual Migration
1. **Week 1-2:** Gateway extraction, CLI basics
2. **Week 3-4:** Web dashboard, service layer refactor
3. **Week 5-6:** Tauri thin client, testing
4. **Future:** Multi-channel, A2UI

---

## File Changes Summary

### New Crates
| Crate | Purpose |
|-------|---------|
| `zero-gateway` | WebSocket/HTTP gateway server |
| `zero-daemon` | Standalone daemon binary |
| `zero-cli` | Command-line interface |

### Modified Files
| File | Changes |
|------|---------|
| `src/services/*.ts` | Refactor to use gateway client |
| `src-tauri/src/main.rs` | Optional daemon connection |
| `Cargo.toml` | Add workspace members |
| `package.json` | Add web build target |

### New Frontend Files
| File | Purpose |
|------|---------|
| `src/services/gateway.ts` | WebSocket client |
| `src/services/api.ts` | HTTP client |
| `src/features/dashboard/` | Overview dashboard |

---

## Success Metrics

1. **CLI Fully Functional:** Can invoke agents, chat, manage MCP without UI
2. **Web Dashboard Works:** Access dashboard from any browser on LAN
3. **Tauri App Unchanged UX:** Existing features work via daemon
4. **Performance Maintained:** No latency increase from gateway hop

---

## Open Questions

1. **Authentication:** How to secure gateway for LAN/remote access?
   - Option: Bearer tokens, API keys
   - Consider: mTLS for production deployments

2. **Multi-User:** Should gateway support multiple concurrent users?
   - Start: Single-user (personal AI assistant)
   - Later: Add user sessions if needed

3. **Persistence:** Where should gateway store state?
   - Use existing SQLite in vault
   - Gateway is stateless, runtime manages state

4. **Deployment:** How to package for easy installation?
   - Single binary with embedded assets
   - Docker image
   - Platform-specific installers

---

## References

- [Moltbot Repository](https://github.com/moltbot/moltbot)
- [A2UI Framework](https://github.com/moltbot/moltbot/tree/main/vendor/a2ui)
- [Moltbot Documentation](https://github.com/moltbot/moltbot/tree/main/docs)
