/**
 * AgentChannelList - Discord-style agent channel sidebar
 *
 * Displays agents as Discord-style channels with hash icon
 */

import type { Agent, AgentChannel as AgentChannelType } from "@/shared/types";
import { Hash, ChevronDown, Plus, History, Bot } from "lucide-react";
import { cn } from "@/shared/utils";

interface AgentChannelListProps {
  agents: Agent[];
  selectedAgentId?: string;
  agentChannels?: AgentChannelType[];
  onSelectAgent: (agent: Agent) => void;
  onShowHistory?: (agentId: string) => void;
  onCreateAgent?: () => void;
  className?: string;
}

export function AgentChannelList({
  agents,
  selectedAgentId,
  agentChannels = [],
  onSelectAgent,
  onShowHistory,
  onCreateAgent,
  className,
}: AgentChannelListProps) {
  return (
    <div className={cn("w-60 bg-[#2b2d31] flex flex-col", className)}>
      {/* Header */}
      <div className="h-12 border-b border-black/20 flex items-center justify-between px-4 hover:bg-black/10 cursor-pointer group transition-colors">
        <h2 className="text-white font-semibold">Agent Channels</h2>
        <ChevronDown className="size-4 text-gray-400 group-hover:text-white transition-colors" />
      </div>

      {/* Channel Count & Actions */}
      <div className="px-2 py-3 flex items-center justify-between">
        <div className="flex items-center gap-2 px-2">
          <div className="text-xs text-gray-400 uppercase tracking-wide font-semibold">
            {agents.length} agent{agents.length !== 1 ? 's' : ''}
          </div>
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={onCreateAgent}
            className="p-1 text-gray-400 hover:text-white transition-colors"
            title="Create Agent"
          >
            <Plus className="size-4" />
          </button>
          <button
            className="p-1 text-gray-400 hover:text-white transition-colors"
            title="Show History"
          >
            <History className="size-4" />
          </button>
        </div>
      </div>

      {/* Agent List */}
      <div className="flex-1 overflow-y-auto px-2 space-y-0.5">
        {agents.length === 0 ? (
          <EmptyState />
        ) : (
          agents.map((agent) => (
            <button
              key={agent.id}
              onClick={() => onSelectAgent(agent)}
              className={cn(
                'w-full flex items-center gap-2 px-2 py-1.5 rounded hover:bg-[#404249] transition-colors text-left group',
                selectedAgentId === agent.id && 'bg-[#404249] text-white'
              )}
            >
              <Hash
                className={cn(
                  'size-5 shrink-0',
                  selectedAgentId === agent.id ? 'text-white' : 'text-gray-400 group-hover:text-gray-300'
                )}
              />
              <span
                className={cn(
                  'text-[15px] truncate',
                  selectedAgentId === agent.id
                    ? 'text-white font-medium'
                    : 'text-gray-400 group-hover:text-gray-300'
                )}
              >
                {agent.displayName}
              </span>
            </button>
          ))
        )}
      </div>
    </div>
  );
}

/**
 * Empty state when no agents exist
 */
function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center h-full px-6 text-center">
      <div className="w-14 h-14 rounded-xl bg-gradient-to-br from-violet-600/20 to-purple-700/20 flex items-center justify-center mb-4 border border-white/10">
        <Bot className="size-7 text-violet-400" />
      </div>
      <h3 className="text-base font-semibold text-white mb-2">No agents yet</h3>
      <p className="text-sm text-gray-400 max-w-xs">
        Create an agent to start chatting. Each agent has its own daily channel.
      </p>
    </div>
  );
}
