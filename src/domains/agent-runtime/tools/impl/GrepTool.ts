import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import { BaseTool } from "../base/BaseTool";

/**
 * Input schema for GrepTool
 */
const GrepToolInputSchema = z.object({
  pattern: z.string().describe("Regular expression pattern to search for"),
  path: z.string().describe("Absolute path to search in (file or directory)"),
  recursive: z.boolean().optional().default(true).describe("Search recursively in subdirectories"),
  caseInsensitive: z.boolean().optional().default(false).describe("Case-insensitive search"),
  contextBefore: z.number().optional().default(0).describe("Number of lines to show before match"),
  contextAfter: z.number().optional().default(0).describe("Number of lines to show after match"),
  maxResults: z.number().optional().default(100).describe("Maximum number of matches to return"),
});

type GrepToolInput = z.infer<typeof GrepToolInputSchema>;

/**
 * GrepTool - Search for patterns in files using regex
 */
export class GrepTool extends BaseTool<GrepToolInput> {
  readonly name = "grep";
  readonly description = "Search for a regex pattern in files. Supports recursive directory search, case-insensitive matching, and context lines. Returns matches with line numbers.";
  readonly schema = GrepToolInputSchema;

  async execute(inputs: GrepToolInput): Promise<string> {
    const {
      pattern,
      path,
      recursive,
      caseInsensitive,
      contextBefore,
      contextAfter,
      maxResults,
    } = inputs;

    try {
      const result = await invoke<string>("grep_files", {
        pattern,
        path,
        recursive: recursive ?? true,
        caseInsensitive: caseInsensitive ?? false,
        contextBefore: contextBefore ?? 0,
        contextAfter: contextAfter ?? 0,
        maxResults: maxResults ?? 100,
      });

      if (!result || result === "[]") {
        return `No matches found for pattern '${pattern}' in '${path}'`;
      }

      return result;
    } catch (error) {
      throw new Error(
        `Grep failed for pattern '${pattern}' in '${path}': ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }
}
