/**
 * AgentChannelList - Discord-style agent channel sidebar
 *
 * Displays agents as Discord-style channels with hash icon
 */

import type { Agent } from "@/shared/types";
import { Hash, ChevronDown, ChevronRight, Bot, Plus } from "lucide-react";
import { cn } from "@/shared/utils";
import { memo } from "react";
import { VaultSwitcher } from "@/features/vaults/VaultSwitcher";

interface AgentChannelListProps {
  agents: Agent[];
  selectedAgentId?: string;
  onSelectAgent: (agent: Agent) => void;
  onToggleVault?: () => void;
  showVaultSwitcher?: boolean;
  onCreateAgent?: () => void;
  vaultName?: string;
  className?: string;
}

interface AgentChannelItemProps {
  agent: Agent;
  isSelected: boolean;
  onSelectAgent: (agent: Agent) => void;
}

const AgentChannelItem = memo(function AgentChannelItem({ agent, isSelected, onSelectAgent }: AgentChannelItemProps) {
  return (
    <button
      role="listitem"
      onClick={() => onSelectAgent(agent)}
      aria-label={`Select ${agent.displayName}`}
      aria-pressed={isSelected}
      className={cn(
        'w-full flex items-center gap-2 px-2 py-1.5 rounded hover:bg-[#404249] transition-colors text-left group',
        isSelected && 'bg-[#404249] text-white'
      )}
    >
      <Hash
        className={cn(
          'size-5 shrink-0',
          isSelected ? 'text-white' : 'text-gray-300 group-hover:text-gray-200'
        )}
        aria-hidden="true"
      />
      <span
        className={cn(
          'text-[15px] truncate',
          isSelected
            ? 'text-white font-medium'
            : 'text-gray-300 group-hover:text-gray-200'
        )}
      >
        {agent.displayName}
      </span>
    </button>
  );
});

export const AgentChannelList = memo(function AgentChannelList({
  agents,
  selectedAgentId,
  onSelectAgent,
  onToggleVault,
  showVaultSwitcher = false,
  onCreateAgent,
  vaultName,
  className,
}: AgentChannelListProps) {
  return (
    <div className={cn("w-60 bg-[#2b2d31] flex flex-col", className)} role="navigation" aria-label="Agent channels">
      {/* Header - Click chevron to toggle vault switcher */}
      <div className="h-12 border-b border-black/20 flex items-center justify-between px-4">
        <div className="flex-1 flex items-center justify-center gap-2">
          <h2 className="text-white font-bold text-base">{vaultName || 'Agent Channels'}</h2>
          {onCreateAgent && (
            <button
              onClick={onCreateAgent}
              className="p-1 hover:bg-black/10 rounded transition-colors text-purple-400 hover:text-purple-300"
              aria-label="Create new agent"
              title="Create new agent"
            >
              <Plus className="size-4" />
            </button>
          )}
        </div>
        <button
          onClick={onToggleVault}
          className="hover:bg-black/10 p-1 rounded transition-colors"
          aria-label="Toggle vault switcher"
          aria-expanded={showVaultSwitcher}
        >
          {showVaultSwitcher ? (
            <ChevronDown className="size-4 text-gray-300 hover:text-white transition-colors" aria-hidden="true" />
          ) : (
            <ChevronRight className="size-4 text-gray-300 hover:text-white transition-colors" aria-hidden="true" />
          )}
        </button>
      </div>

      {/* Vault Switcher - Shown between header and agent list when expanded */}
      {showVaultSwitcher && (
        <div className="px-2 py-3 border-b border-black/10">
          <VaultSwitcher className="bg-transparent border-0" />
        </div>
      )}

      {/* Channel Count */}
      <div className="px-2 py-3 flex items-center">
        <div className="flex items-center gap-2 px-2">
          <div className="text-xs text-gray-300 uppercase tracking-wide font-semibold" aria-live="polite">
            {agents.length} agent{agents.length !== 1 ? 's' : ''}
          </div>
        </div>
      </div>

      {/* Agent List */}
      <div className="flex-1 overflow-y-auto px-2 space-y-0.5" role="list" aria-label="Available agents">
        {agents.length === 0 ? (
          <EmptyState />
        ) : (
          agents.map((agent) => (
            <AgentChannelItem
              key={agent.id}
              agent={agent}
              isSelected={selectedAgentId === agent.id}
              onSelectAgent={onSelectAgent}
            />
          ))
        )}
      </div>
    </div>
  );
});

/**
 * Empty state when no agents exist
 */
const EmptyState = memo(function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center h-full px-6 text-center" role="status" aria-live="polite">
      <div className="w-14 h-14 rounded-xl bg-gradient-to-br from-violet-600/20 to-purple-700/20 flex items-center justify-center mb-4 border border-white/10" aria-hidden="true">
        <Bot className="size-7 text-violet-400" />
      </div>
      <h3 className="text-base font-semibold text-white mb-2">No agents yet</h3>
      <p className="text-sm text-gray-300 max-w-xs">
        Create an agent to start chatting. Each agent has its own daily channel.
      </p>
    </div>
  );
});
