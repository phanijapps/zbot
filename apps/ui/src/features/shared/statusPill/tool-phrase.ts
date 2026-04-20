import type { PillCategory } from "./types";

/**
 * Phrase used by the status pill for a given tool call.
 *
 * `narration` is the top-row verb ("Running shell", "Creating foo.py").
 * `suffix` is the raw command-ish detail rendered in the bottom terminal
 * row ("ls -la ~", "foo.py", "writer-agent"). It carries NO formatting
 * prefix — the pill component adds the prompt glyph.
 */
export interface ToolPhrase {
  narration: string;
  suffix: string;
  category: PillCategory;
}

function basename(p: string): string {
  const idx = p.lastIndexOf("/");
  return idx >= 0 ? p.slice(idx + 1) : p;
}

function argPath(args: Record<string, unknown>): string | undefined {
  const v = args.path ?? args.filePath ?? args.file_path;
  return typeof v === "string" ? v : undefined;
}

export function describeTool(
  tool: string,
  args: Record<string, unknown> = {}
): ToolPhrase {
  switch (tool) {
    case "write_file": {
      const p = argPath(args) ?? "";
      const b = basename(p);
      return { narration: `Creating ${b}`, suffix: b, category: "write" };
    }
    case "edit_file": {
      const p = argPath(args) ?? "";
      const b = basename(p);
      return { narration: `Editing ${b}`, suffix: b, category: "write" };
    }
    case "shell": {
      const cmd = typeof args.command === "string" ? args.command : "";
      if (cmd.startsWith("cat ") || cmd.startsWith("head ") || cmd.startsWith("tail ")) {
        const tail = cmd.split(" ").slice(1).join(" ");
        return { narration: `Reading ${basename(tail)}`, suffix: cmd, category: "read" };
      }
      if (cmd.startsWith("ls")) {
        return { narration: "Listing files", suffix: cmd, category: "read" };
      }
      return { narration: "Running shell", suffix: cmd, category: "neutral" };
    }
    case "load_skill": {
      const s = typeof args.skill === "string" ? args.skill : "skill";
      return { narration: `Loading ${s} skill`, suffix: s, category: "read" };
    }
    case "delegate_to_agent": {
      const a = (args.agent_id ?? args.agentId ?? "subagent") as string;
      return { narration: `Delegating to ${a}`, suffix: a, category: "delegate" };
    }
    case "memory": {
      const action = typeof args.action === "string" ? args.action : "";
      if (action === "recall") return { narration: "Recalling from memory", suffix: "recall", category: "read" };
      if (action === "save_fact") return { narration: "Saving fact", suffix: "save", category: "write" };
      return { narration: `Memory ${action}`, suffix: action, category: "neutral" };
    }
    case "graph_query":
      return { narration: "Searching knowledge graph", suffix: "graph search", category: "read" };
    case "ingest":
      return { narration: "Ingesting to graph", suffix: "ingest", category: "write" };
    case "ward": {
      const action = typeof args.action === "string" ? args.action : "";
      const name = typeof args.name === "string" ? args.name : "";
      if (action === "use" && name) return { narration: `Entering ${name}`, suffix: name, category: "neutral" };
      return { narration: `Ward ${action}`, suffix: action, category: "neutral" };
    }
    case "respond":
      return { narration: "Responding", suffix: "respond", category: "respond" };
    default:
      return { narration: `Running ${tool}`, suffix: tool, category: "neutral" };
  }
}
