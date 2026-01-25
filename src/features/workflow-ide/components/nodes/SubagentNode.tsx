import { memo } from 'react';
import { Handle, Position, NodeProps } from '@xyflow/react';
import { Bot, Wrench, Server } from 'lucide-react';
import { cn } from '@/core/utils/cn';

export const SubagentNode = memo(({ data, selected }: NodeProps) => {
  const hasTools = ((data.skills as string[])?.length ?? 0) > 0 || ((data.mcps as string[])?.length ?? 0) > 0;
  const description = data.description as string | undefined;
  const displayName = data.displayName as string | undefined;
  const subagentId = data.subagentId as string | undefined;
  const model = data.model as string | undefined;
  const skills = data.skills as string[] | undefined;
  const mcps = data.mcps as string[] | undefined;

  return (
    <div
      className={cn(
        'rounded-lg border-2 bg-white shadow-md min-w-[200px]',
        'transition-all duration-200',
        selected ? 'border-blue-500 shadow-lg' : 'border-gray-200',
      )}
    >
      {/* Input Handle */}
      <Handle
        type="target"
        position={Position.Left}
        className="!w-3 !h-3 !bg-blue-500 !border-2 !border-white"
      />

      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 bg-gradient-to-r from-purple-500 to-purple-600 text-white rounded-t-md">
        <Bot size={16} />
        <span className="font-medium text-sm truncate">
          {displayName || subagentId}
        </span>
      </div>

      {/* Body */}
      <div className="px-3 py-2 space-y-2">
        {/* Description */}
        {description && (
          <p className="text-xs text-gray-500 line-clamp-2">
            {description}
          </p>
        )}

        {/* Model Badge */}
        <div className="flex items-center gap-1">
          <span className="text-xs px-2 py-0.5 bg-gray-100 rounded text-gray-600">
            {model || 'No model'}
          </span>
        </div>

        {/* Tools/MCPs indicator */}
        {hasTools && (
          <div className="flex items-center gap-2 text-xs text-gray-500">
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

SubagentNode.displayName = 'SubagentNode';
