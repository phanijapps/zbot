import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import { BaseTool } from "../base/BaseTool";

/**
 * Input schema for BashTool
 */
const BashToolInputSchema = z.object({
  command: z.string().describe("Shell command to execute"),
});

type BashToolInput = z.infer<typeof BashToolInputSchema>;

/**
 * BashTool - Execute shell commands cross-platform
 *
 * Platform behavior:
 * - Linux/macOS: Uses bash, fallback to sh
 * - Windows: Uses PowerShell, fallback to WSL bash
 */
export class BashTool extends BaseTool<BashToolInput> {
  readonly name = "bash";
  readonly description = "Execute a shell command. Automatically detects and uses the appropriate shell for the platform (bash on Linux/macOS, PowerShell on Windows). Returns combined stdout and stderr.";
  readonly schema = BashToolInputSchema;

  async execute(inputs: BashToolInput): Promise<string> {
    const { command } = inputs;

    try {
      const result = await invoke<string>("execute_shell_command", {
        command,
      });

      return result || "(no output)";
    } catch (error) {
      throw new Error(
        `Command failed: ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }
}
