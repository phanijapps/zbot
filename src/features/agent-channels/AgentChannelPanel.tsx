// ============================================================================
// AGENT CHANNELS FEATURE
// Discord-like agent interface with daily sessions
// ============================================================================

import { useState, useEffect, useRef, useCallback, startTransition } from "react";
import { MessageSquare, Bot, Loader2, Paperclip, Send, History, Hash, Trash2, X, ChevronDown, ChevronRight, Network, Mic, FileText, GitBranch } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cn } from "@/shared/utils";

// Conditional logging - only log in development mode
const isDev = import.meta.env.DEV;
const debugLog = (...args: unknown[]) => {
  if (isDev) {
    console.log("[AgentChannelPanel]", ...args);
  }
};
import { Textarea } from "@/shared/ui/textarea";
import {
  AgentChannelList,
  useStreamEvents,
  GenerativeCanvas,
  InlineToolCallsList,
  type MessageWithThinking,
} from "@/domains/agent-runtime/components";
import { ClearHistoryDialog } from "./ClearHistoryDialog";
import { DaySeparator } from "./DaySeparator";
import { KnowledgeGraphVisualizer } from "./KnowledgeGraphVisualizer";
import { VoiceRecordingDialog } from "./VoiceRecordingDialog";
import { TranscriptCommentDialog, type TranscriptAttachmentInfo } from "./TranscriptCommentDialog";
import { useNavigate, useLocation } from "react-router-dom";
import { AttachmentsPanel, type Attachment } from "./AttachmentsPanel";
import type { Agent, DailySession, DaySummary, SessionMessage } from "@/shared/types";
import {
  getOrCreateTodaySession,
  listPreviousDays,
  loadSessionMessages,
  formatSessionDate,
} from "@/services/agentChannels";
import { listAgents } from "@/services/agent";
import { useVaults } from "@/features/vaults/useVaults";

/**
 * Messages grouped by session date
 */
interface DayMessages {
  sessionId: string;
  sessionDate: string;
  formattedDate: string;
  summary?: string;
  messageCount: number;
  messages: MessageWithThinking[];
}

type ExecutionStage = "idle" | "thinking" | "using_tools" | "generating" | "done";

export function AgentChannelPanel() {
  const { currentVault } = useVaults();
  const navigate = useNavigate();
  const location = useLocation();

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
  const currentUnlistenRef = useRef<(() => void) | null>(null); // Track current event listener for cleanup

  // Execution stage for better UX
  const [executionStage, setExecutionStage] = useState<ExecutionStage>("idle");
  const [activeToolName, setActiveToolName] = useState<string | null>(null);

  // Day Messages - stores messages grouped by session date
  // Structure: array of DayMessages, ordered by date (newest first)
  const [loadedDays, setLoadedDays] = useState<DayMessages[]>([]);

  // Track which days are expanded (by session ID)
  const [expandedDays, setExpandedDays] = useState<Set<string>>(new Set());

  // Track which days are currently loading
  const [loadingDays, setLoadingDays] = useState<Set<string>>(new Set());


  // Generative Canvas state
  const [canvasOpen, setCanvasOpen] = useState(false);
  const [canvasContent, setCanvasContent] = useState<{
    type: "request_input" | "show_content";
    event: any;
  } | null>(null);

  // History Panel state
  const [historyPanelOpen, setHistoryPanelOpen] = useState(false);

  // Knowledge Graph Panel state
  const [knowledgeGraphOpen, setKnowledgeGraphOpen] = useState(false);

  // Clear History Dialog state
  const [showClearHistoryDialog, setShowClearHistoryDialog] = useState(false);

  // Voice Recording state
  const [voiceRecordingOpen, setVoiceRecordingOpen] = useState(false);

  // Transcript Comment Dialog state
  const [commentDialogOpen, setCommentDialogOpen] = useState(false);
  const [pendingTranscript, setPendingTranscript] = useState<TranscriptAttachmentInfo | null>(null);
  const [sendingTranscript, setSendingTranscript] = useState(false);

  // Attachments Panel state
  const [attachmentsPanelOpen, setAttachmentsPanelOpen] = useState(false);
  const [attachments, setAttachments] = useState<Attachment[]>([]);

  // Vault Switcher state - toggled by chevron in AgentChannelList
  const [showVaultSwitcher, setShowVaultSwitcher] = useState(false);

  // Track the current request_input tool ID for marking it as completed when form is submitted
  const [pendingRequestInputToolId, setPendingRequestInputToolId] = useState<string | null>(null);

  // Stream events handling
  const {
    state: _thinkingState,
    handleEvent: handleThinkingEvent,
    reset: resetThinking,
    setCurrentMessage,
  } = useStreamEvents(true, false);

  // Load agents on mount and when vault changes
  useEffect(() => {
    isMountedRef.current = true;
    loadAgents();

    return () => {
      isMountedRef.current = false;
      // Clean up any dangling event listener on unmount
      if (currentUnlistenRef.current) {
        debugLog("Cleaning up event listener on unmount");
        currentUnlistenRef.current();
        currentUnlistenRef.current = null;
      }
    };
  }, [currentVault?.id]); // Reload when vault changes

  // Restore selected agent from navigation state (e.g., returning from workflow IDE)
  useEffect(() => {
    const state = location.state as { restoreAgentId?: string } | null;
    const restoreAgentId = state?.restoreAgentId;

    if (restoreAgentId && agents.length > 0) {
      const agentToRestore = agents.find(a => a.id === restoreAgentId);
      if (agentToRestore && selectedAgent?.id !== restoreAgentId) {
        debugLog(`Restoring selected agent: ${restoreAgentId}`);
        setSelectedAgent(agentToRestore);
        // Clear the state so we don't restore again unnecessarily
        window.history.replaceState({}, '', location.pathname);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [location.state, agents]); // Remove selectedAgent from deps to prevent re-running when user clicks a different agent

  // Reset state when vault changes
  useEffect(() => {
    // Clear selected agent, session, and messages when vault changes
    setSelectedAgent(null);
    setCurrentSession(null);
    setMessages([]);
    setPreviousDays([]);
    setLoadedDays([]);
    setExpandedDays(new Set());
    debugLog("Vault changed, state reset");
  }, [currentVault?.id]); // Reset when vault changes

  // Load session when agent is selected
  useEffect(() => {
    if (selectedAgent) {
      loadTodaySession(selectedAgent.id);
    } else {
      setCurrentSession(null);
      setMessages([]);
      setPreviousDays([]);
      setLoadedDays([]);
      setExpandedDays(new Set());
    }
    resetThinking();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedAgent?.id, resetThinking]);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    if (messages.length > 0) {
      const timer = setTimeout(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: "auto" });
      }, 50);
      return () => clearTimeout(timer);
    }
  }, [messages]);

  /**
   * Scroll to bottom of messages
   */
  const scrollToBottom = useCallback(() => {
    // Note: Not adding cleanup here since this is called multiple times
    // and the timeout is short (50ms). The useEffect above has proper cleanup.
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
      // Parse tool calls from the database format
      let toolCalls: { toolCount: number; toolCalls?: any[] } | undefined;

      if (msg.toolCalls && Object.keys(msg.toolCalls).length > 0) {
        try {
          const toolCallsArray = Array.isArray(msg.toolCalls)
            ? msg.toolCalls as any[]
            : Object.values(msg.toolCalls);

          // Convert toolResults array to a map for easy lookup by tool_call_id
          const toolResultsMap = new Map<string, string>();
          if (Array.isArray(msg.toolResults)) {
            for (const tr of msg.toolResults) {
              if (tr.tool_call_id && tr.output) {
                toolResultsMap.set(tr.tool_call_id, tr.output);
              }
            }
          }

          toolCalls = {
            toolCount: toolCallsArray.length,
            toolCalls: toolCallsArray.map((tc: any) => {
              // Ensure id is a string
              const id = tc.id || tc.tool_call_id || `tool-${Date.now()}-${Math.random()}`;
              // Find matching result from the map
              const result = toolResultsMap.get(id || tc.tool_call_id);
              
              return {
                id: String(id), // Ensure id is always a string
                name: tc.name || tc.function?.name || 'unknown',
                status: 'completed' as "completed" | "failed",
                result: result,
              };
            })
          };
        } catch (e) {
          console.warn('[AgentChannelPanel] Failed to parse tool calls:', e);
          toolCalls = { toolCount: Object.keys(msg.toolCalls).length };
        }
      }

      return {
        id: msg.id,
        conversationId: msg.sessionId,
        role: msg.role as "user" | "assistant" | "system",
        content: msg.content,
        timestamp: new Date(msg.createdAt).getTime(),
        // Only include thinking if there are actual tool calls
        ...(toolCalls ? { thinking: toolCalls } : {}),
      };
    });
  }, []);

  /**
   * Load all agents
   * Note: agent-creator is filtered out as it's only accessible via the + button
   */
  const loadAgents = useCallback(async () => {
    try {
      const agentList = await listAgents();
      // Filter out agent-creator from the list - it's only accessible via the + button
      setAgents(agentList.filter(agent => agent.id !== "agent-creator"));
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
      const convertedMessages = convertSessionMessagesToWithThinking(sessionMessages);
      setMessages(convertedMessages);

      // Initialize loadedDays with today's session
      const todayDate = new Date().toISOString().split('T')[0];
      setLoadedDays([{
        sessionId: session.id,
        sessionDate: todayDate,
        formattedDate: 'Today',
        messageCount: convertedMessages.length,
        messages: convertedMessages,
      }]);

      // Expand today by default
      setExpandedDays(new Set([session.id]));

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
   * Toggle a day's expanded state
   */
  const toggleDayExpansion = useCallback((sessionId: string) => {
    setExpandedDays((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(sessionId)) {
        newSet.delete(sessionId);
      } else {
        newSet.add(sessionId);
      }
      return newSet;
    });
  }, []);

  /**
   * Load a historical day's messages and expand it
   */
  const loadHistoricalDay = useCallback(async (daySummary: DaySummary) => {
    if (!selectedAgent || loadingDays.has(daySummary.sessionId)) {
      return;
    }

    // Check if already loaded
    if (loadedDays.find(d => d.sessionId === daySummary.sessionId)) {
      // Just expand it
      toggleDayExpansion(daySummary.sessionId);
      return;
    }

    // Mark as loading
    setLoadingDays((prev) => new Set(prev).add(daySummary.sessionId));

    try {
      // Load messages for this day
      const sessionMessages = await loadSessionMessages(daySummary.sessionId);
      const convertedMessages = convertSessionMessagesToWithThinking(sessionMessages);

      // Add to loadedDays (maintain order: today first, then other days by date desc)
      setLoadedDays((prev) => {
        const newDay: DayMessages = {
          sessionId: daySummary.sessionId,
          sessionDate: daySummary.sessionDate,
          formattedDate: formatSessionDate(daySummary.sessionDate),
          summary: daySummary.summary,
          messageCount: convertedMessages.length,
          messages: convertedMessages,
        };

        // Insert after today's session, before other historical days
        const todayIndex = prev.findIndex(d => d.sessionId === currentSession?.id);
        if (todayIndex === -1) {
          // Shouldn't happen, but handle it
          return [...prev, newDay];
        }

        // Insert after today
        const newDays = [...prev];
        newDays.splice(todayIndex + 1, 0, newDay);
        return newDays;
      });

      // Expand the newly loaded day
      setExpandedDays((prev) => new Set(prev).add(daySummary.sessionId));

      // Close the history panel after loading
      setHistoryPanelOpen(false);
    } catch (err) {
      console.error("Failed to load historical day:", err);
      alert('Failed to load messages: ' + (err as Error).message);
    } finally {
      setLoadingDays((prev) => {
        const newSet = new Set(prev);
        newSet.delete(daySummary.sessionId);
        return newSet;
      });
    }
  }, [selectedAgent, loadingDays, loadedDays, currentSession, toggleDayExpansion, convertSessionMessagesToWithThinking]);

  /**
   * Execute agent with a message
   * User message should already be added to state before calling this.
   * @param message - The message to send to the agent
   */
  const executeAgentWithMessage = async (message: string) => {
    if (!currentSession || !selectedAgent) return;

    // Clean up any previous event listener before starting a new execution
    if (currentUnlistenRef.current) {
      debugLog("Cleaning up previous event listener");
      currentUnlistenRef.current();
      currentUnlistenRef.current = null;
    }

    // Wait for any ongoing execution to complete before starting a new one
    // Add a timeout to prevent infinite waiting
    const maxWaitTime = 5000; // 5 seconds max wait
    const waitStart = Date.now();
    while (isExecutingRef.current) {
      if (Date.now() - waitStart > maxWaitTime) {
        console.warn("[AgentChannelPanel] Previous execution timed out, forcing cleanup and starting new execution");
        isExecutingRef.current = false; // Force reset
        break;
      }
      debugLog("Waiting for previous execution to complete...");
      await new Promise(resolve => setTimeout(resolve, 100));
    }

    isExecutingRef.current = true;
    setIsLoading(true);
    setExecutionStage("thinking");

    // Close history sidebar when sending a new message
    setHistoryPanelOpen(false);

    // Create assistant message placeholder for streaming response
    const assistantMessageId = crypto.randomUUID();
    const initialAssistantMessage: MessageWithThinking = {
      id: assistantMessageId,
      conversationId: currentSession.id,
      role: "assistant",
      content: "",
      timestamp: Date.now(),
      // Don't set thinking initially - only add when tools are actually used
    };
    setMessages((prev) => [...prev, initialAssistantMessage]);
    // Also add to loadedDays
    setLoadedDays((prev) => prev.map((day) =>
      day.sessionId === currentSession.id
        ? { ...day, messages: [...day.messages, initialAssistantMessage], messageCount: day.messages.length + 1 }
        : day
    ));

    // Set current message for thinking events
    setCurrentMessage(assistantMessageId);

    // Listen for streaming events from the backend
    const eventChannel = `agent-stream://${currentSession.id}`;
    debugLog("Setting up event listener for channel:", eventChannel);
    let toolCallCount = 0; // Track tool calls locally during streaming
    const toolCallsData: Array<{ id: string; name: string; status: "running" | "completed" | "failed"; result?: string }> = []; // Track actual tool call data

    const finishProcessing = () => {
      // Only update state if component is still mounted
      if (!isMountedRef.current) return;

      isExecutingRef.current = false;
      setIsLoading(false);
      setExecutionStage("done");
      setActiveToolName(null);

      handleThinkingEvent({
        type: "done",
        finalMessage: "",
        tokenCount: 0,
        timestamp: Date.now()
      });

      scrollToBottom();

      // Clean up event listener
      if (currentUnlistenRef.current) {
        currentUnlistenRef.current();
        currentUnlistenRef.current = null;
      }
    };

    const unlistenPromise = listen(eventChannel, (event) => {
      // Don't process events if component is unmounted
      if (!isMountedRef.current) return;

      const data = event.payload as { type: string; content?: string; finalMessage?: string; error?: string; toolName?: string; toolId?: string; args?: string; result?: string; formId?: string; title?: string; description?: string; schema?: any; submitButton?: string };
      if (!data) return;

      // Map simplified events to AgentStreamEvent format and pass to thinking panel
      switch (data.type) {
        case "token":
          setExecutionStage("generating");
          handleThinkingEvent({
            type: "token",
            content: data.content || "",
            timestamp: Date.now()
          });
          // Batch state updates using startTransition for better performance
          startTransition(() => {
            // Update both messages and loadedDays
            setMessages((prev) => prev.map((msg) =>
              msg.id === assistantMessageId
                ? { ...msg, content: msg.content + (data.content || "") }
                : msg
            ));
            setLoadedDays((prev) => prev.map((day) =>
              day.sessionId === currentSession?.id
                ? {
                    ...day,
                    messages: day.messages.map((msg) =>
                      msg.id === assistantMessageId
                        ? { ...msg, content: msg.content + (data.content || "") }
                        : msg
                    ),
                  }
                : day
            ));
          });
          break;

        case "tool_call_start":
          setExecutionStage("using_tools");
          setActiveToolName(data.toolName || null);
          toolCallCount++; // Increment local tool counter
          // Store tool call data
          const toolId = data.toolId || `tool-${Date.now()}`;
          toolCallsData.push({
            id: toolId,
            name: data.toolName || "unknown",
            status: "running",
          });
          handleThinkingEvent({
            type: "tool_call_start",
            toolId: toolId,
            toolName: data.toolName || "unknown",
            timestamp: Date.now()
          });
          break;

        case "tool_result":
          setExecutionStage("generating");
          setActiveToolName(null);
          // Update the tool call with result
          const toolIdx = toolCallsData.findIndex(tc => tc.id === data.toolId);
          if (toolIdx !== -1) {
            toolCallsData[toolIdx] = {
              ...toolCallsData[toolIdx],
              status: data.error ? "failed" : "completed",
              result: data.result,
            };
          }
          handleThinkingEvent({
            type: "tool_result",
            toolId: data.toolId || `tool-${Date.now()}`,
            toolName: data.toolName || "unknown",
            result: data.result || "",
            error: data.error,
            timestamp: Date.now()
          });
          // Batch state updates using startTransition for better performance
          startTransition(() => {
            // Update tool count in real-time as tools complete
            const updatedMsg = { toolCount: toolCallCount, toolCalls: [...toolCallsData] };
            setMessages((prev) => prev.map((msg) =>
              msg.id === assistantMessageId
                ? { ...msg, thinking: updatedMsg }
                : msg
            ));
            setLoadedDays((prev) => prev.map((day) =>
              day.sessionId === currentSession?.id
                ? {
                    ...day,
                    messages: day.messages.map((msg) =>
                      msg.id === assistantMessageId
                        ? { ...msg, thinking: updatedMsg }
                        : msg
                    ),
                  }
                : day
            ));
          });
          break;

        case "done":
          handleThinkingEvent({
            type: "done",
            finalMessage: data.finalMessage || "",
            tokenCount: 0,
            timestamp: Date.now()
          });
          // Batch state updates using startTransition for better performance
          startTransition(() => {
            // Update with final content and tool data
            const finalMsg = {
              content: data.finalMessage || "",
              ...(toolCallCount > 0 ? { thinking: { toolCount: toolCallCount, toolCalls: [...toolCallsData] } } : {})
            };
            setMessages((prev) => prev.map((msg) =>
              msg.id === assistantMessageId
                ? { ...msg, ...finalMsg }
                : msg
            ));
            setLoadedDays((prev) => prev.map((day) =>
              day.sessionId === currentSession?.id
                ? {
                    ...day,
                    messages: day.messages.map((msg) =>
                      msg.id === assistantMessageId
                        ? { ...msg, ...finalMsg }
                        : msg
                    ),
                  }
                : day
            ));
          });
          finishProcessing();
          break;

        case "request_input":
          // Open form for user input via GenerativeCanvas
          // Track the toolId so we can mark it as completed when form is submitted
          setPendingRequestInputToolId(data.toolId || null);
          handleThinkingEvent({
            type: "request_input",
            formId: data.formId || `form-${Date.now()}`,
            formType: "json_schema",
            title: data.title || "Input Required",
            description: data.description || "",
            schema: data.schema || {},
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

        case "show_content": {
          // Show content in GenerativeCanvas (HTML, PDF, image, etc.)
          const showContentData = data as {
            contentType?: string;
            content?: string;
            filePath?: string;
            base64?: boolean;
            title?: string;
            isAttachment?: boolean;
          };
          handleThinkingEvent({
            type: "show_content",
            contentType: showContentData.contentType || "text",
            title: showContentData.title || "Content",
            timestamp: Date.now()
          } as any);
          setCanvasContent({
            type: "show_content",
            event: {
              contentType: showContentData.contentType || "text",
              content: showContentData.content || "",
              filePath: showContentData.filePath,
              base64: showContentData.base64,
              title: showContentData.title || "Content",
              isAttachment: showContentData.isAttachment || false,
            }
          });
          setCanvasOpen(true);
          break;
        }

        case "error":
          debugLog("ERROR event received");
          handleThinkingEvent({
            type: "error",
            error: data.error || "Unknown error",
            recoverable: false,
            timestamp: Date.now()
          });
          const errorMsg = `Error: ${data.error || "Unknown error"}`;
          // Batch state updates using startTransition for better performance
          startTransition(() => {
            setMessages((prev) => prev.map((msg) =>
              msg.id === assistantMessageId && msg.content === ""
                ? { ...msg, content: errorMsg }
                : msg
            ));
            setLoadedDays((prev) => prev.map((day) =>
              day.sessionId === currentSession?.id
                ? {
                    ...day,
                    messages: day.messages.map((msg) =>
                      msg.id === assistantMessageId && msg.content === ""
                        ? { ...msg, content: errorMsg }
                        : msg
                    ),
                  }
                : day
            ));
          });
          finishProcessing();
          break;
      }
    });

    currentUnlistenRef.current = await unlistenPromise;

    try {
      await invoke("execute_agent_stream", {
        agentId: selectedAgent.id,
        message: message,
      });
      // Fallback: if invoke completes but isExecuting is still true, call finishProcessing
      // This handles the case where backend doesn't send a done event
      if (isMountedRef.current && isExecutingRef.current) {
        finishProcessing();
      }
    } catch (error) {
      if (isMountedRef.current) {
        const errorMsg = `Error: ${error instanceof Error ? error.message : String(error)}`;
        // Batch state updates using startTransition for better performance
        startTransition(() => {
          setMessages((prev) => prev.map((msg) =>
            msg.id === assistantMessageId && msg.content === ""
              ? { ...msg, content: errorMsg }
              : msg
          ));
          setLoadedDays((prev) => prev.map((day) =>
            day.sessionId === currentSession?.id
              ? {
                  ...day,
                  messages: day.messages.map((msg) =>
                    msg.id === assistantMessageId && msg.content === ""
                      ? { ...msg, content: errorMsg }
                      : msg
                  ),
                }
              : day
          ));
        });
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

    // Show loading spinner immediately
    setIsLoading(true);
    setExecutionStage("thinking");

    // Add user message immediately - React will render it
    const tempUserMessage: MessageWithThinking = {
      id: crypto.randomUUID(),
      conversationId: currentSession.id,
      role: "user",
      content: userMessage,
      timestamp: Date.now(),
    };
    setMessages((prev) => [...prev, tempUserMessage]);
    // Also add to loadedDays
    setLoadedDays((prev) => prev.map((day) =>
      day.sessionId === currentSession.id
        ? { ...day, messages: [...day.messages, tempUserMessage], messageCount: day.messages.length + 1 }
        : day
    ));

    // Then call backend
    await executeAgentWithMessage(userMessage);
  };

  /**
   * Handle transcript complete - open comment dialog for user to add context
   */
  const handleTranscriptComplete = async (filename: string) => {
    if (!selectedAgent || !currentSession) return;

    try {
      const info = await invoke<TranscriptAttachmentInfo>("get_transcript_attachment_info", {
        agentId: selectedAgent.id,
        filename,
      });

      debugLog("Transcript complete:", info);

      // Set pending transcript and open comment dialog
      setPendingTranscript(info);
      setCommentDialogOpen(true);

    } catch (err) {
      console.error("Failed to get transcript info:", err);
    }
  };

  /**
   * Handle sending transcript with comments to agent
   */
  const handleSendTranscript = async (comments: string) => {
    if (!pendingTranscript || !selectedAgent || !currentSession) return;

    setSendingTranscript(true);

    try {
      // Create message content with file path for agent to scan
      const messageContent = `TRANSCRIPT_FILE: ${pendingTranscript.file_path}\nUSER_COMMENTS: ${comments}\n\nPlease analyze this transcript, extract entities to the knowledge graph, provide a summary, and suggest actions.`;

      // Create message object for UI
      const transcriptMessage: MessageWithThinking = {
        id: crypto.randomUUID(),
        conversationId: currentSession.id,
        role: "user",
        content: messageContent,
        timestamp: Date.now(),
      };

      // Add message to UI
      setMessages((prev) => [...prev, transcriptMessage]);
      setLoadedDays((prev) => prev.map((day) =>
        day.sessionId === currentSession.id
          ? { ...day, messages: [...day.messages, transcriptMessage], messageCount: day.messages.length + 1 }
          : day
      ));

      // Add to attachments
      const newAttachment: Attachment = {
        id: crypto.randomUUID(),
        type: "transcript",
        filename: pendingTranscript.filename,
        filePath: pendingTranscript.file_path,
        createdAt: Date.now(),
        metadata: {
          duration: pendingTranscript.duration_seconds,
          speakerCount: pendingTranscript.speaker_count,
        },
      };
      setAttachments((prev) => [...prev, newAttachment]);

      // Execute agent with this message (fire and forget - dialog closes immediately)
      executeAgentWithMessage(messageContent).catch((err) => {
        console.error("Agent execution failed:", err);
      });

      // Close dialog and clear pending transcript immediately
      setCommentDialogOpen(false);
      setPendingTranscript(null);

    } catch (err) {
      console.error("Failed to send transcript:", err);
      // Still close dialog even on error
      setCommentDialogOpen(false);
      setPendingTranscript(null);
    } finally {
      setSendingTranscript(false);
    }
  };

  /**
   * Handle attachment actions
   */
  const handleViewAttachment = async (attachment: Attachment) => {
    // TODO: Open transcript viewer dialog
    debugLog("View attachment:", attachment);
  };

  const handleSendAttachment = async (attachment: Attachment) => {
    if (!selectedAgent || !currentSession) return;

    try {
      const messageContent = `TRANSCRIPT_FILE: ${attachment.filePath}\n\nPlease analyze this transcript, extract entities to the knowledge graph, provide a summary, and suggest actions.`;

      const message: MessageWithThinking = {
        id: crypto.randomUUID(),
        conversationId: currentSession.id,
        role: "user",
        content: messageContent,
        timestamp: Date.now(),
      };

      setMessages((prev) => [...prev, message]);
      setLoadedDays((prev) => prev.map((day) =>
        day.sessionId === currentSession.id
          ? { ...day, messages: [...day.messages, message], messageCount: day.messages.length + 1 }
          : day
      ));

      await executeAgentWithMessage(messageContent);
    } catch (err) {
      console.error("Failed to send attachment:", err);
    }
  };

  const handleDeleteAttachment = async (attachmentId: string) => {
    // TODO: Implement delete
    debugLog("Delete attachment:", attachmentId);
    setAttachments((prev) => prev.filter((a) => a.id !== attachmentId));
  };

  return (
    <div className="flex h-full bg-background">
      {/* Sidebar - Agent Channels */}
      <AgentChannelList
        agents={agents}
        selectedAgentId={selectedAgent?.id}
        onSelectAgent={setSelectedAgent}
        onToggleVault={() => setShowVaultSwitcher(!showVaultSwitcher)}
        showVaultSwitcher={showVaultSwitcher}
        vaultName={currentVault?.name}
      />

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0">
        {selectedAgent && currentSession ? (
          <>
            {/* Header */}
            <div className="h-12 border-b border-border flex items-center justify-between px-4 shrink-0">
              <div className="flex items-center gap-2 group">
                <Hash className="size-5 text-muted-foreground" />
                <h2 className="text-foreground font-semibold cursor-default" title={`${loadedDays.reduce((sum, d) => sum + d.messageCount, 0)} total messages across ${loadedDays.length} day${loadedDays.length !== 1 ? 's' : ''}`}>
                  {selectedAgent.displayName}
                </h2>
                <span className="text-xs text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity">
                  {loadedDays.reduce((sum, d) => sum + d.messageCount, 0)}
                </span>
              </div>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => setHistoryPanelOpen(true)}
                  className="p-2 text-muted-foreground hover:text-foreground transition-colors rounded hover:bg-accent"
                  aria-label="Show history"
                >
                  <History className="size-5" />
                </button>
                <button
                  onClick={() => setKnowledgeGraphOpen(true)}
                  className="p-2 text-muted-foreground hover:text-foreground transition-colors rounded hover:bg-accent"
                  aria-label="Show knowledge graph"
                >
                  <Network className="size-5" />
                </button>
                <button
                  onClick={() => setAttachmentsPanelOpen(true)}
                  className="p-2 text-muted-foreground hover:text-foreground transition-colors rounded hover:bg-accent"
                  aria-label="Show attachments"
                >
                  <FileText className="size-5" />
                </button>
                {/* Only show voice recording if enabled in agent config */}
                {selectedAgent.voiceRecordingEnabled !== false && (
                  <button
                    onClick={() => setVoiceRecordingOpen(true)}
                    disabled={isLoading}
                    className="p-2 text-muted-foreground hover:text-foreground transition-colors rounded hover:bg-accent disabled:opacity-50"
                    aria-label="Record voice note"
                  >
                    <Mic className="size-5" />
                  </button>
                )}
                <button
                  onClick={() => selectedAgent && navigate(`/workflow/${selectedAgent.id}`, { state: { from: '/', restoreAgentId: selectedAgent.id } })}
                  disabled={!selectedAgent}
                  className="p-2 text-muted-foreground hover:text-foreground transition-colors rounded hover:bg-accent disabled:text-muted-foreground/50 disabled:hover:bg-transparent disabled:cursor-not-allowed"
                  aria-label="Edit workflow"
                >
                  <GitBranch className="size-5" />
                </button>
              </div>
            </div>

            {/* Messages Area */}
            <div className="flex-1 overflow-y-auto">
              {loadedDays.length === 0 || (loadedDays.length === 1 && loadedDays[0].messages.length === 0) ? (
                isLoading ? (
                  <div className="flex items-center justify-center h-full">
                    <Loader2 className="size-6 text-violet-400 animate-spin" />
                  </div>
                ) : (
                  <div className="flex flex-col items-center justify-center h-full px-6 text-center">
                    <div className="w-14 h-14 rounded-xl bg-gradient-to-br from-violet-600/20 to-purple-700/20 flex items-center justify-center mb-4 border border-border">
                      <MessageSquare className="size-7 text-violet-400" />
                    </div>
                    <h3 className="text-base font-semibold text-foreground mb-2">
                      Today's session
                    </h3>
                    <p className="text-sm text-muted-foreground max-w-xs">
                      Start a conversation with {selectedAgent.displayName}. Messages are saved to today's session.
                    </p>
                  </div>
                )
              ) : (
                <div className="px-4 py-6">
                  <div className="space-y-2">
                    {loadedDays.map((day) => {
                      const isExpanded = expandedDays.has(day.sessionId);
                      const isLoading = loadingDays.has(day.sessionId);

                      return (
                        <div key={day.sessionId}>
                          {/* Day Separator */}
                          <DaySeparator
                            date={day.formattedDate}
                            messageCount={day.messageCount}
                            isExpanded={isExpanded}
                            onToggle={() => toggleDayExpansion(day.sessionId)}
                            summary={day.summary}
                          />

                          {/* Messages for this day */}
                          {isExpanded && (
                            <div className="space-y-4 mt-2">
                              {isLoading ? (
                                <div className="flex items-center gap-2 text-muted-foreground text-sm py-4">
                                  <Loader2 className="size-4 animate-spin" />
                                  <span>Loading messages...</span>
                                </div>
                              ) : (
                                <>
                                  {day.messages.map((msg) => (
                                    <div
                                      key={msg.id}
                                      className={cn(
                                        "group -mx-4 px-4 py-0.5",
                                        msg.role === 'user'
                                          ? 'bg-card hover:bg-accent'
                                          : 'bg-transparent hover:bg-accent/50'
                                      )}
                                    >
                                      {/* User message */}
                                      {msg.role === 'user' ? (
                                        <div className="flex gap-4">
                                          <div className="size-10 rounded-full bg-gradient-to-br from-violet-600 to-purple-700 flex items-center justify-center shrink-0 text-white font-semibold">
                                            U
                                          </div>
                                          <div className="flex-1 min-w-0">
                                            <div className="flex items-baseline gap-2 mb-1">
                                              <span className="font-semibold text-foreground">You</span>
                                            </div>
                                            <p className="text-foreground/90 text-[15px] leading-relaxed whitespace-pre-wrap break-words">
                                              {msg.content}
                                            </p>
                                          </div>
                                        </div>
                                      ) : (
                                        /* Assistant message with optional tool cards */
                                        <>
                                          {/* Inline tool cards - displayed before the message */}
                                          {msg.thinking?.toolCalls && msg.thinking.toolCalls.length > 0 && (
                                            <div className="ml-14 mb-2">
                                              <InlineToolCallsList
                                                tools={msg.thinking.toolCalls.map(tc => ({
                                                  id: tc.id,
                                                  name: tc.name,
                                                  status: tc.status,
                                                  result: tc.result,
                                                  error: tc.error,
                                                }))}
                                              />
                                            </div>
                                          )}
                                          {/* Assistant message content */}
                                          <div className="flex gap-4">
                                            <div className="size-10 rounded-full bg-gradient-to-br from-violet-600 to-purple-700 flex items-center justify-center shrink-0 text-white font-semibold">
                                              AI
                                            </div>
                                            <div className="flex-1 min-w-0">
                                              <div className="flex items-baseline gap-2 mb-1">
                                                <span className="font-semibold text-foreground">{selectedAgent.displayName}</span>
                                              </div>
                                              <p className="text-foreground/90 text-[15px] leading-relaxed whitespace-pre-wrap break-words">
                                                {msg.content}
                                              </p>
                                            </div>
                                          </div>
                                        </>
                                      )}
                                    </div>
                                  ))}
                                </>
                              )}
                            </div>
                          )}
                        </div>
                      );
                    })}
                    {/* Show loading indicator for today's session */}
                    {isLoading && expandedDays.has(currentSession?.id || '') && (
                      <div className="flex items-center gap-2 text-muted-foreground text-sm py-4">
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
              <div className="relative bg-input rounded-lg">
                <div className="flex items-start gap-3 p-3">
                  <button className="p-2 text-muted-foreground hover:text-foreground transition-colors rounded hover:bg-accent mt-1">
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
                    className="flex-1 min-h-[24px] max-h-[200px] bg-transparent border-0 text-foreground placeholder:text-muted-foreground resize-none focus-visible:ring-0 p-0"
                    rows={1}
                  />
                  <div className="flex items-center gap-1 mt-1">
                    <button
                      onClick={handleSendMessage}
                      disabled={!input.trim() || isLoading}
                      className={cn(
                        'p-1.5 rounded transition-colors',
                        input.trim() && !isLoading
                          ? 'text-foreground hover:bg-accent'
                          : 'text-muted-foreground cursor-not-allowed'
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
              <div className="w-16 h-16 rounded-xl bg-gradient-to-br from-violet-600/20 to-purple-700/20 flex items-center justify-center mx-auto mb-4 border border-border">
                <Bot className="size-8 text-violet-400" />
              </div>
              <h2 className="text-xl font-semibold text-foreground mb-2">
                Select an Agent Channel
              </h2>
              <p className="text-muted-foreground max-w-md mx-auto">
                Choose an agent from the sidebar to start your conversation. Each agent has its own daily session.
              </p>
            </div>
          </div>
        )}
      </div>

      {/* History Panel */}
      {historyPanelOpen && (
        <div className="fixed inset-y-0 right-0 w-80 bg-sidebar border-l border-border shadow-xl z-50 flex flex-col">
          {/* Header */}
          <div className="h-12 border-b border-border flex items-center justify-between px-4 shrink-0">
            <div className="flex items-center gap-2">
              <History className="size-5 text-muted-foreground" />
              <h2 className="text-foreground font-semibold">History</h2>
            </div>
            <button
              onClick={() => setHistoryPanelOpen(false)}
              className="p-2 text-muted-foreground hover:text-foreground transition-colors rounded hover:bg-accent"
              aria-label="Close history"
            >
              <X className="size-5" />
            </button>
          </div>

          {/* Content */}
          <div className="flex-1 overflow-y-auto p-4">
            {/* Clear Today Button (always shown if there's a session with messages) */}
            {currentSession && loadedDays.some(d => d.sessionId === currentSession.id && d.messages.length > 0) && (
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
                <p className="text-sm text-muted-foreground">No previous days found</p>
              </div>
            ) : (
              <>
                {/* Clear All Button */}
                <button
                  onClick={() => setShowClearHistoryDialog(true)}
                  className="w-full mb-3 px-3 py-2 bg-red-500/20 hover:bg-red-500/30 text-red-300 text-sm rounded-lg transition-colors flex items-center justify-center gap-2"
                >
                  <Trash2 className="size-4" />
                  Clear All History
                </button>

                {/* Days List */}
                <div className="space-y-2">
                  {previousDays.map((day) => {
                    const isLoaded = loadedDays.find(d => d.sessionId === day.sessionId);
                    const isExpanded = expandedDays.has(day.sessionId);
                    const isDayLoading = loadingDays.has(day.sessionId);

                    return (
                      <div
                        key={day.sessionId}
                        className={cn(
                          "p-3 rounded-lg border border-border transition-colors",
                          isLoaded ? 'bg-card' : 'bg-input hover:bg-card'
                        )}
                      >
                        <div className="flex items-start justify-between gap-2">
                          {/* Clickable area to load/expand day */}
                          <button
                            onClick={() => loadHistoricalDay(day)}
                            className="flex-1 text-left"
                            disabled={isDayLoading}
                          >
                            <div className="flex items-center gap-2 mb-1">
                              {isDayLoading ? (
                                <Loader2 className="size-3 text-muted-foreground animate-spin" />
                              ) : isLoaded && isExpanded ? (
                                <ChevronDown className="size-3 text-muted-foreground" />
                              ) : isLoaded ? (
                                <ChevronRight className="size-3 text-muted-foreground" />
                              ) : null}
                              <div className="text-sm text-foreground font-medium">
                                {formatSessionDate(day.sessionDate)}
                              </div>
                              {isLoaded && (
                                <span className="text-xs text-primary">• Loaded</span>
                              )}
                            </div>
                            <div className="text-xs text-muted-foreground">
                              {day.messageCount} message{day.messageCount !== 1 ? 's' : ''}
                            </div>
                            {day.summary && (
                              <div className="text-xs text-muted-foreground/70 mt-1 line-clamp-2">
                                {day.summary}
                              </div>
                            )}
                          </button>
                          {/* Delete button */}
                          <button
                            onClick={async (e) => {
                              e.stopPropagation();
                              if (confirm(`Delete history for ${formatSessionDate(day.sessionDate)}?`)) {
                                try {
                                  // Add 1 day to the date so we delete this day and everything before it
                                  const targetDate = new Date(day.sessionDate + 'T00:00:00');

                                  // Check if date is valid
                                  if (isNaN(targetDate.getTime())) {
                                    throw new Error(`Invalid date: ${day.sessionDate}`);
                                  }

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
                            className="p-1.5 text-muted-foreground hover:text-red-400 transition-colors rounded hover:bg-accent shrink-0"
                            aria-label={`Delete ${formatSessionDate(day.sessionDate)}`}
                          >
                            <Trash2 className="size-4" />
                          </button>
                        </div>
                      </div>
                    );
                  })}
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
            await executeAgentWithMessage(JSON.stringify(data, null, 2));
          }}
          onCanvasCancel={() => {
            // Focus the input when canvas is cancelled
            inputRef.current?.focus();
          }}
          conversationId={currentSession?.id}
        />
      )}

      {/* Clear History Dialog */}
      {selectedAgent && (
        <ClearHistoryDialog
          open={showClearHistoryDialog}
          onClose={() => setShowClearHistoryDialog(false)}
          agentId={selectedAgent.id}
          agentName={selectedAgent.displayName}
        />
      )}

      {/* Knowledge Graph Visualizer */}
      {knowledgeGraphOpen && selectedAgent && (
        <KnowledgeGraphVisualizer
          agentId={selectedAgent.id}
          agentName={selectedAgent.displayName}
          onClose={() => setKnowledgeGraphOpen(false)}
        />
      )}

      {/* Voice Recording Dialog */}
      {selectedAgent && (
        <VoiceRecordingDialog
          open={voiceRecordingOpen}
          onClose={() => setVoiceRecordingOpen(false)}
          agentId={selectedAgent.id}
          agentName={selectedAgent.displayName}
          onTranscriptComplete={handleTranscriptComplete}
        />
      )}

      {/* Transcript Comment Dialog */}
      {pendingTranscript && selectedAgent && (
        <TranscriptCommentDialog
          open={commentDialogOpen}
          transcript={pendingTranscript}
          agentName={selectedAgent.displayName}
          onSend={handleSendTranscript}
          onCancel={() => {
            setCommentDialogOpen(false);
            setPendingTranscript(null);
          }}
          loading={sendingTranscript}
        />
      )}

      {/* Attachments Panel */}
      {selectedAgent && (
        <AttachmentsPanel
          open={attachmentsPanelOpen}
          onClose={() => setAttachmentsPanelOpen(false)}
          attachments={attachments}
          onView={handleViewAttachment}
          onSend={handleSendAttachment}
          onDelete={handleDeleteAttachment}
        />
      )}
    </div>
  );
}
