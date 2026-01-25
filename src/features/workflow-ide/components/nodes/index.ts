import { OrchestratorNode } from './OrchestratorNode';
import { SubagentNode } from './SubagentNode';

// Register all custom node types
export const nodeTypes = {
  orchestrator: OrchestratorNode,
  subagent: SubagentNode,
  // Add more node types here as implemented:
  // tool: ToolNode,
  // input: InputNode,
  // output: OutputNode,
};

export { OrchestratorNode, SubagentNode };
