// ============================================================================
// extractMetaField — coerces metadata values to strings
// ============================================================================

import { describe, it, expect } from "vitest";
import { extractMetaField } from "./useSessionTrace";
import type { ExecutionLog } from "@/services/transport/types";

function log(metadata: Record<string, unknown>): ExecutionLog {
  return {
    id: "log-1",
    session_id: "sess-1",
    conversation_id: "conv-1",
    agent_id: "agent:root",
    timestamp: "2026-04-25T00:00:00Z",
    level: "info",
    category: "tool_call",
    message: "test",
    metadata,
  };
}

describe("extractMetaField", () => {
  it("returns undefined when metadata is missing entirely", () => {
    const l: ExecutionLog = { ...log({}) };
    delete (l as Partial<ExecutionLog>).metadata;
    expect(extractMetaField(l, "args")).toBeUndefined();
  });

  it("returns undefined for missing fields", () => {
    expect(extractMetaField(log({ other: "x" }), "args")).toBeUndefined();
  });

  it("returns undefined for null/undefined values", () => {
    expect(extractMetaField(log({ args: null }), "args")).toBeUndefined();
    expect(extractMetaField(log({ args: undefined }), "args")).toBeUndefined();
  });

  it("returns string fields verbatim", () => {
    expect(extractMetaField(log({ args: "hello" }), "args")).toBe("hello");
  });

  it("stringifies numeric and boolean fields", () => {
    expect(extractMetaField(log({ args: 42 }), "args")).toBe("42");
    expect(extractMetaField(log({ args: true }), "args")).toBe("true");
  });

  it("REGRESSION: JSON-stringifies object fields (was '[object Object]')", () => {
    // The popover used to show "[object Object]" because String({...}) was
    // called. The fix JSON-stringifies first; the popover then pretty-prints.
    const out = extractMetaField(log({ args: { url: "https://example.com", limit: 10 } }), "args");
    expect(out).toBe('{"url":"https://example.com","limit":10}');
  });

  it("JSON-stringifies array fields", () => {
    const out = extractMetaField(log({ args: ["a", "b", "c"] }), "args");
    expect(out).toBe('["a","b","c"]');
  });
});
