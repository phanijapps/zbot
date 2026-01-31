# UI

Web dashboard for Agent Zero. React 19 + TypeScript + Vite.

## Structure

```
ui/
├── src/
│   ├── features/           # Feature modules
│   │   ├── agent/          # Chat + agent management
│   │   ├── skills/         # Skill management
│   │   ├── integrations/   # Provider management
│   │   ├── logs/           # Execution logs dashboard
│   │   └── cron/           # Scheduled tasks
│   ├── services/
│   │   └── transport/      # HTTP/WebSocket client
│   └── shared/             # UI components, types
├── public/                 # Static assets
├── index.html              # Entry point
├── vite.config.ts          # Vite configuration
└── package.json            # Dependencies
```

## Development

```bash
# Install dependencies
cd ui && npm install

# Start dev server (port 3000)
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview
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

## Key Features

### Chat Panel
- Real-time streaming via WebSocket
- Markdown rendering
- Tool call visualization
- Conversation history

### Logs Dashboard
- Execution session tree view
- Activity stream drill-down
- Filtering by agent/level
- Session metrics

### Agent Management
- Create/edit agents
- System prompt editor
- Model configuration

## API Integration

The transport layer (`src/services/transport/`) abstracts HTTP/WebSocket:

```typescript
const transport = await getTransport();
await transport.invoke({ agent_id, conversation_id, message });
```

## Build Output

Production build goes to `ui/dist/`. The daemon serves this via `--static-dir ./ui/dist`.
