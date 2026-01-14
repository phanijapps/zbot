# Agent IDE Feature

## Overview

The Agent IDE is a full-page, IDE-style interface for creating and editing AI agents. It features a hierarchical file explorer, markdown editor for `AGENTS.md`, form-based config editor for `config.yaml`, and auto-save functionality.

## Components

- **AgentIDEPage.tsx** - Main IDE component
- **AgentsPanel.tsx** - List view of all agents
- **ConfigYamlForm.tsx** - Form-based editor for agent metadata
- **AgentCard.tsx** - Card component for agent display

## State Management

The IDE manages multiple pieces of state:

```typescript
// Agent metadata
const [name, setName] = useState("");
const [displayName, setDisplayName] = useState("");
const [description, setDescription] = useState("");
const [providerId, setProviderId] = useState("");
const [model, setModel] = useState("");
const [temperature, setTemperature] = useState(0.7);
const [maxTokens, setMaxTokens] = useState(2000);
const [selectedMcpIds, setSelectedMcpIds] = useState<string[]>([]);
const [selectedSkillIds, setSelectedSkillIds] = useState<string[]>([]);
const [instructions, setInstructions] = useState("");

// File explorer
const [files, setFiles] = useState<AgentFile[]>([]);
const [selectedFile, setSelectedFile] = useState<AgentFile | null>(null);
const [fileContent, setFileContent] = useState<AgentFileContent | null>(null);
const [editingContent, setEditingContent] = useState("");
const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());

// UI state
const [saving, setSaving] = useState(false);
const [lastSaved, setLastSaved] = useState<Date | null>(null);
const [isAutoSaving, setIsAutoSaving] = useState(false);
const [showCloseConfirm, setShowCloseConfirm] = useState(false);

// Context menu
const [contextMenu, setContextMenu] = useState<{
  show: boolean; x: number; y: number;
  file: AgentFile | null; isEmptyArea: boolean;
}>({ show: false, x: 0, y: 0, file: null, isEmptyArea: false });
```

## Key Patterns

### Hierarchical File Tree

**Problem**: Files can be nested in subdirectories. Need to display them hierarchically with expand/collapse.

**Solution**: Build a tree structure from flat file list, then render recursively:

```typescript
interface FileNode {
  file: AgentFile;
  children: FileNode[];
  level: number;
}

const buildFileTree = (): FileNode[] => {
  const rootNodes: FileNode[] = [];
  const nodeMap = new Map<string, FileNode>();

  // Create all nodes first
  files.forEach(file => {
    const node: FileNode = { file, children: [], level: 0 };
    nodeMap.set(file.path, node);
  });

  // Organize hierarchically by path
  const sortedFiles = [...files].sort((a, b) => a.path.localeCompare(b.path));

  sortedFiles.forEach(file => {
    const node = nodeMap.get(file.path)!;
    const parts = file.path.split('/');

    if (parts.length === 1) {
      node.level = 0;
      rootNodes.push(node);
    } else {
      const parentPath = parts.slice(0, -1).join('/');
      const parentNode = nodeMap.get(parentPath);
      if (parentNode) {
        parentNode.children.push(node);
        node.level = parentNode.level + 1;
      } else {
        // Parent doesn't exist, add to root
        node.level = 0;
        rootNodes.push(node);
      }
    }
  });

  return rootNodes;
};

const renderFileNode = (node: FileNode): React.ReactElement => {
  const isExpanded = expandedFolders.has(file.path);

  return (
    <div key={file.path}>
      <div onClick={() => handleFileSelect(file)}>
        {/* File/folder icon and name */}
      </div>
      {!file.isFile && isExpanded && node.children.map(renderFileNode)}
    </div>
  );
};
```

**Learnings**:
- Use `Map<path, node>` for O(1) parent lookup
- Sort files alphabetically before building tree for consistent rendering
- Calculate indentation: `paddingLeft: ${8 + node.level * 12}px`
- Store expanded folders in `Set<string>` for O(1) lookup

### Auto-Save with Debouncing

**Problem**: Existing agents should auto-save changes, but new agents need explicit save button.

**Solution**: Use `useEffect` with editingContent dependency and guard clauses:

```typescript
useEffect(() => {
  // Only auto-save for existing agents, not new agents
  if (!initialAgent || !selectedFile || !fileContent) return;

  // Don't auto-save config.yaml (handled separately)
  if (selectedFile.name === "config.yaml") return;

  // Only auto-save if content actually changed
  if (editingContent === fileContent.content) return;

  const timer = setTimeout(async () => {
    setIsAutoSaving(true);
    try {
      await agentService.writeAgentFile(getAgentId(), selectedFile.path, editingContent);
      setFileContent({ ...fileContent, content: editingContent });
      setLastSaved(new Date());
    } finally {
      setIsAutoSaving(false);
    }
  }, 1000); // 1 second debounce

  return () => clearTimeout(timer);
}, [editingContent, initialAgent, selectedFile, fileContent]);
```

**Learnings**:
- Multiple guard clauses prevent unwanted saves
- 1 second debounce prevents excessive IPC calls
- Store `lastSaved` timestamp for user feedback
- Update local `fileContent` to prevent re-save

### Context Menu Pattern

**Problem**: Need right-click menu for file/folder operations (create, import, delete, open).

**Solution**: Track menu position, target file, and inline input state:

```typescript
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

// Close on click outside
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
- `e.preventDefault()` prevents browser context menu
- `e.stopPropagation()` prevents triggering parent handlers
- Use `e.clientX/Y` for menu positioning
- Clean up event listener in useEffect return

### Staging Mode for New Agents

**Problem**: New agents need a place to store files before officially creating the agent.

**Solution**: Use "staging" or "temp" as special agent IDs:

```typescript
const getAgentId = () => name || initialAgent?.name || "temp";

// Backend recognizes staging mode
// Frontend shows save button only for new agents
{!initialAgent && (
  <Button onClick={handleSave}>Save Agent</Button>
)}

// Show confirmation on cancel
const handleConfirmClose = async () => {
  await cleanupStaging();  // Delete staging files
  setShowCloseConfirm(false);
  onClose();
};
```

**Learnings**:
- Use `name || initialAgent?.name || "temp"` for flexible ID resolution
- Staging prevents orphaned files on cancel
- Show confirmation dialog before discarding work
- Cleanup staging area on successful save

### Markdown Editor Integration

**Problem**: `AGENTS.md` and `.md` files need proper markdown editing with preview.

**Solution**: Use `@uiw/react-md-editor` conditionally:

```typescript
import MDEditor from '@uiw/react-md-editor';
import '@uiw/react-md-editor/markdown-editor.css';
import '@uiw/react-markdown-preview/markdown.css';

{selectedFile?.name.endsWith('.md') ? (
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
- `data-color-mode="dark"` matches app theme
- `onChange` returns `string | undefined` - use `val || ""`
- `visibleDragbar` (lowercase 'b') controls drag handle
- Use Textarea for non-markdown files to save bundle size

### Auto-generate Name from Display Name

**Problem**: User enters display name like "My Agent" - need URL-friendly name like "my-agent".

**Solution**: Auto-generate on display name change:

```typescript
useEffect(() => {
  if (!initialAgent && displayName) {
    const generatedName = displayName
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9\s-]/g, '')   // Remove special chars
      .replace(/\s+/g, '-')              // Spaces to hyphens
      .replace(/-+/g, '-')               // Multiple hyphens to one
      .replace(/^-|-$/g, '');            // Remove leading/trailing
    setName(generatedName);
  }
}, [displayName, initialAgent]);
```

**Learnings**:
- Only auto-generate for new agents (not editing)
- Chain `replace()` calls for transformations
- Result is URL-safe and filesystem-safe

### Protected File Handling

**Problem**: `config.yaml` and `AGENTS.md` shouldn't show delete button.

**Solution**: Conditional rendering based on `isProtected` flag:

```typescript
{!selectedFile.isProtected && (
  <Button onClick={handleDeleteFile}>
    <Trash2 />
  </Button>
)}

// Context menu also shows lockout message
{contextMenu.file && contextMenu.file.isProtected && (
  <div className="text-xs text-gray-500">
    {contextMenu.file.name} is protected and cannot be deleted
  </div>
)}
```

**Learnings**:
- Backend sets `isProtected` flag
- Frontend respects flag in both delete button and context menu
- Red color/hover state signals destructive action

## Header Display

**Problem**: When editing, show agent name and truncated description in header.

**Solution**: Conditional rendering with description truncation:

```typescript
{initialAgent ? (
  <>
    <h1 className="text-2xl font-bold text-white">
      {initialAgent.displayName}
    </h1>
    <p className="text-sm text-gray-400 truncate max-w-md">
      {initialAgent.description.length > 50
        ? initialAgent.description.substring(0, 50) + "..."
        : initialAgent.description}
    </p>
  </>
) : (
  <h1 className="text-2xl font-bold text-white">New Agent</h1>
)}
```

## File Operations

### Create File/Folder

```typescript
const handleCreateItem = async () => {
  const parentFolder = selectedFile && !selectedFile.isFile ? selectedFile.path : "";

  if (newInputType === "folder") {
    const folderPath = parentFolder ? `${parentFolder}/${newFolderName}` : newFolderName;
    await agentService.createAgentFolder(getAgentId(), folderPath);
  } else {
    const filePath = parentFolder ? `${parentFolder}/${newFileName}` : newFileName;
    await agentService.writeAgentFile(getAgentId(), filePath, "");
  }

  // Auto-expand parent folder
  if (parentFolder) {
    setExpandedFolders(prev => new Set([...prev, parentFolder]));
  }

  loadFiles();
};
```

### Import Files

```typescript
const handleFileUpload = async () => {
  const input = document.createElement('input');
  input.type = 'file';
  input.multiple = true;
  input.onchange = async (e) => {
    const uploadFiles = (e.target as HTMLInputElement).files;
    if (!uploadFiles) return;

    for (const file of Array.from(uploadFiles)) {
      const content = await file.text();
      await agentService.writeAgentFile(getAgentId(), file.name, content);
    }
    loadFiles();
  };
  input.click();
};
```

## Time Formatting

```typescript
function formatTimeAgo(date: Date): string {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);

  if (seconds < 10) return "just now";
  if (seconds < 60) return `${seconds}s ago`;

  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;

  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}
```

## Future Considerations

1. **Split Screen**: Side-by-side editor and preview
2. **File Templates**: Quick-create from templates (README, etc.)
3. **Search**: Full-text search across agent files
4. **Drag & Drop**: Drag files to move/copy between folders
5. **Keyboard Shortcuts**: Ctrl+S to save, Ctrl+N to new file, etc.
