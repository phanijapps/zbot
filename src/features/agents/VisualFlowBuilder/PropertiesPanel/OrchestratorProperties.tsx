// ============================================================================
// ZERO IDE - ORCHESTRATOR PROPERTIES
// Flow-level Orchestrator Agent configuration (LLM, tools, MCPs, skills, middleware)
// ============================================================================

import { memo, useState, useEffect, useCallback, lazy, Suspense } from "react";
import type { OrchestratorConfig, ToolSelection } from "../types";
import { useAgentResources } from "../hooks/useAgentResources";
import { TOOL_CATEGORIES_CONFIG } from "../constants";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";
import { Button } from "@/shared/ui/button";
import { ToolsConfigDialog } from "./ToolsConfigDialog";
import { SkillsConfigDialog } from "./SkillsConfigDialog";
import { MCPsConfigDialog } from "./MCPsConfigDialog";

// Lazy load SystemInstructionsDialog to reduce initial bundle size
const SystemInstructionsDialog = lazy(() =>
  import("./SystemInstructionsDialog").then((module) => ({
    default: module.SystemInstructionsDialog,
  }))
);

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const BotIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M12 8V4H8" /><rect width="16" height="12" x="4" y="8" rx="2" /><path d="M2 14h2" /><path d="M20 14h2" /><path d="M15 13v2" /><path d="M9 13v2" />
  </svg>
);

const SettingsIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.47a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.39a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
    <circle cx="12" cy="12" r="3" />
  </svg>
);

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface OrchestratorPropertiesProps {
  config: OrchestratorConfig;
  onUpdate: (updates: Partial<OrchestratorConfig>) => void;
}

// -----------------------------------------------------------------------------
// Helper: Count enabled tools
// -----------------------------------------------------------------------------

function countEnabledTools(tools: ToolSelection | undefined): number {
  // If no tools config exists, all tools are selected by default
  if (!tools || Object.keys(tools).length === 0) {
    return countTotalTools();
  }

  let count = 0;
  for (const [categoryKey, category] of Object.entries(tools)) {
    if (category?.enabled) {
      const categoryTools = category.tools || {};
      const hasExplicitSelection = Object.keys(categoryTools).length > 0;

      if (hasExplicitSelection) {
        // Count only explicitly selected tools
        for (const enabled of Object.values(categoryTools)) {
          if (enabled) count++;
        }
      } else {
        // Empty tools object means all tools in this category are enabled
        const categoryConfig = TOOL_CATEGORIES_CONFIG[categoryKey as keyof typeof TOOL_CATEGORIES_CONFIG];
        if (categoryConfig) {
          count += Object.keys(categoryConfig.tools).length;
        }
      }
    }
  }
  return count;
}

function countTotalTools(): number {
  let count = 0;
  for (const category of Object.values(TOOL_CATEGORIES_CONFIG)) {
    count += Object.keys(category.tools).length;
  }
  return count;
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export const OrchestratorProperties = memo(({ config, onUpdate }: OrchestratorPropertiesProps) => {
  const [localConfig, setLocalConfig] = useState(config);
  const [maxTokensInput, setMaxTokensInput] = useState(String(config.maxTokens ?? 4096));

  // Dialog open states
  const [toolsDialogOpen, setToolsDialogOpen] = useState(false);
  const [skillsDialogOpen, setSkillsDialogOpen] = useState(false);
  const [mcpsDialogOpen, setMCPsDialogOpen] = useState(false);
  const [systemInstructionsDialogOpen, setSystemInstructionsDialogOpen] = useState(false);

  // Load actual resources from backend
  const { providers, mcps: availableMCPs, skills: availableSkills, loading } = useAgentResources();

  // Get current selections
  const tools = localConfig.tools || {};
  const mcps = localConfig.mcps || [];
  const skills = localConfig.skills || [];
  const middleware = localConfig.middleware || {};

  // Get the selected provider to show its models
  const selectedProvider = providers.find(p => p.id === localConfig.providerId || p.name === localConfig.providerId);

  useEffect(() => {
    setLocalConfig(config);
    setMaxTokensInput(String(config.maxTokens ?? 4096));
  }, [config]);

  const handleChange = useCallback((field: keyof OrchestratorConfig, value: unknown) => {
    const newConfig = { ...localConfig, [field]: value };
    setLocalConfig(newConfig);
    onUpdate(newConfig);
  }, [localConfig, onUpdate]);

  // Dialog handlers
  const handleOpenToolsDialog = useCallback(() => setToolsDialogOpen(true), []);
  const handleOpenSkillsDialog = useCallback(() => setSkillsDialogOpen(true), []);
  const handleOpenMCPsDialog = useCallback(() => setMCPsDialogOpen(true), []);
  const handleOpenSystemInstructionsDialog = useCallback(() => setSystemInstructionsDialogOpen(true), []);

  const handleSaveTools = useCallback((newTools: ToolSelection) => {
    handleChange("tools", newTools);
  }, [handleChange]);

  const handleSaveSkills = useCallback((newSkills: string[]) => {
    handleChange("skills", newSkills);
  }, [handleChange]);

  const handleSaveMCPs = useCallback((newMCPs: string[]) => {
    handleChange("mcps", newMCPs);
  }, [handleChange]);

  const handleSaveSystemInstructions = useCallback((newInstructions: string) => {
    handleChange("systemInstructions", newInstructions);
  }, [handleChange]);

  const handleToggleMiddleware = useCallback((type: "summarization" | "contextEditing") => {
    const current = middleware[type] as { enabled?: boolean } | undefined;
    const isEnabled = current && typeof current === "object" && "enabled" in current && current.enabled === true;
    const newValue = isEnabled ? undefined : { enabled: true };
    const newMiddleware = { ...middleware, [type]: newValue };
    handleChange("middleware", newMiddleware);
  }, [middleware, handleChange]);

  return (
    <div className="space-y-4">
      {/* Basic Settings */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Orchestrator Settings
        </h3>
        <div className="space-y-3">
          <div>
            <Label className="text-white text-xs mb-1.5 block">Display Name</Label>
            <Input
              value={localConfig.displayName || ""}
              onChange={(e) => handleChange("displayName", e.target.value)}
              placeholder="My Agent"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Description</Label>
            <Input
              value={localConfig.description || ""}
              onChange={(e) => handleChange("description", e.target.value)}
              placeholder="Brief description of this agent..."
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>
        </div>
      </div>

      {/* Model Configuration */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3 flex items-center gap-2">
          <BotIcon />
          Model Configuration
        </h3>
        <div className="space-y-3">
          <div>
            <Label className="text-white text-xs mb-1.5 block">Provider</Label>
            {loading ? (
              <Input
                value="Loading..."
                disabled
                className="bg-white/5 border-white/10 text-gray-500 text-sm h-8"
              />
            ) : providers.length > 0 ? (
              <Select
                value={localConfig.providerId || ""}
                onValueChange={(value) => handleChange("providerId", value)}
              >
                <SelectTrigger size="sm" className="bg-white/5 border-white/10 text-white">
                  <SelectValue placeholder="Select a provider" />
                </SelectTrigger>
                <SelectContent>
                  {providers.map((provider) => (
                    <SelectItem key={provider.id || provider.name} value={provider.id || provider.name}>
                      {provider.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            ) : (
              <Input
                value={localConfig.providerId || ""}
                onChange={(e) => handleChange("providerId", e.target.value)}
                placeholder="openai"
                className="bg-white/5 border-white/10 text-white text-sm h-8"
              />
            )}
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Model</Label>
            {selectedProvider?.models && selectedProvider.models.length > 0 ? (
              <Select
                value={localConfig.model || ""}
                onValueChange={(value) => handleChange("model", value)}
              >
                <SelectTrigger size="sm" className="bg-white/5 border-white/10 text-white">
                  <SelectValue placeholder="Select a model" />
                </SelectTrigger>
                <SelectContent>
                  {selectedProvider.models.map((model) => (
                    <SelectItem key={model} value={model}>
                      {model}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            ) : (
              <Input
                value={localConfig.model || ""}
                onChange={(e) => handleChange("model", e.target.value)}
                placeholder="gpt-4o"
                className="bg-white/5 border-white/10 text-white text-sm h-8"
              />
            )}
          </div>

          <div>
            <div className="flex items-center justify-between mb-1.5">
              <Label className="text-white text-xs">Temperature</Label>
              <span className="text-[10px] text-gray-400">{localConfig.temperature?.toFixed(1) ?? "0.7"}</span>
            </div>
            <input
              type="range"
              min={0}
              max={2}
              step={0.1}
              value={localConfig.temperature ?? 0.7}
              onChange={(e) => handleChange("temperature", parseFloat(e.target.value))}
              className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-violet-500"
            />
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Max Tokens</Label>
            <Input
              type="text"
              inputMode="numeric"
              value={maxTokensInput}
              onChange={(e) => {
                const value = e.target.value;
                // Allow empty input or valid numbers
                if (value === "" || /^\d+$/.test(value)) {
                  setMaxTokensInput(value);
                  const numValue = parseInt(value);
                  if (!isNaN(numValue) && numValue > 0) {
                    handleChange("maxTokens", numValue);
                  }
                }
              }}
              onBlur={() => {
                // Default to 100 if empty when user leaves the field
                if (maxTokensInput === "" || parseInt(maxTokensInput) <= 0) {
                  setMaxTokensInput("100");
                  handleChange("maxTokens", 100);
                }
              }}
              className="bg-white/5 border-white/10 text-white text-sm h-8"
              placeholder="4096"
            />
            {maxTokensInput === "" && (
              <p className="text-[10px] text-red-400 mt-1">
                ⚠️ Max tokens required (will default to 100)
              </p>
            )}
            {maxTokensInput !== "" && (localConfig.maxTokens ?? 4096) > 8192 && (
              <p className="text-[10px] text-yellow-400 mt-1">
                ⚠️ Warning: Large token counts may hit LLM limits
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Tools Configuration */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Tools
        </h3>
        <Button
          variant="outline"
          size="sm"
          onClick={handleOpenToolsDialog}
          className="w-full justify-start border-white/20 text-white hover:bg-white/5 h-9"
        >
          <SettingsIcon />
          Configure Tools
          <span className="ml-auto text-xs text-gray-400">
            {countEnabledTools(tools)} selected
          </span>
        </Button>
        <p className="text-[10px] text-gray-500 mt-2">
          {countEnabledTools(tools)} of {countTotalTools()} tools selected
        </p>
      </div>

      {/* MCPs Configuration */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          MCPs
        </h3>
        <Button
          variant="outline"
          size="sm"
          onClick={handleOpenMCPsDialog}
          className="w-full justify-start border-white/20 text-white hover:bg-white/5 h-9"
        >
          <SettingsIcon />
          Configure MCPs
          <span className="ml-auto text-xs text-gray-400">
            {mcps.length} selected
          </span>
        </Button>
        <p className="text-[10px] text-gray-500 mt-2">
          {availableMCPs.length === 0
            ? "No MCPs configured. Add MCPs in Settings."
            : `${availableMCPs.length} available`
          }
        </p>
      </div>

      {/* Skills Configuration */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Skills
        </h3>
        <Button
          variant="outline"
          size="sm"
          onClick={handleOpenSkillsDialog}
          className="w-full justify-start border-white/20 text-white hover:bg-white/5 h-9"
        >
          <SettingsIcon />
          Configure Skills
          <span className="ml-auto text-xs text-gray-400">
            {skills.length} selected
          </span>
        </Button>
        <p className="text-[10px] text-gray-500 mt-2">
          {availableSkills.length === 0
            ? "No skills configured. Add skills in the Vault."
            : `${availableSkills.length} available`
          }
        </p>
      </div>

      {/* Middleware Configuration */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Middleware
        </h3>
        <div className="space-y-2">
          <label className={`flex items-center gap-2 p-2 rounded cursor-pointer transition-colors ${
            middleware.summarization && typeof middleware.summarization === "object" && "enabled" in middleware.summarization && middleware.summarization.enabled
              ? "bg-violet-500/10 border border-violet-500/30"
              : "bg-white/5 hover:bg-white/10 border border-transparent"
          }`}>
            <input
              type="checkbox"
              checked={Boolean(middleware.summarization && typeof middleware.summarization === "object" && "enabled" in middleware.summarization && (middleware.summarization as { enabled: boolean }).enabled)}
              onChange={() => handleToggleMiddleware("summarization")}
              className="rounded"
            />
            <div className="flex-1">
              <p className="text-xs text-white font-medium">Summarization</p>
              <p className="text-[10px] text-gray-500">Compress context when it gets too large</p>
            </div>
          </label>

          <label className={`flex items-center gap-2 p-2 rounded cursor-pointer transition-colors ${
            middleware.contextEditing && typeof middleware.contextEditing === "object" && "enabled" in middleware.contextEditing && middleware.contextEditing.enabled
              ? "bg-violet-500/10 border border-violet-500/30"
              : "bg-white/5 hover:bg-white/10 border border-transparent"
          }`}>
            <input
              type="checkbox"
              checked={Boolean(middleware.contextEditing && typeof middleware.contextEditing === "object" && "enabled" in middleware.contextEditing && (middleware.contextEditing as { enabled: boolean }).enabled)}
              onChange={() => handleToggleMiddleware("contextEditing")}
              className="rounded"
            />
            <div className="flex-1">
              <p className="text-xs text-white font-medium">Context Editing</p>
              <p className="text-[10px] text-gray-500">Edit tool results to reduce token usage</p>
            </div>
          </label>
        </div>
      </div>

      {/* System Instructions */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          System Instructions
        </h3>
        <Button
          variant="outline"
          size="sm"
          onClick={handleOpenSystemInstructionsDialog}
          className="w-full justify-start border-white/20 text-white hover:bg-white/5 h-9"
        >
          <SettingsIcon />
          Configure System Instructions
          {localConfig.systemInstructions && (
            <span className="ml-auto text-xs text-gray-400">Configured</span>
          )}
        </Button>
        <p className="text-[10px] text-gray-500 mt-2">
          Define the behavior and personality of this agent using Markdown
        </p>
      </div>

      {/* Info */}
      <div className="p-3 rounded-lg bg-violet-500/10 border border-violet-500/20">
        <p className="text-[10px] text-violet-300">
          The Orchestrator is the root agent that manages the entire flow. Configure its LLM, tools, MCPs, and skills here. Subagents defined in the flow will be passed as tools to this Orchestrator.
        </p>
      </div>

      {/* Configuration Dialogs */}
      <ToolsConfigDialog
        open={toolsDialogOpen}
        onClose={() => setToolsDialogOpen(false)}
        onSave={handleSaveTools}
        initialTools={tools}
      />

      <SkillsConfigDialog
        open={skillsDialogOpen}
        onClose={() => setSkillsDialogOpen(false)}
        onSave={handleSaveSkills}
        availableSkills={availableSkills}
        initialSkills={skills}
      />

      <MCPsConfigDialog
        open={mcpsDialogOpen}
        onClose={() => setMCPsDialogOpen(false)}
        onSave={handleSaveMCPs}
        availableMCPs={availableMCPs}
        initialMCPs={mcps}
      />

      <Suspense fallback={null}>
        <SystemInstructionsDialog
          open={systemInstructionsDialogOpen}
          onClose={() => setSystemInstructionsDialogOpen(false)}
          onSave={handleSaveSystemInstructions}
          initialInstructions={localConfig.systemInstructions || ""}
        />
      </Suspense>
    </div>
  );
});

OrchestratorProperties.displayName = "OrchestratorProperties";
