import { memo } from 'react';
import { Handle, Position, NodeProps } from '@xyflow/react';
import { Crown, Loader2, CheckCircle2, XCircle, AlertTriangle, AlertCircle } from 'lucide-react';
import { cn } from '@/core/utils/cn';
import { useWorkflowStore } from '../../stores/workflowStore';

type ExecutionStatus = 'idle' | 'running' | 'completed' | 'failed';

export const OrchestratorNode = memo(({ id, data, selected }: NodeProps) => {
  const description = data.description as string | undefined;
  const displayName = data.displayName as string | undefined;
  const model = data.model as string | undefined;
  const providerId = data.providerId as string | undefined;
  const status = (data._executionStatus as ExecutionStatus) || 'idle';

  // Get validation state for this node
  const validation = useWorkflowStore((s) => s.validation.nodes[id || '']);

  // Status styles
  const getStatusStyles = () => {
    switch (status) {
      case 'running':
        return {
          border: 'border-blue-500 shadow-lg shadow-blue-500/30',
          header: 'from-blue-500 to-blue-600',
          icon: <Loader2 size={16} className="animate-spin" />,
        };
      case 'completed':
        return {
          border: 'border-green-500 shadow-lg shadow-green-500/30',
          header: 'from-green-500 to-green-600',
          icon: <CheckCircle2 size={16} />,
        };
      case 'failed':
        return {
          border: 'border-red-500 shadow-lg shadow-red-500/30',
          header: 'from-red-500 to-red-600',
          icon: <XCircle size={16} />,
        };
      default:
        return {
          border: 'border-gray-700',
          header: 'from-amber-500 to-orange-500',
          icon: <Crown size={16} />,
        };
    }
  };

  const statusStyles = getStatusStyles();

  return (
    <div
      className={cn(
        'rounded-lg border-2 min-w-[220px]',
        'transition-all duration-200',
        'bg-gray-800 border-gray-700',
        selected && status === 'idle' && 'border-amber-500 shadow-lg shadow-amber-500/20',
        statusStyles.border,
      )}
    >
      {/* Input Handle - at top for incoming connections */}
      <Handle
        type="target"
        position={Position.Top}
        className="!w-3 !h-3 !bg-amber-500 !border-2 !border-gray-800"
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
          <p className="text-xs text-gray-400 line-clamp-2">
            {description}
          </p>
        )}

        <div className="flex items-center gap-1">
          <span className={cn(
            'text-xs px-2 py-0.5 rounded font-medium',
            status === 'idle' ? 'bg-amber-500/20 text-amber-300' :
            status === 'running' ? 'bg-blue-500/20 text-blue-300' :
            status === 'completed' ? 'bg-green-500/20 text-green-300' :
            'bg-red-500/20 text-red-300'
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
            {status === 'running' && <span className="text-blue-400">Running...</span>}
            {status === 'completed' && <span className="text-green-400">Completed</span>}
            {status === 'failed' && <span className="text-red-400">Failed</span>}
          </div>
        )}

        {/* Validation indicators */}
        {validation && (validation.errors.length > 0 || validation.warnings.length > 0) && (
          <div className="flex flex-col gap-1 pt-2 border-t border-gray-700">
            {validation.errors.length > 0 && (
              <div className="flex items-center gap-1 text-xs text-red-400">
                <AlertCircle size={12} />
                <span>{validation.errors.length} error{validation.errors.length > 1 ? 's' : ''}</span>
              </div>
            )}
            {validation.warnings.length > 0 && (
              <div className="flex items-center gap-1 text-xs text-amber-400">
                <AlertTriangle size={12} />
                <span>{validation.warnings.length} warning{validation.warnings.length > 1 ? 's' : ''}</span>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Output Handle - at bottom for outgoing connections */}
      <Handle
        type="source"
        position={Position.Bottom}
        className="!w-3 !h-3 !bg-amber-500 !border-2 !border-gray-800"
      />
    </div>
  );
});

OrchestratorNode.displayName = 'OrchestratorNode';
