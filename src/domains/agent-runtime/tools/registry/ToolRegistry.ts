import { DynamicStructuredTool } from "@langchain/core/tools";
import { BaseTool } from "../base/BaseTool";

/**
 * Singleton registry for managing agent tools.
 * Provides access to individual tools and LangChain-compatible tool arrays.
 */
export class ToolRegistry {
  private static instance: ToolRegistry;
  private tools: Map<string, BaseTool> = new Map();

  private constructor() {}

  /**
   * Get the singleton instance
   */
  static getInstance(): ToolRegistry {
    if (!ToolRegistry.instance) {
      ToolRegistry.instance = new ToolRegistry();
    }
    return ToolRegistry.instance;
  }

  /**
   * Register a tool
   */
  register(tool: BaseTool): void {
    this.tools.set(tool.name, tool);
  }

  /**
   * Unregister a tool by name
   */
  unregister(name: string): boolean {
    return this.tools.delete(name);
  }

  /**
   * Get a specific tool by name
   */
  getTool(name: string): BaseTool | undefined {
    return this.tools.get(name);
  }

  /**
   * Get all registered tools
   */
  getAllTools(): BaseTool[] {
    return Array.from(this.tools.values());
  }

  /**
   * Get all tools as LangChain DynamicStructuredTool array
   * Used when creating agents with LangChain
   */
  getLangChainTools(): DynamicStructuredTool[] {
    return this.getAllTools().map((tool) => tool.toLangChainTool());
  }

  /**
   * Check if a tool is registered
   */
  has(name: string): boolean {
    return this.tools.has(name);
  }

  /**
   * Clear all registered tools
   */
  clear(): void {
    this.tools.clear();
  }
}
