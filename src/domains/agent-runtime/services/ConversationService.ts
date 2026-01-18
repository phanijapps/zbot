/**
 * Conversation Service
 * Manages agent conversations using Rust backend
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Message,
  MessageHistory,
  ToolCall,
  ToolResult,
} from "@/shared/types/agent";
import { MessageRole } from "@/shared/types/agent";

// ============================================================================
// CONVERSATION SERVICE
// ============================================================================

export class ConversationService {
  /**
   * Load conversation history for an agent
   */
  async loadHistory(conversationId: string): Promise<MessageHistory> {
    const messages = await invoke<any[]>("list_messages", {
      conversationId,
    });

    // Convert to MessageHistory format
    const history: MessageHistory = {
      messages: messages.map((msg: any) => ({
        role: msg.role as "user" | "assistant" | "system",
        content: msg.content,
        tool_calls: msg.tool_calls,
        tool_results: msg.tool_results,
      })),
    };

    return history;
  }

  /**
   * Save a message to the conversation
   */
  async saveMessage(
    conversationId: string,
    role: MessageRole,
    content: string,
    toolCalls?: ToolCall[],
    toolResults?: ToolResult[],
    tokenCount?: number
  ): Promise<Message> {
    const message = await invoke<any>("create_message", {
      data: {
        id: `msg_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
        conversationId,
        role,
        content,
        toolCalls,
        toolResults,
        tokenCount: tokenCount || 0,
      },
    });

    return message;
  }

  /**
   * Execute agent using Rust backend with real-time streaming
   * The Rust backend emits events during execution via Tauri's event system
   */
  async executeAgentStream(
    conversationId: string,
    agentId: string,
    userMessage: string,
    onEvent: (event: any) => void
  ): Promise<void> {
    console.log("[ConversationService] executeAgentStream starting (Rust backend with real-time streaming)", {
      conversationId,
      agentId,
      userMessage,
    });

    // Set up event listener for real-time events from Rust backend
    const unlisten = await listen<any>(
      `agent-stream://${conversationId}`,
      (event) => {
        // Forward event to the caller's handler
        onEvent(event.payload);
      }
    );

    try {
      // Call Rust backend command - this will start execution and emit events
      await invoke<any>("execute_agent_zero_stream", {
        conversationId,
        agentId,
        message: userMessage,
      });

    } catch (error) {
      console.error("[ConversationService] Error:", error);
      // Emit error event
      onEvent({
        type: "error",
        timestamp: Date.now(),
        error: error instanceof Error ? error.message : String(error),
        recoverable: false,
      });
    } finally {
      // Clean up event listener
      unlisten();
    }
  }

  /**
   * Execute agent without streaming (simpler but less responsive)
   */
  async executeAgent(
    conversationId: string,
    agentId: string,
    userMessage: string
  ): Promise<{
    response: string;
    toolCalls?: ToolCall[];
    toolResults?: ToolResult[];
  }> {
    // Call Rust backend command
    const result = await invoke<any>("execute_agent_zero_stream", {
      conversationId,
      agentId,
      message: userMessage,
    });

    return {
      response: result.response || "",
      toolCalls: result.tool_calls || [],
      toolResults: [], // Backend doesn't return these yet
    };
  }
}

// ============================================================================
// SERVICE INSTANCE
// ============================================================================

export const conversationService = new ConversationService();
