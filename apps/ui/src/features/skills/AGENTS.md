# Skill IDE Feature

## Overview

The Skill IDE is a full-page, IDE-style interface for creating and editing AI skills. It features a hierarchical file explorer, markdown editor for all `.md` files (including `SKILL.md`), and auto-save functionality. Skills use YAML frontmatter for metadata embedded directly in `SKILL.md`.

## Components

- **SkillIDEPage.tsx** - Main IDE component
- **SkillsPanel.tsx** - List view of all skills with category filtering

## State Management

```typescript
// Skill metadata
const [name, setName] = useState("");
const [displayName, setDisplayName] = useState("");
const [description, setDescription] = useState("");
const [category, setCategory] = useState("utility");
const [instructions, setInstructions] = useState("");

// File explorer
const [files, setFiles] = useState<SkillFile[]>([]);
const [selectedFile, setSelectedFile] = useState<SkillFile | null>(null);
const [fileContent, setFileContent] = useState<SkillFileContent | null>(null);
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
  file: SkillFile | null; isEmptyArea: boolean;
}>({ show: false, x: 0, y: 0, file: null, isEmptyArea: false });
```

## Key Patterns

### SKILL.md Editing (Simplified Approach)

**Problem**: Initial implementation had complex form-based editor for SKILL.md frontmatter. This caused issues with auto-save and closure problems.

**Solution**: Treat SKILL.md as a normal file - full markdown with frontmatter is editable directly:

```typescript
// Auto-select SKILL.md on load
const loadFiles = async () => {
  const loadedFiles = await skillsService.listSkillFiles(currentSkillId);
  setFiles(loadedFiles);

  if (initialSkill) {
    const skillMdFile = loadedFiles.find(f => f.name === "SKILL.md");
    if (skillMdFile) {
      setSelectedFile(skillMdFile);
      const content = await skillsService.readSkillFile(currentSkillId, skillMdFile.path);
      setFileContent(content);
      setEditingContent(content.content);
    }
  }
};

// Edit like any other file with markdown editor
{selectedFile?.name.endsWith('.md') ? (
  <MDEditor value={editingContent} onChange={(val) => setEditingContent(val || "")} />
) : (
  <Textarea value={editingContent} onChange={(e) => setEditingContent(e.target.value)} />
)}
```

**Learnings**:
- Simpler is better - treat SKILL.md as a normal markdown file
- Frontmatter is just YAML at the top - users can edit it directly
- No separate form state means no sync issues
- Auto-save works uniformly across all markdown files

### Hierarchical File Tree (Same as Agent IDE)

**Solution**: Identical implementation to Agent IDE for consistency:

```typescript
interface FileNode {
  file: SkillFile;
  children: FileNode[];
  level: number;
}

const buildFileTree = (): FileNode[] => {
  const rootNodes: FileNode[] = [];
  const nodeMap = new Map<string, FileNode>();

  files.forEach(file => {
    const node: FileNode = { file, children: [], level: 0 };
    nodeMap.set(file.path, node);
  });

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
      }
    }
  });

  return rootNodes;
};
```

### Context Menu (Consistent with Agent IDE)

**Solution**: Same pattern as Agent IDE:

```typescript
const handleContextMenu = (e: React.MouseEvent, file: SkillFile | null) => {
  e.preventDefault();
  e.stopPropagation();
  setContextMenu({
    show: true,
    x: e.clientX,
    y: e.clientY,
    file,
    isEmptyArea: !file,
  });
};

// Create new file/folder via inline input
const handleContextCreateConfirm = async () => {
  let parentFolder = "";
  if (!contextMenu.isEmptyArea && contextMenu.file) {
    parentFolder = !contextMenu.file.isFile ? contextMenu.file.path : "";
  }

  if (contextNewType === "folder") {
    const path = parentFolder ? `${parentFolder}/${contextNewInput}` : contextNewInput;
    await skillsService.createSkillFolder(getSkillId(), path);
  } else {
    const path = parentFolder ? `${parentFolder}/${contextNewInput}` : contextNewInput;
    await skillsService.writeSkillFile(getSkillId(), path, "");
  }

  // Auto-expand parent folder
  if (parentFolder) {
    setExpandedFolders(prev => new Set([...prev, parentFolder]));
  }

  loadFiles();
};
```

### Auto-Save Pattern

**Solution**: Same pattern as Agent IDE but only for existing skills:

```typescript
useEffect(() => {
  // Only auto-save for existing skills, not new skills
  if (!initialSkill || !selectedFile || !fileContent) return;

  // Only auto-save if content actually changed
  if (editingContent === fileContent.content) return;

  const timer = setTimeout(async () => {
    setIsAutoSaving(true);
    try {
      await skillsService.writeSkillFile(getSkillId(), selectedFile.path, editingContent);

      // If SKILL.md was edited, sync to instructions state
      if (selectedFile.name === "SKILL.md") {
        setInstructions(editingContent);
      }

      setFileContent({ ...fileContent, content: editingContent });
      setLastSaved(new Date());
    } finally {
      setIsAutoSaving(false);
    }
  }, 1000);

  return () => clearTimeout(timer);
}, [editingContent, initialSkill, selectedFile, fileContent]);
```

**Learnings**:
- Sync editingContent to instructions state when SKILL.md changes
- This keeps the skill metadata in sync with file content
- Used for final save when creating new skill

### Staging Mode

**Solution**: Similar to Agent IDE but uses "staging" ID:

```typescript
const getSkillId = () => initialSkill?.name || "staging";

const cleanupStaging = async () => {
  try {
    await skillsService.deleteSkillFile("staging", "SKILL.md");
  } catch (e) {
    // Ignore if file doesn't exist
  }
};

const handleConfirmClose = async () => {
  await cleanupStaging();
  setShowCloseConfirm(false);
  onClose();
};
```

### Category System

**Problem**: Skills need category filtering in the list view.

**Solution**: Fixed categories with filter buttons:

```typescript
const SKILL_CATEGORIES = [
  "utility", "coding", "writing", "analysis",
  "communication", "productivity", "research",
  "creative", "automation", "other",
];

const categories = ["all", ...SKILL_CATEGORIES];
const filteredSkills = selectedCategory === "all"
  ? skills
  : skills.filter((s) => s.category === selectedCategory);

// Render filter buttons
<div className="flex flex-wrap gap-2">
  {categories.map((cat) => (
    <button
      onClick={() => setSelectedCategory(cat)}
      className={selectedCategory === cat ? "bg-blue-600 text-white" : "bg-white/5"}
    >
      {cat === "all" ? "All Skills" : cat}
    </button>
  ))}
</div>
```

**Learnings**:
- "all" option shows everything
- Active category gets different styling
- Categories match backend `SKILL_CATEGORIES`

### Category Gradient Icons

**Problem**: Each skill category should have distinct visual identity.

**Solution**: Gradient mapping by category:

```typescript
const getCategoryGradient = (category: string) => {
  const gradients: Record<string, string> = {
    "coding": "from-blue-500 to-purple-600",
    "analysis": "from-green-500 to-teal-600",
    "automation": "from-orange-500 to-red-600",
    "utility": "from-yellow-500 to-orange-600",
    "communication": "from-pink-500 to-rose-600",
    "research": "from-indigo-500 to-blue-600",
    "writing": "from-cyan-500 to-blue-600",
    "productivity": "from-violet-500 to-purple-600",
    "creative": "from-gray-500 to-slate-600",
    "other": "from-emerald-500 to-green-600",
  };
  return gradients[category] || "from-purple-500 to-pink-600";
};

// Usage
<div className={`bg-gradient-to-br ${getCategoryGradient(skill.category)} p-3 rounded-xl`}>
  <Sparkles className="size-5 text-white" />
</div>
```

## New File/Folder Creation

**Problem**: Need to create files directly in subdirectories via inline input.

**Solution**: Two approaches - toolbar button and context menu:

```typescript
// Toolbar button with inline input
{showNewInput && (
  <div className="p-2 border-b border-white/10 bg-white/5">
    <div className="flex items-center gap-2">
      {newInputType === "file" ? <File className="size-4 text-blue-400" /> : <Folder className="size-4 text-blue-400" />}
      <Input
        placeholder={newInputType === "file" ? "filename.md" : "folder-name"}
        value={newInputType === "file" ? newFileName : newFolderName}
        onChange={(e) => newInputType === "file" ? setNewFileName(e.target.value) : setNewFolderName(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            // Create item
            skillsService[newInputType === "file" ? "writeSkillFile" : "createSkillFolder"](
              getSkillId(),
              name,
              newInputType === "file" ? "" : undefined as any
            ).then(() => {
              // Reset and reload
              loadFiles();
            });
          }
        }}
        autoFocus
      />
      <Plus onClick={/* create */} />
      <X onClick={() => setShowNewInput(false)} />
    </div>
  </div>
)}
```

**Learnings**:
- Same service method call for both toolbar and context menu
- Auto-focus input for immediate typing
- Enter key creates, Escape cancels
- Conditional icon based on type

## Import Files to Folder

```typescript
const handleContextMenuImport = async () => {
  const targetFile = contextMenu.isEmptyArea ? null : contextMenu.file;
  if (targetFile && targetFile.isFile) return;  // Can't import to file

  const input = document.createElement('input');
  input.type = 'file';
  input.multiple = true;
  input.onchange = async (e) => {
    const uploadFiles = (e.target as HTMLInputElement).files;
    if (!uploadFiles) return;

    for (const file of Array.from(uploadFiles)) {
      const content = await file.text();
      const destPath = targetFile ? `${targetFile.path}/${file.name}` : file.name;
      await skillsService.writeSkillFile(getSkillId(), destPath, content);
    }

    // Auto-expand the folder
    if (targetFile) {
      setExpandedFolders(prev => new Set([...prev, targetFile.path]));
    }

    loadFiles();
  };
  input.click();
};
```

## Header Display (Same Pattern as Agent IDE)

```typescript
{initialSkill ? (
  <>
    <h1 className="text-2xl font-bold text-white">
      {initialSkill.displayName}
    </h1>
    <p className="text-sm text-gray-400 truncate max-w-md">
      {initialSkill.description.length > 50
        ? initialSkill.description.substring(0, 50) + "..."
        : initialSkill.description}
    </p>
  </>
) : (
  <h1 className="text-2xl font-bold text-white">New Skill</h1>
)}
```

## Differences from Agent IDE

| Aspect | Agent IDE | Skill IDE |
|--------|-----------|-----------|
| **Main File** | `AGENTS.md` (plain) + `config.yaml` (form) | `SKILL.md` (frontmatter + markdown) |
| **Metadata Editor** | Separate ConfigYamlForm component | Embedded in SKILL.md |
| **Categories** | None | 10 fixed categories |
| **Default Folders** | None | assets/, resources/, scripts/ |
| **List View** | Simple cards | Category filter buttons |
| **Icons** | Single gradient (by name hash) | Gradient by category |

## Future Considerations

1. **Skill Templates**: Start from templates (web-search, code-review, etc.)
2. **Frontmatter Validation**: Real-time validation of YAML
3. **Skill Testing**: Test harness to verify skill behavior
4. **Dependencies**: Skills could reference/include other skills
5. **Export**: Package skill as shareable file
