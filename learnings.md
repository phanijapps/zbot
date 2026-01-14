# Agent Zero - Architecture Learnings

This document captures architectural decisions, patterns, and learnings as the project evolves.

## Project Overview

Agent Zero is a desktop application (similar to Claude Desktop) built with:
- **Tauri 2.9** - Cross-platform desktop framework with Rust backend
- **React 19** - Frontend UI framework
- **TypeScript** - Type safety across the stack
- **React Router** - Client-side routing
- **LangChain** - AI/LLM integration framework (planned)
- **Tailwind CSS v4** - Utility-first CSS framework with modern engine
- **Radix UI** - Unstyled, accessible component primitives

## Architecture Principles

### 1. Modular by Domain

The codebase is organized by **business domain**, not by technical layer:

```
src/
├── core/           # Core shell, routing, layout (cross-cutting)
├── features/       # Feature modules (conversations, agents, providers, etc.)
├── shared/         # Shared types, constants, utilities, UI components
└── services/       # API services, storage abstraction

src-tauri/src/
├── commands/       # Tauri commands organized by domain
├── services/       # Business logic services
└── state/          # Managed state (Tauri)
```

**Why?** As the app grows complex, grouping by domain makes it easier to:
- Find code related to a specific feature
- Understand the boundaries between features
- Test and refactor features independently
- Onboard new developers

### 2. Separation of Concerns

| Layer | Responsibility | Location |
|-------|----------------|----------|
| **UI Components** | Presentational logic, user interactions | `src/features/*/` |
| **Core Shell** | Layout, routing, navigation | `src/core/` |
| **Services** | API calls, data persistence | `src/services/`, `src-tauri/src/services/` |
| **Commands** | Bridge between frontend and Rust backend | `src-tauri/src/commands/` |
| **Types** | Shared type definitions | `src/shared/types/` |

### 3. Type Safety First

- All types defined in `src/shared/types/index.ts`
- TypeScript types shared between frontend and Rust via serde
- Tauri commands use `Result<T, String>` for error handling

## Key Decisions

### Why Tauri over Electron?

- **Package size**: Tauri apps are ~10MB vs Electron's ~100MB+
- **Performance**: Rust backend is faster and more memory efficient
- **Security**: Smaller attack surface with Rust
- **System integration**: Better native OS integration

### Why React Router over Tauri router?

- **Client-side routing**: Faster navigation, no IPC overhead
- **Browser APIs**: Works with web standards
- **Development**: Easier testing and debugging

### Why Custom CSS over UI Library?

- Started with lightweight custom CSS variables
- **Now integrated**: Tailwind CSS v4 with Radix UI primitives
- Modern design system inspired by the zero repo
- Dark-first theme with gradient accents
- Modular component architecture in `src/shared/ui/`

## Development Workflow

### Running the App

```bash
# Install dependencies
npm install

# Development mode (hot reload)
npm run tauri dev

# Build for production
npm run tauri build
```

### System Dependencies (Linux)

```bash
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.1-dev \
                 build-essential \
                 curl \
                 wget \
                 file \
                 libssl-dev \
                 libayatana-appindicator3-dev \
                 librsvg2-dev

# Fedora
sudo dnf install webkit2gtk4.1-devel \
                 openssl-devel \
                 curl \
                 wget \
                 file \
                 libappindicator-gtk3-devel \
                 librsvg2-devel
```

See https://tauri.app/guides/prerequisites/ for full details.

## Tauri Commands Pattern

Commands are organized by domain in `src-tauri/src/commands/`:

```rust
// src-tauri/src/commands/agents.rs

#[tauri::command]
pub async fn list_agents() -> Result<Vec<Agent>, String> {
    // Implementation
}

#[tauri::command]
pub async fn create_agent(agent: Agent) -> Result<Agent, String> {
    // Implementation
}
```

All commands are registered in `lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    commands::list_agents,
    commands::create_agent,
    // ...
])
```

## Frontend Service Pattern

Services abstract Tauri command calls:

```typescript
// src/services/agents.ts
import { invoke } from "@tauri-apps/api/core";

export async function listAgents(): Promise<Agent[]> {
  return invoke("list_agents");
}
```

## Future Considerations

### State Management
- Consider Zustand or Jotai for complex state
- Keep state close to where it's used
- Use Tauri's managed state for backend state

### Storage
- Use Tauri's `tauri-plugin-store` for settings
- Consider SQLite for conversations/messages (tauri-plugin-sqlite)
- Keep storage abstracted behind service layer

### MCP Integration
- MCP servers run as child processes
- Need process lifecycle management
- Consider stdio transport for communication

## Lessons Learned

### 1. Module Resolution
- Relative imports can be tricky (`../../` vs `../`)
- Consider path aliases for cleaner imports
- Keep the folder structure flat enough to avoid deep nesting

### 2. CSS Variables for Theming
- **Now using**: Tailwind CSS v4 with oklch color space
- Theme defined in `src/styles/theme.css` with CSS custom properties
- Dark-first design (#0a0a0a background)
- Gradient accents: `from-blue-500 to-purple-600`, `from-orange-500 to-pink-600`, etc.
- Tailwind v4 uses `@import 'tailwindcss' source(none)` with `@source` directives

### 3. React Router with Tauri
- Use `BrowserRouter` for client-side routing
- The `AppShell` pattern with `<Outlet />` enables nested layouts
- Keep route definitions centralized

### 4. Path Aliases with Vite
- Added `@` alias pointing to `src/` directory
- Cleaner imports: `@/shared/ui/button` instead of `../../shared/ui/button`
- Configured in `vite.config.ts` with `path.resolve(__dirname, "./src")`

## Design System

### Overview

Agent Zero uses a modern design system inspired by the zero repository, featuring:
- **Dark-first theme** with deep blacks (#0a0a0a)
- **Gradient accents** for visual hierarchy
- **Glassmorphism** with semi-transparent overlays
- **Icon-based navigation** with lucide-react icons

### Tech Stack

| Technology | Purpose |
|------------|---------|
| **Tailwind CSS v4.1.12** | Utility-first styling with new engine |
| **@tailwindcss/vite** | Official Vite plugin for Tailwind v4 |
| **Radix UI Primitives** | Unstyled, accessible components |
| **class-variance-authority** | Component variant management |
| **lucide-react** | Icon library |

### File Structure

```
src/
├── styles/
│   ├── index.css      # Entry point (imports fonts, tailwind, theme)
│   ├── tailwind.css   # Tailwind v4 configuration
│   ├── theme.css      # Color system with oklch colors
│   └── fonts.css      # Font imports (placeholder)
└── shared/
    └── ui/
        ├── utils.ts           # cn() utility for className merging
        ├── button.tsx         # CVA-based button variants
        ├── card.tsx           # Card components
        ├── input.tsx          # Form inputs
        ├── textarea.tsx       # Multi-line inputs
        ├── switch.tsx         # Toggle switches
        ├── label.tsx          # Form labels
        ├── badge.tsx          # Status badges
        ├── dialog.tsx         # Modal dialogs
        ├── dropdown-menu.tsx  # Dropdown menus
        ├── tooltip.tsx        # Hover tooltips
        ├── tabs.tsx           # Tabbed content
        ├── select.tsx         # Dropdown selects
        ├── scroll-area.tsx    # Custom scrollbars
        ├── separator.tsx      # Visual dividers
        └── index.ts           # Barrel exports
```

### Tailwind CSS v4 Configuration

**`vite.config.ts`**
```typescript
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "./src") },
  },
});
```

**`src/styles/tailwind.css`**
```css
@import 'tailwindcss' source(none);
@source '../**/*.{js,ts,jsx,tsx}';
@import 'tw-animate-css';
```

### Design Tokens

**Background Colors**
- `bg-[#0a0a0a]` - Main background
- `bg-[#0f0f0f]` - Sidebar background
- `bg-white/5` - Card backgrounds
- `bg-white/10` - Hover states

**Border Colors**
- `border-white/5` - Subtle borders
- `border-white/10` - Standard borders
- `border-white/20` - Strong borders

**Gradients**
- `from-blue-500 to-purple-600` - Primary actions
- `from-orange-500 to-pink-600` - Accent
- `from-green-500 to-teal-600` - Success

**Text Colors**
- `text-white` - Primary text
- `text-gray-400` - Secondary text
- `text-gray-500` - Muted text

### Component Patterns

**1. Button with Variants (CVA)**
```typescript
const buttonVariants = cva(
  "inline-flex items-center justify-center rounded-md text-sm font-medium",
  {
    variants: {
      variant: {
        default: "bg-white text-black hover:bg-white/90",
        gradient: "bg-gradient-to-r from-blue-600 to-purple-600 text-white",
        outline: "border border-white/20 bg-transparent hover:bg-white/10",
      },
    },
  }
);
```

**2. Card Pattern**
```typescript
<Card className="bg-white/5 border-white/10 hover:bg-white/10 transition-colors">
  <CardHeader>
    <CardTitle className="text-white">Title</CardTitle>
    <CardDescription className="text-gray-400">Description</CardDescription>
  </CardHeader>
  <CardContent>Content</CardContent>
</Card>
```

**3. Sidebar Navigation**
```typescript
<NavLink to="/" className={({ isActive }) =>
  cn("flex items-center justify-center p-3 rounded-lg", isActive ? "bg-blue-600" : "hover:bg-white/5")
}>
  <Icon className="size-5 text-white" />
</NavLink>
```

### Responsive Grid Patterns

```typescript
// Cards grid
<div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">

// Two-column layout
<div className="grid grid-cols-1 md:grid-cols-2 gap-4">
```

### Icon Usage

All icons from `lucide-react`:
```typescript
import { MessageSquare, Bot, Plus, Settings } from "lucide-react";

<Icon className="size-4" />  // Small
<Icon className="size-5" />  // Medium (default in sidebar)
```

### Accessibility

- Radix UI primitives handle keyboard navigation
- Focus states on all interactive elements
- ARIA labels on icon-only buttons
- High contrast ratios in dark theme

### Future Enhancements

- Add `shadcn/ui` components as needed
- Implement light mode toggle
- Add custom fonts via `fonts.css`
- Consider animation library (framer-motion)

## References

- [Tauri Documentation](https://tauri.app/)
- [React Router Documentation](https://reactrouter.com/)
- [LangChain Documentation](https://js.langchain.com/)

---

## Recent Session Learnings (2025-01-14)

### File Explorer with Hierarchical Tree

**Overview**: Implemented IDE-style file explorers for both Agent IDE and Skill IDE features with hierarchical tree structures supporting nested folders.

**Key Patterns**:

1. **Recursive File Scanning** (Rust backend):
```rust
fn collect_files(dir: &PathBuf, base_path: &PathBuf, relative_path: &str, files: &mut Vec<AgentFile>) -> Result<(), String> {
    let entries = fs::read_dir(dir)?;
    for entry in entries.flatten() {
        // ... process entry ...
        files.push(AgentFile { /* ... */ });
        // Recursively scan subdirectories
        if !is_file {
            collect_files(&path, base_path, &new_relative_path, files)?;
        }
    }
    Ok(())
}
```

2. **Hierarchical Tree Building** (TypeScript frontend):
```typescript
const buildFileTree = (): FileNode[] => {
    const nodeMap = new Map<string, FileNode>();
    // Create all nodes first
    files.forEach(file => {
        const node: FileNode = { file, children: [], level: 0 };
        nodeMap.set(file.path, node);
    });
    // Organize hierarchically by path
    sortedFiles.forEach(file => {
        const parts = file.path.split('/');
        if (parts.length === 1) {
            rootNodes.push(node);
        } else {
            const parentPath = parts.slice(0, -1).join('/');
            const parentNode = nodeMap.get(parentPath);
            if (parentNode) {
                parentNode.children.push(node);
                node.level = parentNode.level + 1;
            }
        }
    });
    return rootNodes;
};
```

3. **Recursive Rendering**:
```typescript
const renderFileNode = (node: FileNode): React.ReactElement => {
    return (
        <div key={file.path}>
            <div onClick={() => handleFileSelect(file)}>
                {/* File/folder icon with name */}
            </div>
            {!file.isFile && isExpanded && node.children.map(childNode => renderFileNode(childNode))}
        </div>
    );
};
```

**Learnings**:
- Backend must scan recursively to capture all files at any depth
- Frontend tree must be built with parent-child relationships for proper rendering
- Use `path.split('/')` to determine hierarchy level
- Store expanded folders in a `Set<string>` for efficient state management
- Indentation calculated as `8 + node.level * 12` pixels

### Markdown Editor Integration

**Overview**: Added `@uiw/react-md-editor` for proper markdown editing with live preview, toolbar, and syntax highlighting.

**Implementation**:

1. **Install and Import**:
```bash
npm install @uiw/react-md-editor
```

```typescript
import MDEditor from '@uiw/react-md-editor';
import '@uiw/react-md-editor/markdown-editor.css';
import '@uiw/react-markdown-preview/markdown.css';
```

2. **Conditional Rendering** (only for .md files):
```typescript
{selectedFile.name.endsWith('.md') ? (
    <div data-color-mode="dark" className="flex-1 flex flex-col">
        <MDEditor
            value={editingContent}
            onChange={(val) => setEditingContent(val || "")}
            height={700}
            preview="edit"
            hideToolbar={false}
            visibleDragbar={false}
        />
    </div>
) : (
    <Textarea value={editingContent} onChange={(e) => setEditingContent(e.target.value)} />
)}
```

**Learnings**:
- Use `data-color-mode="dark"` to match dark theme
- The `visibleDragbar` prop (lowercase 'b') controls drag bar visibility
- `onChange` returns `string | undefined`, so use `val || ""` for safety
- Keep Textarea for non-markdown files to avoid unnecessary overhead

### Auto-Save Pattern with Debouncing

**Overview**: Implemented auto-save for existing items while keeping manual save for new items.

**Pattern**:
```typescript
useEffect(() => {
    // Only auto-save for existing items, not new items
    if (!initialItem || !selectedFile || !fileContent) return;

    // Don't auto-save if content hasn't changed
    if (editingContent === fileContent.content) return;

    const timer = setTimeout(async () => {
        setIsAutoSaving(true);
        try {
            await service.writeFile(getItemId(), selectedFile.path, editingContent);
            setLastSaved(new Date());
        } finally {
            setIsAutoSaving(false);
        }
    }, 1000); // 1 second debounce

    return () => clearTimeout(timer);
}, [editingContent, initialItem, selectedFile, fileContent]);
```

**Learnings**:
- Use `useEffect` with editingContent as dependency for auto-save
- Debounce with 1 second delay to avoid excessive saves
- Show "Saving..." indicator during save operation
- Display "Last saved: X ago" for feedback
- Don't auto-save protected files (config.yaml) - handle separately

### Context Menu for File Operations

**Overview**: Implemented right-click context menus for file/folder operations (create, import, delete).

**Key States**:
```typescript
const [contextMenu, setContextMenu] = useState<{
    show: boolean;
    x: number;
    y: number;
    file: AgentFile | null;
    isEmptyArea: boolean;
}>({ show: false, x: 0, y: 0, file: null, isEmptyArea: false });

const [contextNewInput, setContextNewInput] = useState("");
const [showContextNewInput, setShowContextNewInput] = useState(false);
const [contextNewType, setContextNewType] = useState<"file" | "folder">("file");
```

**Pattern**:
```typescript
// Right-click handler
const handleContextMenu = (e: React.MouseEvent, file: AgentFile | null) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({
        show: true,
        x: e.clientX,
        y: e.clientY,
        file,
        isEmptyArea: file === null,
    });
};

// Click outside to close
useEffect(() => {
    const handleClickOutside = () => {
        if (contextMenu.show) {
            setContextMenu(prev => ({ ...prev, show: false }));
        }
    };
    if (contextMenu.show) {
        document.addEventListener('click', handleClickOutside);
        return () => document.removeEventListener('click', handleClickOutside);
    }
}, [contextMenu.show]);
```

**Learnings**:
- Always call `e.preventDefault()` and `e.stopPropagation()` on context menu
- Track `isEmptyArea` to enable/disable options like "Import Files"
- Inline input for create new (file/folder) is better UX than dialog
- Protected files should show lockout message instead of delete option

### Staging Mode Pattern

**Overview**: New items are created in a "staging" area before being saved to actual location.

**Backend Logic**:
```rust
fn is_staging_mode(agent_id: &str) -> bool {
    agent_id == "staging" || agent_id == "temp"
}

pub async fn list_agent_files(agent_id: String) -> Result<Vec<AgentFile>, String> {
    let (base_dir, is_staging) = if is_staging_mode(&agent_id) {
        (get_staging_dir()?, true)
    } else {
        (agents_dir.join(&agent_id), false)
    };

    // For staging, ensure default files exist
    if is_staging {
        fs::create_dir_all(&base_dir)?;
        // Create default config.yaml, AGENTS.md if not exists
    }
    // ...
}
```

**Frontend Logic**:
```typescript
const getAgentId = () => name || initialAgent?.name || "temp";

// For new agents, show save button; for existing, show auto-save indicator
{!initialAgent && (
    <Button onClick={handleSave}>Save Agent</Button>
)}
```

**Learnings**:
- Staging prevents orphaned files when user cancels creation
- Use `temp` or `staging` as special identifiers for staging mode
- Cleanup staging area on successful save or explicit cancel
- Show confirmation dialog when canceling new item creation

### File Path Handling

**Key Learnings**:
- Backend returns relative paths (e.g., `assets/image.png`, not full path)
- Frontend builds relative paths using `folder/file` format with `/` separator
- When writing files, backend uses `base_dir.join(&file_path)` which handles both Unix and Windows paths
- Always use forward slashes `/` in relative paths for cross-platform compatibility

### Protected Files Pattern

**Overview**: Certain files (config.yaml, AGENTS.md, SKILL.md) are protected from direct deletion.

**Implementation**:
```typescript
// Backend - flag in file metadata
let is_protected = name == "config.yaml" || name == "AGENTS.md";

// Frontend - conditional rendering
{!selectedFile.isProtected && (
    <Button onClick={handleDeleteFile}>
        <Trash2 />
    </Button>
)}

// Context menu - show message
{contextMenu.file && contextMenu.file.isProtected && (
    <div className="text-xs text-gray-500">
        🔒 {contextMenu.file.name} is protected and cannot be deleted
    </div>
)}
```

### Performance Considerations

1. **Bundle Size**: Markdown editor adds ~1MB to bundle. Consider lazy loading if needed.
2. **Debouncing**: 1 second debounce for auto-save prevents excessive IPC calls.
3. **Recursive Scanning**: Keep directory structure reasonably shallow to avoid performance issues.
4. **State Management**: Use `Set<string>` for expanded folders - O(1) lookup vs O(n) array search.
