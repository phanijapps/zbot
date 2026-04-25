// ============================================================================
// getAgentEmoji algorithm — locks in the codePointAt switch (Sonar S7758).
// The helper is module-private; this test reproduces the same algorithm
// here and asserts parity with the prior charCodeAt implementation for
// ASCII identifiers (the only kind of agent IDs we have today).
// ============================================================================

import { describe, it, expect } from "vitest";

const AGENT_EMOJIS = [
  "\u{1F916}", "\u{1F9E0}", "\u{26A1}", "\u{1F4A1}", "\u{1F680}",
  "\u{2B50}", "\u{1F3AF}", "\u{1F525}", "\u{1F48E}", "\u{1F50D}",
  "\u{1F4DD}", "\u{1F517}", "\u{1F30D}", "\u{1F4CA}", "\u{1F527}",
];

function getAgentEmojiCodePoint(id: string): string {
  let hash = 0;
  for (let i = 0; i < id.length; i++) {
    hash = Math.trunc((hash << 5) - hash + (id.codePointAt(i) ?? 0));
  }
  return AGENT_EMOJIS[Math.abs(hash) % AGENT_EMOJIS.length];
}

function getAgentEmojiCharCode(id: string): string {
  let hash = 0;
  for (let i = 0; i < id.length; i++) {
    hash = Math.trunc((hash << 5) - hash + id.charCodeAt(i));
  }
  return AGENT_EMOJIS[Math.abs(hash) % AGENT_EMOJIS.length];
}

describe("getAgentEmoji (Sonar S7758: codePointAt parity)", () => {
  it("is deterministic — same id yields same emoji every call", () => {
    expect(getAgentEmojiCodePoint("agent-1")).toBe(getAgentEmojiCodePoint("agent-1"));
    expect(getAgentEmojiCodePoint("researcher")).toBe(getAgentEmojiCodePoint("researcher"));
  });

  it("returns one of the 15 known emoji glyphs", () => {
    for (const id of ["a", "b", "agent-1", "agent-2", "researcher", "coder", "tutor"]) {
      expect(AGENT_EMOJIS).toContain(getAgentEmojiCodePoint(id));
    }
  });

  it("matches the prior charCodeAt implementation for ASCII ids", () => {
    for (const id of ["agent-1", "researcher", "code-agent", "abc", "xyz-123"]) {
      expect(getAgentEmojiCodePoint(id)).toBe(getAgentEmojiCharCode(id));
    }
  });

  it("handles single-character ids without crashing", () => {
    expect(() => getAgentEmojiCodePoint("a")).not.toThrow();
    expect(AGENT_EMOJIS).toContain(getAgentEmojiCodePoint("a"));
  });

  it("handles empty string by returning the first emoji (hash=0)", () => {
    expect(getAgentEmojiCodePoint("")).toBe(AGENT_EMOJIS[0]);
  });
});
