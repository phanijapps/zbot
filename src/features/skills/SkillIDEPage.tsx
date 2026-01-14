// ============================================================================
// SKILL IDE PAGE
// Full-page IDE-style skill editor with file explorer
// ============================================================================

import { useState, useEffect } from "react";
import {
  X, Save, File, FileText, FolderPlus, Trash2, Folder,
  Upload, RefreshCw, Sparkles, ChevronRight, ChevronDown, Loader2,
  Plus, Lock, AlertTriangle
} from "lucide-react";
import { Button } from "@/shared/ui/button";
import { Input } from "@/shared/ui/input";
import { Textarea } from "@/shared/ui/textarea";
import MDEditor from '@uiw/react-md-editor';
import '@uiw/react-md-editor/markdown-editor.css';
import '@uiw/react-markdown-preview/markdown.css';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";
import type { Skill } from "@/shared/types";
import type { SkillFile, SkillFileContent } from "@/services/skills";
import * as skillsService from "@/services/skills";

interface SkillIDEPageProps {
  onSave: (skill: Omit<Skill, "id" | "createdAt">) => void;
  onClose: () => void;
  initialSkill?: Skill | null;
}

interface FileNode {
  file: SkillFile;
  children: FileNode[];
  level: number;
}

// Format time ago utility
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

export function SkillIDEPage({ onSave, onClose, initialSkill }: SkillIDEPageProps) {
  // Skill metadata state
  const [name, setName] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [description, setDescription] = useState("");
  const [category, setCategory] = useState("utility");
  const [instructions, setInstructions] = useState("");

  // File explorer state
  const [files, setFiles] = useState<SkillFile[]>([]);
  const [selectedFile, setSelectedFile] = useState<SkillFile | null>(null);
  const [fileContent, setFileContent] = useState<SkillFileContent | null>(null);
  const [editingContent, setEditingContent] = useState("");
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());
  const [newFolderName, setNewFolderName] = useState("");
  const [newFileName, setNewFileName] = useState("");
  const [showNewInput, setShowNewInput] = useState(false);
  const [newInputType, setNewInputType] = useState<"file" | "folder">("folder");

  // UI state
  const [saving, setSaving] = useState(false);
  const [lastSaved, setLastSaved] = useState<Date | null>(null);
  const [isAutoSaving, setIsAutoSaving] = useState(false);
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);

  // Context menu state
  const [contextMenu, setContextMenu] = useState<{
    show: boolean;
    x: number;
    y: number;
    file: SkillFile | null;
    isEmptyArea: boolean;
  }>({ show: false, x: 0, y: 0, file: null, isEmptyArea: false });

  // Context menu create new state
  const [contextNewInput, setContextNewInput] = useState("");
  const [showContextNewInput, setShowContextNewInput] = useState(false);
  const [contextNewType, setContextNewType] = useState<"file" | "folder">("file");

  // Get current skill ID (for editing or temp for new)
  const currentSkillId = initialSkill?.name || name || "temp";

  // Load files when skill changes
  useEffect(() => {
    loadFiles();
  }, [currentSkillId]);

  const loadFiles = async () => {
    try {
      const loadedFiles = await skillsService.listSkillFiles(currentSkillId);
      setFiles(loadedFiles);

      // Auto-select SKILL.md for existing skills
      if (initialSkill) {
        const skillMdFile = loadedFiles.find(f => f.name === "SKILL.md");
        if (skillMdFile) {
          setSelectedFile(skillMdFile);
          // Load the content
          const content = await skillsService.readSkillFile(currentSkillId, skillMdFile.path);
          setFileContent(content);
          setEditingContent(content.content);
        }
      }
    } catch (error) {
      console.error("Failed to load files:", error);
    }
  };

  // Populate form when editing
  useEffect(() => {
    if (initialSkill) {
      setName(initialSkill.name);
      setDisplayName(initialSkill.displayName);
      setDescription(initialSkill.description);
      setCategory(initialSkill.category);
      setInstructions(initialSkill.instructions);
    }
  }, [initialSkill]);

  // Auto-generate name from displayName for new skills
  useEffect(() => {
    if (!initialSkill && displayName) {
      const generatedName = displayName
        .toLowerCase()
        .trim()
        .replace(/[^a-z0-9\s-]/g, '')
        .replace(/\s+/g, '-')
        .replace(/-+/g, '-')
        .replace(/^-|-$/g, '');
      setName(generatedName);
    }
  }, [displayName, initialSkill]);

  // Close context menu when clicking elsewhere
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

  // Auto-save for existing skills when file content changes
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
      } catch (error) {
        console.error("Auto-save failed:", error);
      } finally {
        setIsAutoSaving(false);
      }
    }, 1000);

    return () => clearTimeout(timer);
  }, [editingContent, initialSkill, selectedFile, fileContent]);

  const getSkillId = () => initialSkill?.name || "staging";

  // Clean up staging area when cancelling new skill creation
  const cleanupStaging = async () => {
    try {
      await skillsService.deleteSkillFile("staging", "SKILL.md");
    } catch (e) {
      // Ignore if file doesn't exist
    }
  };

  // Handle close button click
  const handleClose = () => {
    if (!initialSkill) {
      setShowCloseConfirm(true);
    } else {
      onClose();
    }
  };

  // Confirm close and cleanup staging
  const handleConfirmClose = async () => {
    await cleanupStaging();
    setShowCloseConfirm(false);
    onClose();
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      const skill: Omit<Skill, "id" | "createdAt"> = {
        name: name.toLowerCase().replace(/\s+/g, "-"),
        displayName,
        description,
        category,
        instructions,
      };

      await skillsService.createSkill(skill);
      await onSave(skill);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  const handleFileSelect = async (file: SkillFile) => {
    if (!file.isFile) {
      setExpandedFolders(prev => {
        const newSet = new Set(prev);
        if (newSet.has(file.path)) {
          newSet.delete(file.path);
        } else {
          newSet.add(file.path);
        }
        return newSet;
      });
      return;
    }

    setSelectedFile(file);
    try {
      const content = await skillsService.readSkillFile(getSkillId(), file.path);
      setFileContent(content);
      setEditingContent(content.content);
    } catch (error) {
      console.error("Failed to load file:", error);
    }
  };

  const handleDeleteFile = async () => {
    if (!selectedFile) return;
    if (!confirm(`Delete "${selectedFile.name}"?`)) return;

    try {
      await skillsService.deleteSkillFile(getSkillId(), selectedFile.path);
      await loadFiles();
      setSelectedFile(null);
      setFileContent(null);
      setEditingContent("");
    } catch (error) {
      console.error("Failed to delete file:", error);
      alert("Failed to delete: " + error);
    }
  };

  const handleFileUpload = async () => {
    try {
      const input = document.createElement('input');
      input.type = 'file';
      input.multiple = true;
      input.onchange = async (e) => {
        const target = e.target as HTMLInputElement;
        const files = Array.from(target.files || []);
        for (const file of files) {
          const path = file.name;
          const content = await file.text();
          await skillsService.writeSkillFile(getSkillId(), path, content);
        }
        await loadFiles();
      };
      input.click();
    } catch (error) {
      console.error("Failed to upload files:", error);
      alert("Failed to upload: " + error);
    }
  };

  // Context menu handlers
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

  const closeContextMenu = () => {
    setContextMenu(prev => ({ ...prev, show: false }));
    setContextNewInput("");
    setShowContextNewInput(false);
  };

  const handleContextMenuOpen = () => {
    if (contextMenu.file) {
      handleFileSelect(contextMenu.file);
    }
    setContextMenu(prev => ({ ...prev, show: false }));
  };

  const handleContextMenuImport = async () => {
    const targetFile = contextMenu.isEmptyArea ? null : contextMenu.file;
    if (targetFile && targetFile.isFile) return;

    try {
      const input = document.createElement('input');
      input.type = 'file';
      input.multiple = true;
      input.onchange = async (e) => {
        const uploadFiles = (e.target as HTMLInputElement).files;
        if (!uploadFiles) return;

        for (const file of Array.from(uploadFiles)) {
          try {
            const content = await file.text();
            const destPath = targetFile ? `${targetFile.path}/${file.name}` : file.name;
            await skillsService.writeSkillFile(getSkillId(), destPath, content);
          } catch (error) {
            console.error("Failed to upload file:", error);
          }
        }
        // Auto-expand the folder
        if (targetFile) {
          setExpandedFolders(prev => new Set([...prev, targetFile.path]));
        }
        setContextMenu(prev => ({ ...prev, show: false }));
        loadFiles();
      };
      input.click();
    } catch (error) {
      console.error("Failed to import:", error);
    }
  };

  const handleContextMenuCreateNew = (type: "file" | "folder") => {
    setContextNewType(type);
    setContextNewInput("");
    setShowContextNewInput(true);
  };

  const handleContextCreateConfirm = async () => {
    if (!contextNewInput.trim()) return;

    // Get parent folder path
    let parentFolder = "";
    if (!contextMenu.isEmptyArea && contextMenu.file) {
      parentFolder = !contextMenu.file.isFile ? contextMenu.file.path : "";
    }

    try {
      if (contextNewType === "folder") {
        const path = parentFolder ? `${parentFolder}/${contextNewInput}` : contextNewInput;
        await skillsService.createSkillFolder(getSkillId(), path);
      } else {
        const path = parentFolder ? `${parentFolder}/${contextNewInput}` : contextNewInput;
        await skillsService.writeSkillFile(getSkillId(), path, "");
      }
      setContextNewInput("");
      setShowContextNewInput(false);
      // Auto-expand the parent folder
      if (parentFolder) {
        setExpandedFolders(prev => new Set([...prev, parentFolder]));
      }
      setContextMenu(prev => ({ ...prev, show: false }));
      loadFiles();
    } catch (error) {
      console.error("Failed to create:", error);
      alert(`Failed to create ${contextNewType}: ` + error);
    }
  };

  const handleContextMenuDelete = async () => {
    if (!contextMenu.file) return;
    closeContextMenu();

    if (!confirm(`Delete "${contextMenu.file.name}"?`)) return;

    try {
      await skillsService.deleteSkillFile(getSkillId(), contextMenu.file.path);
      loadFiles();
    } catch (error) {
      console.error("Failed to delete:", error);
      alert("Failed to delete: " + error);
    }
  };

  // Build hierarchical file tree
  const buildFileTree = (): FileNode[] => {
    const rootNodes: FileNode[] = [];
    const nodeMap = new Map<string, FileNode>();

    // First, create all nodes
    files.forEach(file => {
      const node: FileNode = { file, children: [], level: 0 };
      nodeMap.set(file.path, node);
    });

    // Then organize them hierarchically
    const sortedFiles = [...files].sort((a, b) => a.path.localeCompare(b.path));

    sortedFiles.forEach(file => {
      const node = nodeMap.get(file.path)!;
      const parts = file.path.split('/');

      if (parts.length === 1) {
        // Root level item
        node.level = 0;
        rootNodes.push(node);
      } else {
        // Find parent
        const parentPath = parts.slice(0, -1).join('/');
        const parentNode = nodeMap.get(parentPath);
        if (parentNode) {
          parentNode.children.push(node);
          node.level = parentNode.level + 1;
        } else {
          // Parent doesn't exist yet, add to root
          node.level = 0;
          rootNodes.push(node);
        }
      }
    });

    // Recursively sort children (folders first, then alphabetically)
    const sortChildren = (nodes: FileNode[]) => {
      nodes.sort((a, b) => {
        // Folders first
        if (!a.file.isFile && b.file.isFile) return -1;
        if (a.file.isFile && !b.file.isFile) return 1;
        // Then alphabetically
        return a.file.name.localeCompare(b.file.name);
      });
      nodes.forEach(node => {
        if (node.children.length > 0) {
          sortChildren(node.children);
        }
      });
    };
    sortChildren(rootNodes);

    return rootNodes;
  };

  const renderFileNode = (node: FileNode): React.ReactNode => {
    const isExpanded = expandedFolders.has(node.file.path);
    const hasChildren = node.children.length > 0;
    const isSelected = selectedFile?.path === node.file.path;

    return (
      <div key={node.file.path}>
        <div
          className={`flex items-center gap-1.5 py-1 px-2 rounded cursor-pointer text-sm ${
            isSelected
              ? 'bg-blue-600/30 text-white'
              : 'text-gray-300 hover:bg-white/5'
          }`}
          style={{ paddingLeft: `${8 + node.level * 12}px` }}
          onClick={() => handleFileSelect(node.file)}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handleContextMenu(e, node.file);
          }}
        >
          {!node.file.isFile ? (
            <>
              {isExpanded ? (
                <ChevronDown className="size-3.5 text-gray-500 shrink-0" />
              ) : (
                <ChevronRight className="size-3.5 text-gray-500 shrink-0" />
              )}
              <Folder className="size-4 text-yellow-400 shrink-0" />
            </>
          ) : node.file.isBinary ? (
            <File className="size-4 text-gray-500 shrink-0" />
          ) : (
            <FileText className="size-4 text-blue-400 shrink-0" />
          )}
          <span className="truncate">{node.file.name}</span>
          {node.file.isProtected && (
            <Lock className="size-3 text-gray-500 shrink-0 ml-auto" />
          )}
        </div>
        {!node.file.isFile && isExpanded && hasChildren && (
          <div onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handleContextMenu(e, node.file);
          }}>
            {node.children.map(renderFileNode)}
          </div>
        )}
      </div>
    );
  };

  const fileTree = buildFileTree();
  const isValid = name && displayName && description && category;

  return (
    <div className="fixed inset-0 bg-[#0a0a0a] z-50 flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 bg-[#141414] border-b border-white/10 shrink-0">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-gradient-to-br from-blue-500 to-purple-600">
            <Sparkles className="size-6 text-white" />
          </div>
          <div>
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
          </div>
        </div>
        <div className="flex items-center gap-3">
          {/* For existing skills: show last saved indicator */}
          {initialSkill && (
            <div className="flex items-center gap-2 text-sm">
              {isAutoSaving ? (
                <span className="text-gray-400 flex items-center gap-1.5">
                  <Loader2 className="size-3 animate-spin" />
                  Saving...
                </span>
              ) : lastSaved ? (
                <span className="text-gray-400">
                  Last saved: <span className="text-gray-300">{formatTimeAgo(lastSaved)}</span>
                </span>
              ) : (
                <span className="text-gray-500">No changes yet</span>
              )}
            </div>
          )}

          {/* For new skills: show save button */}
          {!initialSkill && (
            <Button
              onClick={handleSave}
              disabled={!isValid || saving}
              className="bg-gradient-to-br from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white disabled:opacity-50"
            >
              {saving ? (
                <>
                  <Loader2 className="size-5 mr-2 animate-spin" />
                  Saving...
                </>
              ) : (
                <>
                  <Save className="size-5 mr-2" />
                  Save Skill
                </>
              )}
            </Button>
          )}

          <Button
            onClick={handleClose}
            variant="ghost"
            className="text-gray-400 hover:text-white"
          >
            <X className="size-5" />
          </Button>
        </div>
      </div>

      {/* Main content area with sidebar */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left Sidebar - Explorer */}
        <div className="w-72 border-r border-white/10 flex flex-col bg-[#0f0f0f] shrink-0">
          {/* Section Header */}
          <div className="flex border-b border-white/10">
            <div className="flex-1 flex items-center justify-center gap-2 py-3 text-sm font-medium text-white border-b-2 border-blue-500 bg-white/5">
              <FileText className="size-4" />
              Files
            </div>
          </div>

          {/* Section Content */}
          <div className="flex-1 overflow-y-auto">
            <div>
              {/* Toolbar */}
              <div className="p-2 border-b border-white/10 flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => { setNewInputType("file"); setNewFileName(""); setShowNewInput(true); }}
                  className="text-gray-400 hover:text-white h-7 px-2 text-xs"
                  title="New File"
                >
                  <FileText className="size-3 mr-1" />
                  File
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => { setNewInputType("folder"); setNewFolderName(""); setShowNewInput(true); }}
                  className="text-gray-400 hover:text-white h-7 px-2 text-xs"
                  title="New Folder"
                >
                  <FolderPlus className="size-3 mr-1" />
                  Folder
                </Button>
                <div className="flex-1" />
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleFileUpload}
                  className="text-gray-400 hover:text-white h-7 px-2"
                  title="Upload"
                >
                  <Upload className="size-3" />
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={loadFiles}
                  className="text-gray-400 hover:text-white h-7 px-2"
                  title="Refresh"
                >
                  <RefreshCw className="size-3" />
                </Button>
              </div>

              {/* New file/folder input */}
              {showNewInput && (
                <div className="p-2 border-b border-white/10 bg-white/5">
                  <div className="flex items-center gap-2">
                    {newInputType === "file" ? <File className="size-4 text-blue-400" /> : <Folder className="size-4 text-blue-400" />}
                    <Input
                      placeholder={newInputType === "file" ? "filename.md" : "folder-name"}
                      value={newInputType === "file" ? newFileName : newFolderName}
                      onChange={(e) => newInputType === "file" ? setNewFileName(e.target.value) : setNewFolderName(e.target.value)}
                      className="flex-1 bg-white/5 border-white/10 text-white text-sm h-7"
                      autoFocus
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          const name = newInputType === "file" ? newFileName : newFolderName;
                          if (name.trim()) {
                            skillsService[newInputType === "file" ? "writeSkillFile" : "createSkillFolder"](
                              getSkillId(),
                              name,
                              newInputType === "file" ? "" : undefined as any
                            ).then(() => {
                              setNewFileName("");
                              setNewFolderName("");
                              setShowNewInput(false);
                              loadFiles();
                            }).catch(console.error);
                          }
                        } else if (e.key === "Escape") {
                          setNewFileName("");
                          setNewFolderName("");
                          setShowNewInput(false);
                        }
                      }}
                    />
                    <Button
                      size="sm"
                      onClick={() => {
                        const name = newInputType === "file" ? newFileName : newFolderName;
                        if (name.trim()) {
                          skillsService[newInputType === "file" ? "writeSkillFile" : "createSkillFolder"](
                            getSkillId(),
                            name,
                            newInputType === "file" ? "" : undefined as any
                          ).then(() => {
                            setNewFileName("");
                            setNewFolderName("");
                            setShowNewInput(false);
                            loadFiles();
                          }).catch(console.error);
                        }
                      }}
                      className="h-7 px-2 text-gray-400"
                    >
                      <Plus className="size-3" />
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setShowNewInput(false)}
                      className="h-7 px-2 text-gray-400"
                    >
                      <X className="size-3" />
                    </Button>
                  </div>
                </div>
              )}

              {/* File Tree */}
              <div
                className="py-1 min-h-32"
                onContextMenu={(e) => {
                  e.preventDefault();
                  handleContextMenu(e, null);
                }}
              >
                {fileTree.map(node => renderFileNode(node))}
              </div>
            </div>
          </div>
        </div>

        {/* Main Content Area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Editor/Viewer Area */}
          <div className="flex-1 overflow-hidden flex flex-col">
            {selectedFile ? (
              fileContent?.isBinary ? (
                <div className="flex-1 flex items-center justify-center">
                  <div className="text-center">
                    <File className="size-16 text-gray-600 mx-auto mb-4" />
                    <h3 className="text-lg font-medium text-white mb-2">Binary File</h3>
                    <p className="text-gray-400 text-sm mb-4">
                      This file type cannot be displayed or edited
                    </p>
                    <p className="text-xs text-gray-500">{selectedFile.name}</p>
                  </div>
                </div>
              ) : (
                <div className="flex-1 flex flex-col">
                  <div className="flex items-center justify-between px-4 py-2 border-b border-white/10 bg-black/20 shrink-0">
                    <span className="text-sm text-gray-400">{selectedFile.path}</span>
                    <div className="flex items-center gap-2">
                      {!selectedFile.isProtected && (
                        <Button
                          size="sm"
                          onClick={handleDeleteFile}
                          variant="ghost"
                          className="text-gray-400 hover:text-red-400 h-7 px-2"
                          title="Delete File"
                        >
                          <Trash2 className="size-4" />
                        </Button>
                      )}
                    </div>
                  </div>
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
                    <Textarea
                      value={editingContent}
                      onChange={(e) => setEditingContent(e.target.value)}
                      className="flex-1 bg-[#0a0a0a] border-0 text-white font-mono text-sm resize-none p-4"
                      spellCheck={false}
                    />
                  )}
                  <div className="px-4 py-2 border-t border-white/10 bg-black/20 text-xs text-gray-500">
                    {initialSkill ? (
                      <>Changes auto-save after 1 second of inactivity</>
                    ) : (
                      <>Changes will be saved when you click Save Skill</>
                    )}
                  </div>
                </div>
              )
            ) : (
              <div className="flex-1 flex items-center justify-center text-gray-500">
                Select a file to view or edit
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Context Menu */}
      {contextMenu.show && (
        <div
          className="fixed z-[100] bg-[#1a1a1a] border border-white/20 rounded-lg shadow-xl py-1 min-w-48"
          style={{
            left: `${contextMenu.x}px`,
            top: `${contextMenu.y}px`,
          }}
          onClick={(e) => e.stopPropagation()}
        >
          {/* Create New Options */}
          <div className="px-3 py-2 border-b border-white/10">
            <p className="text-xs text-gray-500 uppercase tracking-wide mb-2">Create New</p>
            <div className="flex gap-1">
              <button
                type="button"
                onClick={() => handleContextMenuCreateNew("file")}
                className="flex-1 flex items-center justify-center gap-1 px-2 py-1.5 text-xs text-gray-300 hover:bg-white/10 hover:text-white rounded"
              >
                <FileText className="size-3" />
                File
              </button>
              <button
                type="button"
                onClick={() => handleContextMenuCreateNew("folder")}
                className="flex-1 flex items-center justify-center gap-1 px-2 py-1.5 text-xs text-gray-300 hover:bg-white/10 hover:text-white rounded"
              >
                <FolderPlus className="size-3" />
                Folder
              </button>
            </div>
            {showContextNewInput && (
              <div className="flex gap-1 mt-2">
                <Input
                  placeholder={contextNewType === "folder" ? "folder-name" : "file.txt"}
                  value={contextNewInput}
                  onChange={(e) => setContextNewInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleContextCreateConfirm()}
                  autoFocus
                  className="bg-black/30 border-white/10 text-white text-xs h-7"
                />
                <Button
                  size="sm"
                  onClick={handleContextCreateConfirm}
                  className="h-7 px-2 bg-blue-600 hover:bg-blue-700"
                >
                  <Plus className="size-3" />
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setShowContextNewInput(false)}
                  className="h-7 px-2 text-gray-400"
                >
                  <X className="size-3" />
                </Button>
              </div>
            )}
          </div>

          {/* Import Files (only for folders or empty area) */}
          {(!contextMenu.file || !contextMenu.file.isFile) && (
            <>
              <button
                type="button"
                onClick={handleContextMenuImport}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-300 hover:bg-white/10 hover:text-white text-left"
              >
                <Upload className="size-4 shrink-0" />
                Import Files...
              </button>
            </>
          )}

          {/* Open (only for files) */}
          {contextMenu.file && contextMenu.file.isFile && (
            <button
              type="button"
              onClick={handleContextMenuOpen}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-300 hover:bg-white/10 hover:text-white text-left"
            >
              <FileText className="size-4 shrink-0" />
              Open
            </button>
          )}

          {/* Delete (not for protected files or empty area) */}
          {contextMenu.file && !contextMenu.file.isProtected && (
            <>
              <div className="border-t border-white/10 my-1" />
              <button
                type="button"
                onClick={handleContextMenuDelete}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm text-red-400 hover:bg-red-500/10 hover:text-red-300 text-left"
              >
                <Trash2 className="size-4 shrink-0" />
                Delete
              </button>
            </>
          )}

          {/* Protected files indicator */}
          {contextMenu.file && contextMenu.file.isProtected && (
            <div className="px-3 py-2 text-xs text-gray-500 italic border-t border-white/10">
              🔒 {contextMenu.file.name} is protected and cannot be deleted
            </div>
          )}
        </div>
      )}

      {/* Close Confirmation Dialog for New Skills */}
      <Dialog open={showCloseConfirm} onOpenChange={setShowCloseConfirm}>
        <DialogContent className="bg-[#141414] border-white/10 text-white max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-3 text-xl">
              <div className="p-2 rounded-lg bg-yellow-500/20">
                <AlertTriangle className="size-5 text-yellow-400" />
              </div>
              Discard New Skill?
            </DialogTitle>
            <DialogDescription className="text-gray-400">
              You are about to leave without creating this skill. Any changes you made will be lost.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="gap-3">
            <Button
              onClick={() => setShowCloseConfirm(false)}
              variant="outline"
              className="border-white/20 text-white hover:bg-white/5"
            >
              Keep Editing
            </Button>
            <Button
              onClick={handleConfirmClose}
              variant="destructive"
              className="bg-red-600 hover:bg-red-700 text-white"
            >
              Discard & Close
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
