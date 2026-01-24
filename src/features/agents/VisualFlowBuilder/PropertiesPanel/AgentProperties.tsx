// ============================================================================
// VISUAL FLOW BUILDER - AGENT PROPERTIES
// Properties panel for agent nodes with tools/MCPs/skills selectors
// ============================================================================

import { memo, useState, useEffect } from "react";
import type { BaseNode, AgentNodeData } from "../types";
import { Label } from "@/shared/ui/label";
import { Input } from "@/shared/ui/input";
import { BUILTIN_TOOLS, MCP_TEMPLATES, BUILTIN_SKILLS, TOOL_CATEGORIES, SKILL_CATEGORIES, getToolById, getMCPById, getSkillById } from "../constants/agentResources";
import { MiddlewareConfig } from "./MiddlewareConfig";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const BotIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M12 8V4H8" /><rect width="16" height="12" x="4" y="8" rx="2" /><path d="M2 14h2" /><path d="M20 14h2" /><path d="M15 13v2" /><path d="M9 13v2" />
  </svg>
);

const ChevronDownIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="m6 9 6 6 6-6" />
  </svg>
);

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface AgentPropertiesProps {
  node: BaseNode;
  onUpdate: (updates: Partial<BaseNode>) => void;
}

interface ExpandableSectionProps {
  title: string;
  icon: React.ReactNode;
  count: number;
  defaultExpanded?: boolean;
  children: React.ReactNode;
}

// -----------------------------------------------------------------------------
// Expandable Section Component
// -----------------------------------------------------------------------------

const ExpandableSection = memo(({ title, icon, count, defaultExpanded = true, children }: ExpandableSectionProps) => {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);

  return (
    <div className="border border-white/10 rounded-lg overflow-hidden">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between px-3 py-2 bg-white/5 hover:bg-white/10 transition-colors"
      >
        <div className="flex items-center gap-2">
          {icon}
          <span className="text-xs font-medium text-white">{title}</span>
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-violet-500/20 text-violet-400">
            {count}
          </span>
        </div>
        <span className={`transition-transform ${isExpanded ? "rotate-180" : ""}`}>
          <ChevronDownIcon />
        </span>
      </button>
      {isExpanded && (
        <div className="p-3 border-t border-white/10">
          {children}
        </div>
      )}
    </div>
  );
});

ExpandableSection.displayName = "ExpandableSection";

// -----------------------------------------------------------------------------
// Tool Selector Item
// -----------------------------------------------------------------------------

interface ToolSelectorItemProps {
  toolId: string;
  isSelected: boolean;
  onToggle: () => void;
}

const ToolSelectorItem = memo(({ toolId, isSelected, onToggle }: ToolSelectorItemProps) => {
  const tool = getToolById(toolId);
  if (!tool) return null;

  const category = TOOL_CATEGORIES[tool.category as keyof typeof TOOL_CATEGORIES];

  return (
    <label className={`flex items-center gap-2 p-2 rounded cursor-pointer transition-colors ${
      isSelected ? "bg-violet-500/10 border border-violet-500/30" : "bg-white/5 hover:bg-white/10 border border-transparent"
    }`}>
      <input
        type="checkbox"
        checked={isSelected}
        onChange={onToggle}
        className="rounded"
      />
      <span className={`text-lg ${category?.color || "text-gray-400"}`}>
        {category?.icon}
      </span>
      <div className="flex-1 min-w-0">
        <p className="text-xs text-white font-medium">{tool.name}</p>
        <p className="text-[10px] text-gray-500 truncate">{tool.description}</p>
      </div>
    </label>
  );
});

ToolSelectorItem.displayName = "ToolSelectorItem";

// -----------------------------------------------------------------------------
// MCP Selector Item
// -----------------------------------------------------------------------------

interface MCPSelectorItemProps {
  mcpId: string;
  isSelected: boolean;
  onToggle: () => void;
}

const MCPSelectorItem = memo(({ mcpId, isSelected, onToggle }: MCPSelectorItemProps) => {
  const mcp = getMCPById(mcpId);
  if (!mcp) return null;

  return (
    <label className={`flex items-center gap-2 p-2 rounded cursor-pointer transition-colors ${
      isSelected ? "bg-blue-500/10 border border-blue-500/30" : "bg-white/5 hover:bg-white/10 border border-transparent"
    }`}>
      <input
        type="checkbox"
        checked={isSelected}
        onChange={onToggle}
        className="rounded"
      />
      <span className="text-lg">🔌</span>
      <div className="flex-1 min-w-0">
        <p className="text-xs text-white font-medium">{mcp.name}</p>
        <p className="text-[10px] text-gray-500 truncate">{mcp.description}</p>
      </div>
    </label>
  );
});

MCPSelectorItem.displayName = "MCPSelectorItem";

// -----------------------------------------------------------------------------
// Skill Selector Item
// -----------------------------------------------------------------------------

interface SkillSelectorItemProps {
  skillId: string;
  isSelected: boolean;
  onToggle: () => void;
}

const SkillSelectorItem = memo(({ skillId, isSelected, onToggle }: SkillSelectorItemProps) => {
  const skill = getSkillById(skillId);
  if (!skill) return null;

  const category = SKILL_CATEGORIES[skill.category as keyof typeof SKILL_CATEGORIES];

  return (
    <label className={`flex items-center gap-2 p-2 rounded cursor-pointer transition-colors ${
      isSelected ? "bg-green-500/10 border border-green-500/30" : "bg-white/5 hover:bg-white/10 border border-transparent"
    }`}>
      <input
        type="checkbox"
        checked={isSelected}
        onChange={onToggle}
        className="rounded"
      />
      <span className={`text-lg ${category?.color || "text-gray-400"}`}>
        {category?.icon}
      </span>
      <div className="flex-1 min-w-0">
        <p className="text-xs text-white font-medium">{skill.name}</p>
        <p className="text-[10px] text-gray-500 truncate">{skill.description}</p>
      </div>
    </label>
  );
});

SkillSelectorItem.displayName = "SkillSelectorItem";

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export const AgentProperties = memo(({ node, onUpdate }: AgentPropertiesProps) => {
  const data = node.data as AgentNodeData;
  const [localData, setLocalData] = useState(data);

  // Get current selections
  const tools = (localData.tools || []) as string[];
  const mcps = (localData.mcps || []) as string[];
  const skills = (localData.skills || []) as string[];
  const middlewares = (localData.middleware || []) as string[];

  useEffect(() => {
    setLocalData(data);
  }, [node.data]);

  const handleChange = (field: keyof AgentNodeData, value: unknown) => {
    const newData = { ...localData, [field]: value };
    setLocalData(newData);
    onUpdate({ ...node, data: newData });
  };

  const handleToggleTool = (toolId: string) => {
    const newTools = tools.includes(toolId)
      ? tools.filter((t) => t !== toolId)
      : [...tools, toolId];
    handleChange("tools", newTools);
  };

  const handleToggleMCP = (mcpId: string) => {
    const newMCPs = mcps.includes(mcpId)
      ? mcps.filter((m) => m !== mcpId)
      : [...mcps, mcpId];
    handleChange("mcps", newMCPs);
  };

  const handleToggleSkill = (skillId: string) => {
    const newSkills = skills.includes(skillId)
      ? skills.filter((s) => s !== skillId)
      : [...skills, skillId];
    handleChange("skills", newSkills);
  };

  const handleAddMiddleware = (type: string) => {
    const newMiddlewares = [...middlewares, type];
    handleChange("middleware", newMiddlewares);
  };

  const handleRemoveMiddleware = (type: string) => {
    const newMiddlewares = middlewares.filter((m) => m !== type);
    handleChange("middleware", newMiddlewares);
  };

  const handleConfigureMiddleware = (_type: string, _config: Record<string, unknown>) => {
    // For now, we'll just trigger an update - config storage could be enhanced later
    handleChange("middleware", middlewares);
  };

  return (
    <div className="space-y-4">
      {/* Basic Settings */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Basic Settings
        </h3>
        <div className="space-y-3">
          <div>
            <Label className="text-white text-xs mb-1.5 block">Display Name</Label>
            <Input
              value={localData.displayName || ""}
              onChange={(e) => handleChange("displayName", e.target.value)}
              placeholder="My Agent"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Description</Label>
            <Input
              value={localData.description || ""}
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
            <Input
              value={localData.providerId || ""}
              onChange={(e) => handleChange("providerId", e.target.value)}
              placeholder="OpenAI"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Model</Label>
            <Input
              value={localData.model || ""}
              onChange={(e) => handleChange("model", e.target.value)}
              placeholder="gpt-4o"
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>

          <div>
            <div className="flex items-center justify-between mb-1.5">
              <Label className="text-white text-xs">Temperature</Label>
              <span className="text-[10px] text-gray-400">{localData.temperature?.toFixed(1) ?? "0.7"}</span>
            </div>
            <input
              type="range"
              min={0}
              max={2}
              step={0.1}
              value={localData.temperature ?? 0.7}
              onChange={(e) => handleChange("temperature", parseFloat(e.target.value))}
              className="w-full h-2 bg-white/10 rounded-lg appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-violet-500"
            />
          </div>

          <div>
            <Label className="text-white text-xs mb-1.5 block">Max Tokens</Label>
            <Input
              type="number"
              value={localData.maxTokens ?? 4096}
              onChange={(e) => handleChange("maxTokens", parseInt(e.target.value) || 4096)}
              className="bg-white/5 border-white/10 text-white text-sm h-8"
            />
          </div>
        </div>
      </div>

      {/* Tools Selector */}
      <ExpandableSection
        title="Tools"
        icon={<span className="text-sm">🔧</span>}
        count={tools.length}
      >
        <div className="space-y-2">
          {BUILTIN_TOOLS.map((tool) => (
            <ToolSelectorItem
              key={tool.id}
              toolId={tool.id}
              isSelected={tools.includes(tool.id)}
              onToggle={() => handleToggleTool(tool.id)}
            />
          ))}
        </div>
        {tools.length === 0 && (
          <p className="text-[10px] text-gray-500 italic">No tools selected</p>
        )}
      </ExpandableSection>

      {/* MCPs Selector */}
      <ExpandableSection
        title="MCPs"
        icon={<span className="text-sm">🔌</span>}
        count={mcps.length}
      >
        <div className="space-y-2">
          {MCP_TEMPLATES.map((mcp) => (
            <MCPSelectorItem
              key={mcp.id}
              mcpId={mcp.id}
              isSelected={mcps.includes(mcp.id)}
              onToggle={() => handleToggleMCP(mcp.id)}
            />
          ))}
        </div>
        {mcps.length === 0 && (
          <p className="text-[10px] text-gray-500 italic">No MCPs selected</p>
        )}
      </ExpandableSection>

      {/* Skills Selector */}
      <ExpandableSection
        title="Skills"
        icon={<span className="text-sm">📚</span>}
        count={skills.length}
      >
        <div className="space-y-2">
          {BUILTIN_SKILLS.map((skill) => (
            <SkillSelectorItem
              key={skill.id}
              skillId={skill.id}
              isSelected={skills.includes(skill.id)}
              onToggle={() => handleToggleSkill(skill.id)}
            />
          ))}
        </div>
        {skills.length === 0 && (
          <p className="text-[10px] text-gray-500 italic">No skills selected</p>
        )}
      </ExpandableSection>

      {/* Middleware Configuration */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
          Advanced
        </h3>
        <MiddlewareConfig
          middlewares={middlewares}
          onAdd={handleAddMiddleware}
          onRemove={handleRemoveMiddleware}
          onConfigure={handleConfigureMiddleware}
        />
      </div>

      {/* System Instructions */}
      <div>
        <Label className="text-white text-xs mb-1.5 block">System Instructions</Label>
        <textarea
          value={localData.systemInstructions || ""}
          onChange={(e) => handleChange("systemInstructions", e.target.value)}
          placeholder="You are a helpful assistant..."
          rows={4}
          className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-white text-sm placeholder:text-gray-600 resize-none focus:outline-none focus:ring-1 focus:ring-violet-500"
        />
      </div>
    </div>
  );
});

AgentProperties.displayName = "AgentProperties";
