// ============================================================================
// AGENT IDE DIALOG
// IDE-style dialog for creating/editing agents with file explorer
// ============================================================================

import { useState, useEffect } from "react";
import {
  X, Save, FolderOpen, File, FileText, FolderPlus,
  Upload, RefreshCw, Bot, Brain, ChevronRight, ChevronDown, Settings
} from "lucide-react";
import { Dialog, DialogContent } from "@/shared/ui/dialog";
import { Input } from "@/shared/ui/input";
import { Button } from "@/shared/ui/button";
import { Label } from "@/shared/ui/label";
import { Textarea } from "@/shared/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";
import { Tabs, TabsList, TabsTrigger } from "@/shared/ui/tabs";
import type { Agent } from "@/shared/types";
import type { Provider } from "@/shared/types";
import type { MCPServer } from "@/features/mcp/types";
import type { Skill } from "@/shared/types";
import type { AgentFile, AgentFileContent } from "@/services/agent";
import * as agentService from "@/services/agent";
import * as providerService from "@/services/provider";
import * as mcpService from "@/services/mcp";
import * as skillsService from "@/services/skills";

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

interface AgentIDEDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (agent: Omit<Agent, "id" | "createdAt">) => void;
  editingAgent?: Agent | null;
}

export function AgentIDEDialog({ open, onClose, onSave, editingAgent }: AgentIDEDialogProps) {
  // Agent metadata state
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
  const [middleware, setMiddleware] = useState(DEFAULT_MIDDLEWARE);

  // File explorer state
  const [files, setFiles] = useState<AgentFile[]>([]);
  const [selectedFile, setSelectedFile] = useState<AgentFile | null>(null);
  const [fileContent, setFileContent] = useState<AgentFileContent | null>(null);
  const [editingContent, setEditingContent] = useState("");
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());
  const [isEditingAgentsMd, setIsEditingAgentsMd] = useState(false);
  const [newFolderName, setNewFolderName] = useState("");
  const [showNewFolderInput, setShowNewFolderInput] = useState(false);
  const [activeTab, setActiveTab] = useState<"config" | "middleware">("config");

  // Options state
  const [providers, setProviders] = useState<Provider[]>([]);
  const [mcps, setMcps] = useState<MCPServer[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [loadingOptions, setLoadingOptions] = useState(true);
  const [saving, setSaving] = useState(false);

  // Get current agent ID (for editing or temp for new)
  const currentAgentId = editingAgent?.name || name || "temp";

  // Load options on mount
  useEffect(() => {
    if (open) {
      loadOptions();
    }
  }, [open]);

  // Load files when agent changes
  useEffect(() => {
    if (open) {
      loadFiles();
    }
  }, [open, currentAgentId]);

  const loadOptions = async () => {
    setLoadingOptions(true);
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
    } finally {
      setLoadingOptions(false);
    }
  };

  const loadFiles = async () => {
    try {
      const loadedFiles = await agentService.listAgentFiles(currentAgentId);
      setFiles(loadedFiles);
    } catch (error) {
      console.error("Failed to load files:", error);
    }
  };

  // Populate form when editing
  useEffect(() => {
    if (editingAgent) {
      setName(editingAgent.name);
      setDisplayName(editingAgent.displayName);
      setDescription(editingAgent.description);
      setProviderId(editingAgent.providerId);
      setModel(editingAgent.model);
      setTemperature(editingAgent.temperature);
      setMaxTokens(editingAgent.maxTokens || 2000);
      setInstructions(editingAgent.instructions);
      setSelectedMcpIds(editingAgent.mcps);
      setSelectedSkillIds(editingAgent.skills);
      setMiddleware(editingAgent.middleware || DEFAULT_MIDDLEWARE);
    } else {
      setName("");
      setDisplayName("");
      setDescription("");
      setProviderId("");
      setModel("");
      setTemperature(0.7);
      setMaxTokens(2000);
      setInstructions("");
      setSelectedMcpIds([]);
      setSelectedSkillIds([]);
      setMiddleware(DEFAULT_MIDDLEWARE);
    }
  }, [editingAgent, open]);

  // Update model when provider changes
  useEffect(() => {
    if (providerId) {
      const provider = providers.find((p) => p.id === providerId);
      if (provider && provider.models.length > 0) {
        setModel(provider.models[0]);
      }
    }
  }, [providerId, providers]);

  const getAgentId = () => name || editingAgent?.name || "temp";

  const handleSave = async () => {
    setSaving(true);
    try {
      const agent: Omit<Agent, "id" | "createdAt"> = {
        name: name.toLowerCase().replace(/\s+/g, "-"),
        displayName,
        description,
        providerId,
        model,
        temperature,
        maxTokens,
        instructions,
        mcps: selectedMcpIds,
        skills: selectedSkillIds,
        middleware: middleware.trim() || undefined,
      };

      // First save the agent
      await agentService.createAgent(agent);

      // If editing content in a file, save it
      if (selectedFile && editingContent !== fileContent?.content) {
        await agentService.writeAgentFile(getAgentId(), selectedFile.path, editingContent);
      }

      await onSave(agent);
      onClose();
    } finally {
      setSaving(false);
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

    // Check if it's AGENTS.md (special handling)
    if (file.name === "AGENTS.md") {
      setIsEditingAgentsMd(true);
      setSelectedFile(file);
      // Load and parse AGENTS.md
      try {
        const content = await agentService.readAgentFile(getAgentId(), file.path);
        setFileContent(content);
        setEditingContent(content.content);
        // Parse frontmatter and update form
        const frontmatterMatch = content.content.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
        if (frontmatterMatch) {
          const yaml = frontmatterMatch[1];
          const body = frontmatterMatch[2];
          setInstructions(body);
          // Parse YAML fields (simplified)
          yaml.split('\n').forEach(line => {
            const [key, ...valueParts] = line.split(':');
            if (key && valueParts.length) {
              const value = valueParts.join(':').trim();
              switch (key.trim()) {
                case 'displayName':
                  setDisplayName(value.replace(/^["']|["']$/g, ''));
                  break;
                case 'description':
                  setDescription(value.replace(/^["']|["']$/g, ''));
                  break;
                case 'providerId':
                  setProviderId(value.replace(/^["']|["']$/g, ''));
                  break;
                case 'model':
                  setModel(value.replace(/^["']|["']$/g, ''));
                  break;
                case 'temperature':
                  setTemperature(parseFloat(value) || 0.7);
                  break;
              }
            }
          });
        }
      } catch (error) {
        console.error("Failed to load AGENTS.md:", error);
      }
      return;
    }

    setIsEditingAgentsMd(false);
    setSelectedFile(file);
    try {
      const content = await agentService.readAgentFile(getAgentId(), file.path);
      setFileContent(content);
      setEditingContent(content.content);
    } catch (error) {
      console.error("Failed to load file:", error);
    }
  };

  const handleCreateFolder = async () => {
    if (!newFolderName.trim()) return;
    try {
      await agentService.createAgentFolder(getAgentId(), newFolderName);
      setNewFolderName("");
      setShowNewFolderInput(false);
      loadFiles();
    } catch (error) {
      console.error("Failed to create folder:", error);
      alert("Failed to create folder: " + error);
    }
  };

  const handleSaveFile = async () => {
    if (!selectedFile) return;
    try {
      await agentService.writeAgentFile(getAgentId(), selectedFile.path, editingContent);
      // Reload file content
      const content = await agentService.readAgentFile(getAgentId(), selectedFile.path);
      setFileContent(content);
      alert("File saved!");
    } catch (error) {
      console.error("Failed to save file:", error);
      alert("Failed to save: " + error);
    }
  };

  const handleFileUpload = async () => {
    try {
      // For now, we'll use a simple approach - in a real app you'd use a file picker
      const input = document.createElement('input');
      input.type = 'file';
      input.multiple = true;
      input.onchange = async (e) => {
        const files = (e.target as HTMLInputElement).files;
        if (!files) return;

        for (const file of Array.from(files)) {
          try {
            // Read file content
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

  // Organize files into a tree structure
  const buildFileTree = () => {
    const tree: Record<string, AgentFile[]> = {};
    files.forEach(file => {
      const parts = file.path.split('/');
      if (parts.length === 1) {
        // Root level file/folder
        if (!tree['']) tree[''] = [];
        tree[''].push(file);
      } else {
        const parent = parts.slice(0, -1).join('/');
        if (!tree[parent]) tree[parent] = [];
        tree[parent].push(file);
      }
    });
    return tree;
  };

  const fileTree = buildFileTree();
  const selectedProvider = providers.find(p => p.id === providerId);

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="bg-[#141414] border-white/10 text-white max-w-[95vw] w-[1400px] max-h-[95vh] overflow-hidden p-0">
        {/* Header with save/close buttons */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
          <div className="flex items-center gap-3">
            <Bot className="size-5 text-blue-400" />
            <h2 className="text-lg font-semibold">
              {editingAgent ? "Edit Agent" : "New Agent"}
            </h2>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={handleSave}
              disabled={saving || !name || !displayName || !description || !providerId}
              className="border-white/20 text-white hover:bg-white/5"
            >
              <Save className="size-4 mr-2" />
              {saving ? "Saving..." : "Save"}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={onClose}
              className="text-gray-400 hover:text-white h-8 w-8 p-0"
            >
              <X className="size-4" />
            </Button>
          </div>
        </div>

        <div className="flex h-[calc(95vh-60px)]">
          {/* Left Sidebar - File Explorer */}
          <div className="w-64 border-r border-white/10 flex flex-col bg-black/20">
            <div className="p-3 border-b border-white/10">
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-semibold text-gray-400 uppercase tracking-wide">
                  Explorer
                </span>
                <div className="flex items-center gap-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={handleFileUpload}
                    className="text-gray-400 hover:text-white h-6 w-6 p-0"
                    title="Upload Files"
                  >
                    <Upload className="size-3" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setShowNewFolderInput(!showNewFolderInput)}
                    className="text-gray-400 hover:text-white h-6 w-6 p-0"
                    title="New Folder"
                  >
                    <FolderPlus className="size-3" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={loadFiles}
                    className="text-gray-400 hover:text-white h-6 w-6 p-0"
                    title="Refresh"
                  >
                    <RefreshCw className="size-3" />
                  </Button>
                </div>
              </div>
              {showNewFolderInput && (
                <div className="flex gap-1">
                  <Input
                    placeholder="folder-name"
                    value={newFolderName}
                    onChange={(e) => setNewFolderName(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && handleCreateFolder()}
                    className="bg-white/5 border-white/10 text-white text-xs h-7"
                  />
                  <Button
                    size="sm"
                    onClick={handleCreateFolder}
                    className="h-7 px-2 bg-blue-600 hover:bg-blue-700"
                  >
                    <FolderPlus className="size-3" />
                  </Button>
                </div>
              )}
            </div>

            <div className="flex-1 overflow-y-auto p-2">
              {/* AGENTS.md special file */}
              <div
                className={`flex items-center gap-2 px-2 py-1 rounded cursor-pointer text-sm ${
                  selectedFile?.name === 'AGENTS.md'
                    ? 'bg-blue-600/30 text-white'
                    : 'text-gray-300 hover:bg-white/5'
                }`}
                onClick={() => handleFileSelect({ name: 'AGENTS.md', path: 'AGENTS.md', isFile: true, isBinary: false, isProtected: true, size: 0 })}
              >
                <FileText className="size-4 text-blue-400 shrink-0" />
                <span>AGENTS.md</span>
                <span className="ml-auto text-xs text-gray-500">⚙</span>
              </div>

              {/* File tree */}
              {Object.entries(fileTree).map(([parent, children]) =>
                parent === '' ? (
                  <div key="root" className="space-y-0.5">
                    {children.map(file => (
                      <div key={file.path}>
                        <div
                          className={`flex items-center gap-2 px-2 py-1 rounded cursor-pointer text-sm ${
                            selectedFile?.path === file.path
                              ? 'bg-blue-600/30 text-white'
                              : 'text-gray-300 hover:bg-white/5'
                          }`}
                          onClick={() => handleFileSelect(file)}
                        >
                          {file.isFile ? (
                            file.isBinary ? (
                              <File className="size-4 text-gray-500 shrink-0" />
                            ) : (
                              <FileText className="size-4 text-green-400 shrink-0" />
                            )
                          ) : (
                            <>
                              {expandedFolders.has(file.path) ? (
                                <ChevronDown className="size-3 text-gray-500 shrink-0" />
                              ) : (
                                <ChevronRight className="size-3 text-gray-500 shrink-0" />
                              )}
                              <FolderOpen className="size-4 text-yellow-400 shrink-0" />
                            </>
                          )}
                          <span className="truncate">{file.name}</span>
                          {!file.isFile && (
                            <span className="ml-auto text-xs text-gray-500">
                              {children.filter(f => f.path.startsWith(file.path)).length}
                            </span>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                ) : null
              )}
            </div>
          </div>

          {/* Main Content Area */}
          <div className="flex-1 flex flex-col overflow-hidden">
            {/* Tab Navigation */}
            <div className="border-b border-white/10 bg-black/20">
              <div className="flex items-center justify-between px-4 py-2">
                <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as "config" | "middleware")} className="w-full">
                  <TabsList className="bg-black/40 border border-white/10">
                    <TabsTrigger value="config" className="data-[state=active]:bg-blue-600/30 data-[state=active]:text-blue-300">
                      <Bot className="size-4 mr-2" />
                      Configuration
                    </TabsTrigger>
                    <TabsTrigger value="middleware" className="data-[state=active]:bg-blue-600/30 data-[state=active]:text-blue-300">
                      <Settings className="size-4 mr-2" />
                      Middleware
                    </TabsTrigger>
                  </TabsList>
                </Tabs>
              </div>
            </div>

            {/* Tab Content */}
            <div className="flex-1 overflow-hidden flex flex-col">
              {activeTab === "config" ? (
                <>
                  {/* Top metadata section - 2 rows */}
                  <div className="p-4 border-b border-white/10 bg-black/20">
                    <div className="grid grid-cols-4 gap-3 mb-3">
                      {/* Row 1 */}
                      <div>
                        <Label className="text-gray-400 text-xs mb-1 block">Name (ID)</Label>
                        <Input
                          placeholder="my-agent"
                          value={name}
                          onChange={(e) => setName(e.target.value)}
                          className="bg-white/5 border-white/10 text-white text-sm h-8"
                        />
                      </div>
                      <div>
                        <Label className="text-gray-400 text-xs mb-1 block">Display Name</Label>
                        <Input
                          placeholder="My Agent"
                          value={displayName}
                          onChange={(e) => setDisplayName(e.target.value)}
                          className="bg-white/5 border-white/10 text-white text-sm h-8"
                        />
                      </div>
                      <div className="col-span-2">
                        <Label className="text-gray-400 text-xs mb-1 block">Description</Label>
                        <Input
                          placeholder="What does this agent do?"
                          value={description}
                          onChange={(e) => setDescription(e.target.value)}
                          className="bg-white/5 border-white/10 text-white text-sm h-8"
                        />
                      </div>
                    </div>

                    <div className="grid grid-cols-4 gap-3">
                      {/* Row 2 */}
                      <div>
                        <Label className="text-gray-400 text-xs mb-1 block flex items-center gap-1">
                          <Brain className="size-3 text-purple-400" />
                          Provider
                        </Label>
                        <Select value={providerId} onValueChange={setProviderId} disabled={loadingOptions}>
                          <SelectTrigger className="bg-white/5 border-white/10 text-white h-8">
                            <SelectValue placeholder="Select" />
                          </SelectTrigger>
                          <SelectContent>
                            {providers.map(p => (
                              <SelectItem key={p.id} value={p.id}>{p.name}</SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </div>
                      <div>
                        <Label className="text-gray-400 text-xs mb-1 block">Model</Label>
                        <Select value={model} onValueChange={setModel} disabled={!selectedProvider || loadingOptions}>
                          <SelectTrigger className="bg-white/5 border-white/10 text-white h-8">
                            <SelectValue placeholder="Select" />
                          </SelectTrigger>
                          <SelectContent>
                            {selectedProvider?.models.map(m => (
                              <SelectItem key={m} value={m}>
                                {m.length > 25 ? m.substring(0, 25) + '...' : m}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </div>
                      <div>
                        <Label className="text-gray-400 text-xs mb-1 block">Temperature</Label>
                        <div className="flex items-center gap-2">
                          <input
                            type="range"
                            min="0"
                            max="2"
                            step="0.1"
                            value={temperature}
                            onChange={(e) => setTemperature(parseFloat(e.target.value))}
                            className="flex-1 h-1 bg-white/10 rounded-lg appearance-none cursor-pointer accent-purple-500"
                          />
                          <span className="text-xs text-purple-400 w-8 text-right">{temperature.toFixed(1)}</span>
                        </div>
                      </div>
                      <div>
                        <Label className="text-gray-400 text-xs mb-1 block">MCPs</Label>
                        <div className="flex flex-wrap gap-1">
                          {selectedMcpIds.slice(0, 2).map(id => {
                            const mcp = mcps.find(m => m.id === id);
                            return mcp ? (
                              <span key={id} className="px-1.5 py-0.5 bg-green-500/20 rounded text-xs text-green-300">
                                {mcp.name}
                              </span>
                            ) : null;
                          })}
                          {selectedMcpIds.length > 2 && (
                            <span className="px-1.5 py-0.5 bg-gray-500/20 rounded text-xs text-gray-300">
                              +{selectedMcpIds.length - 2}
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Editor/Viewer Area */}
                  <div className="flex-1 overflow-hidden flex flex-col">
                    {isEditingAgentsMd ? (
                      /* AGENTS.md Special Editor - Show instructions with frontmatter context */
                      <div className="flex-1 flex flex-col p-4">
                        <div className="flex items-center justify-between mb-3">
                          <span className="text-sm text-gray-400">
                            AGENTS.md <span className="text-gray-600">— Instructions (metadata managed above)</span>
                          </span>
                        </div>
                        <Textarea
                          value={instructions}
                          onChange={(e) => setInstructions(e.target.value)}
                          placeholder="You are a helpful AI assistant..."
                          className="flex-1 bg-white/5 border-white/10 text-white font-mono text-sm resize-none min-h-[200px]"
                        />
                        <div className="mt-3 p-3 bg-purple-500/10 border border-purple-500/20 rounded-lg">
                          <p className="text-xs text-purple-300">
                            The frontmatter (metadata) is managed via the form above. This area is for the agent's instructions only.
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
                          <div className="flex items-center justify-between px-4 py-2 border-b border-white/10 bg-black/20">
                            <span className="text-sm text-gray-400">{selectedFile.name}</span>
                            <Button
                              size="sm"
                              onClick={handleSaveFile}
                              className="bg-blue-600 hover:bg-blue-700 text-xs h-7"
                            >
                              <Save className="size-3 mr-1" />
                              Save File
                            </Button>
                          </div>
                          <Textarea
                            value={editingContent}
                            onChange={(e) => setEditingContent(e.target.value)}
                            className="flex-1 bg-[#0a0a0a] border-0 text-white font-mono text-sm resize-none p-4"
                            spellCheck={false}
                          />
                        </div>
                      )
                    ) : (
                      /* Empty state */
                      <div className="flex-1 flex items-center justify-center">
                        <div className="text-center">
                          <FileText className="size-16 text-gray-600 mx-auto mb-4" />
                          <h3 className="text-lg font-medium text-white mb-2">No File Selected</h3>
                          <p className="text-gray-400 text-sm mb-4">
                            Select a file from the explorer to view or edit
                          </p>
                          <div className="flex justify-center gap-2">
                            <Button
                              size="sm"
                              onClick={() => setShowNewFolderInput(!showNewFolderInput)}
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
                </>
              ) : (
                <>
                  {/* Middleware Configuration Tab */}
                  <div className="flex-1 flex flex-col p-4 overflow-hidden">
                    <div className="flex items-center justify-between mb-3">
                      <div>
                        <h3 className="text-sm font-semibold text-white flex items-center gap-2">
                          <Settings className="size-4 text-blue-400" />
                          Middleware Configuration
                        </h3>
                        <p className="text-xs text-gray-400 mt-1">
                          Configure middleware for conversation management (YAML format)
                        </p>
                      </div>
                    </div>
                    <div className="flex-1 overflow-hidden">
                      <Textarea
                        value={middleware}
                        onChange={(e) => setMiddleware(e.target.value)}
                        placeholder={DEFAULT_MIDDLEWARE}
                        className="flex-1 bg-[#0a0a0a] border-white/10 text-white font-mono text-sm resize-none p-4"
                        spellCheck={false}
                      />
                    </div>
                    <div className="mt-3 p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                      <p className="text-xs text-blue-300">
                        <strong>Middlewares:</strong> Summarization - compresses conversation history when approaching token limits. Context Editing - clears older tool call outputs while keeping recent ones.
                      </p>
                    </div>
                  </div>
                </>
              )}
            </div>

            {/* Bottom panel - MCPs and Skills */}
            {selectedFile?.name !== 'AGENTS.md' && (
              <div className="border-t border-white/10 bg-black/20 p-3">
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <Label className="text-gray-400 text-xs mb-2 block">MCP Servers</Label>
                    <div className="flex flex-wrap gap-1.5">
                      {mcps.map(mcp => (
                        <button
                          key={mcp.id}
                          type="button"
                          onClick={() => toggleMcp(mcp.id)}
                          className={`px-2 py-1 rounded text-xs border transition-all ${
                            selectedMcpIds.includes(mcp.id)
                              ? 'bg-green-500/20 text-green-300 border-green-500/30'
                              : 'bg-white/5 text-gray-400 border-white/10 hover:border-white/20'
                          }`}
                        >
                          {mcp.name}
                        </button>
                      ))}
                    </div>
                  </div>
                  <div>
                    <Label className="text-gray-400 text-xs mb-2 block">Skills</Label>
                    <div className="flex flex-wrap gap-1.5">
                      {skills.map(skill => (
                        <button
                          key={skill.id}
                          type="button"
                          onClick={() => toggleSkill(skill.id)}
                          className={`px-2 py-1 rounded text-xs border transition-all ${
                            selectedSkillIds.includes(skill.id)
                              ? 'bg-blue-500/20 text-blue-300 border-blue-500/30'
                              : 'bg-white/5 text-gray-400 border-white/10 hover:border-white/20'
                          }`}
                        >
                          {skill.name}
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
