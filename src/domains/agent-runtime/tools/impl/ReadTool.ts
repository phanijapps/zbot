import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import { BaseTool } from "../base/BaseTool";

/**
 * Input schema for ReadTool
 */
const ReadToolInputSchema = z.object({
  path: z.string().describe("Absolute path to the file to read"),
  offset: z.number().optional().default(0).describe("Starting line number (0-indexed)"),
  limit: z.number().optional().describe("Maximum number of lines to read"),
});

type ReadToolInput = z.infer<typeof ReadToolInputSchema>;

/**
 * ReadTool - Read file contents with optional offset/limit
 */
export class ReadTool extends BaseTool<ReadToolInput> {
  readonly name = "read";
  readonly description = "Read the contents of a file. Supports optional offset and limit for reading specific line ranges. Returns file content as a string.";
  readonly schema = ReadToolInputSchema;

  async execute(inputs: ReadToolInput): Promise<string> {
    const { path, offset, limit } = inputs;

    try {
      const result = await invoke<string>("read_file_lines", {
        path,
        offset: offset ?? 0,
        limit: limit ?? -1, // -1 means read all lines
      });

      return result;
    } catch (error) {
      throw new Error(
        `Failed to read file '${path}': ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }
}
