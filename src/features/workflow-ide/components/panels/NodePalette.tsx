import React from 'react';
import { Crown, Bot } from 'lucide-react';

interface NodeTypeDefinition {
  type: string;
  label: string;
  icon: React.ReactNode;
  description: string;
  color: string;
}

const nodeDefinitions: NodeTypeDefinition[] = [
  {
    type: 'orchestrator',
    label: 'Orchestrator',
    icon: <Crown size={18} />,
    description: 'Main coordinating agent',
    color: 'bg-amber-100 border-amber-300 text-amber-700',
  },
  {
    type: 'subagent',
    label: 'Subagent',
    icon: <Bot size={18} />,
    description: 'Specialized worker agent',
    color: 'bg-purple-100 border-purple-300 text-purple-700',
  },
  // Future node types:
  // {
  //   type: 'tool',
  //   label: 'Tool',
  //   icon: <Wrench size={18} />,
  //   description: 'Built-in or MCP tool',
  //   color: 'bg-green-100 border-green-300 text-green-700',
  // },
];

export const NodePalette: React.FC = () => {
  const onDragStart = (event: React.DragEvent, nodeType: string) => {
    event.dataTransfer.setData('application/workflow-node-type', nodeType);
    event.dataTransfer.effectAllowed = 'move';
  };

  return (
    <div className="w-64 border-r bg-gray-50 p-4 overflow-y-auto">
      <h3 className="text-sm font-semibold text-gray-700 mb-4">Node Palette</h3>

      <div className="space-y-2">
        {nodeDefinitions.map((node) => (
          <div
            key={node.type}
            className={`
              flex items-center gap-3 p-3 rounded-lg border-2 cursor-grab
              transition-all duration-200 hover:shadow-md
              ${node.color}
            `}
            draggable
            onDragStart={(e) => onDragStart(e, node.type)}
          >
            <div className="flex-shrink-0">{node.icon}</div>
            <div className="min-w-0">
              <div className="font-medium text-sm">{node.label}</div>
              <div className="text-xs opacity-70 truncate">{node.description}</div>
            </div>
          </div>
        ))}
      </div>

      <div className="mt-6 pt-4 border-t">
        <h4 className="text-xs font-semibold text-gray-500 mb-2 uppercase">
          Instructions
        </h4>
        <p className="text-xs text-gray-500">
          Drag nodes onto the canvas to build your workflow. Connect nodes by
          dragging from output handles (right) to input handles (left).
        </p>
      </div>
    </div>
  );
};
