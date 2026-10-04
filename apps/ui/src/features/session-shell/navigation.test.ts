import { describe, expect, it } from "vitest";
import { conversationDestination } from "./navigation";

// STUB: AC11 — existing strict return helper remains the boundary, not a new parser.
describe("administration conversation return contract", () => {
  it.each(["/session", "/session?mode=research", "/session/sess-kept", "/session/chat-1?mode=chat"])("preserves the valid local destination %s", target => {
    expect(conversationDestination(target)).toBe(target);
  });

  it.each([null, "", "https://example.test/session", "//example.test/session", "https://user:key@example.test/session", "/session/../settings", "/session/a/../../settings", "/session/a?token=secret", "/session/a#fragment", `/session/${"a".repeat(129)}`, "/session/a?mode=unknown", "/session/%2e%2e"])("falls back rather than exposing an unsafe destination %s", target => {
    expect(conversationDestination(target)).toBe("/session");
  });
});
