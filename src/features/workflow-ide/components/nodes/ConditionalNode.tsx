// ============================================================================
// CONDITIONAL NODE (DRAFT)
// BPMN-style diamond gateway for branching logic
// NOTE: This is a draft/placeholder for future implementation
// ============================================================================

import { memo } from 'react';
import { Handle, Position, type NodeProps } from '@xyflow/react';

/**
 * ConditionalNode - DRAFT/PLACEHOLDER
 *
 * TODO: Implement conditional branching logic
 * - Visual: Diamond shape (BPMN gateway style)
 * - Handles: Top (input), Bottom/Sides (outputs for different branches)
 * - Properties: Conditions, branch mappings
 * - Execution: Evaluate conditions and route to appropriate branch
 *
 * Example use cases:
 * - Route based on user input type
 * - Branch based on API response status
 * - Conditional subagent selection
 */
export const ConditionalNode = memo(({ data, selected }: NodeProps) => {
  const label = data?.label as string | undefined;
  // const condition = data?.condition as string | undefined; // TODO: Display condition on node

  return (
    <div className="relative">
      {/* Input Handle - at top for incoming connections */}
      <Handle
        type="target"
        position={Position.Top}
        className="!bg-amber-500 !border-amber-600 !w-3 !h-3"
      />

      {/* SVG Diamond - BPMN gateway style */}
      <svg
        width={80}
        height={80}
        className={`block transition-opacity ${selected ? 'opacity-80' : ''}`}
      >
        {/* Diamond shape using rotated square or polygon */}
        <polygon
          points="40,5 75,40 40,75 5,40"
          fill="rgba(245, 158, 11, 0.1)"
          stroke="#f59e0b"
          strokeWidth={2}
          className={`transition-all ${selected ? 'stroke-[3px]' : ''}`}
        />
        {/* Question mark icon inside */}
        <text
          x={40}
          y={47}
          textAnchor="middle"
          fontSize={20}
          fill="#f59e0b"
          fontWeight="bold"
        >
          ?
        </text>
      </svg>

      {/* Label below the diamond */}
      {label && (
        <div className="absolute top-full left-1/2 -translate-x-1/2 mt-2 whitespace-nowrap">
          <span className="text-xs text-gray-300">{label}</span>
        </div>
      )}

      {/* Draft indicator badge */}
      <div className="absolute -top-2 -right-2 bg-amber-500 text-white text-[10px] px-1.5 py-0.5 rounded font-semibold">
        DRAFT
      </div>

      {/* Output Handles - at bottom and sides for different branches */}
      {/* Primary branch (bottom) */}
      <Handle
        type="source"
        position={Position.Bottom}
        id="default"
        className="!bg-green-500 !border-green-600 !w-3 !h-3"
      />
      {/* Alternative branch (right) */}
      <Handle
        type="source"
        position={Position.Right}
        id="alternative"
        className="!bg-blue-500 !border-blue-600 !w-3 !h-3"
      />
    </div>
);
});

ConditionalNode.displayName = 'ConditionalNode';
