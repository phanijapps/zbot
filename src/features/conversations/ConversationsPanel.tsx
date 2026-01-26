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
  GenerativeCanvas,
  type ContentState,
  type AttachmentInfo,
} from "@/domains/agent-runtime/components";
import type {
  ConversationWithAgent,
  MessageWithThinking,
  AgentOption,
} from "@/domains/agent-runtime/components";
import type { ShowContentEvent, RequestInputEvent } from "@/shared/types/agent";
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
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Agent selector state
  const [showAgentSelector, setShowAgentSelector] = useState(false);
  const [pendingAgentId, setPendingAgentId] = useState<string | null>(null);

  // State for showing historical tool calls and reasoning from messages
  const [historicalToolCalls, setHistoricalToolCalls] = useState<any[] | null>(null);
  const [historicalReasoning, setHistoricalReasoning] = useState<string[] | null>(null);

  // Generative Canvas state
  const [canvasOpen, setCanvasOpen] = useState(false);
  const [canvasContent, setCanvasContent] = useState<ContentState>(null);

  // Stream events handling
  const {
    state: thinkingState,
    handleEvent,
    reset: resetThinking,
    openPanel,
  } = useStreamEvents(true, false);

  // Combine current state with historical tool calls and reasoning
  const displayThinkingState = historicalToolCalls !== null || historicalReasoning !== null
    ? {
        ...thinkingState,
        toolCalls: historicalToolCalls ?? [],
        reasoning: historicalReasoning ?? [],
        isOpen: true,
      }
    : thinkingState;

  // Show historical tool calls and reasoning when clicking on a message
  const handleShowHistoricalThinking = (toolCalls: any[], reasoning?: string[]) => {
    setHistoricalToolCalls(toolCalls);
    setHistoricalReasoning(reasoning || null);
    openPanel();
  };

  // Open attachment in canvas
  const handleOpenAttachment = (attachment: AttachmentInfo) => {
    console.log("[ConversationsPanel] Opening attachment:", attachment);
    // For output files, we could open directly in browser, but for now use canvas
    setCanvasContent({
      type: "show_content",
      event: {
        type: "show_content",
        contentType: attachment.contentType as any,
        title: attachment.filename,
        content: attachment.filename,  // Will be loaded via Tauri based on isAttachment
        timestamp: Date.now(),
        isAttachment: true,
        filePath: attachment.relativePath,
        base64: attachment.contentType === "image" || attachment.contentType === "pdf",
      }
    });
    setCanvasOpen(true);
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
   * Note: agent-creator is filtered out as it's only accessible via the + button
   */
  const loadAgents = async () => {
    try {
      const agentList = await listAgents();
      // Filter out agent-creator - it's only accessible via + button in agent channels
      setAgents(
        agentList
          .filter(agent => agent.id !== "agent-creator")
          .map((a) => ({
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
   * Reconstruct attachment info from tool_calls and tool_results
   */
  const reconstructAttachments = (toolCalls: any[], toolResults: any[]): AttachmentInfo[] => {
    const attachments: AttachmentInfo[] = [];

    console.log("[reconstructAttachments] toolCalls:", toolCalls);
    console.log("[reconstructAttachments] toolResults:", toolResults);

    toolResults.forEach((result: any) => {
      console.log("[reconstructAttachments] Processing result:", result);
      const toolCall = toolCalls.find((tc: any) => tc.id === result.tool_call_id);
      console.log("[reconstructAttachments] Found toolCall:", toolCall, "for tool_call_id:", result.tool_call_id);
      if (!toolCall || !result.output) {
        console.log("[reconstructAttachments] Skipping - no toolCall or output");
        return;
      }

      // Check if this is a successful write tool result
      if (toolCall.name === "write" && !result.error) {
        console.log("[reconstructAttachments] This is a write tool, parsing output:", result.output);
        try {
          const parsed = JSON.parse(result.output);
          console.log("[reconstructAttachments] Parsed result:", parsed);
          // WriteTool returns {path, bytes_written} - no success field needed
          if (parsed.path) {
            const fullPath = parsed.path;
            const filename = fullPath.split('/').pop() || fullPath.split('\\').pop() || "file";
            const isOutput = fullPath.includes("/outputs/") || fullPath.includes("\\outputs\\");

            // Detect content type from extension
            const ext = filename.split('.').pop()?.toLowerCase();
            let contentType = "text";
            if (ext === "html" || ext === "htm") contentType = "html";
            else if (ext === "pdf") contentType = "pdf";
            else if (ext === "md" || ext === "markdown") contentType = "markdown";
            else if (["png", "jpg", "jpeg", "gif", "svg", "webp"].includes(ext || "")) contentType = "image";

            // Build relative path
            let relativePath: string;
            if (isOutput) {
              relativePath = `outputs/${filename}`;
            } else {
              // Extract conv_id/attachments/filename from full path
              const parts = fullPath.split('/');
              const attachmentsIdx = parts.indexOf('attachments');
              if (attachmentsIdx > 0 && attachmentsIdx + 1 < parts.length) {
                const convId = parts[attachmentsIdx - 1];
                relativePath = `${convId}/attachments/${filename}`;
              } else {
                relativePath = filename;
              }
            }

            const attachment = {
              filename,
              fullPath,
              relativePath,
              contentType,
              size: parsed.bytes_written || 0,
              isOutput,
            };
            console.log("[reconstructAttachments] Adding attachment:", attachment);
            attachments.push(attachment);
          }
        } catch (e) {
          console.log("[reconstructAttachments] Failed to parse result:", e);
          // Skip invalid results
        }
      }
    });

    console.log("[reconstructAttachments] Final attachments:", attachments);
    return attachments;
  };

  /**
   * Load messages for a conversation
   */
  const loadMessages = async (conversationId: string) => {
    try {
      const msgs = await listMessages(conversationId);
      console.log("[loadMessages] Raw messages from backend:", msgs);
      setMessages(
        msgs.map((msg: any) => {
          // Reconstruct thinking data from tool_calls and tool_results
          const hasTools = msg.tool_calls && msg.tool_calls.length > 0;
          const hasResults = msg.tool_results && msg.tool_results.length > 0;

          console.log("[loadMessages] Message", msg.id, "hasTools:", hasTools, "hasResults:", hasResults);
          if (hasTools) {
            console.log("[loadMessages] tool_calls:", msg.tool_calls);
          }
          if (hasResults) {
            console.log("[loadMessages] tool_results:", msg.tool_results);
          }

          let thinking = undefined;
          if (hasTools) {
            // Reconstruct attachments from tool results
            const attachments = hasResults
              ? reconstructAttachments(msg.tool_calls, msg.tool_results)
              : [];

            console.log("[loadMessages] Reconstructed attachments:", attachments);

            thinking = {
              toolCalls: msg.tool_calls,
              toolCount: msg.tool_calls.length,
              attachments,
            };
          }

          return {
            id: msg.id,
            conversationId,
            role: msg.role,
            content: msg.content,
            timestamp: msg.created_at ? new Date(msg.created_at).getTime() : Date.now(),
            thinking,
          };
        })
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
    let collectedAttachments: AttachmentInfo[] = [];

    // Collect streaming events to build assistant message
    const eventHandler = (event: any) => {
      // Forward to thinking panel state manager FIRST
      handleEvent(event);

      // Track tool calls
      if (event.type === "tool_call_start") {
        collectedToolCalls.push({
          id: event.toolId,
          name: event.toolName,
          status: "running" as const,
        });

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
                  attachments: [...collectedAttachments],
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
                      attachments: [...collectedAttachments],
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

        // Parse write tool results to extract attachment info
        if (toolCall && toolCall.name === "write" && !event.error) {
          try {
            const parsed = JSON.parse(event.result);
            // WriteTool returns {path, bytes_written} - no success field needed
            if (parsed.path) {
              const fullPath = parsed.path;
              const filename = fullPath.split('/').pop() || fullPath.split('\\').pop() || "file";
              const isOutput = fullPath.includes("/outputs/") || fullPath.includes("\\\\");

              // Detect content type from extension
              const ext = filename.split('.').pop()?.toLowerCase();
              let contentType = "text";
              if (ext === "html" || ext === "htm") contentType = "html";
              else if (ext === "pdf") contentType = "pdf";
              else if (ext === "md" || ext === "markdown") contentType = "markdown";
              else if (["png", "jpg", "jpeg", "gif", "svg", "webp"].includes(ext || "")) contentType = "image";

              // Build relative path
              let relativePath: string;
              if (isOutput) {
                relativePath = `outputs/${filename}`;
              } else {
                // Extract conv_id/attachments/filename from full path
                const parts = fullPath.split('/');
                const attachmentsIdx = parts.indexOf('attachments');
                if (attachmentsIdx > 0 && attachmentsIdx + 1 < parts.length) {
                  const convId = parts[attachmentsIdx - 1];
                  relativePath = `${convId}/attachments/${filename}`;
                } else {
                  relativePath = filename;
                }
              }

              collectedAttachments.push({
                filename,
                fullPath,
                relativePath,
                contentType,
                size: parsed.bytes_written || 0,
                isOutput,
              });
            }
          } catch (e) {
            console.error("[ConversationsPanel] Failed to parse write result:", e);
          }
        }

        // Update message with tool result and attachments
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
                      attachments: [...collectedAttachments],
                    }
                  }
                : m
            );
          }
          return prev;
        });
      } else if (event.type === "done") {
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
                      attachments: [...collectedAttachments],
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
        // Update or add assistant message using functional update
        setMessages((prev) => {
          const existingMsg = prev.find((m) => m.id === assistantMessageId);
          if (existingMsg) {
            // Append new content to existing message content
            return prev.map((m) =>
              m.id === assistantMessageId
                ? {
                    ...m,
                    content: m.content + event.content,
                    thinking: {
                      toolCalls: [...collectedToolCalls],
                      toolCount: collectedToolCalls.length,
                      attachments: [...collectedAttachments],
                    }
                  }
                : m
            );
          } else {
            // Create new message with first token
            return [
              ...prev,
              {
                id: assistantMessageId,
                conversationId: selectedConversation!.id,
                role: "assistant",
                content: event.content,
                timestamp: Date.now(),
                thinking: {
                  toolCalls: [...collectedToolCalls],
                  toolCount: collectedToolCalls.length,
                  attachments: [...collectedAttachments],
                },
              },
            ];
          }
        });
      } else if (event.type === "reasoning") {
        // Update or add assistant message with reasoning content
        setMessages((prev) => {
          const existingMsg = prev.find((m) => m.id === assistantMessageId);
          if (existingMsg) {
            // Append reasoning to existing message
            return prev.map((m) =>
              m.id === assistantMessageId
                ? {
                    ...m,
                    thinking: {
                      ...m.thinking,
                      reasoning: (m.thinking?.reasoning || "") + event.content,
                      toolCalls: [...collectedToolCalls],
                      toolCount: collectedToolCalls.length,
                      attachments: [...collectedAttachments],
                    }
                  }
                : m
            );
          } else {
            // Create new message with reasoning first
            return [
              ...prev,
              {
                id: assistantMessageId,
                conversationId: selectedConversation!.id,
                role: "assistant",
                content: "",
                timestamp: Date.now(),
                thinking: {
                  reasoning: event.content,
                  toolCalls: [...collectedToolCalls],
                  toolCount: collectedToolCalls.length,
                  attachments: [...collectedAttachments],
                },
              },
            ];
          }
        });
      } else if (event.type === "show_content") {
        // Show content in generative canvas
        console.log("🎨 Opening content viewer:", event.title || event.contentType);
        setCanvasContent({ type: "show_content", event: event as ShowContentEvent });
        setCanvasOpen(true);
      } else if (event.type === "request_input") {
        // Request input via generative form
        console.log("📝 Opening input form:", event.title);
        setCanvasContent({ type: "request_input", event: event as RequestInputEvent });
        setCanvasOpen(true);
      }
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
        className="w-80 bg-sidebar shrink-0"
      />

      {/* Right: Chat Area or Empty State */}
      <div className="flex-1 flex min-w-0 bg-background">
        {selectedConversation ? (
          /* When conversation IS selected - show chat with embedded thought panel */
          <div className="flex-1 flex flex-col min-w-0">
            {/* Agent Info Header */}
            <div className="h-14 px-6 bg-card flex items-center shrink-0">
              <div className="flex items-center gap-3">
                <span className="text-xl" role="img">
                  {selectedConversation.agentIcon || "🤖"}
                </span>
                <div>
                  <div className="text-sm font-medium text-foreground">
                    {selectedConversation.title}
                  </div>
                  <div className="text-xs text-muted-foreground">
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
                        onShowThinking={() => message.thinking && handleShowHistoricalThinking(
                          message.thinking.toolCalls || [],
                          message.thinking.reasoning
                        )}
                        onOpenAttachment={handleOpenAttachment}
                      />
                    ))}
                    {isLoading && <TypingIndicator />}
                    <div ref={messagesEndRef} />
                  </div>
                )}
              </div>

              {/* Thought Panel - next to messages area */}
              {displayThinkingState.isOpen && (
                <div className="w-80 border-l border-border flex flex-col bg-card overflow-y-auto">
                  {/* Reasoning / Chain of Thought */}
                  {displayThinkingState.reasoning.length > 0 && (
                    <ThinkingContent reasoning={displayThinkingState.reasoning.join("")} />
                  )}

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
                  {displayThinkingState.toolCalls.length === 0 &&
                   displayThinkingState.reasoning.length === 0 &&
                   displayThinkingState.isActive && (
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
                        : displayThinkingState.reasoning.length > 0
                        ? "Thinking..."
                        : displayThinkingState.isActive
                        ? "Agent is working..."
                        : "Ready"}
                    </div>
                  </div>
                </div>
              )}
            </div>

            {/* Input Area */}
            <div className="border-t border-border p-4">
              <div className="max-w-3xl mx-auto">
                <div className="relative bg-input rounded-2xl border border-border focus-within:border-primary/50 transition-colors">
                  <Textarea
                    ref={inputRef}
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
                    className="min-h-[60px] max-h-[200px] bg-transparent border-0 text-foreground placeholder:text-muted-foreground resize-none pr-24 focus-visible:ring-0"
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
              <Loader2 className="size-8 text-foreground animate-spin" />
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
          <div className="bg-popover border border-border rounded-xl p-6 w-full max-w-md mx-4 shadow-2xl">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-foreground">Select an Agent</h3>
              <button
                onClick={() => {
                  setShowAgentSelector(false);
                  setPendingAgentId(null);
                }}
                className="text-muted-foreground hover:text-foreground transition-colors"
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
                      ? "bg-primary/10 border-primary/30 text-foreground"
                      : "bg-muted border-transparent hover:bg-accent hover:border-border text-foreground"
                  )}
                >
                  <div className="w-10 h-10 rounded-lg bg-purple-500/20 flex items-center justify-center">
                    <span className="text-xl">{getAgentIcon(agent.name)}</span>
                  </div>
                  <div>
                    <div className="font-medium">{agent.displayName}</div>
                    <div className="text-xs text-muted-foreground">{agent.name}</div>
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
                className="flex-1 px-4 py-2 text-sm font-medium text-foreground bg-muted hover:bg-accent rounded-lg transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleAgentSelected}
                disabled={!pendingAgentId}
                className={cn(
                  "flex-1 px-4 py-2 text-sm font-medium rounded-lg transition-all",
                  pendingAgentId
                    ? "bg-primary hover:bg-primary/90 text-primary-foreground shadow-lg"
                    : "bg-muted text-muted-foreground cursor-not-allowed"
                )}
              >
                Start Chat
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Generative Canvas - Slides in for content display and input forms */}
      <GenerativeCanvas
        isOpen={canvasOpen}
        onClose={() => setCanvasOpen(false)}
        content={canvasContent}
        conversationId={selectedConversation?.id}
        onFormSubmit={async (data) => {
          // Format form data as JSON string and send as user message
          const jsonMessage = JSON.stringify(data, null, 2);
          await handleSendMessage(jsonMessage);
        }}
        onCanvasCancel={() => {
          // Focus the text input when canvas is cancelled
          inputRef.current?.focus();
        }}
      />
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
      <h2 className="text-2xl font-bold text-foreground mb-2">No conversations yet</h2>
      <p className="text-muted-foreground mb-8 max-w-md">
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
      <div className="w-20 h-20 rounded-2xl bg-card flex items-center justify-center mb-6 border border-border">
        <Bot className="size-10 text-muted-foreground" />
      </div>
      <h2 className="text-2xl font-bold text-foreground mb-2">Select a conversation</h2>
      <p className="text-muted-foreground max-w-md">
        Choose a conversation from the sidebar to view and continue your chat.
      </p>
      <div className="mt-8 flex items-center gap-2 text-sm text-muted-foreground">
        <ChevronRight className="size-4" />
        <span>Click on any conversation in the left sidebar</span>
      </div>
    </div>
  );
}

// ============================================================================
// MESSAGE COMPONENTS
// ============================================================================

// Attachment Pill Component
interface AttachmentPillProps {
  attachment: AttachmentInfo;
  onClick: () => void;
}

function AttachmentPill({ attachment, onClick }: AttachmentPillProps) {
  const getIcon = () => {
    switch (attachment.contentType) {
      case "html": return "🌐";
      case "pdf": return "📄";
      case "image": return "🖼️";
      case "markdown": return "📝";
      default: return "📎";
    }
  };

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes}B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
  };

  return (
    <button
      onClick={onClick}
      className="inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-white/5 hover:bg-white/10 border border-white/10 transition-colors text-xs"
      title={`${attachment.filename} (${formatSize(attachment.size)})${attachment.isOutput ? " - Chrome accessible" : ""}`}
    >
      <span>{getIcon()}</span>
      <span className="text-gray-300 max-w-[150px] truncate">{attachment.filename}</span>
      {attachment.isOutput && (
        <span className="text-[10px] text-green-400 bg-green-400/10 px-1 rounded">Chrome</span>
      )}
    </button>
  );
}

interface MessageBubbleProps {
  message: MessageWithThinking;
  onShowThinking: () => void;
  onOpenAttachment: (attachment: AttachmentInfo) => void;
}

function MessageBubble({ message, onShowThinking, onOpenAttachment }: MessageBubbleProps) {
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
              ? "bg-primary text-primary-foreground"
              : "bg-card text-foreground"
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

        {/* Thinking Indicator + Attachment Pills (for assistant messages) */}
        {!isUser && (
          <div className="mt-2 flex items-center gap-2 flex-wrap">
            {/* Tools/Thinking indicator */}
            {message.thinking && (message.thinking.toolCount > 0 || (message.thinking.reasoning && message.thinking.reasoning.length > 0)) && (
              <button
                onClick={onShowThinking}
                className="text-xs text-gray-500 hover:text-purple-400 transition-colors flex items-center gap-1.5"
              >
                <span>🧠</span>
                <span>
                  {message.thinking.toolCount > 0
                    ? `Used ${message.thinking.toolCount} tool${message.thinking.toolCount !== 1 ? "s" : ""}`
                    : "Thinking"}
                </span>
              </button>
            )}

            {/* Attachment Pills */}
            {message.thinking?.attachments && message.thinking.attachments.length > 0 &&
              message.thinking.attachments.map((att) => (
                <AttachmentPill
                  key={att.fullPath}
                  attachment={att}
                  onClick={() => onOpenAttachment(att)}
                />
              ))
            }
          </div>
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
      <div className="bg-card rounded-2xl px-4 py-3">
        <div className="flex gap-1">
          <span className="size-2 bg-gray-500 rounded-full animate-bounce [animation-delay:-0.3s]" />
          <span className="size-2 bg-gray-500 rounded-full animate-bounce [animation-delay:-0.15s]" />
          <span className="size-2 bg-gray-500 rounded-full animate-bounce" />
        </div>
      </div>
    </div>
  );
}

// Truncated reasoning component with expand toggle
function ThinkingContent({ reasoning }: { reasoning: string }) {
  const [isExpanded, setIsExpanded] = useState(false);
  const truncateAt = 50;
  const shouldTruncate = reasoning.length > truncateAt;
  const displayContent = shouldTruncate && !isExpanded
    ? reasoning.substring(0, truncateAt) + "..."
    : reasoning;

  return (
    <div className="p-4 space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm font-medium text-gray-300">
          <span>🧠</span>
          <span>Thinking</span>
        </div>
        {shouldTruncate && (
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="text-xs text-purple-400 hover:text-purple-300 transition-colors"
          >
            {isExpanded ? "Show less" : "Show more"}
          </button>
        )}
      </div>
      <div className="bg-white/5 rounded-md p-3 max-h-60 overflow-y-auto">
        <p className="text-sm text-gray-400 whitespace-pre-wrap font-mono break-words">
          {displayContent}
        </p>
      </div>
    </div>
  );
}

function EmptyChatState({ agentName }: { agentName?: string }) {
  return (
    <div className="flex flex-col items-center justify-center h-full text-center px-8">
      <div className="text-5xl mb-4">💬</div>
      <h3 className="text-lg font-medium text-foreground mb-2">
        Start a conversation with {agentName || "the agent"}
      </h3>
      <p className="text-sm text-muted-foreground max-w-md">
        Send a message to begin. The agent will use its tools to help you with
        your task.
      </p>
    </div>
  );
}
