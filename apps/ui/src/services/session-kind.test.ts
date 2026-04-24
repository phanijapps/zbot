// ============================================================================
// session-kind predicate tests
// ============================================================================

import { describe, it, expect } from "vitest";
import {
  CHAT_SESSION_ID_PREFIX,
  isChatSession,
  isResearchSession,
} from "./session-kind";

describe("isChatSession", () => {
  it("returns true when mode is 'fast'", () => {
    expect(isChatSession({ mode: "fast", conversation_id: "sess-12345" })).toBe(true);
  });

  it("returns true when mode is 'chat'", () => {
    expect(isChatSession({ mode: "chat", conversation_id: "sess-12345" })).toBe(true);
  });

  it("returns true case-insensitively ('FAST' / 'Chat')", () => {
    expect(isChatSession({ mode: "FAST" })).toBe(true);
    expect(isChatSession({ mode: "Chat" })).toBe(true);
  });

  it("returns false when mode is 'deep' even if id looks chat-ish", () => {
    // Pathological: a research session with a misleading id — the
    // explicit mode field must win over the prefix heuristic.
    expect(
      isChatSession({ mode: "deep", conversation_id: "sess-chat-weird" }),
    ).toBe(false);
  });

  it("returns false when mode is 'research'", () => {
    expect(isChatSession({ mode: "research" })).toBe(false);
  });

  it("falls back to the sess-chat- prefix when mode is absent", () => {
    expect(
      isChatSession({ conversation_id: `${CHAT_SESSION_ID_PREFIX}abc` }),
    ).toBe(true);
    expect(isChatSession({ conversation_id: "sess-abc" })).toBe(false);
  });

  it("falls back to the prefix when mode is null", () => {
    expect(
      isChatSession({ mode: null, conversation_id: `${CHAT_SESSION_ID_PREFIX}abc` }),
    ).toBe(true);
  });

  it("returns false when both signals are missing", () => {
    expect(isChatSession({})).toBe(false);
  });

  it("returns false for unknown mode values without a matching id", () => {
    expect(isChatSession({ mode: "weird-future-mode", conversation_id: "sess-x" })).toBe(false);
  });
});

describe("isResearchSession", () => {
  it("is the complement of isChatSession", () => {
    const chatRow = { mode: "fast" };
    const researchRow = { mode: "deep" };
    expect(isResearchSession(chatRow)).toBe(false);
    expect(isResearchSession(researchRow)).toBe(true);
  });
});
