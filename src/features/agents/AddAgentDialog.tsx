// ============================================================================
// ADD AGENT DIALOG
// Dialog for adding/editing AI agents
// ============================================================================

import { useState, useEffect } from "react";
import { Bot, Plus, Loader2, Check, Brain } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/shared/ui/dialog";
import { Input } from "@/shared/ui/input";
import { Button } from "@/shared/ui/button";
import { Label } from "@/shared/ui/label";
import { Textarea } from "@/shared/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";
import type { Agent } from "@/shared/types";
import type { Provider } from "@/shared/types";
import type { MCPServer } from "@/features/mcp/types";
import type { Skill } from "@/shared/types";
import * as providerService from "@/services/provider";
import * as mcpService from "@/services/mcp";
import * as skillsService from "@/services/skills";

interface AddAgentDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (agent: Omit<Agent, "id" | "createdAt">) => void;
  editingAgent?: Agent | null;
}

export function AddAgentDialog({ open, onClose, onSave, editingAgent }: AddAgentDialogProps) {
  const [name, setName] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [description, setDescription] = useState("");
  const [providerId, setProviderId] = useState("");
  const [model, setModel] = useState("");
  const [temperature, setTemperature] = useState(0.7);
  const [maxTokens, setMaxTokens] = useState(2000);
  const [instructions, setInstructions] = useState("");
  const [selectedMcpIds, setSelectedMcpIds] = useState<string[]>([]);
  const [selectedSkillIds, setSelectedSkillIds] = useState<string[]>([]);

  const [providers, setProviders] = useState<Provider[]>([]);
  const [mcps, setMcps] = useState<MCPServer[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [loading, setLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);

  // Load providers, MCPs, and skills on mount
  useEffect(() => {
    if (open) {
      loadOptions();
    }
  }, [open]);

  const loadOptions = async () => {
    setLoading(true);
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
      setLoading(false);
    }
  };

  // Populate form when editing an existing agent
  useEffect(() => {
    if (editingAgent) {
      setName(editingAgent.name);
      setDisplayName(editingAgent.displayName);
      setDescription(editingAgent.description);
      setProviderId(editingAgent.providerId);
      setModel(editingAgent.model);
      setTemperature(editingAgent.temperature);
      setInstructions(editingAgent.instructions);
      setSelectedMcpIds(editingAgent.mcps);
      setSelectedSkillIds(editingAgent.skills);
    } else {
      setName("");
      setDisplayName("");
      setDescription("");
      setProviderId("");
      setModel("");
      setTemperature(0.7);
      setInstructions("");
      setSelectedMcpIds([]);
      setSelectedSkillIds([]);
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

  const handleSave = async () => {
    setIsSaving(true);
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
      };
      await onSave(agent);

      // Reset form
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
      onClose();
    } finally {
      setIsSaving(false);
    }
  };

  const toggleMcp = (mcpId: string) => {
    setSelectedMcpIds((prev) =>
      prev.includes(mcpId) ? prev.filter((id) => id !== mcpId) : [...prev, mcpId]
    );
  };

  const toggleSkill = (skillId: string) => {
    setSelectedSkillIds((prev) =>
      prev.includes(skillId) ? prev.filter((id) => id !== skillId) : [...prev, skillId]
    );
  };

  const selectedProvider = providers.find((p) => p.id === providerId);
  const isValid = name && displayName && description && providerId && model && instructions;

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="bg-[#141414] border-white/10 text-white max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="text-2xl font-bold flex items-center gap-3">
            <div className="p-2 rounded-lg bg-gradient-to-br from-blue-500 to-purple-600">
              <Bot className="size-6 text-white" />
            </div>
            {editingAgent ? "Edit Agent" : "Add Agent"}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-5 mt-4">
          {/* Name and Display Name */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <Label className="text-white mb-2 block flex items-center gap-2">
                <Bot className="size-4 text-blue-400" />
                Name (ID)
              </Label>
              <Input
                placeholder="my-agent"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="bg-white/5 border-white/10 text-white placeholder:text-gray-500"
              />
              <p className="text-xs text-gray-500 mt-1">Lowercase, hyphens only</p>
            </div>

            <div>
              <Label className="text-white mb-2 block">Display Name</Label>
              <Input
                placeholder="My Agent"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
                className="bg-white/5 border-white/10 text-white placeholder:text-gray-500"
              />
            </div>
          </div>

          {/* Description */}
          <div>
            <Label className="text-white mb-2 block">Description</Label>
            <Input
              placeholder="What does this agent do?"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="bg-white/5 border-white/10 text-white placeholder:text-gray-500"
            />
          </div>

          {/* Provider and Model */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <Label className="text-white mb-2 block flex items-center gap-2">
                <Brain className="size-4 text-purple-400" />
                Provider
              </Label>
              <Select value={providerId} onValueChange={setProviderId} disabled={loading}>
                <SelectTrigger className="bg-white/5 border-white/10 text-white">
                  <SelectValue placeholder="Select provider" />
                </SelectTrigger>
                <SelectContent>
                  {providers.map((provider) => (
                    <SelectItem key={provider.id} value={provider.id}>
                      {provider.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div>
              <Label className="text-white mb-2 block">Model</Label>
              <Select
                value={model}
                onValueChange={setModel}
                disabled={!selectedProvider || loading}
              >
                <SelectTrigger className="bg-white/5 border-white/10 text-white">
                  <SelectValue placeholder="Select model" />
                </SelectTrigger>
                <SelectContent>
                  {selectedProvider?.models.map((modelOption) => (
                    <SelectItem key={modelOption} value={modelOption}>
                      {modelOption.length > 30 ? modelOption.substring(0, 30) + "..." : modelOption}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          {/* Temperature */}
          <div>
            <Label className="text-white mb-2 block flex items-center justify-between">
              <span>Temperature</span>
              <span className="text-purple-400 font-mono">{temperature.toFixed(1)}</span>
            </Label>
            <input
              type="range"
              min="0"
              max="2"
              step="0.1"
              value={temperature}
              onChange={(e) => setTemperature(parseFloat(e.target.value))}
              className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer accent-purple-500"
            />
            <div className="flex justify-between text-xs text-gray-500 mt-1">
              <span>Precise (0.0)</span>
              <span>Balanced (1.0)</span>
              <span>Creative (2.0)</span>
            </div>
          </div>

          {/* MCP Servers Selection */}
          <div>
            <Label className="text-white mb-2 block">MCP Servers</Label>
            <div className="flex flex-wrap gap-2">
              {mcps.length === 0 ? (
                <p className="text-sm text-gray-500">No MCP servers configured</p>
              ) : (
                mcps.map((mcp) => (
                  <button
                    key={mcp.id}
                    type="button"
                    onClick={() => toggleMcp(mcp.id)}
                    className={`px-3 py-1.5 rounded-lg text-sm border transition-all ${
                      selectedMcpIds.includes(mcp.id)
                        ? "bg-green-500/20 text-green-300 border-green-500/30"
                        : "bg-white/5 text-gray-400 border-white/10 hover:border-white/20"
                    }`}
                  >
                    {selectedMcpIds.includes(mcp.id) && <Check className="size-3 inline mr-1" />}
                    {mcp.name}
                  </button>
                ))
              )}
            </div>
          </div>

          {/* Skills Selection */}
          <div>
            <Label className="text-white mb-2 block">Skills</Label>
            <div className="flex flex-wrap gap-2">
              {skills.length === 0 ? (
                <p className="text-sm text-gray-500">No skills available</p>
              ) : (
                skills.map((skill) => (
                  <button
                    key={skill.id}
                    type="button"
                    onClick={() => toggleSkill(skill.id)}
                    className={`px-3 py-1.5 rounded-lg text-sm border transition-all ${
                      selectedSkillIds.includes(skill.id)
                        ? "bg-blue-500/20 text-blue-300 border-blue-500/30"
                        : "bg-white/5 text-gray-400 border-white/10 hover:border-white/20"
                    }`}
                  >
                    {selectedSkillIds.includes(skill.id) && <Check className="size-3 inline mr-1" />}
                    {skill.name}
                  </button>
                ))
              )}
            </div>
          </div>

          {/* Instructions */}
          <div>
            <Label className="text-white mb-2 block">Instructions</Label>
            <Textarea
              placeholder="You are a helpful AI assistant..."
              value={instructions}
              onChange={(e) => setInstructions(e.target.value)}
              className="bg-white/5 border-white/10 text-white placeholder:text-gray-500 min-h-[120px] resize-y"
            />
            <p className="text-xs text-gray-500 mt-1">System instructions for the agent</p>
          </div>

          {/* Info Box */}
          <div className="bg-blue-500/10 border border-blue-500/20 rounded-lg p-3">
            <p className="text-xs text-blue-300">
              💾 Configuration saved to: <code className="bg-white/10 px-1.5 py-0.5 rounded">~/.config/zeroagent/agents/{name}/</code>
            </p>
          </div>

          {/* Actions */}
          <div className="flex gap-3 pt-2">
            <Button
              onClick={onClose}
              variant="outline"
              className="flex-1 border-white/20 text-white hover:bg-white/5"
              disabled={isSaving}
            >
              Cancel
            </Button>
            <Button
              onClick={handleSave}
              disabled={!isValid || isSaving}
              className="flex-1 bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white"
            >
              {isSaving ? (
                <>
                  <Loader2 className="size-4 mr-2 animate-spin" />
                  Saving...
                </>
              ) : (
                <>
                  <Plus className="size-4 mr-2" />
                  {editingAgent ? "Update Agent" : "Create Agent"}
                </>
              )}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
