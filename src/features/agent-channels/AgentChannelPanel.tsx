// ============================================================================
// AGENT CHANNELS FEATURE
// Discord-like agent interface with daily sessions
// ============================================================================

import { useState, useEffect, useRef, useCallback } from "react";
import { MessageSquare, Bot, Loader2, Paperclip, Send, History, Hash, Wrench, Trash2, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cn } from "@/shared/utils";
import { Textarea } from "@/shared/ui/textarea";
import {
  AgentChannelList,
  useStreamEvents,
  ThinkingPanel,
  GenerativeCanvas,
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
  const [messages, setMessages] = useState<MessageWithThinking[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const isMountedRef = useRef(true);
  const isExecutingRef = useRef(false); // Prevent concurrent executions

  // Execution stage for better UX
  const [executionStage, setExecutionStage] = useState<ExecutionStage>("idle");
  const [activeToolName, setActiveToolName] = useState<string | null>(null);


  // Generative Canvas state
  const [canvasOpen, setCanvasOpen] = useState(false);
  const [canvasContent, setCanvasContent] = useState<{
    type: "request_input" | "show_content";
    event: any;
  } | null>(null);

  // History Panel state
  const [historyPanelOpen, setHistoryPanelOpen] = useState(false);

  // Track the current request_input tool ID for marking it as completed when form is submitted
  const [pendingRequestInputToolId, setPendingRequestInputToolId] = useState<string | null>(null);

  // Stream events handling
  const {
    state: thinkingState,
    handleEvent: handleThinkingEvent,
    reset: resetThinking,
    setCurrentMessage,
  } = useStreamEvents(true, false);

  // Load agents on mount
  useEffect(() => {
    isMountedRef.current = true;
    loadAgents();

    return () => {
      isMountedRef.current = false;
    };
  }, []);

  // Load session when agent is selected
  useEffect(() => {
    if (selectedAgent) {
      loadTodaySession(selectedAgent.id);
    } else {
      setCurrentSession(null);
      setMessages([]);
      setPreviousDays([]);
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
  const scrollToBottom = useCallback(() => {
    setTimeout(() => {
      messagesEndRef.current?.scrollIntoView({ behavior: "auto" });
    }, 50);
  }, []);

  /**
   * Convert SessionMessage to MessageWithThinking format
   */
  const convertSessionMessagesToWithThinking = useCallback((
    sessionMessages: SessionMessage[]
  ): MessageWithThinking[] => {
    return sessionMessages.map((msg) => {
      // Count tool calls from the Record
      const toolCallCount = msg.toolCalls ? Object.keys(msg.toolCalls).length : 0;

      return {
        id: msg.id,
        conversationId: msg.sessionId,
        role: msg.role as "user" | "assistant" | "system",
        content: msg.content,
        timestamp: new Date(msg.createdAt).getTime(),
        // Only include thinking if there are actual tool calls
        ...(toolCallCount > 0 ? {
          thinking: { toolCount: toolCallCount }
        } : {}),
      };
    });
  }, []);

  /**
   * Load all agents
   */
  const loadAgents = useCallback(async () => {
    try {
      const agentList = await listAgents();
      setAgents(agentList);
    } catch (err) {
      console.error("Failed to load agents:", err);
    }
  }, []);

  /**
   * Load today's session for an agent
   */
  const loadTodaySession = useCallback(async (agentId: string) => {
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
    } finally {
      setIsLoading(false);
    }
  }, [convertSessionMessagesToWithThinking, scrollToBottom]);

  /**
   * Execute agent with a message (optionally show user message)
   * @param message - The message to send to the agent
   * @param showUserMessage - Whether to display the user message in the chat
   */
  const executeAgentWithMessage = async (message: string, showUserMessage = true) => {
    if (!currentSession || !selectedAgent) return;

    // Wait for any ongoing execution to complete before starting a new one
    while (isExecutingRef.current) {
      console.log("[AgentChannelPanel] Waiting for previous execution to complete...");
      await new Promise(resolve => setTimeout(resolve, 100));
    }

    isExecutingRef.current = true;
    setIsLoading(true);
    setExecutionStage("thinking");

    // Close history sidebar when sending a new message
    setHistoryPanelOpen(false);

    // Optionally add user message to UI
    let assistantMessageId: string;
    if (showUserMessage) {
      const tempUserMessage: MessageWithThinking = {
        id: Date.now().toString(),
        conversationId: currentSession.id,
        role: "user",
        content: message,
        timestamp: Date.now(),
      };
      setMessages((prev) => [...prev, tempUserMessage]);
      assistantMessageId = (Date.now() + 1).toString();
    } else {
      // For hidden messages (form submissions), don't show user message
      assistantMessageId = Date.now().toString();
    }

    // Create assistant message placeholder for streaming response
    const initialAssistantMessage: MessageWithThinking = {
      id: assistantMessageId,
      conversationId: currentSession.id,
      role: "assistant",
      content: "",
      timestamp: Date.now(),
      // Don't set thinking initially - only add when tools are actually used
    };
    setMessages((prev) => [...prev, initialAssistantMessage]);

    // Set current message for thinking panel
    setCurrentMessage(assistantMessageId);

    // Listen for streaming events from the backend
    const eventChannel = `agent-stream://${currentSession.id}`;
    let unlisten: (() => void) | null = null;
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let hasStoppedLoading = false;
    let toolCallCount = 0; // Track tool calls locally during streaming

    const finishProcessing = () => {
      // Only update state if component is still mounted
      if (!isMountedRef.current) return;

      isExecutingRef.current = false; // Reset execution flag
      setIsLoading(false);
      setExecutionStage("done");
      setActiveToolName(null);

      // Don't reload from database - we already have the correct state in memory
      // Reloading would overwrite the tool count we just set
      scrollToBottom();

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
      // Don't process events if component is unmounted
      if (!isMountedRef.current) return;

      const data = event.payload as { type: string; content?: string; finalMessage?: string; error?: string; toolName?: string; toolId?: string; args?: string; result?: string; formId?: string; title?: string; description?: string; schema?: any; submitButton?: string };
      if (!data) return;

      console.log("[AgentChannelPanel] Received event:", data.type, "payload:", data);

      // Map simplified events to AgentStreamEvent format and pass to thinking panel
      switch (data.type) {
        case "token":
          setExecutionStage("generating");
          handleThinkingEvent({
            type: "token",
            content: data.content || "",
            timestamp: Date.now()
          });
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
          toolCallCount++; // Increment local tool counter
          handleThinkingEvent({
            type: "tool_call_start",
            toolId: data.toolId || `tool-${Date.now()}`,
            toolName: data.toolName || "unknown",
            timestamp: Date.now()
          });
          break;

        case "tool_result":
          setExecutionStage("generating");
          setActiveToolName(null);
          handleThinkingEvent({
            type: "tool_result",
            toolId: data.toolId || `tool-${Date.now()}`,
            toolName: data.toolName || "unknown",
            result: data.result || "",
            error: data.error,
            timestamp: Date.now()
          });
          // Update tool count in real-time as tools complete (shows tickmark in sidebar)
          setMessages((prev) => prev.map((msg) =>
            msg.id === assistantMessageId
              ? { ...msg, thinking: { toolCount: toolCallCount } }
              : msg
          ));
          break;

        case "done":
          handleThinkingEvent({
            type: "done",
            finalMessage: data.finalMessage || "",
            tokenCount: 0,
            timestamp: Date.now()
          });
          // Update with content and tool count only if tools were used
          setMessages((prev) => prev.map((msg) =>
            msg.id === assistantMessageId
              ? {
                  ...msg,
                  content: data.finalMessage || "",
                  ...(toolCallCount > 0 ? { thinking: { toolCount: toolCallCount } } : {})
                }
              : msg
          ));
          finishProcessing();
          break;

        case "request_input":
          // Open form for user input via GenerativeCanvas
          // Track the toolId so we can mark it as completed when form is submitted
          setPendingRequestInputToolId(data.toolId || null);
          handleThinkingEvent({
            type: "request_input",
            formId: data.formId || `form-${Date.now()}`,
            title: data.title || "Input Required",
            description: data.description || "",
            timestamp: Date.now()
          });
          setCanvasContent({
            type: "request_input",
            event: {
              formId: data.formId || `form-${Date.now()}`,
              title: data.title || "Input Required",
              description: data.description || "",
              schema: data.schema || {},
              submitButton: data.submitButton || "Submit"
            }
          });
          setCanvasOpen(true);
          break;

        case "error":
          handleThinkingEvent({
            type: "error",
            error: data.error || "Unknown error",
            recoverable: false,
            timestamp: Date.now()
          });
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
      if (isMountedRef.current) {
        finishProcessing();
      }
    }, 30000);

    try {
      await invoke("execute_agent_stream", {
        agentId: selectedAgent.id,
        message: message,
      });
    } catch (error) {
      if (isMountedRef.current) {
        setMessages((prev) => prev.map((msg) =>
          msg.id === assistantMessageId && msg.content === ""
            ? { ...msg, content: `Error: ${error instanceof Error ? error.message : String(error)}` }
            : msg
        ));
        finishProcessing();
      }
    }
  };

  /**
   * Handle sending a message from the input field
   */
  const handleSendMessage = async () => {
    if (!input.trim() || !currentSession || !selectedAgent) return;

    const userMessage = input.trim();
    setInput("");
    await executeAgentWithMessage(userMessage, true);
  };

  return (
    <div className="flex h-full bg-[#313338]">
      {/* Sidebar - Agent Channels */}
      <AgentChannelList
        agents={agents}
        selectedAgentId={selectedAgent?.id}
        onSelectAgent={setSelectedAgent}
      />

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0">
        {selectedAgent && currentSession ? (
          <>
            {/* Header */}
            <div className="h-12 border-b border-black/20 flex items-center justify-between px-4 shrink-0">
              <div className="flex items-center gap-2 group">
                <Hash className="size-5 text-gray-300" />
                <h2 className="text-white font-semibold cursor-default" title={`${messages.length} message${messages.length !== 1 ? 's' : ''} today`}>
                  {selectedAgent.displayName}
                </h2>
                <span className="text-xs text-gray-400 opacity-0 group-hover:opacity-100 transition-opacity">
                  {messages.length}
                </span>
              </div>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => setHistoryPanelOpen(true)}
                  className="p-2 text-gray-300 hover:text-white transition-colors rounded hover:bg-white/5"
                  aria-label="Show history"
                >
                  <History className="size-5" />
                </button>
              </div>
            </div>

            {/* Messages Area */}
            <div className="flex-1 overflow-y-auto">
              {messages.length === 0 ? (
                isLoading ? (
                  <div className="flex items-center justify-center h-full">
                    <Loader2 className="size-6 text-violet-400 animate-spin" />
                  </div>
                ) : (
                  <div className="flex flex-col items-center justify-center h-full px-6 text-center">
                    <div className="w-14 h-14 rounded-xl bg-gradient-to-br from-violet-600/20 to-purple-700/20 flex items-center justify-center mb-4 border border-white/10">
                      <MessageSquare className="size-7 text-violet-400" />
                    </div>
                    <h3 className="text-base font-semibold text-white mb-2">
                      Today's session
                    </h3>
                    <p className="text-sm text-gray-300 max-w-xs">
                      Start a conversation with {selectedAgent.displayName}. Messages are saved to today's session.
                    </p>
                  </div>
                )
              ) : (
                <div className="px-4 py-6">
                  <div className="space-y-4">
                    {messages.map((msg) => (
                      <div
                        key={msg.id}
                        className={cn(
                          "group -mx-4 px-4 py-0.5",
                          msg.role === 'user'
                            ? 'bg-[#404249] hover:bg-[#45474f]'
                            : 'bg-transparent hover:bg-black/5'
                        )}
                      >
                        <div className="flex gap-4">
                          <div className="size-10 rounded-full bg-gradient-to-br from-violet-600 to-purple-700 flex items-center justify-center shrink-0 text-white font-semibold">
                            {msg.role === 'user' ? 'U' : 'AI'}
                          </div>
                          <div className="flex-1 min-w-0">
                            <div className="flex items-baseline gap-2 mb-1">
                              <span className="font-semibold text-white">
                                {msg.role === 'user' ? 'You' : selectedAgent.displayName}
                              </span>
                              {msg.thinking?.toolCount && msg.thinking.toolCount > 0 && (
                                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-violet-500/20 text-violet-300 text-xs font-medium">
                                  <Wrench className="size-3" />
                                  {msg.thinking.toolCount} tool{msg.thinking.toolCount !== 1 ? 's' : ''}
                                </span>
                              )}
                            </div>
                            <p className="text-gray-200 text-[15px] leading-relaxed whitespace-pre-wrap break-words">
                              {msg.content}
                            </p>
                          </div>
                        </div>
                      </div>
                    ))}
                    {isLoading && (
                      <div className="flex items-center gap-2 text-gray-300 text-sm py-4">
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
                </div>
              )}
            </div>

            {/* Input Area */}
            <div className="px-4 pb-6 shrink-0">
              <div className="relative bg-[#383a40] rounded-lg">
                <div className="flex items-start gap-3 p-3">
                  <button className="p-2 text-gray-300 hover:text-white transition-colors rounded hover:bg-white/5 mt-1">
                    <Paperclip className="size-5" />
                  </button>
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
                    placeholder={`Message ${selectedAgent.displayName}`}
                    className="flex-1 min-h-[24px] max-h-[200px] bg-transparent border-0 text-white placeholder:text-gray-500 resize-none focus-visible:ring-0 p-0"
                    rows={1}
                  />
                  <div className="flex items-center gap-1 mt-1">
                    <button
                      onClick={handleSendMessage}
                      disabled={!input.trim() || isLoading}
                      className={cn(
                        'p-1.5 rounded transition-colors',
                        input.trim() && !isLoading
                          ? 'text-white hover:bg-white/5'
                          : 'text-gray-600 cursor-not-allowed'
                      )}
                    >
                      <Send className="size-5" />
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </>
        ) : (
          /* Empty State - No Agent Selected */
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center px-6">
              <div className="w-16 h-16 rounded-xl bg-gradient-to-br from-violet-600/20 to-purple-700/20 flex items-center justify-center mx-auto mb-4 border border-white/10">
                <Bot className="size-8 text-violet-400" />
              </div>
              <h2 className="text-xl font-semibold text-white mb-2">
                Select an Agent Channel
              </h2>
              <p className="text-gray-300 max-w-md mx-auto">
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

      {/* History Panel */}
      {historyPanelOpen && (
        <div className="fixed inset-y-0 right-0 w-80 bg-[#2b2d31] border-l border-black/20 shadow-xl z-50 flex flex-col">
          {/* Header */}
          <div className="h-12 border-b border-black/20 flex items-center justify-between px-4 shrink-0">
            <div className="flex items-center gap-2">
              <History className="size-5 text-gray-300" />
              <h2 className="text-white font-semibold">History</h2>
            </div>
            <button
              onClick={() => setHistoryPanelOpen(false)}
              className="p-2 text-gray-300 hover:text-white transition-colors rounded hover:bg-white/5"
              aria-label="Close history"
            >
              <X className="size-5" />
            </button>
          </div>

          {/* Content */}
          <div className="flex-1 overflow-y-auto p-4">
            {/* Clear Today Button (always shown if there's a session) */}
            {currentSession && messages.length > 0 && (
              <button
                onClick={async () => {
                  if (!selectedAgent) return;
                  if (confirm(`Clear all messages from today's session with ${selectedAgent.displayName}?`)) {
                    try {
                      // Use tomorrow to delete today and everything before
                      const tomorrow = new Date();
                      tomorrow.setDate(tomorrow.getDate() + 1);
                      const beforeDate = tomorrow.toISOString().split('T')[0];

                      await invoke('delete_agent_history', {
                        agentId: selectedAgent.id,
                        beforeDate: beforeDate
                      });
                      // Refresh to get a fresh session
                      await loadTodaySession(selectedAgent.id);
                    } catch (err) {
                      console.error('Failed to clear today:', err);
                      alert('Failed to clear today: ' + (err as Error).message);
                    }
                  }
                }}
                className="w-full mb-4 px-3 py-2 bg-orange-500/20 hover:bg-orange-500/30 text-orange-300 text-sm rounded-lg transition-colors flex items-center justify-center gap-2"
              >
                <Trash2 className="size-4" />
                Clear Today's Messages
              </button>
            )}

            {previousDays.length === 0 ? (
              <div className="text-center py-8">
                <p className="text-sm text-gray-300">No previous days found</p>
              </div>
            ) : (
              <>
                {/* Clear All Button */}
                <button
                  onClick={async () => {
                    if (!selectedAgent) return;
                    if (confirm('Are you sure you want to delete all history?')) {
                      try {
                        // Use a far future date to delete all history (SQLite will interpret this correctly)
                        await invoke('delete_agent_history', {
                          agentId: selectedAgent.id,
                          beforeDate: '2099-12-31'
                        });
                        // Refresh the session to update previous days list
                        await loadTodaySession(selectedAgent.id);
                      } catch (err) {
                        console.error('Failed to delete history:', err);
                        alert('Failed to delete history: ' + (err as Error).message);
                      }
                    }
                  }}
                  className="w-full mb-3 px-3 py-2 bg-red-500/20 hover:bg-red-500/30 text-red-300 text-sm rounded-lg transition-colors flex items-center justify-center gap-2"
                >
                  <Trash2 className="size-4" />
                  Clear All History
                </button>

                {/* Days List */}
                <div className="space-y-2">
                  {previousDays.map((day) => (
                    <div
                      key={day.sessionId}
                      className="p-3 rounded-lg bg-[#383a40] border border-black/20"
                    >
                      <div className="flex items-start justify-between mb-2">
                        <div className="flex-1">
                          <div className="text-sm text-white font-medium mb-1">
                            {formatSessionDate(day.sessionDate)}
                          </div>
                          <div className="text-xs text-gray-300">
                            {day.messageCount} message{day.messageCount !== 1 ? 's' : ''}
                          </div>
                          {day.summary && (
                            <div className="text-xs text-gray-400 mt-1 line-clamp-2">
                              {day.summary}
                            </div>
                          )}
                        </div>
                        <button
                          onClick={async () => {
                            if (confirm(`Delete history for ${formatSessionDate(day.sessionDate)}?`)) {
                              try {
                                // Add 1 day to the date so we delete this day and everything before it
                                const targetDate = new Date(day.sessionDate + 'T00:00:00');
                                targetDate.setDate(targetDate.getDate() + 1);
                                const beforeDate = targetDate.toISOString().split('T')[0];

                                await invoke('delete_agent_history', {
                                  agentId: selectedAgent?.id,
                                  beforeDate: beforeDate
                                });
                                if (selectedAgent) {
                                  await loadTodaySession(selectedAgent.id);
                                }
                              } catch (err) {
                                console.error('Failed to delete day:', err);
                                alert('Failed to delete: ' + (err as Error).message);
                              }
                            }
                          }}
                          className="p-1.5 text-gray-400 hover:text-red-400 transition-colors rounded hover:bg-white/5"
                          aria-label={`Delete ${formatSessionDate(day.sessionDate)}`}
                        >
                          <Trash2 className="size-4" />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </>
            )}
          </div>
        </div>
      )}

      {/* Generative Canvas */}
      {canvasOpen && (
        <GenerativeCanvas
          content={canvasContent}
          isOpen={canvasOpen}
          onClose={() => {
            setCanvasOpen(false);
            setCanvasContent(null);
          }}
          onFormSubmit={async (data) => {
            // Mark the request_input tool as completed with checkmark
            if (pendingRequestInputToolId) {
              handleThinkingEvent({
                type: "tool_result",
                toolId: pendingRequestInputToolId,
                toolName: "request_input",
                result: JSON.stringify(data),
                timestamp: Date.now()
              });
              setPendingRequestInputToolId(null);
            }

            // Close the canvas
            setCanvasOpen(false);
            setCanvasContent(null);

            // Execute agent directly with form data (no user message shown)
            await executeAgentWithMessage(JSON.stringify(data, null, 2), false);
          }}
          onCanvasCancel={() => {
            // Focus the input when canvas is cancelled
            inputRef.current?.focus();
          }}
          conversationId={currentSession?.id}
        />
      )}
    </div>
  );
}
