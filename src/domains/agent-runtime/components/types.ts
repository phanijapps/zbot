/**
 * UI Types for Agent Runtime Components
 */

import type { AgentStreamEvent } from "@/shared/types/agent";

// ============================================================================
// PLAN / TODO LIST TYPES
// ============================================================================

/**
 * A plan item represents a step in the agent's execution plan
 * This is shown when the planning module is active
 */
export interface PlanItem {
  id: string;
  title: string;
  status: "pending" | "in_progress" | "completed" | "failed";
  order: number;
}

// ============================================================================
// TOOL CALL DISPLAY TYPES
// ============================================================================

/**
 * Minimal tool call info for display in the thinking panel
 */
export interface ToolCallDisplay {
  id: string;
  name: string;
  status: "pending" | "running" | "completed" | "failed";
  duration?: number; // in seconds
  result?: string; // truncated result for preview
  error?: string;
}

// ============================================================================
// THINKING PANEL STATE
// ============================================================================

/**
 * The thinking panel state derived from stream events
 */
export interface ThinkingPanelState {
  isOpen: boolean;
  isActive: boolean; // Agent currently working
  hasPlan: boolean;
  planItems: PlanItem[];
  toolCalls: ToolCallDisplay[];
  reasoning: string[]; // Accumulated reasoning blocks
  currentMessageId: string | null; // Message this panel is for
}

// ============================================================================
// THINKING TAB PROPS
// ============================================================================

export interface ThinkingTabProps {
  isActive: boolean;
  toolCount?: number;
  onClick: () => void;
}

// ============================================================================
// THINKING PANEL PROPS
// ============================================================================

export interface ThinkingPanelProps {
  isOpen: boolean;
  onClose: () => void;
  state: ThinkingPanelState;
}

// ============================================================================
// MESSAGE WITH THINKING
// ============================================================================

/**
 * A message that may have associated thinking/tool calls
 */
export interface MessageWithThinking {
  id: string;
  conversationId: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: number;
  // Thinking data (for completed messages)
  thinking?: {
    planItems?: PlanItem[];
    toolCalls?: ToolCallDisplay[];
    reasoning?: string[];
    toolCount: number;
  };
}

// ============================================================================
// CONVERSATION TYPES
// ============================================================================

/**
 * A conversation with its associated agent
 */
export interface ConversationWithAgent {
  id: string;
  title: string;
  agentId: string;
  agentName: string;
  agentIcon?: string; // emoji or icon identifier
  lastMessage?: string;
  lastMessageTime: number;
  messageCount: number;
  model?: string;
}

// ============================================================================
// STREAM EVENT HANDLER
// ============================================================================

/**
 * Hook return type for handling stream events
 */
export interface UseStreamEventsReturn {
  state: ThinkingPanelState;
  handleEvent: (event: AgentStreamEvent) => void;
  reset: () => void;
  setCurrentMessage: (messageId: string) => void;
  togglePanel: () => void;
  openPanel: () => void;
  closePanel: () => void;
}
