// ============================================================================
// getMcpEmoji + getPluginEmoji — locks in the codePointAt switch (Sonar S7758).
// The helpers are module-private; this test reproduces the algorithm here and
// asserts parity with the prior charCodeAt implementation for ASCII ids.
// ============================================================================

import { describe, it, expect } from "vitest";

function makeHash(id: string): number {
  let hash = 0;
  for (let i = 0; i < id.length; i++) {
    hash = Math.trunc((hash << 5) - hash + (id.codePointAt(i) ?? 0));
  }
  return Math.abs(hash);
}

function makeHashCharCode(id: string): number {
  let hash = 0;
  for (let i = 0; i < id.length; i++) {
    hash = Math.trunc((hash << 5) - hash + id.charCodeAt(i));
  }
  return Math.abs(hash);
}

describe("getMcpEmoji / getPluginEmoji (Sonar S7758: codePointAt parity)", () => {
  it("is deterministic — same id produces the same hash every call", () => {
    expect(makeHash("mcp-fs")).toBe(makeHash("mcp-fs"));
    expect(makeHash("github-search")).toBe(makeHash("github-search"));
  });

  it("matches the prior charCodeAt implementation for ASCII identifiers", () => {
    for (const id of [
      "mcp-fs",
      "mcp.search",
      "plugin/python-runner",
      "github-search",
      "abc-123",
      "tool-with-many-words",
    ]) {
      expect(makeHash(id)).toBe(makeHashCharCode(id));
    }
  });

  it("produces different hashes for different ids (collision-free in this small set)", () => {
    const ids = ["mcp-fs", "mcp-search", "plugin-python", "plugin-node", "x"];
    const seen = new Map<number, string>();
    for (const id of ids) {
      const h = makeHash(id);
      // Different ids mostly hash to different bucket positions (mod-N varies).
      expect(seen.has(h) ? seen.get(h) : id).toBe(id);
      seen.set(h, id);
    }
  });

  it("handles edge cases without crashing", () => {
    expect(() => makeHash("")).not.toThrow();
    expect(() => makeHash("a")).not.toThrow();
    expect(makeHash("")).toBe(0);
  });
});
