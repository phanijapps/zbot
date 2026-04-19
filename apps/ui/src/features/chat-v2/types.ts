export type QuickChatMessageRole = "user" | "assistant";

export interface QuickChatInlineChip {
  id: string;
  kind: "recall" | "skill" | "delegate";
  label: string;   // e.g., "recalled 2", "loaded web-read", "→ writer-agent"
  detail?: string; // expanded tooltip / panel content
}

export interface QuickChatMessage {
  id: string;
  role: QuickChatMessageRole;
  content: string;                       // markdown for assistant, plain for user
  timestamp: number;
  chips?: QuickChatInlineChip[];         // assistant-only
  streaming?: boolean;                   // true while Token events still arriving
}

export type QuickChatStatus = "idle" | "running" | "error";

export interface QuickChatState {
  sessionId: string | null;
  conversationId: string;                // always set; new on "New chat"
  messages: QuickChatMessage[];
  status: QuickChatStatus;
  activeWardName: string | null;
  olderCursor: string | null;            // for lazy-load "Show N earlier turns"
  hasMoreOlder: boolean;
}

export const EMPTY_QUICK_CHAT_STATE: QuickChatState = {
  sessionId: null,
  conversationId: "",
  messages: [],
  status: "idle",
  activeWardName: null,
  olderCursor: null,
  hasMoreOlder: false,
};
