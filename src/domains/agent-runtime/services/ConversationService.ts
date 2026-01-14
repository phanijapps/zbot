/**
 * Conversation Service
 * Manages agent conversations with message persistence and streaming
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  Message,
  MessageHistory,
  ToolCall,
  ToolResult,
} from "@/shared/types/agent";
import { MessageRole } from "@/shared/types/agent";
import { createAgentExecutor } from "../executor/AgentExecutor";

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
        conversation_id: conversationId,
        role,
        content,
        tool_calls: toolCalls,
        tool_results: toolResults,
        token_count: tokenCount || 0,
      },
    });

    return message;
  }

  /**
   * Execute agent with streaming
   * Returns a callback that yields stream events
   */
  async executeAgentStream(
    conversationId: string,
    agentId: string,
    userMessage: string,
    onEvent: (event: any) => void
  ): Promise<void> {
    try {
      // Save user message
      await this.saveMessage(conversationId, MessageRole.User, userMessage);

      // Get agent configuration
      const agent = await invoke<any>("get_agent", { id: agentId });

      // Create executor
      const executor = await createAgentExecutor(
        agentId,
        agent.provider_id,
        agent.model,
        {
          temperature: agent.temperature,
          maxTokens: agent.max_tokens,
        }
      );

      // Load history
      const history = await this.loadHistory(conversationId);

      let fullResponse = "";
      let tokenCount = 0;
      const activeToolCalls: ToolCall[] = [];
      const toolResults: ToolResult[] = [];

      // Execute and stream events
      for await (const event of executor.executeStream(userMessage, history)) {
        // Forward event to UI
        onEvent(event);

        // Collect data for saving
        switch (event.type) {
          case "token":
            fullResponse += event.content;
            tokenCount++;
            break;

          case "tool_call_end":
            activeToolCalls.push({
              id: event.toolId,
              name: event.toolName,
              arguments: event.args,
            });
            break;

          case "tool_result":
            toolResults.push({
              toolCallId: event.toolId,
              output: event.result,
              error: event.error,
            });
            break;
        }
      }

      // Save assistant message with all data
      await this.saveMessage(
        conversationId,
        MessageRole.Assistant,
        fullResponse,
        activeToolCalls,
        toolResults,
        tokenCount
      );

    } catch (error) {
      // Emit error event
      onEvent({
        type: "error",
        timestamp: Date.now(),
        error: error instanceof Error ? error.message : String(error),
        recoverable: false,
      });
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
    // Save user message
    await this.saveMessage(conversationId, MessageRole.User, userMessage);

    // Get agent configuration
    const agent = await invoke<any>("get_agent", { id: agentId });

    // Create executor
    const executor = await createAgentExecutor(
      agentId,
      agent.provider_id,
      agent.model
    );

    // Load history
    const history = await this.loadHistory(conversationId);

    // Execute
    const result = await executor.execute(userMessage, history);

    // Save assistant message
    await this.saveMessage(
      conversationId,
      MessageRole.Assistant,
      result.response,
      result.toolCalls,
      result.toolResults,
      result.tokenCount
    );

    return {
      response: result.response,
      toolCalls: result.toolCalls,
      toolResults: result.toolResults,
    };
  }
}

// ============================================================================
// SERVICE INSTANCE
// ============================================================================

export const conversationService = new ConversationService();
