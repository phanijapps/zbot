import React, { useState, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { X, Loader2 } from 'lucide-react';
import { cn } from '@/core/utils/cn';

interface ConfigItem {
  id: string;
  name: string;
  description?: string;
}

interface NodeConfigPopoverProps {
  isOpen: boolean;
  onClose: () => void;
  anchorEl: HTMLElement | null;
  title: string;
  icon: React.ReactNode;
  iconColor: string;
  items: ConfigItem[];
  selectedIds: string[];
  onSelectionChange: (ids: string[]) => void;
  loading?: boolean;
  emptyMessage?: string;
  // For middleware - single text input instead of checkboxes
  isTextInput?: boolean;
  textValue?: string;
  onTextChange?: (value: string) => void;
  textPlaceholder?: string;
}

export const NodeConfigPopover: React.FC<NodeConfigPopoverProps> = ({
  isOpen,
  onClose,
  anchorEl,
  title,
  icon,
  iconColor,
  items,
  selectedIds,
  onSelectionChange,
  loading = false,
  emptyMessage = 'No items available',
  isTextInput = false,
  textValue = '',
  onTextChange,
  textPlaceholder = '',
}) => {
  const popoverRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ top: 0, left: 0 });

  // Calculate position based on anchor element
  useEffect(() => {
    if (!anchorEl || !isOpen) return;

    const rect = anchorEl.getBoundingClientRect();
    const popoverWidth = 280;
    const popoverHeight = isTextInput ? 200 : 300;

    // Position below the chip, centered
    let left = rect.left + rect.width / 2 - popoverWidth / 2;
    let top = rect.bottom + 8;

    // Adjust if going off screen
    if (left < 10) left = 10;
    if (left + popoverWidth > window.innerWidth - 10) {
      left = window.innerWidth - popoverWidth - 10;
    }
    if (top + popoverHeight > window.innerHeight - 10) {
      top = rect.top - popoverHeight - 8;
    }

    setPosition({ top, left });
  }, [anchorEl, isOpen, isTextInput]);

  // Close on click outside
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        // Don't close if clicking the anchor
        if (anchorEl && anchorEl.contains(e.target as Node)) return;
        onClose();
      }
    };

    // Delay to prevent immediate close
    const timer = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 0);

    return () => {
      clearTimeout(timer);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen, onClose, anchorEl]);

  // Close on escape
  useEffect(() => {
    if (!isOpen) return;

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };

    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const handleToggle = (id: string) => {
    if (selectedIds.includes(id)) {
      onSelectionChange(selectedIds.filter(i => i !== id));
    } else {
      onSelectionChange([...selectedIds, id]);
    }
  };

  const content = (
    <div
      ref={popoverRef}
      className="fixed z-[9999] w-[280px] bg-gray-900 border border-gray-700 rounded-lg shadow-2xl overflow-hidden"
      style={{ top: position.top, left: position.left }}
      onClick={(e) => e.stopPropagation()}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 bg-gray-800 border-b border-gray-700">
        <div className="flex items-center gap-2">
          <span className={iconColor}>{icon}</span>
          <span className="text-sm font-medium text-white">{title}</span>
          {!isTextInput && selectedIds.length > 0 && (
            <span className="text-xs text-gray-400">({selectedIds.length})</span>
          )}
        </div>
        <button
          onClick={onClose}
          className="p-1 text-gray-400 hover:text-white hover:bg-gray-700 rounded"
        >
          <X size={14} />
        </button>
      </div>

      {/* Content */}
      <div className={cn(
        "p-3",
        isTextInput ? "min-h-[150px]" : "max-h-[250px] overflow-y-auto"
      )}>
        {loading ? (
          <div className="flex items-center justify-center gap-2 py-8 text-gray-500">
            <Loader2 size={16} className="animate-spin" />
            <span className="text-sm">Loading...</span>
          </div>
        ) : isTextInput ? (
          <textarea
            className="w-full h-[130px] px-3 py-2 bg-gray-800 border border-gray-700 rounded-md text-xs font-mono text-white focus:border-purple-500 focus:outline-none resize-none"
            value={textValue}
            onChange={(e) => onTextChange?.(e.target.value)}
            placeholder={textPlaceholder}
          />
        ) : items.length === 0 ? (
          <p className="text-sm text-gray-500 text-center py-4">{emptyMessage}</p>
        ) : (
          <div className="space-y-1">
            {items.map((item) => (
              <label
                key={item.id}
                className={cn(
                  "flex items-start gap-3 p-2 rounded-md cursor-pointer transition-colors",
                  "hover:bg-gray-800",
                  selectedIds.includes(item.id) && "bg-gray-800"
                )}
              >
                <input
                  type="checkbox"
                  className="mt-0.5 rounded border-gray-600 bg-gray-700 text-purple-500 focus:ring-purple-500 focus:ring-offset-0"
                  checked={selectedIds.includes(item.id)}
                  onChange={() => handleToggle(item.id)}
                />
                <div className="flex-1 min-w-0">
                  <span className="text-sm text-white block">{item.name}</span>
                  {item.description && (
                    <span className="text-xs text-gray-500 block truncate">{item.description}</span>
                  )}
                </div>
              </label>
            ))}
          </div>
        )}
      </div>
    </div>
  );

  // Render in portal to escape XY Flow's transform
  return createPortal(content, document.body);
};
