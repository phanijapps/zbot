import React, { useState, useEffect } from 'react';
import { Plug, Sparkles, Wrench, Code } from 'lucide-react';
import { cn } from '@/core/utils/cn';
import { NodeConfigPopover } from './NodeConfigPopover';
import * as skillsService from '@/services/skills';
import * as mcpService from '@/services/mcp';
import type { Skill } from '@/shared/types';
import type { MCPServer } from '@/features/mcp/types';

// Built-in tools from agent-tools crate
const BUILTIN_TOOLS = [
  { id: 'read', name: 'Read', description: 'Read file contents', category: 'File' },
  { id: 'write', name: 'Write', description: 'Write/create files', category: 'File' },
  { id: 'edit', name: 'Edit', description: 'Edit existing files', category: 'File' },
  { id: 'grep', name: 'Grep', description: 'Search file contents', category: 'Search' },
  { id: 'glob', name: 'Glob', description: 'Find files by pattern', category: 'Search' },
  { id: 'python', name: 'Python', description: 'Execute Python code', category: 'Execution' },
  { id: 'shell', name: 'Shell', description: 'Execute shell commands (bash/zsh/PowerShell)', category: 'Execution' },
  { id: 'load_skill', name: 'Load Skill', description: 'Load and execute skills', category: 'Execution' },
  { id: 'request_input', name: 'Request Input', description: 'Ask user for input', category: 'UI' },
  { id: 'show_content', name: 'Show Content', description: 'Display content to user', category: 'UI' },
  { id: 'list_entities', name: 'List Entities', description: 'List knowledge graph entities', category: 'Knowledge' },
  { id: 'search_entities', name: 'Search Entities', description: 'Search knowledge graph', category: 'Knowledge' },
  { id: 'get_relationships', name: 'Get Relationships', description: 'Get entity relationships', category: 'Knowledge' },
  { id: 'add_entity', name: 'Add Entity', description: 'Add to knowledge graph', category: 'Knowledge' },
  { id: 'add_relationship', name: 'Add Relationship', description: 'Add relationship to graph', category: 'Knowledge' },
  { id: 'create_agent', name: 'Create Agent', description: 'Create new agents', category: 'Agent' },
];

interface NodeConfigChipsProps {
  mcps: string[];
  skills: string[];
  tools: string[];
  middleware?: string;
  onMcpsChange: (mcps: string[]) => void;
  onSkillsChange: (skills: string[]) => void;
  onToolsChange: (tools: string[]) => void;
  onMiddlewareChange: (middleware: string) => void;
}

type PopoverType = 'mcps' | 'skills' | 'tools' | 'middleware' | null;

export const NodeConfigChips: React.FC<NodeConfigChipsProps> = ({
  mcps,
  skills,
  tools,
  middleware,
  onMcpsChange,
  onSkillsChange,
  onToolsChange,
  onMiddlewareChange,
}) => {
  const [activePopover, setActivePopover] = useState<PopoverType>(null);
  const [anchorEl, setAnchorEl] = useState<HTMLElement | null>(null);

  // Data state
  const [allSkills, setAllSkills] = useState<Skill[]>([]);
  const [allMcps, setAllMcps] = useState<MCPServer[]>([]);
  const [loadingSkills, setLoadingSkills] = useState(false);
  const [loadingMcps, setLoadingMcps] = useState(false);

  // Load data when popover opens
  useEffect(() => {
    if (activePopover === 'skills' && allSkills.length === 0) {
      loadSkills();
    }
    if ((activePopover === 'mcps' || activePopover === 'tools') && allMcps.length === 0) {
      loadMcps();
    }
  }, [activePopover]);

  const loadSkills = async () => {
    setLoadingSkills(true);
    try {
      const loaded = await skillsService.listSkills();
      setAllSkills(loaded);
    } catch (error) {
      console.error('Failed to load skills:', error);
    } finally {
      setLoadingSkills(false);
    }
  };

  const loadMcps = async () => {
    setLoadingMcps(true);
    try {
      const loaded = await mcpService.listMCPServers();
      setAllMcps(loaded);
    } catch (error) {
      console.error('Failed to load MCPs:', error);
    } finally {
      setLoadingMcps(false);
    }
  };

  const handleChipClick = (type: PopoverType, e: React.MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    if (activePopover === type) {
      setActivePopover(null);
      setAnchorEl(null);
    } else {
      setActivePopover(type);
      setAnchorEl(e.currentTarget);
    }
  };

  const handleClose = () => {
    setActivePopover(null);
    setAnchorEl(null);
  };

  return (
    <div className="flex items-center gap-1.5 flex-wrap">
      {/* MCPs Chip */}
      <button
        onClick={(e) => handleChipClick('mcps', e)}
        className={cn(
          "flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors",
          "bg-gray-700/50 hover:bg-gray-700",
          mcps.length > 0 ? "text-blue-400" : "text-gray-500",
          activePopover === 'mcps' && "ring-1 ring-blue-500"
        )}
      >
        <Plug size={12} />
        <span>{mcps.length}</span>
      </button>

      {/* Skills Chip */}
      <button
        onClick={(e) => handleChipClick('skills', e)}
        className={cn(
          "flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors",
          "bg-gray-700/50 hover:bg-gray-700",
          skills.length > 0 ? "text-yellow-400" : "text-gray-500",
          activePopover === 'skills' && "ring-1 ring-yellow-500"
        )}
      >
        <Sparkles size={12} />
        <span>{skills.length}</span>
      </button>

      {/* Tools Chip - Built-in tools that can be enabled/disabled */}
      <button
        onClick={(e) => handleChipClick('tools', e)}
        className={cn(
          "flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors",
          "bg-gray-700/50 hover:bg-gray-700",
          tools.length > 0 ? "text-orange-400" : "text-gray-500",
          activePopover === 'tools' && "ring-1 ring-orange-500"
        )}
      >
        <Wrench size={12} />
        <span>{tools.length}</span>
      </button>

      {/* Middleware Chip */}
      <button
        onClick={(e) => handleChipClick('middleware', e)}
        className={cn(
          "flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors",
          "bg-gray-700/50 hover:bg-gray-700",
          middleware ? "text-green-400" : "text-gray-500",
          activePopover === 'middleware' && "ring-1 ring-green-500"
        )}
      >
        <Code size={12} />
      </button>

      {/* MCPs Popover */}
      <NodeConfigPopover
        isOpen={activePopover === 'mcps'}
        onClose={handleClose}
        anchorEl={anchorEl}
        title="MCP Servers"
        icon={<Plug size={14} />}
        iconColor="text-blue-400"
        items={allMcps.map(m => ({ id: m.id, name: m.name }))}
        selectedIds={mcps}
        onSelectionChange={onMcpsChange}
        loading={loadingMcps}
        emptyMessage="No MCP servers configured"
      />

      {/* Skills Popover */}
      <NodeConfigPopover
        isOpen={activePopover === 'skills'}
        onClose={handleClose}
        anchorEl={anchorEl}
        title="Skills"
        icon={<Sparkles size={14} />}
        iconColor="text-yellow-400"
        items={allSkills.map(s => ({ id: s.id, name: s.displayName || s.name, description: s.description }))}
        selectedIds={skills}
        onSelectionChange={onSkillsChange}
        loading={loadingSkills}
        emptyMessage="No skills available"
      />

      {/* Tools Popover - Built-in tools */}
      <NodeConfigPopover
        isOpen={activePopover === 'tools'}
        onClose={handleClose}
        anchorEl={anchorEl}
        title="Built-in Tools"
        icon={<Wrench size={14} />}
        iconColor="text-orange-400"
        items={BUILTIN_TOOLS.map(t => ({ id: t.id, name: t.name, description: t.description }))}
        selectedIds={tools}
        onSelectionChange={onToolsChange}
        loading={false}
        emptyMessage="No tools available"
      />

      {/* Middleware Popover */}
      <NodeConfigPopover
        isOpen={activePopover === 'middleware'}
        onClose={handleClose}
        anchorEl={anchorEl}
        title="Middleware"
        icon={<Code size={14} />}
        iconColor="text-green-400"
        items={[]}
        selectedIds={[]}
        onSelectionChange={() => {}}
        isTextInput={true}
        textValue={middleware || ''}
        onTextChange={onMiddlewareChange}
        textPlaceholder="middleware:&#10;  summarization:&#10;    enabled: true&#10;    maxTokens: 1000"
      />
    </div>
  );
};
