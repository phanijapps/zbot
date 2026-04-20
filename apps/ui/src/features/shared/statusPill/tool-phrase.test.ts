import { describe, it, expect } from "vitest";
import { describeTool } from "./tool-phrase";

describe("describeTool", () => {
  it("maps write_file to Creating <basename> with bare suffix", () => {
    const r = describeTool("write_file", { path: "src/yf_fundamentals.py" });
    expect(r).toEqual({
      narration: "Creating yf_fundamentals.py",
      suffix: "yf_fundamentals.py",
      category: "write",
    });
  });

  it("maps edit_file to Editing <basename>", () => {
    const r = describeTool("edit_file", { path: "/a/b/c.ts" });
    expect(r.narration).toBe("Editing c.ts");
    expect(r.suffix).toBe("c.ts");
    expect(r.category).toBe("write");
  });

  it("maps shell with cat to Reading with raw command as suffix", () => {
    const r = describeTool("shell", { command: "cat README.md" });
    expect(r.category).toBe("read");
    expect(r.narration).toContain("Reading");
    expect(r.suffix).toBe("cat README.md");
  });

  it("maps shell with ls to Listing files", () => {
    const r = describeTool("shell", { command: "ls -la ~" });
    expect(r.narration).toBe("Listing files");
    expect(r.suffix).toBe("ls -la ~");
    expect(r.category).toBe("read");
  });

  it("maps generic shell command", () => {
    const r = describeTool("shell", { command: "grep -cE '^#' README.md" });
    expect(r.narration).toBe("Running shell");
    expect(r.suffix).toBe("grep -cE '^#' README.md");
    expect(r.category).toBe("neutral");
  });

  it("maps load_skill", () => {
    const r = describeTool("load_skill", { skill: "web-read" });
    expect(r.narration).toBe("Loading web-read skill");
    expect(r.suffix).toBe("web-read");
    expect(r.category).toBe("read");
  });

  it("maps delegate_to_agent to purple", () => {
    const r = describeTool("delegate_to_agent", { agent_id: "writer-agent" });
    expect(r.category).toBe("delegate");
    expect(r.narration).toBe("Delegating to writer-agent");
    expect(r.suffix).toBe("writer-agent");
  });

  it("maps memory recall to Recalling", () => {
    const r = describeTool("memory", { action: "recall" });
    expect(r.category).toBe("read");
    expect(r.narration).toBe("Recalling from memory");
    expect(r.suffix).toBe("recall");
  });

  it("maps respond to green", () => {
    const r = describeTool("respond", {});
    expect(r.category).toBe("respond");
    expect(r.narration).toBe("Responding");
    expect(r.suffix).toBe("respond");
  });

  it("maps unknown tool to neutral with tool name", () => {
    const r = describeTool("some_tool", { foo: 1 });
    expect(r.category).toBe("neutral");
    expect(r.narration).toBe("Running some_tool");
    expect(r.suffix).toBe("some_tool");
  });

  it("uses camelCase path alias", () => {
    const r = describeTool("write_file", { filePath: "x.py" });
    expect(r.narration).toBe("Creating x.py");
    expect(r.suffix).toBe("x.py");
  });
});
