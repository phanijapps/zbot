// ============================================================================
// AGENT IDE PAGE
// Full-page IDE-style agent editor with file explorer
// ============================================================================

import { useState, useEffect } from "react";
import {
  X, Save, File, FileText, FolderPlus, Trash2, Folder,
  Upload, RefreshCw, Bot, ChevronRight, ChevronDown, Loader2,
  Plus, Lock, AlertTriangle, Settings
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
import type { Agent } from "@/shared/types";
import { ConfigYamlForm } from "./ConfigYamlForm";
import type { Provider } from "@/shared/types";
import type { MCPServer } from "@/features/mcp/types";
import type { Skill } from "@/shared/types";
import type { AgentFile, AgentFileContent } from "@/services/agent";
import * as agentService from "@/services/agent";
import * as providerService from "@/services/provider";
import * as mcpService from "@/services/mcp";
import * as skillsService from "@/services/skills";

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

// Default middleware configuration
const DEFAULT_MIDDLEWARE = `# Middleware Configuration
# Configure how the agent processes conversations

middleware:
  # Summarization - Compress conversation history when approaching token limits
  summarization:
    enabled: true
    # Model to use for summarization (null = use agent's model)
    model: null
    # Provider for summarization (null = use agent's provider)
    provider: null
    trigger:
      # Trigger when token count reaches this value
      tokens: 60000
      # Trigger when message count reaches this value
      messages: null
      # Trigger when fraction of context window is reached (0.0-1.0)
      fraction: null
    keep:
      # Number of messages to keep after summarization
      messages: 6
      # Number of tokens to keep
      tokens: null
      # Fraction of context window to keep (0.0-1.0)
      fraction: null
    summary_prefix: "[Previous conversation summary:]"
    summary_prompt: null

  # Context Editing - Clear older tool call outputs while keeping recent ones
  context_editing:
    enabled: true
    trigger_tokens: 60000
    # Number of tool results to keep (most recent)
    keep_tool_results: 10
    # Minimum tokens to reclaim before clearing
    min_reclaim: 1000
    # Clear tool call inputs (arguments) as well
    clear_tool_inputs: false
    # Tools to exclude from clearing (e.g., "search", "database")
    exclude_tools: []
    placeholder: "[Result cleared due to context limits]"
`;

interface AgentIDEPageProps {
  onSave: (agent: Omit<Agent, "id" | "createdAt">) => void;
  onClose: () => void;
  onAgentUpdated?: (agent: Agent) => void;
  initialAgent?: Agent | null;
}

interface FileNode {
  file: AgentFile;
  children: FileNode[];
  level: number;
}

export function AgentIDEPage({ onSave, onClose, onAgentUpdated, initialAgent }: AgentIDEPageProps) {
  // Agent metadata state
  const [name, setName] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [description, setDescription] = useState("");
  const [agentType, setAgentType] = useState<"llm" | "sequential" | "parallel" | "loop" | "conditional" | "llm_conditional" | "custom">("llm");
  const [providerId, setProviderId] = useState("");
  const [model, setModel] = useState("");
  const [temperature, setTemperature] = useState(0.7);
  const [maxTokens, setMaxTokens] = useState(2000);
  const [thinkingEnabled, setThinkingEnabled] = useState(false);
  const [selectedMcpIds, setSelectedMcpIds] = useState<string[]>([]);
  const [selectedSkillIds, setSelectedSkillIds] = useState<string[]>([]);
  const [instructions, setInstructions] = useState("");
  const [middleware, setMiddleware] = useState(DEFAULT_MIDDLEWARE);

  // File explorer state
  const [files, setFiles] = useState<AgentFile[]>([]);
  const [selectedFile, setSelectedFile] = useState<AgentFile | null>(null);
  const [fileContent, setFileContent] = useState<AgentFileContent | null>(null);
  const [editingContent, setEditingContent] = useState("");
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());
  const [newFolderName, setNewFolderName] = useState("");
  const [newFileName, setNewFileName] = useState("");
  const [showNewInput, setShowNewInput] = useState(false);
  const [newInputType, setNewInputType] = useState<"file" | "folder">("folder");

  // Options state
  const [providers, setProviders] = useState<Provider[]>([]);
  const [mcps, setMcps] = useState<MCPServer[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [saving, setSaving] = useState(false);
  const [savingConfig, setSavingConfig] = useState(false);
  const [lastSaved, setLastSaved] = useState<Date | null>(null);
  const [isAutoSaving, setIsAutoSaving] = useState(false);
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);


  // Context menu state
  const [contextMenu, setContextMenu] = useState<{
    show: boolean;
    x: number;
    y: number;
    file: AgentFile | null;
    isEmptyArea: boolean;
  }>({ show: false, x: 0, y: 0, file: null, isEmptyArea: false });

  // Context menu create new state
  const [contextNewInput, setContextNewInput] = useState("");
  const [showContextNewInput, setShowContextNewInput] = useState(false);
  const [contextNewType, setContextNewType] = useState<"file" | "folder">("file");

  // Get current agent ID (for editing or temp for new)
  const currentAgentId = initialAgent?.name || name || "temp";

  // Load options on mount
  useEffect(() => {
    loadOptions();
  }, []);

  // Load files when agent changes
  useEffect(() => {
    loadFiles();
  }, [currentAgentId]);

  const loadOptions = async () => {
    try {
      const [providersData, mcpsData, skillsData] = await Promise.all([
        providerService.listProviders(),
        mcpService.listMCPServers(),
        skillsService.listSkills(),
      ]);
      setProviders(providersData);
      setMcps(mcpsData);
      setSkills(skillsData);
    } catch (error) {
      console.error("Failed to load options:", error);
    }
  };

  const loadFiles = async () => {
    try {
      const loadedFiles = await agentService.listAgentFiles(currentAgentId);
      setFiles(loadedFiles);

      // Auto-select AGENTS.md for existing agents
      if (initialAgent) {
        const agentsMdFile = loadedFiles.find(f => f.name === "AGENTS.md");
        if (agentsMdFile) {
          setSelectedFile(agentsMdFile);
          // Load the content
          const content = await agentService.readAgentFile(currentAgentId, agentsMdFile.path);
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
    if (initialAgent) {
      setName(initialAgent.name);
      setDisplayName(initialAgent.displayName);
      setDescription(initialAgent.description);
      setAgentType(initialAgent.agentType || "llm");
      setProviderId(initialAgent.providerId);
      setModel(initialAgent.model);
      setTemperature(initialAgent.temperature);
      setMaxTokens(initialAgent.maxTokens || 2000);
      setThinkingEnabled(initialAgent.thinkingEnabled || false);
      setInstructions(initialAgent.instructions);
      setSelectedMcpIds(initialAgent.mcps);
      setSelectedSkillIds(initialAgent.skills);
      setMiddleware(initialAgent.middleware || DEFAULT_MIDDLEWARE);
    }
  }, [initialAgent]);

  // Auto-generate name from displayName for new agents
  useEffect(() => {
    if (!initialAgent && displayName) {
      const generatedName = displayName
        .toLowerCase()
        .trim()
        .replace(/[^a-z0-9\s-]/g, '')  // Remove special chars except spaces and hyphens
        .replace(/\s+/g, '-')            // Replace spaces with hyphens
        .replace(/-+/g, '-')             // Replace multiple hyphens with single
        .replace(/^-|-$/g, '');          // Remove leading/trailing hyphens
      setName(generatedName);
    }
  }, [displayName, initialAgent]);

  // Update model when provider changes
  useEffect(() => {
    if (providerId) {
      const provider = providers.find((p) => p.id === providerId);
      if (provider && provider.models.length > 0) {
        setModel(provider.models[0]);
      }
    }
  }, [providerId, providers]);

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

  // Auto-save for existing agents when file content changes
  useEffect(() => {
    // Only auto-save for existing agents, not new agents
    if (!initialAgent || !selectedFile || !fileContent) return;

    // Don't auto-save config.yaml (handled by saveConfigYaml)
    if (selectedFile.name === "config.yaml") return;

    // Only auto-save if content actually changed
    if (editingContent === fileContent.content) return;

    const timer = setTimeout(async () => {
      setIsAutoSaving(true);
      try {
        await agentService.writeAgentFile(getAgentId(), selectedFile.path, editingContent);

        // If AGENTS.md was edited, sync to instructions state
        if (selectedFile.name === "AGENTS.md") {
          setInstructions(editingContent);
        }

        setFileContent({ ...fileContent, content: editingContent });
        setLastSaved(new Date());
      } catch (error) {
        console.error("Auto-save failed:", error);
      } finally {
        setIsAutoSaving(false);
      }
    }, 1000); // 1 second debounce

    return () => clearTimeout(timer);
  }, [editingContent, initialAgent, selectedFile, fileContent]);

  const getAgentId = () => name || initialAgent?.name || "temp";

  // Clean up staging area when cancelling new agent creation
  const cleanupStaging = async () => {
    try {
      await agentService.deleteAgentFile("staging", "config.yaml");
    } catch (e) {
      // Ignore if file doesn't exist
    }
    try {
      await agentService.deleteAgentFile("staging", "AGENTS.md");
    } catch (e) {
      // Ignore if file doesn't exist
    }
  };

  // Handle close button click
  const handleClose = () => {
    // For new agents, show confirmation dialog
    if (!initialAgent) {
      setShowCloseConfirm(true);
    } else {
      // For existing agents, just close
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
      // If AGENTS.md was edited, sync to instructions state
      let finalInstructions = instructions;
      if (selectedFile?.name === "AGENTS.md" && editingContent !== fileContent?.content) {
        finalInstructions = editingContent;
        setInstructions(editingContent);
      }

      const agent: Omit<Agent, "id" | "createdAt"> = {
        name: name.toLowerCase().replace(/\s+/g, "-"),
        displayName,
        description,
        agentType,
        providerId,
        model,
        temperature,
        maxTokens,
        thinkingEnabled,
        instructions: finalInstructions,
        mcps: selectedMcpIds,
        skills: selectedSkillIds,
        middleware: middleware.trim() || undefined,
      };

      // First save the agent (this writes config.yaml and AGENTS.md)
      await agentService.createAgent(agent);

      // If editing content in other files (not AGENTS.md or config.yaml), save them
      if (selectedFile && editingContent !== fileContent?.content && selectedFile.name !== "AGENTS.md" && selectedFile.name !== "config.yaml") {
        await agentService.writeAgentFile(getAgentId(), selectedFile.path, editingContent);
      }

      await onSave(agent);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  const saveConfigYaml = async () => {
    if (savingConfig) return;
    // Only save for existing agents (not new ones being created)
    if (!initialAgent) return;

    setSavingConfig(true);
    try {
      const updatedAgent = await agentService.updateAgent(initialAgent.id, {
        name,
        displayName,
        description,
        agentType,
        providerId,
        model,
        temperature,
        maxTokens,
        thinkingEnabled,
        instructions,
        skills: selectedSkillIds,
        mcps: selectedMcpIds,
        middleware: middleware.trim() || undefined,
      });
      setLastSaved(new Date());
      // Notify parent that agent was updated
      onAgentUpdated?.(updatedAgent);
    } catch (error) {
      console.error("Failed to save config.yaml:", error);
    } finally {
      setSavingConfig(false);
    }
  };

  const handleFileSelect = async (file: AgentFile) => {
    if (!file.isFile) {
      // Toggle folder expansion
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
      const content = await agentService.readAgentFile(getAgentId(), file.path);
      setFileContent(content);
      setEditingContent(content.content);
    } catch (error) {
      console.error("Failed to load file:", error);
    }
  };

  const handleCreateItem = async () => {
    // Get the parent folder path (if a folder is selected, create inside it)
    const parentFolder = selectedFile && !selectedFile.isFile ? selectedFile.path : "";

    if (newInputType === "folder") {
      if (!newFolderName.trim()) return;
      try {
        const folderPath = parentFolder ? `${parentFolder}/${newFolderName}` : newFolderName;
        await agentService.createAgentFolder(getAgentId(), folderPath);
        setNewFolderName("");
        setShowNewInput(false);
        // Auto-expand the parent folder
        if (parentFolder) {
          setExpandedFolders(prev => new Set([...prev, parentFolder]));
        }
        loadFiles();
      } catch (error) {
        console.error("Failed to create folder:", error);
        alert("Failed to create folder: " + error);
      }
    } else {
      if (!newFileName.trim()) return;
      try {
        const filePath = parentFolder ? `${parentFolder}/${newFileName}` : newFileName;
        await agentService.writeAgentFile(getAgentId(), filePath, "");
        setNewFileName("");
        setShowNewInput(false);
        // Auto-expand the parent folder
        if (parentFolder) {
          setExpandedFolders(prev => new Set([...prev, parentFolder]));
        }
        loadFiles();
      } catch (error) {
        console.error("Failed to create file:", error);
        alert("Failed to create file: " + error);
      }
    }
  };

  const handleDeleteFile = async () => {
    if (!selectedFile) return;
    if (!confirm(`Delete "${selectedFile.name}"?`)) return;
    try {
      await agentService.deleteAgentFile(getAgentId(), selectedFile.path);
      setSelectedFile(null);
      setFileContent(null);
      setEditingContent("");
      loadFiles();
    } catch (error) {
      console.error("Failed to delete file:", error);
      alert("Failed to delete: " + error);
    }
  };

  // Context menu handlers
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

  const handleContextMenuOpen = () => {
    if (contextMenu.file) {
      handleFileSelect(contextMenu.file);
    }
    setContextMenu(prev => ({ ...prev, show: false }));
  };

  const handleContextMenuDelete = async () => {
    if (!contextMenu.file) return;
    // Prevent deleting protected files
    if (contextMenu.file.isProtected) {
      alert(`Cannot delete ${contextMenu.file.name} - it is a protected system file.`);
      setContextMenu(prev => ({ ...prev, show: false }));
      return;
    }
    if (!confirm(`Delete "${contextMenu.file.name}"?`)) return;
    try {
      await agentService.deleteAgentFile(getAgentId(), contextMenu.file.path);
      // Clear selection if we deleted the selected file
      if (selectedFile?.path === contextMenu.file.path) {
        setSelectedFile(null);
        setFileContent(null);
        setEditingContent("");
      }
      setContextMenu(prev => ({ ...prev, show: false }));
      loadFiles();
    } catch (error) {
      console.error("Failed to delete file:", error);
      alert("Failed to delete: " + error);
    }
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
            await agentService.writeAgentFile(getAgentId(), destPath, content);
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
        const folderPath = parentFolder ? `${parentFolder}/${contextNewInput}` : contextNewInput;
        await agentService.createAgentFolder(getAgentId(), folderPath);
        if (parentFolder) {
          setExpandedFolders(prev => new Set([...prev, parentFolder]));
        }
      } else {
        const filePath = parentFolder ? `${parentFolder}/${contextNewInput}` : contextNewInput;
        await agentService.writeAgentFile(getAgentId(), filePath, "");
        if (parentFolder) {
          setExpandedFolders(prev => new Set([...prev, parentFolder]));
        }
      }
      setContextNewInput("");
      setShowContextNewInput(false);
      setContextMenu(prev => ({ ...prev, show: false }));
      loadFiles();
    } catch (error) {
      console.error("Failed to create:", error);
      alert(`Failed to create ${contextNewType}: ` + error);
    }
  };

  const handleFileUpload = async () => {
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
            await agentService.writeAgentFile(getAgentId(), file.name, content);
          } catch (error) {
            console.error("Failed to upload file:", error);
          }
        }
        loadFiles();
      };
      input.click();
    } catch (error) {
      console.error("Failed to upload:", error);
    }
  };

  const toggleMcp = (mcpId: string) => {
    setSelectedMcpIds(prev =>
      prev.includes(mcpId) ? prev.filter(id => id !== mcpId) : [...prev, mcpId]
    );
  };

  const toggleSkill = (skillId: string) => {
    setSelectedSkillIds(prev =>
      prev.includes(skillId) ? prev.filter(id => id !== skillId) : [...prev, skillId]
    );
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

  const renderFileNode = (node: FileNode): React.ReactElement => {
    const { file } = node;
    const isExpanded = expandedFolders.has(file.path);
    const isSelected = selectedFile?.path === file.path;

    return (
      <div key={file.path}>
        <div
          className={`flex items-center gap-1.5 py-1 px-2 rounded cursor-pointer text-sm ${
            isSelected
              ? 'bg-blue-600/30 text-white'
              : 'text-gray-300 hover:bg-white/5'
          }`}
          style={{ paddingLeft: `${8 + node.level * 12}px` }}
          onClick={() => handleFileSelect(file)}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handleContextMenu(e, file);
          }}
        >
          {!file.isFile ? (
            <>
              {isExpanded ? (
                <ChevronDown className="size-3.5 text-gray-500 shrink-0" />
              ) : (
                <ChevronRight className="size-3.5 text-gray-500 shrink-0" />
              )}
              <Folder className="size-4 text-yellow-400 shrink-0" />
            </>
          ) : file.isBinary ? (
            <File className="size-4 text-gray-500 shrink-0" />
          ) : (
            <FileText className="size-4 text-blue-400 shrink-0" />
          )}
          <span className="truncate">{file.name}</span>
          {file.isProtected && (
            <Lock className="size-3 text-gray-500 shrink-0 ml-auto" />
          )}
        </div>
        {!file.isFile && isExpanded && node.children.length > 0 && (
          <div onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handleContextMenu(e, file);
          }}>
            {node.children.map(childNode => renderFileNode(childNode))}
          </div>
        )}
      </div>
    );
  };

  const fileTree = buildFileTree();
  const isValid = name && displayName && description && providerId;

  return (
    <div className="fixed inset-0 bg-[#0a0a0a] z-50 flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 bg-[#141414] border-b border-white/10 shrink-0">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-gradient-to-br from-blue-500 to-purple-600">
            <Bot className="size-6 text-white" />
          </div>
          <div>
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
          </div>
        </div>
        <div className="flex items-center gap-3">
          {/* For existing agents: show last saved indicator */}
          {initialAgent && (
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

          {/* For new agents: show save button */}
          {!initialAgent && (
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
                  Save Agent
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

                {/* New Input */}
                {showNewInput && (
                  <div className="px-3 py-2 border-b border-white/10 bg-white/5">
                    <div className="flex gap-2">
                      <Input
                        placeholder={newInputType === "folder" ? "folder-name" : "file.txt"}
                        value={newInputType === "folder" ? newFolderName : newFileName}
                        onChange={(e) => newInputType === "folder" ? setNewFolderName(e.target.value) : setNewFileName(e.target.value)}
                        onKeyDown={(e) => e.key === 'Enter' && handleCreateItem()}
                        className="bg-black/30 border-white/10 text-white text-xs h-7"
                      />
                      <Button
                        size="sm"
                        onClick={handleCreateItem}
                        className="h-7 px-2 bg-blue-600 hover:bg-blue-700"
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
                  {/* File tree - includes all files (AGENTS.md, config.yaml, mcp.json, and user files) */}
                  {fileTree.map(node => renderFileNode(node))}
                </div>
              </div>
          </div>
        </div>

        {/* Main Content Area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Editor/Viewer Area */}
          <div className="flex-1 overflow-hidden flex flex-col">
            {selectedFile?.name === "config.yaml" ? (
              <ConfigYamlForm
                name={name}
                isNewAgent={!initialAgent}
                displayName={displayName}
                description={description}
                agentType={agentType}
                providerId={providerId}
                model={model}
                temperature={temperature}
                maxTokens={maxTokens}
                thinkingEnabled={thinkingEnabled}
                mcps={selectedMcpIds}
                skills={selectedSkillIds}
                middleware={middleware}
                instructions={instructions}
                providers={providers}
                availableMcps={mcps}
                availableSkills={skills}
                onDisplayNameChange={setDisplayName}
                onDescriptionChange={setDescription}
                onAgentTypeChange={setAgentType}
                onProviderIdChange={setProviderId}
                onModelChange={setModel}
                onTemperatureChange={setTemperature}
                onMaxTokensChange={setMaxTokens}
                onThinkingEnabledChange={setThinkingEnabled}
                onMcpToggle={toggleMcp}
                onSkillToggle={toggleSkill}
                onMiddlewareChange={setMiddleware}
                onInstructionsChange={setInstructions}
                onSave={saveConfigYaml}
              />
            ) : selectedFile?.name === "AGENTS.md" ? (
              /* AGENTS.md Editor */
              <div className="flex-1 flex flex-col p-4">
                <div className="flex items-center justify-between mb-3">
                  <span className="text-sm text-gray-400">
                    AGENTS.md <span className="text-gray-600">— Agent Instructions</span>
                  </span>
                </div>
                <div data-color-mode="dark" className="flex-1 flex flex-col">
                  <MDEditor
                    value={editingContent}
                    onChange={(val) => setEditingContent(val || "")}
                    height={600}
                    preview="edit"
                    hideToolbar={false}
                    visibleDragbar={false}
                    textareaProps={{
                      placeholder: "You are a helpful AI assistant...",
                    }}
                  />
                </div>
                <div className="mt-3 p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                  <p className="text-xs text-blue-300">
                    {initialAgent ? (
                      <>Changes are <strong>auto-saved</strong>. See header for last saved time.</>
                    ) : (
                      <>Changes will be saved when you click <strong>Save Agent</strong> at the top.</>
                    )}
                  </p>
                </div>
              </div>
            ) : selectedFile ? (
              fileContent?.isBinary ? (
                /* Binary file indicator */
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
                /* Text file editor */
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
                </div>
              )
            ) : (
              /* Empty state */
              <div className="flex-1 flex items-center justify-center">
                <div className="text-center">
                  <FileText className="size-16 text-gray-600 mx-auto mb-4" />
                  <h3 className="text-lg font-medium text-white mb-2">No File Selected</h3>
                  <p className="text-gray-400 text-sm mb-4">
                    Select a file from the explorer or create a new one
                  </p>
                  <div className="flex justify-center gap-2">
                    <Button
                      size="sm"
                      onClick={() => { setNewInputType("folder"); setNewFolderName(""); setShowNewInput(true); }}
                      className="bg-white/5 hover:bg-white/10 border-white/10"
                    >
                      <FolderPlus className="size-4 mr-2" />
                      New Folder
                    </Button>
                    <Button
                      size="sm"
                      onClick={handleFileUpload}
                      className="bg-white/5 hover:bg-white/10 border-white/10"
                    >
                      <Upload className="size-4 mr-2" />
                      Upload Files
                    </Button>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Context Menu */}
      {contextMenu.show && contextMenu.file && (
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

      {/* Close Confirmation Dialog for New Agents */}
      <Dialog open={showCloseConfirm} onOpenChange={setShowCloseConfirm}>
        <DialogContent className="bg-[#141414] border-white/10 text-white max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-3 text-xl">
              <div className="p-2 rounded-lg bg-yellow-500/20">
                <AlertTriangle className="size-5 text-yellow-400" />
              </div>
              Discard New Agent?
            </DialogTitle>
            <DialogDescription className="text-gray-400">
              You are about to leave without creating this agent. Any changes you made will be lost.
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
