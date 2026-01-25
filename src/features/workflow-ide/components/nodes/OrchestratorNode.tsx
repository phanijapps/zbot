import { memo } from 'react';
import { Handle, Position, NodeProps } from '@xyflow/react';
import { Crown, Loader2, CheckCircle2, XCircle } from 'lucide-react';
import { cn } from '@/core/utils/cn';

type ExecutionStatus = 'idle' | 'running' | 'completed' | 'failed';

export const OrchestratorNode = memo(({ data, selected }: NodeProps) => {
  const description = data.description as string | undefined;
  const displayName = data.displayName as string | undefined;
  const model = data.model as string | undefined;
  const providerId = data.providerId as string | undefined;
  const status = (data._executionStatus as ExecutionStatus) || 'idle';

  // Status styles
  const getStatusStyles = () => {
    switch (status) {
      case 'running':
        return {
          border: 'border-blue-500 ring-2 ring-blue-200',
          header: 'from-blue-500 to-blue-600',
          icon: <Loader2 size={16} className="animate-spin" />,
        };
      case 'completed':
        return {
          border: 'border-green-500 ring-2 ring-green-200',
          header: 'from-green-500 to-green-600',
          icon: <CheckCircle2 size={16} />,
        };
      case 'failed':
        return {
          border: 'border-red-500 ring-2 ring-red-200',
          header: 'from-red-500 to-red-600',
          icon: <XCircle size={16} />,
        };
      default:
        return {
          border: selected ? 'border-amber-500' : 'border-gray-200',
          header: 'from-amber-500 to-orange-500',
          icon: <Crown size={16} />,
        };
    }
  };

  const statusStyles = getStatusStyles();

  return (
    <div
      className={cn(
        'rounded-lg border-2 bg-white shadow-md min-w-[220px]',
        'transition-all duration-200',
        selected && status === 'idle' && 'shadow-lg ring-2 ring-amber-200',
        statusStyles.border,
      )}
    >
      {/* Input Handle */}
      <Handle
        type="target"
        position={Position.Left}
        className="!w-3 !h-3 !bg-blue-500 !border-2 !border-white"
      />

      {/* Header - Distinguished styling for orchestrator with status */}
      <div className={cn(
        'flex items-center justify-between gap-2 px-3 py-2 text-white rounded-t-md',
        'bg-gradient-to-r',
        statusStyles.header
      )}>
        <span className="font-semibold text-sm">
          {displayName || 'Orchestrator'}
        </span>
        {statusStyles.icon}
      </div>

      {/* Body */}
      <div className="px-3 py-2 space-y-2">
        {description && (
          <p className="text-xs text-gray-500 line-clamp-2">
            {description}
          </p>
        )}

        <div className="flex items-center gap-1">
          <span className={cn(
            'text-xs px-2 py-0.5 rounded font-medium',
            status === 'idle' ? 'bg-amber-100 text-amber-700' :
            status === 'running' ? 'bg-blue-100 text-blue-700' :
            status === 'completed' ? 'bg-green-100 text-green-700' :
            'bg-red-100 text-red-700'
          )}>
            {model || 'No model'}
          </span>
        </div>

        {/* Provider info */}
        <div className="text-xs text-gray-400">
          Provider: {providerId || 'Not set'}
        </div>

        {/* Status text */}
        {status !== 'idle' && (
          <div className="text-xs font-medium text-center pt-1">
            {status === 'running' && <span className="text-blue-600">Running...</span>}
            {status === 'completed' && <span className="text-green-600">Completed</span>}
            {status === 'failed' && <span className="text-red-600">Failed</span>}
          </div>
        )}
      </div>

      {/* Output Handle */}
      <Handle
        type="source"
        position={Position.Right}
        className="!w-3 !h-3 !bg-green-500 !border-2 !border-white"
      />
    </div>
  );
});

OrchestratorNode.displayName = 'OrchestratorNode';
