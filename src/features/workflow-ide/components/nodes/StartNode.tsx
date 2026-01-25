// ============================================================================
// START NODE
// BPMN-style thin circle event for workflow entry point
// ============================================================================

import { memo } from 'react';
import { Handle, Position, type NodeProps } from '@xyflow/react';

export const StartNode = memo(({ data, selected }: NodeProps) => {
  const label = data?.label as string | undefined;
  return (
    <div className="relative">
      {/* SVG Circle - Thin border (BPMN start event style) */}
      <svg
        width={60}
        height={60}
        className={`block transition-opacity ${selected ? 'opacity-80' : ''}`}
      >
        <circle
          cx={30}
          cy={30}
          r={26}
          fill="rgba(34, 197, 94, 0.1)"
          stroke="#22c55e"
          strokeWidth={2}
          className={`transition-all ${selected ? 'stroke-[3px]' : ''}`}
        />
      </svg>

      {/* Label below the circle */}
      {label != null && (
        <div className="absolute top-full left-1/2 -translate-x-1/2 mt-2 whitespace-nowrap">
          <span className="text-xs text-gray-300">{label}</span>
        </div>
      )}

      {/* Output handle - at bottom for outgoing connections */}
      <Handle
        type="source"
        position={Position.Bottom}
        className="!bg-green-500 !border-green-600 !w-3 !h-3"
      />
    </div>
  );
});

StartNode.displayName = 'StartNode';
