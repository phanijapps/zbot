# Agent Zero
Agent Zero is an Agent UI similar to Claude Desktop. The difference being, it can be used to connect to any OpenAI based APIs and be used to build agents, skills and connect to tools for daily use.

## Technology Stack
Read "Technology Stack" section in `memory-bank/architecture.md`

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

Read "Workspace Structure" section in `memory-bank/architecture.md`


## Agent Runtime Architecture
Read  `memory-bank/architecture.md` for clarity on architecture.

## Resources

- **Context7 Docs:** Use `mcp__context7__query-docs` for latest library documentation
- **Figma Design:** Use `mcp__figma-remote-mcp__*` tools for design work
-  `memory-bank/learnings.md` - This has information on how it resolved issues in the past. Use it when needed.
- `.ref/adk-rust` - is an opensource AI Agent Framework written in rust. crates in zero-* are partly inspired from it and langchain. Refer to them when needed but don't copy any code as that is an !!UNSTABLE!! project and cannot be used in our usecase.

## Contributing

When making changes:
1. Keep features modular and independent
2. Test with `npm run tauri dev` before building
3. Document new Tauri commands
4. Run `cargo check` in `src-tauri` to verify Rust code
