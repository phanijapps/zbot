// ============================================================================
// CONVERSATIONS SERVICE
// Frontend service for conversation and message management
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { Conversation, Message } from "@/shared/types";
import type { Agent } from "@/shared/types";
import type { AgentStreamEvent } from "@/shared/types/agent";

// ============================================================================
// CONVERSATION OPERATIONS
// ============================================================================

/**
 * List all conversations
 */
export async function listConversations(): Promise<Conversation[]> {
  return invoke("list_conversations");
}

/**
 * Get a single conversation by ID
 */
export async function getConversation(id: string): Promise<Conversation | null> {
  return invoke("get_conversation", { id });
}

/**
 * Create a new conversation
 */
export async function createConversation(
  agentId: string,
  title?: string
): Promise<Conversation> {
  return invoke("create_conversation", {
    conversation: {
      id: crypto.randomUUID(),
      title: title || "New Chat",
      agentId,
      messages: [],
      createdAt: Date.now(),
      updatedAt: Date.now(),
    },
  });
}

/**
 * Update conversation title
 */
export async function updateConversation(
  id: string,
  updates: Partial<Pick<Conversation, "title" | "agentId">>
): Promise<Conversation> {
  const existing = await getConversation(id);
  if (!existing) {
    throw new Error(`Conversation not found: ${id}`);
  }

  return invoke("update_conversation", {
    id,
    conversation: {
      ...existing,
      ...updates,
      updatedAt: Date.now(),
    },
  });
}

/**
 * Delete a conversation
 */
export async function deleteConversation(id: string): Promise<void> {
  return invoke("delete_conversation", { id });
}

// ============================================================================
// MESSAGE OPERATIONS
// ============================================================================

/**
 * List messages in a conversation
 */
export async function listMessages(
  conversationId: string,
  limit?: number,
  offset?: number
): Promise<Message[]> {
  return invoke("list_messages", { conversationId, limit, offset });
}

/**
 * Get a single message by ID
 */
export async function getMessage(id: string): Promise<Message | null> {
  return invoke("get_message", { id });
}

/**
 * Create a new message
 */
export async function createMessage(
  conversationId: string,
  role: Message["role"],
  content: string,
  _metadata?: Record<string, unknown>
): Promise<Message> {
  return invoke("create_message", {
    data: {
      id: crypto.randomUUID(),
      conversationId,
      role,
      content,
    },
  });
}

/**
 * Delete a message
 */
export async function deleteMessage(id: string): Promise<void> {
  return invoke("delete_message", { id });
}

// ============================================================================
// AGENT INTEGRATION
// ============================================================================

/**
 * Get or create a conversation for an agent
 */
export async function getOrCreateAgentConversation(
  agentId: string,
  conversationId?: string
): Promise<Conversation> {
  return invoke("get_or_create_conversation", { agentId, conversationId });
}

/**
 * Stream agent response
 * Note: Actual execution happens in the ConversationService
 */
export async function streamAgentResponse(
  conversationId: string,
  agentId: string,
  message: string,
  onEvent: (event: AgentStreamEvent) => void
): Promise<void> {
  // Import the conversation service that handles the actual execution
  const { conversationService } = await import(
    "@/domains/agent-runtime/services/ConversationService"
  );

  await conversationService.executeAgentStream(
    conversationId,
    agentId,
    message,
    onEvent
  );
}

// ============================================================================
// CONVERSATION WITH AGENT DATA
// ============================================================================

/**
 * Enriched conversation with agent details
 */
export interface ConversationWithAgent {
  id: string;
  title: string;
  agentId: string;
  agentName: string;
  agentIcon?: string;
  lastMessage?: string;
  lastMessageTime: number;
  messageCount: number;
  model?: string;
}

/**
 * Get conversations enriched with agent information
 */
export async function getConversationWithAgents(): Promise<ConversationWithAgent[]> {
  const [conversations, agents] = await Promise.all([
    listConversations(),
    invoke("list_agents") as Promise<Agent[]>,
  ]);

  console.log("=== DEBUG: getConversationWithAgents ===");
  console.log("Loaded agents count:", agents.length);
  console.log("Loaded agents:", agents.map((a) => ({ id: a.id, name: a.name, displayName: a.displayName })));
  console.log("Loaded conversations count:", conversations.length);
  console.log("Loaded conversations:", conversations.map((c) => ({ id: c.id, agentId: c.agentId, title: c.title })));

  const agentMap = new Map(agents.map((a) => [a.id, a]));

  console.log("Agent map entries:", Array.from(agentMap.entries()).map(([k, v]) => [k, { name: v.name, displayName: v.displayName }]));

  // Fetch messages for each conversation to get accurate counts and last message
  const conversationsWithMessages = await Promise.all(
    conversations.map(async (conv) => {
      console.log("DEBUG: Processing conversation:", conv.id, "agentId:", conv.agentId);
      try {
        const messages = await listMessages(conv.id);
        return {
          ...conv,
          messages,
          messageCount: messages.length,
          lastMessage: messages[messages.length - 1]?.content,
        };
      } catch {
        // If we can't load messages, use empty array
        return {
          ...conv,
          messages: [],
          messageCount: 0,
          lastMessage: undefined,
        };
      }
    })
  );

  return conversationsWithMessages.map((conv) => {
    const agent = agentMap.get(conv.agentId);
    console.log(`Looking up agent for conversation "${conv.title}" (agentId: "${conv.agentId}"):`, {
      agentFound: !!agent,
      agentId: agent?.id,
      agentName: agent?.name,
      agentDisplayName: agent?.displayName,
      finalAgentName: agent?.displayName || agent?.name || "Unknown",
    });
    // Convert ISO string to timestamp (milliseconds)
    const timestamp = conv.updatedAt ? new Date(conv.updatedAt).getTime() : Date.now();
    return {
      id: conv.id,
      title: conv.title,
      agentId: conv.agentId,
      agentName: agent?.displayName || agent?.name || "Unknown",
      agentIcon: getAgentIcon(agent?.name),
      lastMessage: conv.lastMessage,
      lastMessageTime: timestamp,
      messageCount: conv.messageCount,
      model: agent?.model,
    };
  });
}

/**
 * Default agent icons mapping
 */
const AGENT_ICONS: Record<string, string> = {
  codex: "🤖",
  analyst: "📊",
  fileops: "🔧",
  writer: "✍️",
  assistant: "💬",
  default: "🤖",
};

function getAgentIcon(agentName?: string): string {
  if (!agentName) return AGENT_ICONS.default;
  const key = agentName.toLowerCase();
  return AGENT_ICONS[key] || AGENT_ICONS.default;
}
