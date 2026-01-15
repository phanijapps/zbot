// ============================================================================
// CONVERSATIONS FEATURE
// Chat interface with agent integration and thinking panel
// ============================================================================

import { useState, useEffect, useRef } from "react";
import { MessageSquare, Bot, Loader2, ChevronRight, RefreshCw, Paperclip, Send, Check, X } from "lucide-react";
import { cn } from "@/shared/utils";
import { Textarea } from "@/shared/ui/textarea";
import {
  GroupedConversationList,
  useStreamEvents,
} from "@/domains/agent-runtime/components";
import type {
  ConversationWithAgent,
  MessageWithThinking,
  AgentOption,
} from "@/domains/agent-runtime/components";
import {
  getConversationWithAgents,
  getOrCreateAgentConversation,
  streamAgentResponse,
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
  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Agent selector state
  const [showAgentSelector, setShowAgentSelector] = useState(false);
  const [pendingAgentId, setPendingAgentId] = useState<string | null>(null);

  // State for showing historical tool calls from messages
  const [historicalToolCalls, setHistoricalToolCalls] = useState<any[] | null>(null);

  // Stream events handling
  const {
    state: thinkingState,
    handleEvent,
    reset: resetThinking,
    openPanel,
  } = useStreamEvents(true, false);

  // Combine current state with historical tool calls
  const displayThinkingState = historicalToolCalls !== null
    ? {
        ...thinkingState,
        toolCalls: historicalToolCalls,
        isOpen: true,
      }
    : thinkingState;

  // Show historical tool calls when clicking on a message
  const handleShowHistoricalThinking = (toolCalls: any[]) => {
    setHistoricalToolCalls(toolCalls);
    openPanel();
  };

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
      // Reset thinking state when switching conversations
      resetThinking();
      setHistoricalToolCalls(null);
    } else {
      setMessages([]);
      setCurrentAgentId(null);
    }
  }, [selectedConversation]);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

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
        msgs.map((msg: any) => ({
          id: msg.id,
          conversationId,
          role: msg.role,
          content: msg.content,
          // Convert created_at (ISO string) to timestamp (number)
          timestamp: msg.created_at ? new Date(msg.created_at).getTime() : Date.now(),
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
        // Use the already-loaded agents state
        if (agents.length === 0) {
          setError("No agents available. Please create an agent first.");
          setTimeout(() => {
            window.location.href = "#/agents";
          }, 1500);
          return;
        } else if (agents.length === 1) {
          targetAgentId = agents[0].id;
        } else {
          // Multiple agents - show selector dialog
          setShowAgentSelector(true);
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
   * Handle agent selection from the modal
   */
  const handleAgentSelected = async () => {
    setShowAgentSelector(false);
    if (pendingAgentId) {
      await handleNewChat(pendingAgentId);
      setPendingAgentId(null);
    }
  };

  /**
   * Handle sending a message
   */
  const handleSendMessage = async (content: string) => {
    if (!selectedConversation || !currentAgentId) {
      throw new Error("No conversation selected");
    }

    // Add user message immediately (optimistic UI)
    const userMessage: MessageWithThinking = {
      id: crypto.randomUUID(),
      conversationId: selectedConversation.id,
      role: "user",
      content,
      timestamp: Date.now(),
    };
    setMessages((prev) => [...prev, userMessage]);
    setIsLoading(true);

    // Track the assistant response being built
    let assistantResponse = "";
    let assistantMessageId = crypto.randomUUID();
    let collectedToolCalls: any[] = [];

    // Collect streaming events to build assistant message
    const eventHandler = (event: any) => {
      console.log("[ConversationsPanel] Event received:", event.type, event);

      // Track tool calls
      if (event.type === "tool_call_start") {
        collectedToolCalls.push({
          id: event.toolId,
          name: event.toolName,
          status: "running" as const,
        });
        console.log("[ConversationsPanel] Tool call started:", event.toolName, "Total:", collectedToolCalls.length);

        // Create assistant message immediately if it doesn't exist yet
        setMessages((prev) => {
          const exists = prev.some((m) => m.id === assistantMessageId);
          if (!exists) {
            return [
              ...prev,
              {
                id: assistantMessageId,
                conversationId: selectedConversation!.id,
                role: "assistant",
                content: "",
                timestamp: Date.now(),
                thinking: {
                  toolCalls: [...collectedToolCalls],
                  toolCount: collectedToolCalls.length,
                },
              },
            ];
          } else {
            // Update existing message with new tool calls
            return prev.map((m) =>
              m.id === assistantMessageId
                ? {
                    ...m,
                    thinking: {
                      toolCalls: [...collectedToolCalls],
                      toolCount: collectedToolCalls.length,
                    }
                  }
                : m
            );
          }
        });
      } else if (event.type === "tool_result") {
        const toolCall = collectedToolCalls.find(t => t.id === event.toolId);
        if (toolCall) {
          toolCall.status = event.error ? "failed" as const : "completed" as const;
          toolCall.result = event.result;
          toolCall.error = event.error;
        }
        console.log("[ConversationsPanel] Tool result:", event.toolId);
        // Update message with tool result
        setMessages((prev) => {
          const exists = prev.some((m) => m.id === assistantMessageId);
          if (exists) {
            return prev.map((m) =>
              m.id === assistantMessageId
                ? {
                    ...m,
                    thinking: {
                      toolCalls: [...collectedToolCalls],
                      toolCount: collectedToolCalls.length,
                    }
                  }
                : m
            );
          }
          return prev;
        });
      } else if (event.type === "done") {
        console.log("[ConversationsPanel] Stream done, final tool count:", collectedToolCalls.length);
        // Final update to ensure thinking data is saved
        setMessages((prev) => {
          const exists = prev.some((m) => m.id === assistantMessageId);
          if (exists) {
            return prev.map((m) =>
              m.id === assistantMessageId
                ? {
                    ...m,
                    thinking: {
                      toolCalls: [...collectedToolCalls],
                      toolCount: collectedToolCalls.length,
                    }
                  }
                : m
            );
          }
          return prev;
        });
      }

      if (event.type === "token") {
        assistantResponse += event.content;
        // Update or add assistant message
        setMessages((prev) => {
          const exists = prev.some((m) => m.id === assistantMessageId);
          if (exists) {
            return prev.map((m) =>
              m.id === assistantMessageId
                ? {
                    ...m,
                    content: assistantResponse,
                    thinking: {
                      toolCalls: [...collectedToolCalls],
                      toolCount: collectedToolCalls.length,
                    }
                  }
                : m
            );
          } else {
            return [
              ...prev,
              {
                id: assistantMessageId,
                conversationId: selectedConversation!.id,
                role: "assistant",
                content: assistantResponse,
                timestamp: Date.now(),
                thinking: {
                  toolCalls: [...collectedToolCalls],
                  toolCount: collectedToolCalls.length,
                },
              },
            ];
          }
        });
      }
      // Also forward to the original handler for thinking panel
      handleEvent(event);
    };

    try {
      // Backend handles saving the user message, so we don't need to save it here
      resetThinking();

      // Stream agent response (builds assistant message in state via eventHandler)
      await streamAgentResponse(
        selectedConversation.id,
        currentAgentId,
        content,
        eventHandler
      );

      // Reload conversations to update last message info
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
   * Handle deleting all conversations for an agent
   */
  const handleDeleteAgentConversations = async (agentId: string) => {
    try {
      // Get all conversations for this agent
      const agentConversations = conversations.filter(c => c.agentId === agentId);

      // Delete each conversation
      await Promise.all(agentConversations.map(conv => deleteConversation(conv.id)));

      // Clear selected conversation if it was deleted
      if (selectedConversation && agentConversations.some(c => c.id === selectedConversation.id)) {
        setSelectedConversation(null);
        setMessages([]);
      }

      // Reload conversations
      await loadConversations();
    } catch (error) {
      console.error("Failed to delete agent conversations:", error);
      setError(`Failed to delete conversations: ${error instanceof Error ? error.message : String(error)}`);
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

      {/* Left: GroupedConversationList - ALWAYS visible with tree view and + New button */}
      <GroupedConversationList
        conversations={conversations}
        selectedId={selectedConversation?.id}
        onSelect={handleSelectConversation}
        onNewChat={handleNewChat}
        onDelete={handleDeleteConversation}
        onDeleteAgent={handleDeleteAgentConversations}
        agents={agents}
        className="w-80 border-r border-white/10 bg-black/20 shrink-0"
      />

      {/* Right: Chat Area or Empty State */}
      <div className="flex-1 flex min-w-0 bg-black/40">
        {selectedConversation ? (
          /* When conversation IS selected - show chat with embedded thought panel */
          <div className="flex-1 flex flex-col min-w-0">
            {/* Agent Info Header */}
            <div className="h-14 px-6 border-b border-white/10 bg-white/5 flex items-center shrink-0">
              <div className="flex items-center gap-3">
                <span className="text-xl" role="img">
                  {selectedConversation.agentIcon || "🤖"}
                </span>
                <div>
                  <div className="text-sm font-medium text-white">
                    {selectedConversation.title}
                  </div>
                  <div className="text-xs text-gray-500">
                    {selectedConversation.agentName} • {selectedConversation.model || "AI Agent"}
                  </div>
                </div>
              </div>
            </div>

            {/* Messages + Thought Panel Container */}
            <div className="flex-1 flex min-h-0">
              {/* Messages Area */}
              <div className="flex-1 overflow-y-auto px-6 py-4">
                {messages.length === 0 ? (
                  <EmptyChatState agentName={selectedConversation.agentName} />
                ) : (
                  <div className="max-w-3xl mx-auto space-y-6">
                    {messages.map((message) => (
                      <MessageBubble
                        key={message.id}
                        message={message}
                        onShowThinking={() => message.thinking?.toolCalls && handleShowHistoricalThinking(message.thinking.toolCalls)}
                      />
                    ))}
                    {isLoading && <TypingIndicator />}
                    <div ref={messagesEndRef} />
                  </div>
                )}
              </div>

              {/* Thought Panel - next to messages area */}
              {displayThinkingState.isOpen && (
                <div className="w-80 border-l border-white/10 flex flex-col bg-black/30 overflow-y-auto">
                  {/* Tool Calls */}
                  {displayThinkingState.toolCalls.length > 0 && (
                    <div className="p-4 space-y-3">
                      <div className="flex items-center gap-2 text-sm font-medium text-gray-300">
                        <span>🔧</span>
                        <span>Calling Tools</span>
                      </div>
                      {displayThinkingState.toolCalls.map((tool: any) => (
                        <div
                          key={tool.id}
                          className={cn(
                            "flex items-center gap-2.5 py-2 px-3 rounded-md",
                            tool.status === "completed" && "bg-green-500/5",
                            tool.status === "failed" && "bg-red-500/5",
                            tool.status === "running" && "bg-purple-500/5"
                          )}
                        >
                          {tool.status === "completed" && <Check className="size-4 text-green-500 shrink-0" />}
                          {tool.status === "running" && <Loader2 className="size-4 text-purple-500 shrink-0 animate-spin" />}
                          {tool.status === "failed" && <X className="size-4 text-red-500 shrink-0" />}
                          <span className="text-sm font-mono text-gray-400">{tool.name}</span>
                        </div>
                      ))}
                    </div>
                  )}

                  {/* Empty State */}
                  {displayThinkingState.toolCalls.length === 0 && displayThinkingState.isActive && (
                    <div className="p-4 text-center">
                      <div className="flex justify-center mb-3">
                        <div className="size-8 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
                      </div>
                      <p className="text-sm text-gray-500">Agent is working...</p>
                    </div>
                  )}

                  {/* Status Footer */}
                  <div className="mt-auto px-4 py-3 border-t border-white/10">
                    <div className="text-xs text-gray-500 text-center">
                      {displayThinkingState.toolCalls.length > 0
                        ? `${displayThinkingState.toolCalls.length} tool${displayThinkingState.toolCalls.length !== 1 ? "s" : ""} used`
                        : displayThinkingState.isActive
                        ? "Agent is working..."
                        : "Ready"}
                    </div>
                  </div>
                </div>
              )}
            </div>

            {/* Input Area */}
            <div className="border-t border-white/10 p-4">
              <div className="max-w-3xl mx-auto">
                <div className="relative bg-white/5 rounded-2xl border border-white/10 focus-within:border-purple-500/50 transition-colors">
                  <Textarea
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !e.shiftKey) {
                        e.preventDefault();
                        if (input.trim()) {
                          const msg = input;
                          setInput("");
                          handleSendMessage(msg);
                        }
                      }
                    }}
                    placeholder="Type your message... (Press Enter to send, Shift+Enter for new line)"
                    disabled={isLoading}
                    className="min-h-[60px] max-h-[200px] bg-transparent border-0 text-white placeholder:text-gray-500 resize-none pr-24 focus-visible:ring-0"
                  />
                  <div className="absolute bottom-3 right-3 flex items-center gap-2">
                    <button
                      disabled
                      className="text-gray-400 hover:text-white h-8 w-8 p-0 flex items-center justify-center rounded hover:bg-white/5 transition-colors"
                    >
                      <Paperclip className="size-4" />
                    </button>
                    <button
                      onClick={() => {
                        if (input.trim()) {
                          const msg = input;
                          setInput("");
                          handleSendMessage(msg);
                        }
                      }}
                      disabled={!input.trim() || isLoading}
                      className="bg-gradient-to-br from-purple-600 to-pink-600 hover:from-purple-700 hover:to-pink-700 text-white h-8 px-3 rounded-lg flex items-center gap-2 transition-colors disabled:opacity-50"
                    >
                      {isLoading ? (
                        <RefreshCw className="size-4 animate-spin" />
                      ) : (
                        <Send className="size-4" />
                      )}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        ) : (
          /* Empty state when no conversation selected */
          loading ? (
            <div className="flex-1 flex items-center justify-center">
              <Loader2 className="size-8 text-white animate-spin" />
            </div>
          ) : conversations.length === 0 ? (
            <div className="flex-1">
              <EmptyState onNewChat={handleNewChat} />
            </div>
          ) : (
            <div className="flex-1">
              <PlaceholderState />
            </div>
          )
        )}
      </div>

      {/* Agent Selector Modal */}
      {showAgentSelector && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
          <div className="bg-zinc-900 border border-white/10 rounded-xl p-6 w-full max-w-md mx-4 shadow-2xl">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-white">Select an Agent</h3>
              <button
                onClick={() => {
                  setShowAgentSelector(false);
                  setPendingAgentId(null);
                }}
                className="text-gray-400 hover:text-white transition-colors"
              >
                <X className="size-5" />
              </button>
            </div>

            <div className="space-y-2 mb-6">
              {agents.map((agent) => (
                <button
                  key={agent.id}
                  onClick={() => setPendingAgentId(agent.id)}
                  className={cn(
                    "w-full flex items-center gap-3 px-4 py-3 rounded-lg text-left transition-all",
                    "border",
                    pendingAgentId === agent.id
                      ? "bg-purple-500/10 border-purple-500/30 text-white"
                      : "bg-white/5 border-transparent hover:bg-white/10 hover:border-white/10 text-gray-300"
                  )}
                >
                  <div className="w-10 h-10 rounded-lg bg-purple-500/20 flex items-center justify-center">
                    <span className="text-xl">{getAgentIcon(agent.name)}</span>
                  </div>
                  <div>
                    <div className="font-medium">{agent.displayName}</div>
                    <div className="text-xs text-gray-500">{agent.name}</div>
                  </div>
                  {pendingAgentId === agent.id && (
                    <div className="ml-auto">
                      <div className="w-2 h-2 rounded-full bg-purple-500" />
                    </div>
                  )}
                </button>
              ))}
            </div>

            <div className="flex gap-3">
              <button
                onClick={() => {
                  setShowAgentSelector(false);
                  setPendingAgentId(null);
                }}
                className="flex-1 px-4 py-2 text-sm font-medium text-gray-300 bg-white/5 hover:bg-white/10 rounded-lg transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleAgentSelected}
                disabled={!pendingAgentId}
                className={cn(
                  "flex-1 px-4 py-2 text-sm font-medium rounded-lg transition-all",
                  pendingAgentId
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

// ============================================================================
// MESSAGE COMPONENTS
// ============================================================================

interface MessageBubbleProps {
  message: MessageWithThinking;
  onShowThinking: () => void;
}

function MessageBubble({ message, onShowThinking }: MessageBubbleProps) {
  const isUser = message.role === "user";

  return (
    <div className={cn("flex gap-3", isUser ? "flex-row-reverse" : "")}>
      {/* Avatar */}
      <div
        className={cn(
          "size-8 rounded-lg shrink-0 flex items-center justify-center text-sm font-medium",
          isUser
            ? "bg-gradient-to-br from-blue-500 to-purple-600 text-white"
            : "bg-gradient-to-br from-orange-500 to-pink-600 text-white"
        )}
      >
        {isUser ? "U" : message.role === "system" ? "S" : "🤖"}
      </div>

      {/* Content */}
      <div className={cn("flex-1 max-w-2xl", isUser ? "text-right" : "")}>
        {/* Message Content */}
        <div
          className={cn(
            "inline-block rounded-2xl px-4 py-3 text-left",
            isUser
              ? "bg-blue-600 text-white"
              : "bg-white/5 text-gray-100"
          )}
        >
          {isUser ? (
            <p className="text-sm leading-relaxed whitespace-pre-wrap">
              {message.content}
            </p>
          ) : (
            <div className="prose prose-invert prose-sm max-w-none">
              <div className="text-sm leading-relaxed whitespace-pre-wrap">
                {message.content}
              </div>
            </div>
          )}
        </div>

        {/* Thinking Indicator (for assistant messages) */}
        {!isUser && message.thinking && message.thinking.toolCount > 0 && (
          <button
            onClick={onShowThinking}
            className="mt-2 text-xs text-gray-500 hover:text-purple-400 transition-colors flex items-center gap-1.5"
          >
            <span>🧠</span>
            <span>Used {message.thinking.toolCount} tools</span>
          </button>
        )}

        {/* Timestamp */}
        <p className="text-xs text-gray-600 mt-1 px-1">
          {new Date(message.timestamp).toLocaleTimeString()}
        </p>
      </div>
    </div>
  );
}

function TypingIndicator() {
  return (
    <div className="flex items-center gap-3">
      <div className="size-8 rounded-lg bg-gradient-to-br from-orange-500 to-pink-600 flex items-center justify-center">
        <span className="text-sm">🤖</span>
      </div>
      <div className="bg-white/5 rounded-2xl px-4 py-3">
        <div className="flex gap-1">
          <span className="size-2 bg-gray-500 rounded-full animate-bounce [animation-delay:-0.3s]" />
          <span className="size-2 bg-gray-500 rounded-full animate-bounce [animation-delay:-0.15s]" />
          <span className="size-2 bg-gray-500 rounded-full animate-bounce" />
        </div>
      </div>
    </div>
  );
}

function EmptyChatState({ agentName }: { agentName?: string }) {
  return (
    <div className="flex flex-col items-center justify-center h-full text-center px-8">
      <div className="text-5xl mb-4">💬</div>
      <h3 className="text-lg font-medium text-white mb-2">
        Start a conversation with {agentName || "the agent"}
      </h3>
      <p className="text-sm text-gray-500 max-w-md">
        Send a message to begin. The agent will use its tools to help you with
        your task.
      </p>
    </div>
  );
}
