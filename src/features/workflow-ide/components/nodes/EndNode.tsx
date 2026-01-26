// ============================================================================
// END NODE
// BPMN-style thick circle event for workflow exit point
// ============================================================================

import { memo } from 'react';
import { Handle, Position, type NodeProps } from '@xyflow/react';
import { Loader2, CheckCircle2, XCircle } from 'lucide-react';

type ExecutionStatus = 'idle' | 'running' | 'completed' | 'failed';

export const EndNode = memo(({ data, selected }: NodeProps) => {
  const label = data?.label as string | undefined;
  const status = (data?._executionStatus as ExecutionStatus) || 'idle';

  // Get status-based styles
  const getStatusStyles = () => {
    switch (status) {
      case 'running':
        return {
          fill: 'rgba(59, 130, 246, 0.2)',
          stroke: '#3b82f6',
          glow: 'drop-shadow(0 0 8px rgba(59, 130, 246, 0.5))',
        };
      case 'completed':
        return {
          fill: 'rgba(34, 197, 94, 0.2)',
          stroke: '#22c55e',
          glow: 'drop-shadow(0 0 8px rgba(34, 197, 94, 0.5))',
        };
      case 'failed':
        return {
          fill: 'rgba(239, 68, 68, 0.2)',
          stroke: '#ef4444',
          glow: 'drop-shadow(0 0 8px rgba(239, 68, 68, 0.5))',
        };
      default:
        return {
          fill: 'rgba(239, 68, 68, 0.1)',
          stroke: '#ef4444',
          glow: '',
        };
    }
  };

  const statusStyles = getStatusStyles();

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
        style={{ filter: statusStyles.glow }}
      >
        <circle
          cx={30}
          cy={30}
          r={26}
          fill={statusStyles.fill}
          stroke={statusStyles.stroke}
          strokeWidth={status === 'running' ? 6 : 5}
          className={`transition-all ${selected ? 'stroke-[6px]' : ''}`}
        />
        {/* Status icon in center */}
        {status === 'running' && (
          <foreignObject x={18} y={18} width={24} height={24}>
            <Loader2 size={24} className="animate-spin text-blue-400" />
          </foreignObject>
        )}
        {status === 'completed' && (
          <foreignObject x={18} y={18} width={24} height={24}>
            <CheckCircle2 size={24} className="text-green-400" />
          </foreignObject>
        )}
        {status === 'failed' && (
          <foreignObject x={18} y={18} width={24} height={24}>
            <XCircle size={24} className="text-red-400" />
          </foreignObject>
        )}
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
