# Agent Zero

Agent Zero is an Agent UI similar to Claude Desktop. The difference being, it can be used to connect to any OpenAI based APIs and be used to build agents, skills and connect to tools for daily use.

## Technology Stack

| Library/Application | Version |
|--------------------|---------|
| Tauri               | 2.9.5   |
| React               | 19.x    |
| LangChain           | 1.2.6   |
| TypeScript          | 5.x     |
| @lancedb/lancedb    | 0.23.0 |

## Quick Start

### Prerequisites

Install system dependencies for your platform:

**Linux (Ubuntu/Debian):**
```bash
sudo apt install libwebkit2gtk-4.1-dev \
                 build-essential \
                 curl \
                 wget \
                 file \
                 libssl-dev \
                 libayatana-appindicator3-dev \
                 librsvg2-dev
```

**Linux (Fedora):**
```bash
sudo dnf install webkit2gtk4.1-devel \
                 openssl-devel \
                 curl \
                 wget \
                 file \
                 libappindicator-gtk3-devel \
                 librsvg2-devel
```

**macOS:** No additional dependencies needed.

**Windows:** Install [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) and [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).

See https://tauri.app/guides/prerequisites/ for full details.

### Installation

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev
```

### Building

```bash
# Build for production
npm run tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.

## Project Structure

```
agentzero/
├── src/                    # React frontend
│   ├── core/              # Core shell, routing, layout
│   ├── features/          # Feature modules
│   │   ├── conversations/ # Chat conversations
│   │   ├── agents/        # Agent management
│   │   ├── providers/     # LLM provider config
│   │   ├── mcp/           # MCP server management
│   │   ├── skills/        # Skills and plugins
│   │   └── settings/      # App settings
│   ├── shared/            # Shared types, constants
│   └── services/          # API services
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── commands/      # Tauri commands (by domain)
│   │   ├── services/      # Business logic
│   │   └── state/         # Managed state
│   └── Cargo.toml
├── learnings.md           # Architecture decisions & learnings
└── AGENTS.md              # This file
```

## Development Guidelines

### Adding a New Feature

1. Create a new folder in `src/features/your-feature/`
2. Add types to `src/shared/types/index.ts`
3. Create Tauri commands in `src-tauri/src/commands/your-feature.rs`
4. Register commands in `src-tauri/src/lib.rs`
5. Create a service in `src/services/your-feature.ts`
6. Add route to `src/shared/constants/routes.ts`

### Code Style

- Use TypeScript strict mode
- Organize code by domain, not by layer
- Keep components small and focused
- Document complex logic with comments

## Resources

- **Architecture Learnings:** See `learnings.md` for architectural decisions
- **Context7 Docs:** Use `mcp__context7__query-docs` for latest library documentation
- **LangChain Docs:** Use `mcp__langchain-docs__SearchDocsByLangChain` for LangChain help
- **Figma Design:** Use `mcp__figma-remote-mcp__*` tools for design work

## Contributing

When making changes:
1. Update `learnings.md` with any architectural decisions
2. Keep features modular and independent
3. Test with `npm run tauri dev` before building
4. Document new Tauri commands
