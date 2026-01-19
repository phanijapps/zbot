// ============================================================================
// AGENT CHANNELS FEATURE
// Discord-like agent interface with daily sessions
// ============================================================================

import { useState, useEffect, useRef } from "react";
import { MessageSquare, Bot, Loader2, Paperclip, Send, History } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cn } from "@/shared/utils";
import { Textarea } from "@/shared/ui/textarea";
import {
  AgentChannelList,
  useStreamEvents,
  ThinkingPanel,
  GenerativeCanvas,
  type ContentState,
  type MessageWithThinking,
} from "@/domains/agent-runtime/components";
import type { Agent, DailySession, DaySummary, SessionMessage } from "@/shared/types";
import {
  getOrCreateTodaySession,
  listPreviousDays,
  loadSessionMessages,
  formatSessionDate,
} from "@/services/agentChannels";
import { listAgents } from "@/services/agent";

type ExecutionStage = "idle" | "thinking" | "using_tools" | "generating" | "done";

export function AgentChannelPanel() {
  // UI State
  const [agents, setAgents] = useState<Agent[]>([]);
  const [selectedAgent, setSelectedAgent] = useState<Agent | null>(null);
  const [currentSession, setCurrentSession] = useState<DailySession | null>(null);
  const [previousDays, setPreviousDays] = useState<DaySummary[]>([]);
  const [showPreviousDays, setShowPreviousDays] = useState(false);
  const [messages, setMessages] = useState<MessageWithThinking[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [_loading, setLoading] = useState(true);
  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Execution stage for better UX
  const [executionStage, setExecutionStage] = useState<ExecutionStage>("idle");
  const [activeToolName, setActiveToolName] = useState<string | null>(null);

  // Error state
  const [_error, setError] = useState<string | null>(null);

  // Generative Canvas state
  const [canvasOpen, setCanvasOpen] = useState(false);
  const [_canvasContent, _setCanvasContent] = useState<ContentState | null>(null);

  // Stream events handling
  const {
    state: thinkingState,
    reset: resetThinking,
  } = useStreamEvents(true, false);

  // Load agents on mount
  useEffect(() => {
    loadAgents();
  }, []);

  // Load session when agent is selected
  useEffect(() => {
    if (selectedAgent) {
      loadTodaySession(selectedAgent.id);
    } else {
      setCurrentSession(null);
      setMessages([]);
      setPreviousDays([]);
      setShowPreviousDays(false);
    }
    resetThinking();
  }, [selectedAgent]);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    if (messages.length > 0) {
      setTimeout(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: "auto" });
      }, 50);
    }
  }, [messages]);

  /**
   * Scroll to bottom of messages
   */
  const scrollToBottom = () => {
    setTimeout(() => {
      messagesEndRef.current?.scrollIntoView({ behavior: "auto" });
    }, 50);
  };

  /**
   * Load all agents
   */
  const loadAgents = async () => {
    try {
      setLoading(true);
      const agentList = await listAgents();
      setAgents(agentList);
    } catch (err) {
      console.error("Failed to load agents:", err);
      setError("Failed to load agents");
    } finally {
      setLoading(false);
    }
  };

  /**
   * Load today's session for an agent
   */
  const loadTodaySession = async (agentId: string) => {
    try {
      setIsLoading(true);
      const session = await getOrCreateTodaySession(agentId);
      setCurrentSession(session);

      // Load messages for this session
      const sessionMessages = await loadSessionMessages(session.id);
      setMessages(convertSessionMessagesToWithThinking(sessionMessages));

      // Load previous days
      const days = await listPreviousDays(agentId, 30);
      setPreviousDays(days);

      // Scroll to bottom after messages are loaded
      scrollToBottom();
    } catch (err) {
      console.error("Failed to load session:", err);
      setError("Failed to load session");
    } finally {
      setIsLoading(false);
    }
  };

  /**
   * Convert SessionMessage to MessageWithThinking format
   */
  const convertSessionMessagesToWithThinking = (
    sessionMessages: SessionMessage[]
  ): MessageWithThinking[] => {
    return sessionMessages.map((msg) => ({
      id: msg.id,
      conversationId: msg.sessionId,
      role: msg.role as "user" | "assistant" | "system",
      content: msg.content,
      timestamp: new Date(msg.createdAt).getTime(),
      thinking: { toolCount: 0 },
    }));
  };

  /**
   * Handle sending a message
   */
  const handleSendMessage = async () => {
    if (!input.trim() || !currentSession || !selectedAgent) return;

    const userMessage = input.trim();
    setInput("");
    setIsLoading(true);
    setExecutionStage("thinking");

    // Note: User message recording is handled by the backend (agents_runtime.rs)
    // No need to record it here separately

    // Add user message to UI immediately
    const tempUserMessage: MessageWithThinking = {
      id: Date.now().toString(),
      conversationId: currentSession.id,
      role: "user",
      content: userMessage,
      timestamp: Date.now(),
      thinking: { toolCount: 0 },
    };
    setMessages((prev) => [...prev, tempUserMessage]);

    // Create assistant message placeholder for streaming response
    const assistantMessageId = (Date.now() + 1).toString();
    const initialAssistantMessage: MessageWithThinking = {
      id: assistantMessageId,
      conversationId: currentSession.id,
      role: "assistant",
      content: "",
      timestamp: Date.now(),
      thinking: { toolCount: 0 },
    };
    setMessages((prev) => [...prev, initialAssistantMessage]);

    // Listen for streaming events from the backend
    const eventChannel = `agent-stream://${currentSession.id}`;
    let unlisten: (() => void) | null = null;
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let hasStoppedLoading = false;

    const finishProcessing = () => {
      setIsLoading(false);
      setExecutionStage("done");
      setActiveToolName(null);

      loadSessionMessages(currentSession.id).then((sessionMessages) => {
        setMessages(convertSessionMessagesToWithThinking(sessionMessages));
        scrollToBottom();
      }).catch((err) => {
        console.error("Failed to refresh messages:", err);
      });

      if (unlisten) {
        unlisten();
        unlisten = null;
      }
      if (timeoutId) {
        clearTimeout(timeoutId);
        timeoutId = null;
      }
    };

    const unlistenPromise = listen(eventChannel, (event) => {
      const data = event.payload as { type: string; content?: string; finalMessage?: string; error?: string; toolName?: string };
      if (!data) return;

      switch (data.type) {
        case "token":
          setExecutionStage("generating");
          setMessages((prev) => prev.map((msg) =>
            msg.id === assistantMessageId
              ? { ...msg, content: msg.content + (data.content || "") }
              : msg
          ));
          // Stop spinner on first token (only once)
          if (!hasStoppedLoading) {
            hasStoppedLoading = true;
            setTimeout(() => {
              setIsLoading(false);
            }, 100);
          }
          break;

        case "tool_call_start":
          setExecutionStage("using_tools");
          setActiveToolName(data.toolName || null);
          break;

        case "tool_result":
          setExecutionStage("generating");
          setActiveToolName(null);
          break;

        case "done":
          if (data.finalMessage) {
            setMessages((prev) => prev.map((msg) =>
              msg.id === assistantMessageId
                ? { ...msg, content: data.finalMessage || "" }
                : msg
            ));
          }
          finishProcessing();
          break;

        case "error":
          setMessages((prev) => prev.map((msg) =>
            msg.id === assistantMessageId && msg.content === ""
              ? { ...msg, content: `Error: ${data.error || "Unknown error"}` }
              : msg
          ));
          finishProcessing();
          break;
      }
    });

    unlisten = await unlistenPromise;

    // Fallback timeout - if we don't get done/error within 30 seconds, finish anyway
    timeoutId = setTimeout(() => {
      finishProcessing();
    }, 30000);

    try {
      await invoke("execute_agent_stream", {
        agentId: selectedAgent.id,
        message: userMessage,
      });
    } catch (error) {
      setMessages((prev) => prev.map((msg) =>
        msg.id === assistantMessageId && msg.content === ""
          ? { ...msg, content: `Error: ${error instanceof Error ? error.message : String(error)}` }
          : msg
      ));
      finishProcessing();
    }
  };

  /**
   * Handle showing previous days
   */
  const handleShowHistory = () => {
    setShowPreviousDays(!showPreviousDays);
  };

  return (
    <div className="flex h-full bg-zinc-950">
      {/* Sidebar - Agent Channels */}
      <div className="w-80 flex flex-col shrink-0">
        <AgentChannelList
          agents={agents}
          selectedAgentId={selectedAgent?.id}
          onSelectAgent={setSelectedAgent}
          onShowHistory={handleShowHistory}
        />
      </div>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0">
        {selectedAgent && currentSession ? (
          <>
            {/* Header */}
            <div className="h-14 flex items-center justify-between px-4 shrink-0">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-lg bg-purple-500/20 flex items-center justify-center">
                  <Bot className="size-4 text-purple-400" />
                </div>
                <div>
                  <div className="font-medium text-white text-sm">{selectedAgent.displayName}</div>
                  <div className="text-xs text-gray-500">
                    {formatSessionDate(currentSession.sessionDate)} • {currentSession.messageCount} messages
                  </div>
                </div>
              </div>
              {previousDays.length > 0 && (
                <button
                  onClick={() => setShowPreviousDays(!showPreviousDays)}
                  className="flex items-center gap-2 px-3 py-1.5 text-sm text-gray-400 hover:text-white hover:bg-white/5 rounded-lg transition-colors"
                >
                  <History className="size-4" />
                  {showPreviousDays ? "Hide" : "Show"} History
                </button>
              )}
            </div>

            {/* Previous Days Summary (Collapsible) */}
            {showPreviousDays && previousDays.length > 0 && (
              <div className="border-b border-white/10 p-4 bg-white/5">
                <h3 className="text-sm font-medium text-gray-400 mb-3">Previous Days</h3>
                <div className="space-y-2">
                  {previousDays.slice(0, 5).map((day) => (
                    <div
                      key={day.sessionId}
                      className="flex items-center justify-between p-3 rounded-lg bg-zinc-900/50 border border-white/5"
                    >
                      <div>
                        <div className="text-sm text-white">{formatSessionDate(day.sessionDate)}</div>
                        <div className="text-xs text-gray-500">{day.messageCount} messages</div>
                      </div>
                      {day.summary && (
                        <div className="text-xs text-gray-400 max-w-xs truncate">
                          {day.summary}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Messages Area */}
            <div className="flex-1 overflow-y-auto">
              {messages.length === 0 ? (
                isLoading ? (
                  <div className="flex items-center justify-center h-full">
                    <Loader2 className="size-6 text-purple-400 animate-spin" />
                  </div>
                ) : (
                  <div className="flex flex-col items-center justify-center h-full px-6 text-center">
                    <div className="w-14 h-14 rounded-xl bg-gradient-to-br from-purple-500/20 to-blue-500/20 flex items-center justify-center mb-4 border border-white/10">
                      <MessageSquare className="size-7 text-purple-400" />
                    </div>
                    <h3 className="text-base font-semibold text-white mb-2">
                      Today's session
                    </h3>
                    <p className="text-sm text-gray-500 max-w-xs">
                      Start a conversation with {selectedAgent.displayName}. Messages are saved to today's session.
                    </p>
                  </div>
                )
              ) : (
                <div className="max-w-4xl mx-auto py-6 px-4 space-y-6">
                  {messages.map((msg) => (
                    <div
                      key={msg.id}
                      className={cn(
                        "flex gap-3",
                        msg.role === "user" ? "justify-end" : "justify-start"
                      )}
                    >
                      {msg.role === "assistant" && (
                        <div className="w-8 h-8 rounded-lg bg-purple-500/20 flex items-center justify-center shrink-0">
                          <Bot className="size-4 text-purple-400" />
                        </div>
                      )}
                      <div
                        className={cn(
                          "max-w-[80%] rounded-lg px-4 py-3",
                          msg.role === "user"
                            ? "bg-purple-600 text-white"
                            : "bg-white/5 text-gray-100"
                        )}
                      >
                        <p className="text-sm whitespace-pre-wrap">{msg.content}</p>
                      </div>
                    </div>
                  ))}
                  {isLoading && (
                    <div className="flex items-center gap-2 text-gray-400 text-sm py-4">
                      <Loader2 className="size-4 animate-spin" />
                      <span>
                        {executionStage === "thinking" && "Thinking..."}
                        {executionStage === "using_tools" && `Using tool: ${activeToolName || "processing"}...`}
                        {executionStage === "generating" && "Generating response..."}
                      </span>
                    </div>
                  )}
                  <div ref={messagesEndRef} />
                </div>
              )}
            </div>

            {/* Input Area */}
            <div className="border-t border-white/10 p-4 shrink-0">
              <div className="max-w-4xl mx-auto">
                <div className="flex items-end gap-3">
                  <button
                    className="p-2 rounded-lg text-gray-400 hover:text-white hover:bg-white/5 transition-colors"
                    title="Attach file"
                  >
                    <Paperclip className="size-5" />
                  </button>
                  <div className="flex-1 relative">
                    <Textarea
                      ref={inputRef}
                      value={input}
                      onChange={(e) => setInput(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" && !e.shiftKey) {
                          e.preventDefault();
                          handleSendMessage();
                        }
                      }}
                      placeholder="Type your message..."
                      className="min-h-[44px] max-h-[200px] resize-none bg-white/5 border-white/10 text-white placeholder:text-gray-500"
                    />
                  </div>
                  <button
                    onClick={handleSendMessage}
                    disabled={!input.trim() || isLoading}
                    className={cn(
                      "p-3 rounded-lg transition-all",
                      input.trim() && !isLoading
                        ? "bg-gradient-to-r from-purple-600 to-blue-600 hover:from-purple-700 hover:to-blue-700 text-white shadow-lg shadow-purple-500/25"
                        : "bg-white/5 text-gray-500 cursor-not-allowed"
                    )}
                  >
                    <Send className="size-5" />
                  </button>
                </div>
              </div>
            </div>
          </>
        ) : (
          /* Empty State - No Agent Selected */
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center px-6">
              <div className="w-16 h-16 rounded-xl bg-gradient-to-br from-purple-500/20 to-blue-500/20 flex items-center justify-center mx-auto mb-4 border border-white/10">
                <Bot className="size-8 text-purple-400" />
              </div>
              <h2 className="text-xl font-semibold text-white mb-2">
                Select an Agent Channel
              </h2>
              <p className="text-gray-500 max-w-md mx-auto">
                Choose an agent from the sidebar to start your conversation. Each agent has its own daily session.
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Thinking Panel */}
      {thinkingState.isOpen && (
        <ThinkingPanel
          state={thinkingState}
          isOpen={thinkingState.isOpen}
          onClose={() => resetThinking()}
        />
      )}

      {/* Generative Canvas */}
      {canvasOpen && _canvasContent && (
        <GenerativeCanvas
          content={_canvasContent}
          isOpen={canvasOpen}
          onClose={() => setCanvasOpen(false)}
        />
      )}
    </div>
  );
}
