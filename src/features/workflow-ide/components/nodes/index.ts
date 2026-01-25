import { OrchestratorNode } from './OrchestratorNode';
import { SubagentNode } from './SubagentNode';
import { StartNode } from './StartNode';
import { EndNode } from './EndNode';
import { ConditionalNode } from './ConditionalNode';

// Register all custom node types
export const nodeTypes = {
  start: StartNode,
  end: EndNode,
  orchestrator: OrchestratorNode,
  subagent: SubagentNode,
  conditional: ConditionalNode, // DRAFT - Not yet fully implemented
  // Add more node types here as implemented:
  // tool: ToolNode,
  // input: InputNode,
  // output: OutputNode,
};

export { OrchestratorNode, SubagentNode, StartNode, EndNode, ConditionalNode };
