// ============================================================================
// END NODE
// BPMN-style thick circle event for workflow exit point
// ============================================================================

import { memo } from 'react';
import { Handle, Position, type NodeProps } from '@xyflow/react';

export const EndNode = memo(({ data, selected }: NodeProps) => {
  const label = data?.label as string | undefined;
  return (
    <div className="relative">
      {/* Input handle - at top for incoming connections */}
      <Handle
        type="target"
        position={Position.Top}
        className="!bg-red-500 !border-red-600 !w-3 !h-3"
      />

      {/* SVG Circle - Thick border (BPMN end event style) */}
      <svg
        width={60}
        height={60}
        className={`block transition-opacity ${selected ? 'opacity-80' : ''}`}
      >
        <circle
          cx={30}
          cy={30}
          r={26}
          fill="rgba(239, 68, 68, 0.1)"
          stroke="#ef4444"
          strokeWidth={5}
          className={`transition-all ${selected ? 'stroke-[6px]' : ''}`}
        />
      </svg>

      {/* Label below the circle */}
      {label != null && (
        <div className="absolute top-full left-1/2 -translate-x-1/2 mt-2 whitespace-nowrap">
          <span className="text-xs text-gray-300">{label}</span>
        </div>
      )}
    </div>
  );
});

EndNode.displayName = 'EndNode';
