// ============================================================================
// CONVERSATIONS FEATURE
// Chat interface with agent integration and thinking panel
// ============================================================================

import { useState, useEffect } from "react";
import { MessageSquare, Bot, Loader2, ChevronRight } from "lucide-react";
import {
  GroupedConversationList,
  ConversationView,
  useStreamEvents,
} from "@/domains/agent-runtime/components";
import type {
  ConversationWithAgent,
  MessageWithThinking,
} from "@/domains/agent-runtime/components";
import type { AgentOption } from "@/domains/agent-runtime/components";
import {
  getConversationWithAgents,
  getOrCreateAgentConversation,
  streamAgentResponse,
  createMessage,
  listMessages,
  deleteConversation,
} from "@/services/conversation";
import { getAgent, listAgents } from "@/services/agent";

export function ConversationsPanel() {
  // UI State
  const [conversations, setConversations] = useState<ConversationWithAgent[]>([]);
  const [agents, setAgents] = useState<AgentOption[]>([]);
  const [selectedConversation, setSelectedConversation] = useState<ConversationWithAgent | null>(null);
  const [messages, setMessages] = useState<MessageWithThinking[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [currentAgentId, setCurrentAgentId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  // Stream events handling
  const {
    handleEvent,
    reset: resetThinking,
  } = useStreamEvents(true, true);

  // Load conversations and agents on mount
  useEffect(() => {
    loadConversations();
    loadAgents();
  }, []);

  // Load messages when conversation is selected
  useEffect(() => {
    if (selectedConversation) {
      loadMessages(selectedConversation.id);
      setCurrentAgentId(selectedConversation.agentId);
    } else {
      setMessages([]);
      setCurrentAgentId(null);
    }
  }, [selectedConversation]);

  /**
   * Load all agents
   */
  const loadAgents = async () => {
    try {
      const agentList = await listAgents();
      setAgents(
        agentList.map((a) => ({
          id: a.id,
          name: a.name,
          displayName: a.displayName,
        }))
      );
    } catch (error) {
      console.error("Failed to load agents:", error);
    }
  };

  /**
   * Load all conversations with agent details
   */
  const loadConversations = async () => {
    setLoading(true);
    try {
      const data = await getConversationWithAgents();
      setConversations(data);
    } catch (error) {
      console.error("Failed to load conversations:", error);
    } finally {
      setLoading(false);
    }
  };

  /**
   * Load messages for a conversation
   */
  const loadMessages = async (conversationId: string) => {
    try {
      const msgs = await listMessages(conversationId);
      setMessages(
        msgs.map((msg) => ({
          id: msg.id,
          conversationId,
          role: msg.role,
          content: msg.content,
          timestamp: msg.timestamp,
        }))
      );
    } catch (error) {
      console.error("Failed to load messages:", error);
    }
  };

  /**
   * Handle selecting a conversation
   */
  const handleSelectConversation = (conv: ConversationWithAgent) => {
    setSelectedConversation(conv);
  };

  /**
   * Handle creating a new chat
   */
  const handleNewChat = async (agentId?: string) => {
    setError(null);

    try {
      let targetAgentId = agentId;

      if (!targetAgentId) {
        const agents = await listAgents();
        if (agents.length > 0) {
          targetAgentId = agents[0].id;
        } else {
          setError("No agents available. Please create an agent first.");
          setTimeout(() => {
            window.location.href = "#/agents";
          }, 1500);
          return;
        }
      }

      if (!targetAgentId) {
        throw new Error("No agent available");
      }

      const conv = await getOrCreateAgentConversation(targetAgentId);
      const enrichedConv = await enrichConversation(conv);

      setSelectedConversation(enrichedConv);
      setMessages([]);
      resetThinking();

      // Reload conversations to update the list
      await loadConversations();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error("Failed to create conversation:", err);
      setError(errorMessage);
      setTimeout(() => setError(null), 5000);
    }
  };

  /**
   * Handle sending a message
   */
  const handleSendMessage = async (content: string) => {
    if (!selectedConversation || !currentAgentId) {
      throw new Error("No conversation selected");
    }

    const userMessage: MessageWithThinking = {
      id: crypto.randomUUID(),
      conversationId: selectedConversation.id,
      role: "user",
      content,
      timestamp: Date.now(),
    };
    setMessages((prev) => [...prev, userMessage]);
    setIsLoading(true);

    try {
      await createMessage(selectedConversation.id, "user", content);
      resetThinking();

      await streamAgentResponse(
        selectedConversation.id,
        currentAgentId,
        content,
        handleEvent
      );

      await loadMessages(selectedConversation.id);
      await loadConversations();
    } catch (error) {
      console.error("Failed to send message:", error);

      const errorMessage: MessageWithThinking = {
        id: crypto.randomUUID(),
        conversationId: selectedConversation.id,
        role: "assistant",
        content: `Error: ${error instanceof Error ? error.message : String(error)}`,
        timestamp: Date.now(),
      };
      setMessages((prev) => [...prev, errorMessage]);
    } finally {
      setIsLoading(false);
    }
  };

  /**
   * Handle deleting a conversation
   */
  const handleDeleteConversation = async (conversationId: string) => {
    try {
      await deleteConversation(conversationId);

      // Clear selected conversation if it was deleted
      if (selectedConversation?.id === conversationId) {
        setSelectedConversation(null);
        setMessages([]);
      }

      // Reload conversations
      await loadConversations();
    } catch (error) {
      console.error("Failed to delete conversation:", error);
      setError(`Failed to delete conversation: ${error instanceof Error ? error.message : String(error)}`);
      setTimeout(() => setError(null), 5000);
    }
  };

  /**
   * Enrich conversation with agent details
   */
  const enrichConversation = async (
    conv: Awaited<ReturnType<typeof getOrCreateAgentConversation>>
  ): Promise<ConversationWithAgent> => {
    try {
      const agent = await getAgent(conv.agentId);
      const messages = conv.messages || [];
      const icon = getAgentIcon(agent.name);

      return {
        id: conv.id,
        title: conv.title,
        agentId: conv.agentId,
        agentName: agent.displayName || agent.name || "Unknown",
        agentIcon: icon,
        lastMessage: undefined,
        lastMessageTime: conv.updatedAt,
        messageCount: messages.length,
        model: agent.model,
      };
    } catch {
      const messages = conv.messages || [];
      return {
        id: conv.id,
        title: conv.title,
        agentId: conv.agentId,
        agentName: "Unknown",
        agentIcon: "🤖",
        lastMessage: undefined,
        lastMessageTime: conv.updatedAt,
        messageCount: messages.length,
      };
    }
  };

  const getAgentIcon = (agentName?: string): string => {
    if (!agentName) return "🤖";
    const icons: Record<string, string> = {
      codex: "🤖",
      analyst: "📊",
      fileops: "🔧",
      writer: "✍️",
      assistant: "💬",
    };
    return icons[agentName.toLowerCase()] || "🤖";
  };

  return (
    <div className="flex h-full relative">
      {/* Error Notification */}
      {error && (
        <div className="absolute top-4 left-1/2 -translate-x-1/2 z-50 max-w-md">
          <div className="bg-red-500/90 text-white px-4 py-3 rounded-lg shadow-lg text-sm flex items-center gap-2">
            <span>⚠️</span>
            <span>{error}</span>
          </div>
        </div>
      )}

      {/* Left Sidebar - Conversation List */}
      <div className="w-80 border-r border-white/10 flex flex-col bg-black/20">
        <GroupedConversationList
          conversations={conversations}
          selectedId={selectedConversation?.id}
          onSelect={handleSelectConversation}
          onNewChat={handleNewChat}
          onDelete={handleDeleteConversation}
          agents={agents}
          className="h-full"
        />
      </div>

      {/* Right Panel - Chat View or Empty State */}
      <div className="flex-1 flex flex-col min-w-0 bg-black/40">
        {selectedConversation ? (
          <ConversationView
            conversation={selectedConversation}
            messages={messages}
            onSendMessage={handleSendMessage}
            onBack={() => setSelectedConversation(null)}
            onNewChat={handleNewChat}
            isLoading={isLoading}
          />
        ) : loading ? (
          <div className="flex items-center justify-center h-full">
            <Loader2 className="size-8 text-white animate-spin" />
          </div>
        ) : conversations.length === 0 ? (
          <EmptyState onNewChat={handleNewChat} />
        ) : (
          <PlaceholderState />
        )}
      </div>
    </div>
  );
}

/**
 * Empty state when no conversations exist
 */
function EmptyState({ onNewChat }: { onNewChat: (agentId?: string) => void }) {
  return (
    <div className="flex flex-col items-center justify-center h-full px-8 text-center">
      <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-purple-500/20 to-blue-500/20 flex items-center justify-center mb-6 border border-white/10">
        <MessageSquare className="size-10 text-purple-400" />
      </div>
      <h2 className="text-2xl font-bold text-white mb-2">No conversations yet</h2>
      <p className="text-gray-400 mb-8 max-w-md">
        Start chatting with an AI agent to begin. Your conversations will be grouped by agent here.
      </p>
      <button
        onClick={() => onNewChat()}
        className="px-6 py-3 bg-gradient-to-r from-purple-600 to-blue-600 hover:from-purple-700 hover:to-blue-700 text-white font-medium rounded-xl transition-all shadow-lg shadow-purple-500/25"
      >
        Start a conversation
      </button>
    </div>
  );
}

/**
 * Placeholder state when no conversation is selected
 */
function PlaceholderState() {
  return (
    <div className="flex flex-col items-center justify-center h-full px-8 text-center">
      <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-white/5 to-white/[0.02] flex items-center justify-center mb-6 border border-white/10">
        <Bot className="size-10 text-gray-600" />
      </div>
      <h2 className="text-2xl font-bold text-white mb-2">Select a conversation</h2>
      <p className="text-gray-400 max-w-md">
        Choose a conversation from the sidebar to view and continue your chat.
      </p>
      <div className="mt-8 flex items-center gap-2 text-sm text-gray-500">
        <ChevronRight className="size-4" />
        <span>Click on any conversation in the left sidebar</span>
      </div>
    </div>
  );
}
