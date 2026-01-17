// ============================================================================
// CONFIG.YAML FORM
// Form-based editor for config.yaml
// ============================================================================

import { useEffect } from "react";
import { Bot, Brain, Server as ServerIcon, Sparkles, Lock } from "lucide-react";
import { Input } from "@/shared/ui/input";
import { Label } from "@/shared/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";
import { Switch } from "@/shared/ui/switch";
import type { Provider } from "@/shared/types";
import type { MCPServer } from "@/features/mcp/types";
import type { Skill } from "@/shared/types";

interface ConfigYamlFormProps {
  // Read-only ID (name)
  name: string;
  isNewAgent: boolean;

  // Editable fields
  displayName: string;
  description: string;
  providerId: string;
  model: string;
  temperature: number;
  maxTokens: number;
  thinkingEnabled: boolean;
  mcps: string[];
  skills: string[];

  // Options
  providers: Provider[];
  availableMcps: MCPServer[];
  availableSkills: Skill[];

  // Callbacks
  onDisplayNameChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onProviderIdChange: (value: string) => void;
  onModelChange: (value: string) => void;
  onTemperatureChange: (value: number) => void;
  onMaxTokensChange: (value: number) => void;
  onThinkingEnabledChange: (value: boolean) => void;
  onMcpToggle: (mcpId: string) => void;
  onSkillToggle: (skillId: string) => void;
  onSave: () => void;
}

export function ConfigYamlForm({
  name,
  isNewAgent,
  displayName,
  description,
  providerId,
  model,
  temperature,
  maxTokens,
  thinkingEnabled,
  mcps,
  skills,
  providers,
  availableMcps,
  availableSkills,
  onDisplayNameChange,
  onDescriptionChange,
  onProviderIdChange,
  onModelChange,
  onTemperatureChange,
  onMaxTokensChange,
  onThinkingEnabledChange,
  onMcpToggle,
  onSkillToggle,
  onSave,
}: ConfigYamlFormProps) {
  const selectedProvider = providers.find(p => p.id === providerId);

  // Auto-save on any change
  useEffect(() => {
    const timer = setTimeout(() => {
      onSave();
    }, 500);
    return () => clearTimeout(timer);
  }, [displayName, description, providerId, model, temperature, maxTokens, thinkingEnabled, mcps, skills]);

  return (
    <div className="flex-1 overflow-y-auto p-6">
      {/* Header */}
      <div className="flex items-center gap-3 mb-6 pb-4 border-b border-white/10">
        <div className="p-2 rounded-lg bg-yellow-500/20">
          <Lock className="size-5 text-yellow-400" />
        </div>
        <div>
          <h2 className="text-lg font-semibold text-white">Agent Configuration</h2>
          <p className="text-sm text-gray-400">
            Changes are automatically saved to config.yaml
          </p>
        </div>
      </div>

      <div className="space-y-4 max-w-4xl">
        {/* Name (ID) and Display Name - side by side */}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <Label className="text-gray-400 text-xs mb-1.5 flex items-center gap-2">
              <Bot className="size-3.5 text-blue-400" />
              Name (ID) {isNewAgent && <span className="text-gray-500 font-normal">(auto-generated)</span>}
            </Label>
            <Input
              value={name || "(auto-generated from Display Name)"}
              disabled
              className="bg-white/5 border-white/10 text-gray-500 cursor-not-allowed h-9 text-sm"
            />
          </div>
          <div>
            <Label className="text-gray-400 text-xs mb-1.5 block">Display Name</Label>
            <Input
              placeholder="My Agent"
              value={displayName}
              onChange={(e) => onDisplayNameChange(e.target.value)}
              className="bg-white/5 border-white/10 text-white h-9 text-sm"
            />
          </div>
        </div>

        {/* Description - full width */}
        <div>
          <Label className="text-gray-400 text-xs mb-1.5 block">Description</Label>
          <Input
            placeholder="What does this agent do?"
            value={description}
            onChange={(e) => onDescriptionChange(e.target.value)}
            className="bg-white/5 border-white/10 text-white h-9 text-sm"
          />
        </div>

        {/* Provider and Model - side by side */}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <Label className="text-gray-400 text-xs mb-1.5 flex items-center gap-2">
              <Brain className="size-3.5 text-purple-400" />
              Provider
            </Label>
            <Select value={providerId} onValueChange={onProviderIdChange}>
              <SelectTrigger className="bg-white/5 border-white/10 text-white h-9 text-sm">
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
            <Label className="text-gray-400 text-xs mb-1.5 block">Model</Label>
            <Select
              value={model}
              onValueChange={onModelChange}
              disabled={!selectedProvider}
            >
              <SelectTrigger className="bg-white/5 border-white/10 text-white h-9 text-sm">
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

        {/* Temperature and Max Tokens - side by side */}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <Label className="text-gray-400 text-xs mb-1.5 flex items-center justify-between">
              <span>Temperature</span>
              <span className="text-purple-400 font-mono text-xs">{temperature.toFixed(1)}</span>
            </Label>
            <input
              type="range"
              min="0"
              max="2"
              step="0.1"
              value={temperature}
              onChange={(e) => onTemperatureChange(parseFloat(e.target.value))}
              className="w-full h-1.5 bg-white/10 rounded-lg appearance-none cursor-pointer accent-purple-500 mt-2"
            />
            <div className="flex justify-between text-xs text-gray-500 mt-1">
              <span>0.0</span>
              <span>1.0</span>
              <span>2.0</span>
            </div>
          </div>

          <div>
            <Label className="text-gray-400 text-xs mb-1.5 block">Max Tokens</Label>
            <Input
              type="number"
              min="1"
              max="32000"
              step="100"
              value={maxTokens}
              onChange={(e) => onMaxTokensChange(Math.max(1, parseInt(e.target.value) || 2000))}
              className="bg-white/5 border-white/10 text-white h-9 text-sm"
            />
          </div>
        </div>

        {/* Thinking Enabled */}
        <div className="flex items-center justify-between py-2 px-3 bg-white/5 rounded-lg border border-white/10">
          <div className="flex items-center gap-2">
            <Brain className="size-4 text-purple-400" />
            <div>
              <Label className="text-white text-sm cursor-pointer">Thinking Mode</Label>
              <p className="text-xs text-gray-400">Enable chain-of-thought reasoning (DeepSeek, GLM)</p>
            </div>
          </div>
          <Switch
            checked={thinkingEnabled}
            onCheckedChange={onThinkingEnabledChange}
          />
        </div>

        {/* MCP Servers and Skills - compact lists side by side */}
        <div className="grid grid-cols-2 gap-4">
          {/* MCP Servers */}
          <div>
            <Label className="text-gray-400 text-xs mb-2 flex items-center gap-2">
              <ServerIcon className="size-3.5 text-green-400" />
              MCPs ({mcps.length}/{availableMcps.length})
            </Label>
            <div className="space-y-1 max-h-48 overflow-y-auto">
              {availableMcps.length === 0 ? (
                <p className="text-sm text-gray-500 italic">No MCP servers</p>
              ) : (
                availableMcps.map((mcp) => (
                  <button
                    key={mcp.id}
                    type="button"
                    onClick={() => onMcpToggle(mcp.id)}
                    className={`w-full flex items-center gap-2 px-2 py-1.5 rounded text-sm text-left transition-all ${
                      mcps.includes(mcp.id)
                        ? "bg-green-500/20 text-green-300 border border-green-500/30"
                        : "bg-white/5 text-gray-400 border border-white/10 hover:border-white/20"
                    }`}
                  >
                    <ServerIcon className="size-3.5 shrink-0" />
                    <span className="flex-1 truncate text-xs">{mcp.name}</span>
                    {mcps.includes(mcp.id) && (
                      <span className="text-xs">✓</span>
                    )}
                  </button>
                ))
              )}
            </div>
          </div>

          {/* Skills */}
          <div>
            <Label className="text-gray-400 text-xs mb-2 flex items-center gap-2">
              <Sparkles className="size-3.5 text-blue-400" />
              Skills ({skills.length}/{availableSkills.length})
            </Label>
            <div className="space-y-1 max-h-48 overflow-y-auto">
              {availableSkills.length === 0 ? (
                <p className="text-sm text-gray-500 italic">No skills</p>
              ) : (
                availableSkills.map((skill) => (
                  <button
                    key={skill.id}
                    type="button"
                    onClick={() => onSkillToggle(skill.id)}
                    className={`w-full flex items-center gap-2 px-2 py-1.5 rounded text-sm text-left transition-all ${
                      skills.includes(skill.id)
                        ? "bg-blue-500/20 text-blue-300 border border-blue-500/30"
                        : "bg-white/5 text-gray-400 border border-white/10 hover:border-white/20"
                    }`}
                  >
                    <Sparkles className="size-3.5 shrink-0" />
                    <span className="flex-1 truncate text-xs">{skill.name}</span>
                    {skills.includes(skill.id) && (
                      <span className="text-xs">✓</span>
                    )}
                  </button>
                ))
              )}
            </div>
          </div>
        </div>

        {/* Info Box */}
        <div className="bg-blue-500/10 border border-blue-500/20 rounded-lg p-3">
          <p className="text-xs text-blue-300">
            <strong>config.yaml</strong> stores agent configuration. Instructions are in <strong>AGENTS.md</strong>.
          </p>
        </div>
      </div>
    </div>
  );
}
