// ============================================================================
// ZERO IDE - TOOLS CONFIGURATION DIALOG
// Modal dialog for configuring tool selections
// ============================================================================

import { memo, useState, useEffect, useCallback } from "react";
import type { ToolSelection } from "../types";
import { TOOL_CATEGORIES_CONFIG } from "../constants";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/shared/ui/dialog";
import { Button } from "@/shared/ui/button";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const CheckIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2.5" viewBox="0 0 24 24">
    <path d="M20 6 9 17l-5-5" />
  </svg>
);

const ChevronDownIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="m6 9 6 6 6-6" />
  </svg>
);

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface ToolsConfigDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (tools: ToolSelection) => void;
  initialTools: ToolSelection;
}

interface ToolCategoryState {
  enabled: boolean;
  tools: Record<string, boolean>;
}

// -----------------------------------------------------------------------------
// Helper: Initialize tool state with all tools selected by default
// -----------------------------------------------------------------------------

function initializeToolState(tools: ToolSelection): Record<string, ToolCategoryState> {
  const state: Record<string, ToolCategoryState> = {};

  for (const [key, categoryConfig] of Object.entries(TOOL_CATEGORIES_CONFIG)) {
    const existingCategory = tools[key as keyof typeof TOOL_CATEGORIES_CONFIG];

    // If category exists and is enabled
    if (existingCategory?.enabled) {
      // If tools object is empty, treat as "all tools enabled" (default behavior)
      const existingTools = existingCategory.tools || {};
      const hasExplicitSelection = Object.keys(existingTools).length > 0;

      state[key] = {
        enabled: true,
        tools: hasExplicitSelection
          ? existingTools
          : Object.keys(categoryConfig.tools).reduce((acc, toolKey) => ({
              ...acc,
              [toolKey]: true
            }), {}),
      };
    } else {
      // Default: enable all tools
      state[key] = {
        enabled: true,
        tools: Object.keys(categoryConfig.tools).reduce((acc, toolKey) => ({
          ...acc,
          [toolKey]: true
        }), {}),
      };
    }
  }

  return state;
}

// -----------------------------------------------------------------------------
// Tool Category Component
// -----------------------------------------------------------------------------

interface ConfigCategoryProps {
  categoryKey: string;
  category: typeof TOOL_CATEGORIES_CONFIG[keyof typeof TOOL_CATEGORIES_CONFIG];
  state: ToolCategoryState;
  onToggleCategory: (key: string) => void;
  onToggleTool: (categoryKey: string, toolKey: string) => void;
}

const ConfigCategory = memo(({ categoryKey, category, state, onToggleCategory, onToggleTool }: ConfigCategoryProps) => {
  const [isExpanded, setIsExpanded] = useState(true);
  const selectedCount = Object.values(state.tools).filter(v => v).length;
  const totalCount = Object.keys(category.tools).length;

  return (
    <div className="border border-white/10 rounded-lg overflow-hidden">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between px-3 py-2 bg-white/5 hover:bg-white/10 transition-colors"
      >
        <div className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={state.enabled}
            onChange={() => onToggleCategory(categoryKey)}
            className="rounded"
            onClick={(e) => e.stopPropagation()}
          />
          <span className={`text-lg ${category.color}`}>{category.icon}</span>
          <span className="text-sm font-medium text-white">{category.label}</span>
          <span className="text-xs text-gray-500">{category.description}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className={`text-xs px-2 py-0.5 rounded ${
            state.enabled
              ? "bg-violet-500/20 text-violet-400"
              : "bg-white/5 text-gray-500"
          }`}>
            {selectedCount}/{totalCount}
          </span>
          <span className={`transition-transform ${isExpanded ? "rotate-180" : ""}`}>
            <ChevronDownIcon />
          </span>
        </div>
      </button>
      {isExpanded && state.enabled && (
        <div className="p-2 border-t border-white/10 space-y-1 max-h-60 overflow-y-auto">
          {Object.entries(category.tools).map(([toolKey, tool]) => {
            const isEnabled = state.tools[toolKey] ?? false;
            return (
              <div
                key={toolKey}
                className={`flex items-center gap-2 px-2 py-2 rounded cursor-pointer transition-colors ${
                  isEnabled ? "bg-violet-500/10" : "hover:bg-white/5"
                }`}
                onClick={() => onToggleTool(categoryKey, toolKey)}
              >
                <input
                  type="checkbox"
                  checked={isEnabled}
                  onChange={() => onToggleTool(categoryKey, toolKey)}
                  className="rounded"
                  onClick={(e) => e.stopPropagation()}
                />
                {isEnabled && <CheckIcon />}
                <div className="flex-1">
                  <p className="text-xs text-white">{tool.name}</p>
                  <p className="text-[10px] text-gray-500">{tool.description}</p>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
});

ConfigCategory.displayName = "ConfigCategory";

// -----------------------------------------------------------------------------
// Main Dialog Component
// -----------------------------------------------------------------------------

export const ToolsConfigDialog = memo(({ open, onClose, onSave, initialTools }: ToolsConfigDialogProps) => {
  const [toolState, setToolState] = useState<Record<string, ToolCategoryState>>({});

  // Initialize state when dialog opens
  useEffect(() => {
    if (open) {
      setToolState(initializeToolState(initialTools));
    }
  }, [open, initialTools]);

  const handleToggleCategory = useCallback((categoryKey: string) => {
    setToolState(prev => {
      const category = prev[categoryKey];
      const newEnabled = !category.enabled;

      return {
        ...prev,
        [categoryKey]: {
          ...category,
          enabled: newEnabled,
          // When enabling, select all tools; when disabling, clear all
          tools: newEnabled
            ? Object.keys(TOOL_CATEGORIES_CONFIG[categoryKey as keyof typeof TOOL_CATEGORIES_CONFIG].tools)
                .reduce((acc, key) => ({ ...acc, [key]: true }), {})
            : {},
        },
      };
    });
  }, []);

  const handleToggleTool = useCallback((categoryKey: string, toolKey: string) => {
    setToolState(prev => {
      const category = prev[categoryKey];
      return {
        ...prev,
        [categoryKey]: {
          ...category,
          tools: {
            ...category.tools,
            [toolKey]: !category.tools[toolKey],
          },
        },
      };
    });
  }, []);

  const handleSave = useCallback(() => {
    // Convert state to ToolSelection format
    const tools: ToolSelection = {};
    for (const [key, state] of Object.entries(toolState)) {
      if (state.enabled) {
        tools[key as keyof ToolSelection] = {
          enabled: true,
          tools: state.tools,
        };
      }
    }
    onSave(tools);
    onClose();
  }, [toolState, onSave, onClose]);

  // Calculate totals
  const totalCategories = Object.values(toolState).filter(c => c.enabled).length;
  const totalTools = Object.values(toolState).reduce((sum, cat) => {
    return sum + Object.values(cat.tools).filter(v => v).length;
  }, 0);

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="bg-[#141414] border-white/10 text-white max-w-lg max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="text-lg font-semibold">Configure Tools</DialogTitle>
          <p className="text-sm text-gray-400">Select which tools this agent can use</p>
        </DialogHeader>

        <div className="space-y-3 mt-4">
          {Object.entries(TOOL_CATEGORIES_CONFIG).map(([key, category]) => (
            <ConfigCategory
              key={key}
              categoryKey={key}
              category={category}
              state={toolState[key] || { enabled: false, tools: {} }}
              onToggleCategory={handleToggleCategory}
              onToggleTool={handleToggleTool}
            />
          ))}
        </div>

        <div className="flex items-center justify-between pt-4 border-t border-white/10 mt-4">
          <p className="text-xs text-gray-400">
            {totalCategories} categories, {totalTools} tools selected
          </p>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={onClose}
              className="border-white/20 text-white hover:bg-white/5"
            >
              Cancel
            </Button>
            <Button
              size="sm"
              onClick={handleSave}
              className="bg-violet-600 hover:bg-violet-700"
            >
              Save
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
});

ToolsConfigDialog.displayName = "ToolsConfigDialog";
