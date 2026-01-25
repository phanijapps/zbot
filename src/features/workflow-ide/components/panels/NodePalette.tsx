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
    color: 'bg-amber-500/10 border-amber-500/30 text-amber-300 hover:bg-amber-500/20',
  },
  {
    type: 'subagent',
    label: 'Subagent',
    icon: <Bot size={18} />,
    description: 'Specialized worker agent',
    color: 'bg-purple-500/10 border-purple-500/30 text-purple-300 hover:bg-purple-500/20',
  },
  // Future node types:
  // {
  //   type: 'tool',
  //   label: 'Tool',
  //   icon: <Wrench size={18} />,
  //   description: 'Built-in or MCP tool',
  //   color: 'bg-green-500/10 border-green-500/30 text-green-300 hover:bg-green-500/20',
  // },
];

export const NodePalette: React.FC = () => {
  const onDragStart = (event: React.DragEvent, nodeType: string) => {
    event.dataTransfer.setData('application/workflow-node-type', nodeType);
    event.dataTransfer.effectAllowed = 'move';
  };

  return (
    <div className="w-64 border-r border-gray-800 bg-gray-900 p-4 overflow-y-auto">
      <h3 className="text-sm font-semibold text-white mb-4">Node Palette</h3>

      <div className="space-y-2">
        {nodeDefinitions.map((node) => (
          <div
            key={node.type}
            className={`
              flex items-center gap-3 p-3 rounded-lg border-2 cursor-grab
              transition-all duration-200 hover:shadow-lg
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

      <div className="mt-6 pt-4 border-t border-gray-800">
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
