import { describe, it, expect } from "vitest";
import { describeTool } from "./tool-phrase";

describe("describeTool", () => {
  it("maps write_file to Creating <basename>", () => {
    const r = describeTool("write_file", { path: "src/yf_fundamentals.py" });
    expect(r).toEqual({
      narration: "Creating yf_fundamentals.py",
      suffix: "· yf_fundamentals.py",
      category: "write",
    });
  });

  it("maps edit_file to Editing <basename>", () => {
    const r = describeTool("edit_file", { path: "/a/b/c.ts" });
    expect(r.narration).toBe("Editing c.ts");
    expect(r.category).toBe("write");
  });

  it("maps shell with cat to Reading", () => {
    const r = describeTool("shell", { command: "cat README.md" });
    expect(r.category).toBe("read");
    expect(r.narration).toContain("Reading");
  });

  it("maps load_skill", () => {
    const r = describeTool("load_skill", { skill: "web-read" });
    expect(r.narration).toBe("Loading web-read skill");
    expect(r.category).toBe("read");
  });

  it("maps delegate_to_agent to purple", () => {
    const r = describeTool("delegate_to_agent", { agent_id: "writer-agent" });
    expect(r.category).toBe("delegate");
    expect(r.narration).toBe("Delegating to writer-agent");
  });

  it("maps memory recall to Recalling", () => {
    const r = describeTool("memory", { action: "recall" });
    expect(r.category).toBe("read");
    expect(r.narration).toBe("Recalling from memory");
  });

  it("maps respond to green", () => {
    const r = describeTool("respond", {});
    expect(r.category).toBe("respond");
    expect(r.narration).toBe("Responding");
  });

  it("maps unknown tool to neutral with tool name", () => {
    const r = describeTool("some_tool", { foo: 1 });
    expect(r.category).toBe("neutral");
    expect(r.narration).toBe("Running some_tool");
  });

  it("uses camelCase path alias", () => {
    const r = describeTool("write_file", { filePath: "x.py" });
    expect(r.narration).toBe("Creating x.py");
  });
});
