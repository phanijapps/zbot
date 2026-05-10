# UI

Web dashboard for AgentZero. React 19 + TypeScript + Vite.

## Structure

```
ui/
├── src/
│   ├── features/               # Feature modules
│   │   ├── agent/              # Agent management
│   │   ├── chat/               # Chat panel (streaming)
│   │   ├── chat-v2/            # Chat panel v2
│   │   ├── integrations/       # Provider / MCP management
│   │   ├── logs/               # Execution logs dashboard
│   │   ├── memory/             # Memory / knowledge graph
│   │   ├── mission-control/    # Session oversight
│   │   ├── observatory/        # Real-time monitoring
│   │   ├── research-v2/        # Research sessions
│   │   ├── settings/           # App settings
│   │   └── setup/              # Onboarding wizard
│   ├── services/
│   │   └── transport/          # HTTP/WebSocket client
│   ├── hooks/                  # Shared React hooks
│   ├── components/             # Shared components
│   └── shared/                 # Types, utilities
├── public/                     # Static assets
├── index.html                  # Entry point
├── vite.config.ts
└── package.json
```

## Development

```bash
cd apps/ui && npm install
npm run dev       # Dev server (port 3000)
npm run build     # Production build → dist/
npm run preview   # Preview production build
npm run test      # Vitest unit tests
```

## Tech Stack

| Technology | Purpose |
|------------|---------|
| React 19 | UI framework |
| TypeScript | Type safety |
| Vite | Build tool |
| Tailwind CSS v4 | Styling |
| Radix UI | Accessible primitives |
| Lucide | Icons |

## API Integration

The transport layer (`src/services/transport/`) abstracts HTTP/WebSocket:

```typescript
const transport = await getTransport();
await transport.invoke({ agent_id, conversation_id, message });
```

WebSocket delivers `GatewayEvent` JSON for real-time token streaming.

## Build Output

Production build goes to `apps/ui/dist/`. The daemon serves it via `--static-dir ./dist`.
Copy or symlink from workspace root as needed.

## UI Architecture

See `apps/ui/ARCHITECTURE.md` for component patterns, styling conventions, and state management details before making UI changes.
