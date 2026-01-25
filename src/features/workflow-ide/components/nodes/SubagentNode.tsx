import { memo } from 'react';
import { Handle, Position, NodeProps } from '@xyflow/react';
import { Bot, Wrench, Server, Loader2, CheckCircle2, XCircle } from 'lucide-react';
import { cn } from '@/core/utils/cn';

type ExecutionStatus = 'idle' | 'running' | 'completed' | 'failed';

export const SubagentNode = memo(({ data, selected }: NodeProps) => {
  const hasTools = ((data.skills as string[])?.length ?? 0) > 0 || ((data.mcps as string[])?.length ?? 0) > 0;
  const description = data.description as string | undefined;
  const displayName = data.displayName as string | undefined;
  const subagentId = data.subagentId as string | undefined;
  const model = data.model as string | undefined;
  const skills = data.skills as string[] | undefined;
  const mcps = data.mcps as string[] | undefined;
  const status = (data._executionStatus as ExecutionStatus) || 'idle';

  // Status styles
  const getStatusStyles = () => {
    switch (status) {
      case 'running':
        return {
          border: 'border-blue-500 shadow-lg shadow-blue-500/30',
          header: 'from-blue-500 to-blue-600',
          icon: <Loader2 size={14} className="animate-spin" />,
        };
      case 'completed':
        return {
          border: 'border-green-500 shadow-lg shadow-green-500/30',
          header: 'from-green-500 to-green-600',
          icon: <CheckCircle2 size={14} />,
        };
      case 'failed':
        return {
          border: 'border-red-500 shadow-lg shadow-red-500/30',
          header: 'from-red-500 to-red-600',
          icon: <XCircle size={14} />,
        };
      default:
        return {
          border: 'border-gray-700',
          header: 'from-purple-500 to-purple-600',
          icon: null,
        };
    }
  };

  const statusStyles = getStatusStyles();

  return (
    <div
      className={cn(
        'rounded-lg border-2 min-w-[200px]',
        'transition-all duration-200',
        'bg-gray-800 border-gray-700',
        selected && !statusStyles.border.includes('border-') && 'border-blue-500 shadow-lg shadow-blue-500/20',
        statusStyles.border,
      )}
    >
      {/* Input Handle */}
      <Handle
        type="target"
        position={Position.Left}
        className="!w-3 !h-3 !bg-purple-500 !border-2 !border-gray-800"
      />

      {/* Header */}
      <div className={cn(
        'flex items-center justify-between gap-2 px-3 py-2 text-white rounded-t-md',
        'bg-gradient-to-r',
        statusStyles.header
      )}>
        <div className="flex items-center gap-2">
          <Bot size={16} />
          <span className="font-medium text-sm truncate">
            {displayName || subagentId}
          </span>
        </div>
        {statusStyles.icon}
      </div>

      {/* Body */}
      <div className="px-3 py-2 space-y-2">
        {/* Description */}
        {description && (
          <p className="text-xs text-gray-400 line-clamp-2">
            {description}
          </p>
        )}

        {/* Model Badge */}
        <div className="flex items-center gap-1">
          <span className="text-xs px-2 py-0.5 bg-gray-700 rounded text-gray-300">
            {model || 'No model'}
          </span>
        </div>

        {/* Tools/MCPs indicator */}
        {hasTools && (
          <div className="flex items-center gap-2 text-xs text-gray-400">
            {skills && skills.length > 0 && (
              <span className="flex items-center gap-1">
                <Wrench size={10} />
                {skills.length}
              </span>
            )}
            {mcps && mcps.length > 0 && (
              <span className="flex items-center gap-1">
                <Server size={10} />
                {mcps.length}
              </span>
            )}
          </div>
        )}

        {/* Status text */}
        {status !== 'idle' && (
          <div className="text-xs font-medium text-center">
            {status === 'running' && <span className="text-blue-400">Running...</span>}
            {status === 'completed' && <span className="text-green-400">Completed</span>}
            {status === 'failed' && <span className="text-red-400">Failed</span>}
          </div>
        )}
      </div>

      {/* Output Handle */}
      <Handle
        type="source"
        position={Position.Right}
        className="!w-3 !h-3 !bg-purple-500 !border-2 !border-gray-800"
      />
    </div>
  );
});

SubagentNode.displayName = 'SubagentNode';
