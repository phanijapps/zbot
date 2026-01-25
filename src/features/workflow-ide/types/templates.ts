// ============================================================================
// WORKFLOW TEMPLATES
// Pre-defined workflow patterns for common use cases
// ============================================================================

import { createSubagentNode, createEdge, generateNodeId } from '@/services/workflow';
import type { WorkflowNode as ServiceWorkflowNode, WorkflowEdge as ServiceWorkflowEdge, OrchestratorConfig } from '@/services/workflow';

// ============================================================================
// TEMPLATE TYPES
// ============================================================================

export interface WorkflowTemplate {
  id: string;
  name: string;
  description: string;
  category: 'basic' | 'advanced' | 'specialized';
  nodes: ServiceWorkflowNode[];
  edges: ServiceWorkflowEdge[];
  orchestrator?: OrchestratorConfig;
  icon?: string;
}

// ============================================================================
// TEMPLATE DEFINITIONS
// ============================================================================

/**
 * Pipeline Template
 * Sequential execution of subagents
 * Flow: Start -> Subagent A -> Subagent B -> Subagent C -> End
 */
export const pipelineTemplate: WorkflowTemplate = {
  id: 'pipeline',
  name: 'Pipeline',
  description: 'Sequential execution where output of one subagent feeds into the next',
  category: 'basic',
  nodes: [
    {
      id: generateNodeId('start'),
      type: 'start',
      position: { x: 100, y: 100 },
      data: { label: 'Start', triggerType: 'manual' },
    },
    createSubagentNode({
      displayName: 'Step 1: Process',
      subagentId: 'step-1-process',
      description: 'First step in the pipeline',
    }),
    createSubagentNode({
      displayName: 'Step 2: Transform',
      subagentId: 'step-2-transform',
      description: 'Transforms output from step 1',
    }),
    createSubagentNode({
      displayName: 'Step 3: Finalize',
      subagentId: 'step-3-finalize',
      description: 'Final step that produces the output',
    }),
    {
      id: generateNodeId('end'),
      type: 'end',
      position: { x: 100, y: 700 },
      data: { label: 'End' },
    },
  ],
  edges: [],
  orchestrator: {
    displayName: 'Pipeline Orchestrator',
    description: 'Coordinates sequential execution through the pipeline',
    providerId: '',
    model: '',
    temperature: 0.7,
    maxTokens: 2000,
    systemInstructions: 'You are a pipeline orchestrator. Coordinate the sequential execution of subagents, passing outputs from one step to the next.',
    mcps: [],
    skills: [],
  },
  icon: '→',
};

/**
 * Swarm Template
 * Multiple specialized subagents working in parallel
 * Flow: Start -> [Subagent A, Subagent B, Subagent C] -> End
 */
export const swarmTemplate: WorkflowTemplate = {
  id: 'swarm',
  name: 'Swarm',
  description: 'Multiple specialized subagents working on different aspects in parallel',
  category: 'advanced',
  nodes: [
    {
      id: generateNodeId('start'),
      type: 'start',
      position: { x: 100, y: 100 },
      data: { label: 'Start', triggerType: 'manual' },
    },
    createSubagentNode({
      displayName: 'Researcher',
      subagentId: 'researcher',
      description: 'Gathers and analyzes information',
    }),
    createSubagentNode({
      displayName: 'Writer',
      subagentId: 'writer',
      description: 'Drafts content based on research',
    }),
    createSubagentNode({
      displayName: 'Reviewer',
      subagentId: 'reviewer',
      description: 'Reviews and validates output',
    }),
    {
      id: generateNodeId('end'),
      type: 'end',
      position: { x: 100, y: 500 },
      data: { label: 'End' },
    },
  ],
  edges: [],
  orchestrator: {
    displayName: 'Swarm Coordinator',
    description: 'Coordinates parallel execution across multiple subagents',
    providerId: '',
    model: '',
    temperature: 0.7,
    maxTokens: 2000,
    systemInstructions: 'You are a swarm coordinator. Delegate tasks to specialized subagents working in parallel and aggregate their results.',
    mcps: [],
    skills: [],
  },
  icon: '⊕',
};

/**
 * Router Template
 * Conditional routing based on input or intermediate results
 * Flow: Orchestrator -> Router -> [Subagent A | Subagent B]
 */
export const routerTemplate: WorkflowTemplate = {
  id: 'router',
  name: 'Router',
  description: 'Routes tasks to different subagents based on conditions',
  category: 'advanced',
  nodes: [
    {
      id: generateNodeId('start'),
      type: 'start',
      position: { x: 100, y: 100 },
      data: { label: 'Start', triggerType: 'manual' },
    },
    createSubagentNode({
      displayName: 'Handler A',
      subagentId: 'handler-a',
      description: 'Handles type A requests',
    }),
    createSubagentNode({
      displayName: 'Handler B',
      subagentId: 'handler-b',
      description: 'Handles type B requests',
    }),
    {
      id: generateNodeId('end'),
      type: 'end',
      position: { x: 100, y: 500 },
      data: { label: 'End' },
    },
  ],
  edges: [],
  orchestrator: {
    displayName: 'Router Orchestrator',
    description: 'Routes requests to appropriate handler based on conditions',
    providerId: '',
    model: '',
    temperature: 0.7,
    maxTokens: 2000,
    systemInstructions: 'You are a routing orchestrator. Analyze incoming requests and route them to the appropriate handler subagent.',
    mcps: [],
    skills: [],
  },
  icon: '🔀',
};

/**
 * Map-Reduce Template
 * Apply operation to many items, then aggregate results
 * Flow: Orchestrator -> [Mapper 1, Mapper 2, ...] -> Reducer
 */
export const mapReduceTemplate: WorkflowTemplate = {
  id: 'map-reduce',
  name: 'Map-Reduce',
  description: 'Process items in parallel then aggregate results',
  category: 'specialized',
  nodes: [
    {
      id: generateNodeId('start'),
      type: 'start',
      position: { x: 100, y: 100 },
      data: { label: 'Start', triggerType: 'manual' },
    },
    createSubagentNode({
      displayName: 'Mapper 1',
      subagentId: 'mapper-1',
      description: 'Processes a subset of items',
    }),
    createSubagentNode({
      displayName: 'Mapper 2',
      subagentId: 'mapper-2',
      description: 'Processes another subset of items',
    }),
    createSubagentNode({
      displayName: 'Reducer',
      subagentId: 'reducer',
      description: 'Aggregates results from mappers',
    }),
    {
      id: generateNodeId('end'),
      type: 'end',
      position: { x: 100, y: 500 },
      data: { label: 'End' },
    },
  ],
  edges: [],
  orchestrator: {
    displayName: 'Map-Reduce Coordinator',
    description: 'Coordinates mapping and reducing phases',
    providerId: '',
    model: '',
    temperature: 0.7,
    maxTokens: 2000,
    systemInstructions: 'You are a map-reduce orchestrator. Distribute work to mapper subagents in parallel and aggregate their results through the reducer.',
    mcps: [],
    skills: [],
  },
  icon: '⧉',
};

/**
 * Hierarchical Template
 * Multi-level delegation with team structure
 * Flow: Start -> Team Lead -> [Worker 1, Worker 2] -> End
 */
export const hierarchicalTemplate: WorkflowTemplate = {
  id: 'hierarchical',
  name: 'Hierarchical',
  description: 'Multi-level delegation with sub-orchestrators managing their own subtasks',
  category: 'specialized',
  nodes: [
    {
      id: generateNodeId('start'),
      type: 'start',
      position: { x: 100, y: 100 },
      data: { label: 'Start', triggerType: 'manual' },
    },
    createSubagentNode({
      displayName: 'Team A Lead',
      subagentId: 'team-a-lead',
      description: 'Coordinates Team A subtasks',
    }),
    createSubagentNode({
      displayName: 'Team A Worker 1',
      subagentId: 'team-a-worker-1',
      description: 'Executes tasks for Team A',
    }),
    createSubagentNode({
      displayName: 'Team A Worker 2',
      subagentId: 'team-a-worker-2',
      description: 'Executes tasks for Team A',
    }),
    {
      id: generateNodeId('end'),
      type: 'end',
      position: { x: 100, y: 600 },
      data: { label: 'End' },
    },
  ],
  edges: [],
  orchestrator: {
    displayName: 'Main Orchestrator',
    description: 'Top-level coordinator',
    providerId: '',
    model: '',
    temperature: 0.7,
    maxTokens: 2000,
    systemInstructions: 'You are a hierarchical orchestrator. Delegate tasks to team leads who coordinate their own workers.',
    mcps: [],
    skills: [],
  },
  icon: '🌳',
};

// ============================================================================
// TEMPLATE REGISTRY
// ============================================================================

export const WORKFLOW_TEMPLATES: WorkflowTemplate[] = [
  pipelineTemplate,
  swarmTemplate,
  routerTemplate,
  mapReduceTemplate,
  hierarchicalTemplate,
];

/**
 * Get template by ID
 */
export function getTemplateById(id: string): WorkflowTemplate | undefined {
  return WORKFLOW_TEMPLATES.find((t) => t.id === id);
}

/**
 * Get templates by category
 */
export function getTemplatesByCategory(category: WorkflowTemplate['category']): WorkflowTemplate[] {
  return WORKFLOW_TEMPLATES.filter((t) => t.category === category);
}

/**
 * Initialize positions for template nodes (arrange them nicely on canvas)
 */
export function layoutTemplateNodes(template: WorkflowTemplate): WorkflowTemplate {
  const startNode = template.nodes.find((n) => n.type === 'start');
  const endNode = template.nodes.find((n) => n.type === 'end');
  const subagents = template.nodes.filter((n) => n.type === 'subagent');

  if (!startNode || !endNode) return template;

  const positionedNodes: ServiceWorkflowNode[] = [];
  const positionedEdges: ServiceWorkflowEdge[] = [];

  // Position start node at the top
  positionedNodes.push({
    ...startNode,
    position: { x: 100, y: 100 },
  });

  // Position subagents based on template type
  if (template.id === 'pipeline') {
    // Sequential vertical layout
    subagents.forEach((node, i) => {
      positionedNodes.push({
        ...node,
        position: { x: 100, y: 250 + i * 150 },
      });

      // Connect to previous node
      const prevNode = i === 0 ? startNode : positionedNodes[positionedNodes.length - 2];
      positionedEdges.push(
        createEdge(prevNode.id, node.id)
      );
    });

    // Connect last subagent to end
    const lastNode = positionedNodes[positionedNodes.length - 1];
    positionedEdges.push(createEdge(lastNode.id, endNode.id));
  } else if (template.id === 'swarm' || template.id === 'map-reduce') {
    // Parallel horizontal layout
    subagents.forEach((node, i) => {
      positionedNodes.push({
        ...node,
        position: { x: 100 + i * 280, y: 250 },
      });

      // All connect from start
      positionedEdges.push(createEdge(startNode.id, node.id));
      // All connect to end
      positionedEdges.push(createEdge(node.id, endNode.id));
    });
  } else if (template.id === 'router') {
    // Fan-out layout
    subagents.forEach((node, i) => {
      positionedNodes.push({
        ...node,
        position: { x: 100 + i * 320, y: 250 },
      });

      // All connect from start
      positionedEdges.push(createEdge(startNode.id, node.id, `Route ${i + 1}`));
      // All connect to end
      positionedEdges.push(createEdge(node.id, endNode.id));
    });
  } else if (template.id === 'hierarchical') {
    // Tree layout
    const leadNode = subagents[0];
    const workers = subagents.slice(1);

    positionedNodes.push({
      ...leadNode,
      position: { x: 100, y: 250 },
    });

    // Connect start to lead
    positionedEdges.push(createEdge(startNode.id, leadNode.id));

    // Position workers under lead
    workers.forEach((node, i) => {
      positionedNodes.push({
        ...node,
        position: { x: 100 + i * 280, y: 450 },
      });

      // Connect workers to lead
      positionedEdges.push(createEdge(leadNode.id, node.id));
      // Connect workers to end
      positionedEdges.push(createEdge(node.id, endNode.id));
    });
  } else {
    // Default: sequential layout
    subagents.forEach((node, i) => {
      positionedNodes.push({
        ...node,
        position: { x: 100, y: 250 + i * 150 },
      });

      const prevNode = i === 0 ? startNode : positionedNodes[positionedNodes.length - 2];
      positionedEdges.push(createEdge(prevNode.id, node.id));
    });

    // Connect last subagent to end
    if (positionedNodes.length > 0) {
      const lastNode = positionedNodes[positionedNodes.length - 1];
      positionedEdges.push(createEdge(lastNode.id, endNode.id));
    } else {
      // No subagents, connect start directly to end
      positionedEdges.push(createEdge(startNode.id, endNode.id));
    }
  }

  // Position end node
  positionedNodes.push({
    ...endNode,
    position: { x: 100, y: 600 },
  });

  return {
    ...template,
    nodes: positionedNodes,
    edges: positionedEdges,
  };
}
