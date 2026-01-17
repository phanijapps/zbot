/**
 * Simple LLM Executor - Direct API calls to LLM providers
 *
 * Makes HTTP requests directly to LLM provider APIs (OpenAI-compatible).
 * No LangChain dependency for better browser compatibility.
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  AgentConfig,
  AgentStreamEvent,
  MessageHistory,
  ToolCall,
  ToolResult,
} from "@/shared/types/agent";

// ============================================================================
// LLM PROVIDER CONFIG
// ============================================================================

interface ProviderConfig {
  baseURL: string;
  apiKey: string;
  modelName: string;
}

interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

interface ChatRequest {
  model: string;
  messages: ChatMessage[];
  temperature: number;
  max_tokens: number;
  stream: boolean;
}

/**
 * Fetch provider configuration from Rust backend
 */
async function getProviderConfig(providerId: string, model: string): Promise<ProviderConfig> {
  try {
    const provider = await invoke<any>("get_provider", { id: providerId });

    if (!provider) {
      throw new Error(`Provider not found: ${providerId}`);
    }

    return {
      baseURL: provider.baseUrl || provider.base_url,
      apiKey: provider.apiKey || provider.api_key,
      modelName: model || provider.models?.[0] || "default",
    };
  } catch (error) {
    throw new Error(
      `Failed to get provider config for "${providerId}": ${error instanceof Error ? error.message : String(error)}`
    );
  }
}

// ============================================================================
// LLM EXECUTOR
// ============================================================================

export class LLMExecutor {
  private config: AgentConfig;
  private systemPrompt?: string;

  constructor(config: AgentConfig, systemPrompt?: string) {
    this.config = config;
    this.systemPrompt = systemPrompt;
  }

  /**
   * Convert MessageHistory to ChatMessage[]
   */
  private convertToChatMessages(history: MessageHistory): ChatMessage[] {
    const messages: ChatMessage[] = [];

    // Add system prompt if available
    if (this.systemPrompt) {
      messages.push({
        role: "system",
        content: this.systemPrompt,
      });
    }

    // Add conversation history
    for (const msg of history.messages) {
      messages.push({
        role: msg.role as "user" | "assistant" | "system",
        content: msg.content,
      });
    }

    return messages;
  }

  /**
   * Execute LLM with streaming
   */
  async *executeStream(
    userMessage: string,
    history: MessageHistory
  ): AsyncGenerator<AgentStreamEvent, void> {
    try {
      console.log("[LLMExecutor] Starting executeStream", {
        providerId: this.config.providerId,
        model: this.config.model,
        userMessage,
        historyLength: history.messages.length,
      });

      // Get provider configuration from backend
      const providerConfig = await getProviderConfig(
        this.config.providerId,
        this.config.model
      );

      console.log("[LLMExecutor] Got provider config", {
        baseURL: providerConfig.baseURL,
        hasApiKey: !!providerConfig.apiKey,
        modelName: providerConfig.modelName,
      });

      // Emit metadata event
      yield {
        type: "metadata",
        timestamp: Date.now(),
        agentId: this.config.agentId,
        model: this.config.model,
        provider: this.config.providerId,
      } as AgentStreamEvent;

      // Build messages array (history already contains user message)
      const chatMessages = this.convertToChatMessages(history);

      // Build request body
      const requestBody: ChatRequest = {
        model: providerConfig.modelName,
        messages: chatMessages,
        temperature: this.config.temperature ?? 0.7,
        max_tokens: this.config.maxTokens ?? 2000,
        stream: true,
      };

      console.log("[LLMExecutor] Sending API request", {
        url: `${providerConfig.baseURL}/chat/completions`,
        messageCount: chatMessages.length,
        model: requestBody.model,
      });

      // Make streaming request
      const response = await fetch(`${providerConfig.baseURL}/chat/completions`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Authorization": `Bearer ${providerConfig.apiKey}`,
        },
        body: JSON.stringify(requestBody),
      });

      console.log("[LLMExecutor] Got response", {
        status: response.status,
        statusText: response.statusText,
        ok: response.ok,
      });

      if (!response.ok) {
        const errorText = await response.text();
        console.error("[LLMExecutor] API error", {
          status: response.status,
          errorText,
        });
        throw new Error(`API error (${response.status}): ${errorText}`);
      }

      // Read streaming response
      const reader = response.body?.getReader();
      if (!reader) {
        throw new Error("No response body");
      }

      const decoder = new TextDecoder();
      let fullResponse = "";
      let chunkCount = 0;

      console.log("[LLMExecutor] Starting to read stream...");

      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          console.log("[LLMExecutor] Stream done", {
            totalChunks: chunkCount,
            fullResponseLength: fullResponse.length,
          });
          break;
        }

        chunkCount++;
        const chunk = decoder.decode(value);
        const lines = chunk.split("\n").filter((line) => line.trim() !== "");

        for (const line of lines) {
          if (line.startsWith("data: ")) {
            const data = line.slice(6);
            if (data === "[DONE]") continue;

            try {
              const parsed = JSON.parse(data);
              const content = parsed.choices?.[0]?.delta?.content;
              if (content) {
                fullResponse += content;
                yield {
                  type: "token",
                  timestamp: Date.now(),
                  content,
                } as AgentStreamEvent;
              }
            } catch (e) {
              console.warn("[LLMExecutor] Failed to parse chunk", { line, error: e });
            }
          }
        }
      }

      // Emit done event
      yield {
        type: "done",
        timestamp: Date.now(),
        finalMessage: fullResponse,
        tokenCount: fullResponse.length,
      } as AgentStreamEvent;

    } catch (error) {
      // Emit error event
      yield {
        type: "error",
        timestamp: Date.now(),
        error: error instanceof Error ? error.message : String(error),
        recoverable: false,
      } as AgentStreamEvent;
    }
  }

  /**
   * Execute without streaming (for compatibility)
   */
  async execute(
    userMessage: string,
    history: MessageHistory
  ): Promise<{
    response: string;
    toolCalls?: ToolCall[];
    toolResults?: ToolResult[];
    tokenCount: number;
  }> {
    const toolCalls: ToolCall[] = [];
    const toolResults: ToolResult[] = [];
    let fullResponse = "";

    for await (const event of this.executeStream(userMessage, history)) {
      if (event.type === "token") {
        fullResponse += event.content;
      }
    }

    return {
      response: fullResponse,
      toolCalls,
      toolResults,
      tokenCount: fullResponse.length,
    };
  }
}

// ============================================================================
// FACTORY FUNCTION (named createAgentExecutor for compatibility)
// ============================================================================

export async function createAgentExecutor(
  agentId: string,
  providerId: string,
  model: string,
  options?: {
    temperature?: number;
    maxTokens?: number;
  }
): Promise<LLMExecutor> {
  const config: AgentConfig = {
    agentId,
    providerId,
    model,
    temperature: options?.temperature ?? 0.7,
    maxTokens: options?.maxTokens ?? 2000,
  };

  // Load agent instructions from AGENTS.md via Tauri
  let systemPrompt: string | undefined;
  try {
    const agent = await (await import("@tauri-apps/api/core")).invoke("get_agent", { id: agentId });
    if (agent && (agent as any).instructions) {
      systemPrompt = (agent as any).instructions;
    }
  } catch {
    // Ignore if we can't load instructions
  }

  return new LLMExecutor(config, systemPrompt);
}

/**
 * Execute without streaming (for compatibility)
 */
export async function executeAgent(
  executor: LLMExecutor,
  userMessage: string,
  history: MessageHistory
): Promise<{
  response: string;
  toolCalls: ToolCall[];
  toolResults: ToolResult[];
  tokenCount: number;
}> {
  const toolCalls: ToolCall[] = [];
  const toolResults: ToolResult[] = [];
  let fullResponse = "";

  for await (const event of executor.executeStream(userMessage, history)) {
    if (event.type === "token") {
      fullResponse += event.content;
    }
  }

  return {
    response: fullResponse,
    toolCalls,
    toolResults,
    tokenCount: fullResponse.length,
  };
}
