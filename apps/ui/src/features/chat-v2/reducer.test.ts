import { describe, it, expect } from "vitest";
import { reduceQuickChat } from "./reducer";
import { EMPTY_QUICK_CHAT_STATE } from "./types";

describe("reduceQuickChat", () => {
  it("appends user message and flips status to running", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "APPEND_USER",
      message: { id: "u1", role: "user", content: "hello", timestamp: 1 },
    });
    expect(s.messages).toHaveLength(1);
    expect(s.status).toBe("running");
  });

  it("SESSION_BOUND sets sessionId", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "SESSION_BOUND", sessionId: "sess-x",
    });
    expect(s.sessionId).toBe("sess-x");
  });

  it("TOKEN appends to the latest assistant message or creates one", () => {
    const s1 = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, { type: "TOKEN", text: "Hi " });
    expect(s1.messages).toHaveLength(1);
    expect(s1.messages[0].role).toBe("assistant");
    expect(s1.messages[0].content).toBe("Hi ");
    const s2 = reduceQuickChat(s1, { type: "TOKEN", text: "there" });
    expect(s2.messages[0].content).toBe("Hi there");
  });

  it("RESPOND overrides streaming content with final text", () => {
    let s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, { type: "TOKEN", text: "partial" });
    s = reduceQuickChat(s, { type: "RESPOND", text: "final answer" });
    expect(s.messages[0].content).toBe("final answer");
    expect(s.messages[0].streaming).toBe(false);
  });

  it("TURN_COMPLETE sets status back to idle", () => {
    const s = reduceQuickChat(
      { ...EMPTY_QUICK_CHAT_STATE, status: "running" },
      { type: "TURN_COMPLETE" }
    );
    expect(s.status).toBe("idle");
  });

  it("ADD_CHIP attaches chip to latest assistant message", () => {
    let s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, { type: "TOKEN", text: "foo" });
    s = reduceQuickChat(s, {
      type: "ADD_CHIP",
      chip: { id: "c1", kind: "recall", label: "recalled 2" },
    });
    expect(s.messages[0].chips).toHaveLength(1);
  });

  it("RESET clears messages but keeps new conversationId", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "RESET", conversationId: "quick-chat-new",
    });
    expect(s.messages).toHaveLength(0);
    expect(s.conversationId).toBe("quick-chat-new");
    expect(s.sessionId).toBeNull();
  });

  it("WARD_CHANGED updates active ward", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "WARD_CHANGED", wardName: "stock-analysis",
    });
    expect(s.activeWardName).toBe("stock-analysis");
  });

  it("HYDRATE replaces state from snapshot", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "HYDRATE",
      sessionId: "sess-1",
      conversationId: "quick-chat-1",
      messages: [{ id: "m1", role: "user", content: "hi", timestamp: 1 }],
      wardName: "default",
    });
    expect(s.sessionId).toBe("sess-1");
    expect(s.messages).toHaveLength(1);
    expect(s.activeWardName).toBe("default");
  });

  it("AGENT_STARTED flips status to running", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "AGENT_STARTED", agentId: "quick-chat",
    });
    expect(s.status).toBe("running");
  });

  it("ERROR flips status to error", () => {
    const s = reduceQuickChat(
      { ...EMPTY_QUICK_CHAT_STATE, status: "running" },
      { type: "ERROR", message: "network down" }
    );
    expect(s.status).toBe("error");
  });

  it("PREPEND_OLDER prepends messages and derives hasMoreOlder from cursor", () => {
    const existing = [{ id: "new1", role: "user" as const, content: "latest", timestamp: 2 }];
    const older = [{ id: "old1", role: "user" as const, content: "earlier", timestamp: 1 }];

    const withMore = reduceQuickChat(
      { ...EMPTY_QUICK_CHAT_STATE, messages: existing },
      { type: "PREPEND_OLDER", messages: older, nextCursor: "cursor-xyz" }
    );
    expect(withMore.messages.map((m) => m.id)).toEqual(["old1", "new1"]);
    expect(withMore.olderCursor).toBe("cursor-xyz");
    expect(withMore.hasMoreOlder).toBe(true);

    const exhausted = reduceQuickChat(
      { ...EMPTY_QUICK_CHAT_STATE, messages: existing },
      { type: "PREPEND_OLDER", messages: older, nextCursor: null }
    );
    expect(exhausted.hasMoreOlder).toBe(false);
    expect(exhausted.olderCursor).toBeNull();
  });
});
