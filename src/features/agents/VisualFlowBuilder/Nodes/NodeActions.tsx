// ============================================================================
// ZERO IDE - NODE ACTIONS
// Shared action buttons for all node types (delete, edit, etc.)
// ============================================================================

import { memo } from "react";

// -----------------------------------------------------------------------------
// Props
// -----------------------------------------------------------------------------

interface NodeActionsProps {
  onDelete: () => void;
  onEdit?: () => void;
  className?: string;
}

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const TrashIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M3 6h18M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
  </svg>
);

const EditIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
    <path d="m15 5 4 4" />
  </svg>
);

// -----------------------------------------------------------------------------
// Node Actions Component
// -----------------------------------------------------------------------------

export const NodeActions = memo(({ onDelete, onEdit, className = "" }: NodeActionsProps) => {
  return (
    <div className={`flex gap-1 ${className}`}>
      {/* Edit Button */}
      {onEdit && (
        <button
          className="w-6 h-6 rounded-full bg-gray-700 text-white flex items-center justify-center hover:bg-gray-600 shadow-lg transition-colors"
          onClick={(e) => {
            e.stopPropagation();
            onEdit();
          }}
          title="Edit"
        >
          <EditIcon />
        </button>
      )}

      {/* Delete Button */}
      <button
        className="w-6 h-6 rounded-full bg-red-500 text-white flex items-center justify-center hover:bg-red-600 shadow-lg transition-colors"
        onClick={(e) => {
          e.stopPropagation();
          onDelete();
        }}
        title="Delete"
      >
        <TrashIcon />
      </button>
    </div>
  );
});

NodeActions.displayName = "NodeActions";
