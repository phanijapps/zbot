/**
 * ConversationList - List of conversations grouped by agent
 *
 * Displays:
 * - Agent identity (name, icon)
 * - Conversation title
 * - Last message preview
 * - Timestamp
 * - Message count
 *
 * Uses the "Hybrid Card List" approach from our UX design.
 */

import { useState } from "react";
import {
  MessageSquare,
  Clock,
  ChevronRight,
  X,
  Bot,
  BookOpen,
  BarChart3,
  FolderOpen,
  PenTool,
  Trash2,
} from "lucide-react";
import { cn } from "@/shared/utils";
import type { ConversationWithAgent } from "./types";

// Simple Agent type for the selector
export interface AgentOption {
  id: string;
  name: string;
  displayName: string;
}

// Agent icon type - can be a Lucide icon name or emoji
export type AgentIcon = string;

interface ConversationListProps {
  conversations: ConversationWithAgent[];
  selectedId?: string;
  onSelect: (conversation: ConversationWithAgent) => void;
  onNewChat: (agentId?: string) => void;
  onDelete?: (conversationId: string) => void;
  className?: string;
  agents?: AgentOption[];
}

/**
 * Get agent icon name (Lucide icon or fallback emoji)
 */
function getAgentIcon(agentName: string, _agentIcon?: string): string {
  if (!agentName) return "Bot";

  const icons: Record<string, string> = {
    "story-time": "BookOpen",
    "time-agent": "Clock",
    "codex": "Bot",
    "analyst": "BarChart3",
    "fileops": "FolderOpen",
    "writer": "PenTool",
    "assistant": "MessageSquare",
  };

  return icons[agentName.toLowerCase()] || "Bot";
}

/**
 * Render agent icon component
 */
function AgentIcon({ iconName, className }: { iconName: string; className?: string }) {
  // Map of icon names to actual Lucide components
  const icons: Record<string, React.ComponentType<{ className?: string }>> = {
    Bot,
    BookOpen,
    Clock,
    BarChart3,
    FolderOpen,
    PenTool,
    MessageSquare,
  };

  // Fallback emoji icons for unknown types
  const emojiFallbacks: Record<string, string> = {
    "🤖": "Bot",
    "📊": "BarChart3",
    "🔧": "Wrench",
    "✍️": "PenTool",
    "💬": "MessageSquare",
  };

  const IconComponent = icons[iconName] || icons[emojiFallbacks[iconName]] || Bot;

  return <IconComponent className={className} />;
}

/**
 * Truncate text to max length
 */
function truncateText(text: string, maxLength: number): string {
  if (!text || text.length <= maxLength) return text || "No messages yet";
  return text.slice(0, maxLength) + "...";
}

/**
 * Format timestamp as relative time
 */
function formatTimestamp(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;

  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (seconds < 60) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  if (hours < 24) return `${hours}h ago`;
  if (days < 7) return `${days}d ago`;

  // For older dates, show actual date
  const date = new Date(timestamp);
  return date.toLocaleDateString();
}

export function ConversationList({
  conversations,
  selectedId,
  onSelect,
  onNewChat,
  className,
}: ConversationListProps) {
  return (
    <div className={cn("flex flex-col h-full", className)}>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
        <h2 className="text-lg font-semibold text-white">Conversations</h2>
        <button
          onClick={() => onNewChat()}
          className="px-3 py-1.5 text-sm font-medium text-white bg-purple-600 hover:bg-purple-700 rounded-lg transition-colors"
        >
          + New
        </button>
      </div>

      {/* Conversations List */}
      <div className="flex-1 overflow-y-auto">
        {conversations.length === 0 ? (
          <EmptyState onNewChat={onNewChat} />
        ) : (
          <div className="p-3 space-y-2">
            {conversations.map((conversation) => (
              <ConversationCard
                key={conversation.id}
                conversation={conversation}
                isSelected={conversation.id === selectedId}
                onClick={() => onSelect(conversation)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * Individual conversation card
 */
function ConversationCard({
  conversation,
  isSelected,
  onClick,
}: {
  conversation: ConversationWithAgent;
  isSelected: boolean;
  onClick: () => void;
}) {
  const icon = getAgentIcon(conversation.agentName, conversation.agentIcon);

  return (
    <button
      onClick={onClick}
      className={cn(
        "w-full text-left p-4 rounded-xl transition-all duration-200",
        "border border-transparent",
        "hover:bg-white/5 hover:border-white/10",
        isSelected
          ? "bg-purple-500/10 border-purple-500/30"
          : "bg-transparent"
      )}
    >
      {/* Agent Header */}
      <div className="flex items-start justify-between mb-2">
        <div className="flex items-center gap-2.5">
          <span className="text-2xl" role="img" aria-label={conversation.agentName}>
            {icon}
          </span>
          <div className="text-left">
            <div className="font-medium text-white text-sm">
              {conversation.agentName}
            </div>
            <div className="text-xs text-gray-500">
              {conversation.model || "AI Agent"}
            </div>
          </div>
        </div>
      </div>

      {/* Conversation Title */}
      <div className="text-sm font-medium text-gray-200 mb-1 truncate">
        {conversation.title}
      </div>

      {/* Last Message Preview */}
      <div className="text-xs text-gray-500 mb-2 line-clamp-2">
        {truncateText(conversation.lastMessage || "", 80)}
      </div>

      {/* Metadata */}
      <div className="flex items-center gap-3 text-xs text-gray-600">
        <div className="flex items-center gap-1">
          <MessageSquare className="size-3" />
          <span>{conversation.messageCount}</span>
        </div>
        <div className="flex items-center gap-1">
          <Clock className="size-3" />
          <span>{formatTimestamp(conversation.lastMessageTime)}</span>
        </div>
      </div>
    </button>
  );
}

/**
 * Empty state when no conversations exist
 */
function EmptyState({ onNewChat }: { onNewChat: (agentId?: string) => void }) {
  return (
    <div className="flex flex-col items-center justify-center h-full px-6 text-center">
      <div className="w-14 h-14 rounded-xl bg-gradient-to-br from-purple-500/20 to-blue-500/20 flex items-center justify-center mb-4 border border-white/10">
        <MessageSquare className="size-7 text-purple-400" />
      </div>
      <h3 className="text-base font-semibold text-white mb-2">
        No conversations yet
      </h3>
      <p className="text-sm text-gray-500 mb-4 max-w-xs">
        Start a conversation with an agent to see your chat history here.
      </p>
      <button
        onClick={() => onNewChat()}
        className="px-4 py-2 text-sm font-medium text-white bg-gradient-to-r from-purple-600 to-blue-600 hover:from-purple-700 hover:to-blue-700 rounded-lg transition-all shadow-lg shadow-purple-500/25"
      >
        Start a conversation
      </button>
    </div>
  );
}

/**
 * Group conversations by agent
 * Optional: If you want to show agent-grouped view instead of flat list
 */
export function GroupedConversationList({
  conversations,
  selectedId,
  onSelect,
  onNewChat,
  onDelete,
  className,
  agents = [],
}: ConversationListProps) {
  const [showAgentSelector, setShowAgentSelector] = useState(false);
  const [selectedAgent, setSelectedAgent] = useState<string | null>(null);

  // Group by agent
  const grouped = conversations.reduce((acc, conv) => {
    const key = conv.agentId;
    if (!acc[key]) {
      acc[key] = {
        agentId: conv.agentId,
        agentName: conv.agentName,
        agentIcon: conv.agentIcon,
        conversations: [],
      };
    }
    acc[key].conversations.push(conv);
    return acc;
  }, {} as Record<string, { agentId: string; agentName: string; agentIcon?: string; conversations: ConversationWithAgent[] }>);

  const groups = Object.values(grouped);

  const handleNewChatClick = () => {
    if (agents.length > 1) {
      setShowAgentSelector(true);
    } else if (agents.length === 1) {
      onNewChat(agents[0].id);
    } else {
      onNewChat();
    }
  };

  const handleAgentSelect = () => {
    setShowAgentSelector(false);
    if (selectedAgent) {
      onNewChat(selectedAgent);
      setSelectedAgent(null);
    }
  };

  return (
    <div className={cn("flex flex-col h-full", className)}>
      {/* Header - only show button when there are conversations */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
        <h2 className="text-lg font-semibold text-white">Conversations</h2>
        {groups.length > 0 && (
          <button
            onClick={handleNewChatClick}
            className="px-3 py-1.5 text-sm font-medium text-white bg-gradient-to-r from-purple-600 to-blue-600 hover:from-purple-700 hover:to-blue-700 rounded-lg transition-all shadow-lg shadow-purple-500/25"
          >
            + New Chat
          </button>
        )}
      </div>

      {/* Grouped List */}
      <div className="flex-1 overflow-y-auto px-3 py-4 space-y-4">
        {groups.map((group) => (
          <AgentGroup
            key={group.agentId}
            agentName={group.agentName}
            agentIcon={group.agentIcon}
            conversations={group.conversations}
            selectedId={selectedId}
            onSelect={onSelect}
            onDelete={onDelete}
          />
        ))}
      </div>

      {/* Agent Selector Modal */}
      {showAgentSelector && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
          <div className="bg-zinc-900 border border-white/10 rounded-xl p-6 w-full max-w-md mx-4 shadow-2xl">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-white">Select an Agent</h3>
              <button
                onClick={() => setShowAgentSelector(false)}
                className="text-gray-400 hover:text-white transition-colors"
              >
                <X className="size-5" />
              </button>
            </div>

            <div className="space-y-2 mb-6">
              {agents.map((agent) => (
                <button
                  key={agent.id}
                  onClick={() => setSelectedAgent(agent.id)}
                  className={cn(
                    "w-full flex items-center gap-3 px-4 py-3 rounded-lg text-left transition-all",
                    "border",
                    selectedAgent === agent.id
                      ? "bg-purple-500/10 border-purple-500/30 text-white"
                      : "bg-white/5 border-transparent hover:bg-white/10 hover:border-white/10 text-gray-300"
                  )}
                >
                  <div className="w-10 h-10 rounded-lg bg-purple-500/20 flex items-center justify-center">
                    <AgentIcon iconName={getAgentIcon(agent.name)} className="size-5 text-purple-400" />
                  </div>
                  <div>
                    <div className="font-medium">{agent.displayName}</div>
                    <div className="text-xs text-gray-500">{agent.name}</div>
                  </div>
                  {selectedAgent === agent.id && (
                    <div className="ml-auto">
                      <div className="w-2 h-2 rounded-full bg-purple-500" />
                    </div>
                  )}
                </button>
              ))}
            </div>

            <div className="flex gap-3">
              <button
                onClick={() => setShowAgentSelector(false)}
                className="flex-1 px-4 py-2 text-sm font-medium text-gray-300 bg-white/5 hover:bg-white/10 rounded-lg transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleAgentSelect}
                disabled={!selectedAgent}
                className={cn(
                  "flex-1 px-4 py-2 text-sm font-medium rounded-lg transition-all",
                  selectedAgent
                    ? "bg-gradient-to-r from-purple-600 to-blue-600 hover:from-purple-700 hover:to-blue-700 text-white shadow-lg shadow-purple-500/25"
                    : "bg-white/5 text-gray-500 cursor-not-allowed"
                )}
              >
                Start Chat
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function AgentGroup({
  agentName,
  agentIcon,
  conversations,
  selectedId,
  onSelect,
  onDelete,
}: {
  agentName: string;
  agentIcon?: string;
  conversations: ConversationWithAgent[];
  selectedId?: string;
  onSelect: (conversation: ConversationWithAgent) => void;
  onDelete?: (conversationId: string) => void;
}) {
  const [isExpanded, setIsExpanded] = useState(true);
  const iconName = getAgentIcon(agentName, agentIcon);

  return (
    <div className="space-y-2">
      {/* Agent Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-400 hover:text-white hover:bg-white/5 rounded-lg transition-all"
      >
        <ChevronRight
          className={cn(
            "size-4 transition-transform",
            isExpanded && "rotate-90"
          )}
        />
        <div className="w-6 h-6 rounded bg-white/5 flex items-center justify-center">
          <AgentIcon iconName={iconName} className="size-3.5 text-gray-400" />
        </div>
        <span className="font-medium">{agentName}</span>
        <span className="ml-auto text-xs text-gray-600 bg-white/5 px-2 py-0.5 rounded-full">
          {conversations.length}
        </span>
      </button>

      {/* Conversations under this agent */}
      {isExpanded && (
        <div className="ml-4 space-y-1">
          {conversations.map((conv) => (
            <div
              key={conv.id}
              className={cn(
                "group flex items-center gap-1 px-3 py-2.5 rounded-lg text-sm transition-all border",
                conv.id === selectedId
                  ? "bg-purple-500/10 border-purple-500/20 text-white"
                  : "text-gray-400 hover:bg-white/5 border-transparent hover:border-white/5"
              )}
            >
              <button
                onClick={() => onSelect(conv)}
                className="flex-1 text-left min-w-0"
              >
                <div className="truncate mb-1">{conv.title}</div>
                <div className="flex items-center gap-2 text-xs text-gray-600">
                  <span>{formatTimestamp(conv.lastMessageTime)}</span>
                  {conv.messageCount > 0 && (
                    <>
                      <span>•</span>
                      <span>{conv.messageCount} msg{conv.messageCount !== 1 ? "s" : ""}</span>
                    </>
                  )}
                </div>
              </button>
              {onDelete && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(conv.id);
                  }}
                  className={cn(
                    "opacity-0 group-hover:opacity-100 p-1.5 rounded transition-all",
                    "hover:bg-red-500/20 hover:text-red-400 text-gray-600"
                  )}
                  title="Delete conversation"
                >
                  <Trash2 className="size-3.5" />
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
