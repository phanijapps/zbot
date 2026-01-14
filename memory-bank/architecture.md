# Agent Zero - Architecture Documentation

## Solution Architecture

### High-Level Overview

Agent Zero follows a **modular desktop architecture** with clear separation between the Rust backend (system access, file I/O, process management) and React frontend (UI, state management, user interactions).

```mermaid
graph TB
    subgraph "Desktop Application"
        subgraph "Frontend - React 19"
            UI[UI Layer]
            Core[Core Shell]
            Features[Feature Modules]
            Services[Service Layer]
        end

        subgraph "Backend - Rust"
            Commands[Tauri Commands]
            FileIO[File System]
            Proc[Process Manager]
        end
    end

    subgraph "Storage - File System"
        Agents[~/.config/zeroagent/agents/]
        Skills[~/.config/zeroagent/skills/]
        Config[providers.json, mcps.json]
    end

    subgraph "External"
        Providers[LLM Providers APIs]
        MCP[MCP Servers - stdio]
    end

    UI --> Core
    Core --> Features
    Features --> Services
    Services --> Commands
    Commands --> FileIO
    Commands --> Proc
    FileIO --> Agents
    FileIO --> Skills
    FileIO --> Config
    Proc --> MCP
    Services --> Providers
```

## Technical Architecture

### Frontend Structure

```mermaid
graph LR
    subgraph "Entry Point"
        Main[main.tsx] --> App[App.tsx]
    end

    subgraph "Core Layer - Routing & Layout"
        App --> Router[BrowserRouter]
        Router --> Shell[AppShell]
        Shell --> Sidebar[Sidebar]
        Shell --> Outlet[Route Outlet]
    end

    subgraph "Feature Modules"
        Outlet --> Agents[Agents Feature]
        Outlet --> Skills[Skills Feature]
        Outlet --> Providers[Providers Feature]
        Outlet --> MCP[MCP Feature]
        Outlet --> Settings[Settings Feature]
        Outlet --> Conv[Conversations Feature]
    end

    subgraph "Shared Layer"
        UI[UI Components]
        Types[Type Definitions]
        Utils[Utilities]
    end

    Agents --> UI
    Skills --> UI
    Providers --> UI
    MCP --> UI
```

### Backend Command Structure

```mermaid
graph TB
    subgraph "Tauri Commands"
        Lib[lib.rs] --> Agents[agents.rs]
        Lib --> Skills[skills.rs]
        Lib --> Providers[providers.rs]
        Lib --> MCP[mcp.rs]
        Lib --> Settings[settings.rs]
        Lib --> Conversations[conversations.rs]
        Lib --> Core[core.rs]
        Lib --> Windows[windows.rs]
    end

    subgraph "File Operations"
        Agents --> AgentFiles[Agent Files]
        Skills --> SkillFiles[Skill Files]
    end

    subgraph "Process Management"
        MCP --> MCPProcess[MCP stdio Processes]
    end

    subgraph "Storage"
        AgentFiles --> AgentStore[agents/]
        SkillFiles --> SkillStore[skills/]
    end
```

### Domain Organization

The codebase is organized by **business domain**, not technical layer:

```
src/
├── core/              # Cross-cutting (routing, layout)
│   ├── layout/        # AppShell, Sidebar, StatusBar
│   └── utils/         # Shared utilities
├── features/          # Feature modules
│   ├── agents/        # Agent management + IDE
│   ├── skills/        # Skill management + IDE
│   ├── providers/     # LLM provider config
│   ├── mcp/           # MCP server management
│   ├── settings/      # App settings
│   └── conversations/ # Chat interface
├── shared/            # Shared across features
│   ├── types/         # TypeScript definitions
│   ├── ui/            # UI component library
│   ├── constants/     # Routes, constants
│   └── utils/         # Helper functions
└── services/          # API abstraction layer
```

**Why this structure?**
- Easy to find code by feature
- Clear boundaries between features
- Independent feature development
- Better onboarding for new developers

## Data Flow

### Agent Creation Flow

```mermaid
sequenceDiagram
    participant U as User
    participant UI as Agent IDE
    participant S as Agent Service
    participant C as Tauri Command
    participant FS as File System

    U->>UI: Click "Add Agent"
    UI->>UI: Open Agent IDE (staging mode)
    U->>UI: Fill form + edit files
    UI->>S: writeAgentFile(staging, path, content)
    S->>C: write_agent_file("staging", path, content)
    C->>FS: Write to ~/.config/zeroagent/staging/

    U->>UI: Click "Save Agent"
    UI->>S: createAgent(agentData)
    S->>C: create_agent(agent)
    C->>FS: Create ~/.config/zeroagent/agents/{name}/
    C->>FS: Write config.yaml
    C->>FS: Write AGENTS.md
    C->>FS: Copy files from staging/
    C->>FS: Cleanup staging/
```

### File Explorer with Subdirectories

```mermaid
graph TB
    subgraph Frontend
        FE[File Explorer] --> BT[buildFileTree]
        BT --> NM["nodeMap: Map<path, node>"]
        BT --> RN["rootNodes: FileNode[]"]
        RN --> Rec["renderFileNode() recursive"]
    end

    subgraph Backend
        Cmd["list_agent_files()"] --> CF["collect_files() recursive"]
        CF --> RD["fs::read_dir()"]
        RD --> CF2["collect_files() subdirs"]
    end

    Cmd --> Files["File List"]
    Files --> FE
```

### Auto-Save Flow

```mermaid
graph LR
    Input[User typing] --> State["editingContent state"]
    State --> Effect["useEffect trigger"]
    Effect --> Check{Guard checks}
    Check -->|isNewAgent| Skip[Skip save]
    Check -->|unchanged| Skip
    Check -->|config.yaml| Skip
    Check -->|valid| Timer["setTimeout 1s"]
    Timer --> Save["writeAgentFile"]
    Save --> Update["Update lastSaved"]
    Update --> Display["Show saved time"]
```

### MCP Server Communication

```mermaid
sequenceDiagram
    participant A as Agent
    participant M as MCP Manager
    participant C as Child Process
    participant S as MCP Server

    A->>M: listMCPServers()
    M->>M: Return configured servers

    A->>M: Start agent with MCPs
    M->>C: Spawn stdio process
    C->>S: Execute server binary
    S-->>C: stdio communication

    A->>M: Invoke tool
    M->>C: Send JSON-RPC request
    C->>S: Forward request
    S-->>C: Response
    C-->>M: Result
    M-->>A: Return tool result
```

## Component Relationships

### Agent IDE Components

```mermaid
graph TB
    subgraph "Agent IDE"
        Page[AgentIDEPage] --> Config[ConfigYamlForm]
        Page --> Explorer[File Explorer]
        Page --> Editor[File Editor]
        Page --> Dialog[Close Confirm Dialog]

        Explorer --> Tree[File Tree]
        Explorer --> Context[Context Menu]
        Explorer --> Toolbar[Toolbar]

        Editor --> MD[MDEditor for .md]
        Editor --> Text[Textarea for others]
    end

    subgraph "Services"
        AgentSvc[agent.ts] --> Cmd[list_agent_files]
        AgentSvc --> Cmd[write_agent_file]
        AgentSvc --> Cmd[create_agent_folder]
    end

    Page --> AgentSvc
```

## State Management Strategy

### Local Component State (Preferred)

Most state is kept local to components:

```typescript
// File explorer state in Agent IDE
const [files, setFiles] = useState<AgentFile[]>([]);
const [selectedFile, setSelectedFile] = useState<AgentFile | null>(null);
const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());
```

**Why?**
- Simpler than global state
- State close to where it's used
- Easier to reason about
- No need for Zustand/Redux at current scale

### When to Use Global State (Future)

- User session info
- Active conversation state
- Global app settings

## Technology Decisions

### Tauri over Electron

| Aspect | Tauri | Electron |
|--------|-------|----------|
| Bundle Size | ~10MB | ~100MB+ |
| Memory | Lower | Higher |
| Backend | Rust | Node.js |
| Security | Smaller attack surface | Larger |
| Native Integration | Better | Good |

### React Router over Tauri Router

- Client-side routing (faster)
- Works with web standards
- Easier testing/debugging
- No IPC overhead for navigation

### Custom File Explorer over Tree View Library

- Full control over behavior
- Consistent styling with app
- Recursive pattern is straightforward
- No additional dependency

## Storage Patterns

### Agent Storage

```
~/.config/zeroagent/agents/
├── {agent-name}/
│   ├── config.yaml      # Metadata (name, provider, model, etc.)
│   ├── AGENTS.md        # Instructions (plain markdown)
│   └── [user files]/    # Additional resources
```

### Skill Storage

```
~/.config/zeroagent/skills/
├── {skill-name}/
│   ├── SKILL.md         # Frontmatter + markdown
│   ├── assets/          # Placeholder folder
│   ├── resources/       # Placeholder folder
│   └── scripts/         # Placeholder folder
```

### Configuration Files

```json
// ~/.config/zeroagent/providers.json
[{ "id": "openai", "name": "OpenAI", "baseUrl": "...", "models": ["gpt-4"] }]

// ~/.config/zeroagent/mcps.json
[{ "id": "filesystem", "name": "Filesystem", "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path"] }]
```

## Security Considerations

1. **API Keys**: Stored in provider config, user-managed
2. **File Access**: Agents/skills can access files in their folders only
3. **MCP Servers**: Run in child processes, isolated
4. **Staging Cleanup**: Prevents orphaned files on cancel

## Performance Optimizations

1. **Debounced Auto-Save**: 1 second delay prevents excessive IPC
2. **Set for Expanded Folders**: O(1) lookup vs O(n) array search
3. **Lazy Loading**: File explorer only loads when needed
4. **Conditional Markdown Editor**: Only for .md files

## Deployment Architecture

```mermaid
graph LR
    subgraph Development
        Dev["Developer machine"] --> Run["npm run tauri dev"]
        Run --> Vite["Vite Dev Server"]
        Run --> Rust["Rust Dev Build"]
    end

    subgraph Production
        Build["npm run tauri build"] --> Bundle["App Bundle"]
        Bundle --> Linux["Linux deb and AppImage"]
        Bundle --> Win["Windows exe and msi"]
        Bundle --> Mac["macOS dmg and app"]
    end

    subgraph Distribution
        Linux --> GitHub["GitHub Releases"]
        Win --> GitHub
        Mac --> GitHub
    end
```

## Extension Points

1. **New Features**: Add to `src/features/`
2. **New Commands**: Add to `src-tauri/src/commands/`
3. **UI Components**: Add to `src/shared/ui/`
4. **Services**: Add to `src/services/`

## Known Limitations

1. No cloud sync (local-only)
2. Single-user design
3. No real-time collaboration
4. macOS/Windows/Linux desktop only
