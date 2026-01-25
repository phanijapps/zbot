import { memo } from 'react';
import { Handle, Position, NodeProps } from '@xyflow/react';
import { Crown } from 'lucide-react';
import { cn } from '@/core/utils/cn';

export const OrchestratorNode = memo(({ data, selected }: NodeProps) => {
  const description = data.description as string | undefined;
  const displayName = data.displayName as string | undefined;
  const model = data.model as string | undefined;
  const providerId = data.providerId as string | undefined;

  return (
    <div
      className={cn(
        'rounded-lg border-2 bg-white shadow-md min-w-[220px]',
        'transition-all duration-200',
        selected ? 'border-amber-500 shadow-lg ring-2 ring-amber-200' : 'border-gray-200',
      )}
    >
      {/* Input Handle */}
      <Handle
        type="target"
        position={Position.Left}
        className="!w-3 !h-3 !bg-blue-500 !border-2 !border-white"
      />

      {/* Header - Distinguished styling for orchestrator */}
      <div className="flex items-center gap-2 px-3 py-2 bg-gradient-to-r from-amber-500 to-orange-500 text-white rounded-t-md">
        <Crown size={16} />
        <span className="font-semibold text-sm">
          {displayName || 'Orchestrator'}
        </span>
      </div>

      {/* Body */}
      <div className="px-3 py-2 space-y-2">
        {description && (
          <p className="text-xs text-gray-500 line-clamp-2">
            {description}
          </p>
        )}

        <div className="flex items-center gap-1">
          <span className="text-xs px-2 py-0.5 bg-amber-100 rounded text-amber-700 font-medium">
            {model || 'No model'}
          </span>
        </div>

        {/* Provider info */}
        <div className="text-xs text-gray-400">
          Provider: {providerId || 'Not set'}
        </div>
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
