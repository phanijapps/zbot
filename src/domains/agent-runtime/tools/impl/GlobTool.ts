import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import { BaseTool } from "../base/BaseTool";

/**
 * Input schema for GlobTool
 */
const GlobToolInputSchema = z.object({
  pattern: z.string().describe("Glob pattern (e.g., '**/*.ts', 'src/**/*.md')"),
  path: z.string().optional().describe("Base directory to search (defaults to current directory)"),
  includeHidden: z.boolean().optional().default(false).describe("Include hidden files (starting with .)"),
});

type GlobToolInput = z.infer<typeof GlobToolInputSchema>;

/**
 * GlobTool - Find files by pattern matching
 */
export class GlobTool extends BaseTool<GlobToolInput> {
  readonly name = "glob";
  readonly description = "Find files matching a glob pattern. Supports ** for recursive matching, * for single-level wildcards. Returns sorted list of matching file paths.";
  readonly schema = GlobToolInputSchema;

  async execute(inputs: GlobToolInput): Promise<string> {
    const { pattern, path, includeHidden } = inputs;

    try {
      const result = await invoke<string>("glob_files", {
        pattern,
        path: path || ".",
        includeHidden: includeHidden ?? false,
      });

      if (!result || result === "[]") {
        return `No files found matching pattern '${pattern}'`;
      }

      return result;
    } catch (error) {
      throw new Error(
        `Glob failed for pattern '${pattern}': ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }
}
