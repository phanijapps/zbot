import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import { BaseTool } from "../base/BaseTool";

/**
 * Input schema for PythonTool
 */
const PythonToolInputSchema = z.object({
  code: z.string().describe("Python code to execute"),
});

type PythonToolInput = z.infer<typeof PythonToolInputSchema>;

/**
 * PythonTool - Execute Python code in isolated venv
 *
 * Uses virtual environment at: ~/.config/zeroagent/venv/
 * All code is executed with the venv's Python interpreter.
 * Captures stdout, stderr, and return values.
 */
export class PythonTool extends BaseTool<PythonToolInput> {
  readonly name = "python";
  readonly description = "Execute Python code in an isolated virtual environment. Captures stdout and stderr. The code runs in ~/.config/zeroagent/venv/. For multi-line code, use proper Python syntax.";
  readonly schema = PythonToolInputSchema;

  async execute(inputs: PythonToolInput): Promise<string> {
    const { code } = inputs;

    try {
      const result = await invoke<string>("execute_python_code", {
        code,
      });

      return result || "(no output)";
    } catch (error) {
      throw new Error(
        `Python execution failed: ${error instanceof Error ? error.message : String(error)}`
      );
    }
  }
}
