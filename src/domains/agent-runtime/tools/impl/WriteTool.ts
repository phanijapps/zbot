import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import { BaseTool } from "../base/BaseTool";

/**
 * Input schema for WriteTool
 */
const WriteToolInputSchema = z.object({
  path: z.string().describe("Absolute path to the file to write"),
  content: z.string().describe("Content to write to the file"),
});

type WriteToolInput = z.infer<typeof WriteToolInputSchema>;

/**
 * WriteTool - Write content to files, creating parent directories if needed
 */
export class WriteTool extends BaseTool<WriteToolInput> {
  readonly name = "write";
  readonly description = "Write content to a file. Creates parent directories if they don't exist. Overwrites existing files. Returns success message.";
  readonly schema = WriteToolInputSchema;

  async execute(inputs: WriteToolInput): Promise<string> {
    const { path, content } = inputs;

    try {
      await invoke("write_file_with_dirs", {
        path,
        content,
      });

      return `Successfully wrote ${content.length} characters to '${path}'`;
    } catch (error) {
      throw new Error(
        `Failed to write file '${path}': ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }
}
