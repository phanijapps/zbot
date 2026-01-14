import { DynamicStructuredTool } from "@langchain/core/tools";
import { z } from "zod";

/**
 * Abstract base class for all agent tools.
 * Each tool must extend this class and implement the abstract methods.
 */
export abstract class BaseTool<TInput = any> {
  /**
   * Unique name for the tool (lowercase, hyphenated)
   */
  abstract readonly name: string;

  /**
   * Human-readable description of what the tool does
   */
  abstract readonly description: string;

  /**
   * Zod schema for input validation
   */
  abstract readonly schema: z.ZodType<TInput>;

  /**
   * Execute the tool with given inputs
   * @param inputs - Validated inputs according to schema
   * @returns Result as a string
   */
  abstract execute(inputs: TInput): Promise<string>;

  /**
   * Convert to LangChain DynamicStructuredTool
   * Wraps execute() in error handling
   */
  toLangChainTool(): DynamicStructuredTool {
    return new DynamicStructuredTool({
      name: this.name,
      description: this.description,
      schema: this.schema,
      func: async (inputs: TInput) => {
        try {
          return await this.execute(inputs);
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          return `Error executing ${this.name}: ${message}`;
        }
      },
    });
  }
}
