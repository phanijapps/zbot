/**
 * AgentChannelList - Agent Channel sidebar for Discord-like agent interface
 *
 * Displays:
 * - Agent channels (agents as primary interface)
 * - Today's message count per agent
 * - History indicator (previous days available)
 * - Last activity timestamp
 *
 * Key differences from ConversationList:
 * - No conversation list (agents are the face)
 * - Auto-selects today's session when agent is clicked
 * - Previous days expandable via separate component
 */

import type { Agent, AgentChannel as AgentChannelType } from "@/shared/types";
import {
  MessageSquare,
  Clock,
  History,
  Bot,
} from "lucide-react";
import { cn } from "@/shared/utils";
import { getAgentIcon, AgentIcon } from "./ConversationList";

interface AgentChannelListProps {
  agents: Agent[];
  selectedAgentId?: string;
  agentChannels?: AgentChannelType[];
  onSelectAgent: (agent: Agent) => void;
  onShowHistory?: (agentId: string) => void;
  className?: string;
}

export function AgentChannelList({
  agents,
  selectedAgentId,
  agentChannels = [],
  onSelectAgent,
  onShowHistory,
  className,
}: AgentChannelListProps) {
  // Create a map of agentId -> channel data for quick lookup
  const channelMap = new Map(agentChannels.map((ch) => [ch.agentId, ch]));

  return (
    <div className={cn("flex flex-col h-full", className)}>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3">
        <h2 className="text-lg font-semibold text-white">Agent Channels</h2>
        <div className="text-xs text-gray-500">
          {agents.length} {agents.length === 1 ? "agent" : "agents"}
        </div>
      </div>

      {/* Agent Channels List */}
      <div className="flex-1 overflow-y-auto">
        {agents.length === 0 ? (
          <EmptyState />
        ) : (
          <div className="p-3 space-y-1">
            {agents.map((agent) => {
              const channelData = channelMap.get(agent.id);
              const isSelected = agent.id === selectedAgentId;

              return (
                <AgentChannelCard
                  key={agent.id}
                  agent={agent}
                  channelData={channelData}
                  isSelected={isSelected}
                  onClick={() => onSelectAgent(agent)}
                  onShowHistory={
                    channelData?.hasHistory && onShowHistory
                      ? () => onShowHistory(agent.id)
                      : undefined
                  }
                />
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * Individual agent channel card
 */
interface AgentChannelCardProps {
  agent: Agent;
  channelData?: AgentChannelType;
  isSelected: boolean;
  onClick: () => void;
  onShowHistory?: () => void;
}

function AgentChannelCard({
  agent,
  channelData,
  isSelected,
  onClick,
  onShowHistory,
}: AgentChannelCardProps) {
  const iconName = getAgentIcon(agent.name);
  const todayCount = channelData?.todayMessageCount ?? 0;
  const hasHistory = channelData?.hasHistory ?? false;
  const lastActivityText = channelData?.lastActivityText ?? "No activity";

  return (
    <div
      className={cn(
        "group relative rounded-lg transition-all duration-200",
        "border border-transparent",
        isSelected
          ? "bg-purple-500/10 border-purple-500/30"
          : "hover:bg-white/5 hover:border-white/10"
      )}
    >
      {/* Main Agent Button */}
      <button
        onClick={onClick}
        className="w-full text-left p-3"
      >
        {/* Agent Header */}
        <div className="flex items-center gap-3">
          {/* Agent Icon */}
          <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-purple-500/20 to-blue-500/20 flex items-center justify-center shrink-0 border border-white/10">
            <AgentIcon iconName={iconName} className="size-5 text-purple-400" />
          </div>

          {/* Agent Info */}
          <div className="flex-1 min-w-0">
            <div className="font-medium text-white text-sm truncate">
              {agent.displayName}
            </div>
            <div className="flex items-center gap-2 text-xs text-gray-500">
              <span>{agent.name}</span>
              {todayCount > 0 && (
                <>
                  <span>•</span>
                  <span className="flex items-center gap-1">
                    <MessageSquare className="size-3" />
                    {todayCount} {todayCount === 1 ? "message" : "messages"}
                  </span>
                </>
              )}
            </div>
          </div>

          {/* History Indicator */}
          {hasHistory && onShowHistory && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onShowHistory();
              }}
              className="p-2 rounded-lg hover:bg-white/10 transition-colors text-gray-500 hover:text-purple-400"
              title="View previous days"
            >
              <History className="size-4" />
            </button>
          )}
        </div>

        {/* Last Activity (only show if not selected) */}
        {!isSelected && todayCount > 0 && (
          <div className="flex items-center gap-1 text-xs text-gray-600 mt-2">
            <Clock className="size-3" />
            <span>{lastActivityText}</span>
          </div>
        )}
      </button>
    </div>
  );
}

/**
 * Empty state when no agents exist
 */
function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center h-full px-6 text-center">
      <div className="w-14 h-14 rounded-xl bg-gradient-to-br from-purple-500/20 to-blue-500/20 flex items-center justify-center mb-4 border border-white/10">
        <Bot className="size-7 text-purple-400" />
      </div>
      <h3 className="text-base font-semibold text-white mb-2">No agents yet</h3>
      <p className="text-sm text-gray-500 max-w-xs">
        Create an agent to start chatting. Each agent has its own daily channel.
      </p>
    </div>
  );
}

/**
 * Compact version for narrower sidebar
 */
export function CompactAgentChannelList({
  agents,
  selectedAgentId,
  agentChannels = [],
  onSelectAgent,
  className,
}: Omit<AgentChannelListProps, "onShowHistory">) {
  const channelMap = new Map(agentChannels.map((ch) => [ch.agentId, ch]));

  return (
    <div className={cn("flex flex-col", className)}>
      {agents.map((agent) => {
        const channelData = channelMap.get(agent.id);
        const isSelected = agent.id === selectedAgentId;
        const iconName = getAgentIcon(agent.name);

        return (
          <button
            key={agent.id}
            onClick={() => onSelectAgent(agent)}
            className={cn(
              "flex items-center gap-3 px-3 py-2 rounded-lg text-left transition-all",
              isSelected
                ? "bg-purple-500/10 text-white"
                : "text-gray-400 hover:bg-white/5 hover:text-white"
            )}
            title={agent.displayName}
          >
            <div className="w-8 h-8 rounded-lg bg-white/5 flex items-center justify-center shrink-0">
              <AgentIcon iconName={iconName} className="size-4" />
            </div>
            <span className="text-sm font-medium truncate">
              {agent.displayName}
            </span>
            {(channelData?.todayMessageCount ?? 0) > 0 && (
              <span className="ml-auto text-xs text-gray-500">
                {channelData?.todayMessageCount ?? 0}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
