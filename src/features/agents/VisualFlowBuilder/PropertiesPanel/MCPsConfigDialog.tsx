// ============================================================================
// ZERO IDE - MCPS CONFIGURATION DIALOG
// Modal dialog for configuring MCP server selections
// ============================================================================

import { memo, useState, useEffect, useCallback } from "react";
import type { MCPServer } from "../hooks/useAgentResources";
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

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface MCPsConfigDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (mcps: string[]) => void;
  availableMCPs: MCPServer[];
  initialMCPs: string[];
}

// -----------------------------------------------------------------------------
// MCP Item Component
// -----------------------------------------------------------------------------

interface MCPItemProps {
  mcp: MCPServer;
  isSelected: boolean;
  onToggle: () => void;
}

const MCPItem = memo(({ mcp, isSelected, onToggle }: MCPItemProps) => {
  return (
    <div
      className={`flex items-center gap-3 p-3 rounded-lg cursor-pointer transition-colors ${
        isSelected
          ? "bg-blue-500/10 border border-blue-500/30"
          : "bg-white/5 hover:bg-white/10 border border-transparent"
      }`}
      onClick={onToggle}
    >
      <input
        type="checkbox"
        checked={isSelected}
        onChange={onToggle}
        className="rounded"
        onClick={(e) => e.stopPropagation()}
      />
      {isSelected && <CheckIcon />}
      <span style={{ fontSize: '20px' }}>🔌</span>
      <div className="flex-1 min-w-0">
        <p className="text-sm text-white font-medium">{mcp.name}</p>
        <p className="text-xs text-gray-500 truncate">{mcp.description}</p>
        {mcp.type && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-white/10 text-gray-400 inline-block mt-1">
            {mcp.type}
          </span>
        )}
      </div>
    </div>
  );
});

MCPItem.displayName = "MCPItem";

// -----------------------------------------------------------------------------
// Main Dialog Component
// -----------------------------------------------------------------------------

export const MCPsConfigDialog = memo(({
  open,
  onClose,
  onSave,
  availableMCPs,
  initialMCPs,
}: MCPsConfigDialogProps) => {
  const [selectedMCPs, setSelectedMCPs] = useState<Set<string>>(new Set());

  // Initialize state when dialog opens - default to empty (no MCPs selected)
  useEffect(() => {
    if (open) {
      // Use initialMCPs if provided, otherwise default to empty set
      setSelectedMCPs(new Set(initialMCPs));
    }
  }, [open, initialMCPs]);

  const handleToggle = useCallback((mcpId: string) => {
    setSelectedMCPs((prev) => {
      const next = new Set(prev);
      if (next.has(mcpId)) {
        next.delete(mcpId);
      } else {
        next.add(mcpId);
      }
      return next;
    });
  }, []);

  const handleSave = useCallback(() => {
    onSave(Array.from(selectedMCPs));
    onClose();
  }, [selectedMCPs, onSave, onClose]);

  const handleSelectAll = useCallback(() => {
    setSelectedMCPs(new Set(availableMCPs.map((m) => m.id || m.name)));
  }, [availableMCPs]);

  const handleClearAll = useCallback(() => {
    setSelectedMCPs(new Set());
  }, []);

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="bg-[#141414] border-white/10 text-white max-w-lg max-h-[80vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="text-lg font-semibold">Configure MCPs</DialogTitle>
          <p className="text-sm text-gray-400">Select which MCP servers this agent can use</p>
        </DialogHeader>

        <div className="flex gap-2 mb-4">
          <Button
            variant="outline"
            size="sm"
            onClick={handleSelectAll}
            className="border-white/20 text-white hover:bg-white/5 text-xs"
          >
            Select All
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleClearAll}
            className="border-white/20 text-white hover:bg-white/5 text-xs"
          >
            Clear All
          </Button>
        </div>

        <div className="flex-1 overflow-y-auto space-y-2 pr-2">
          {availableMCPs.length === 0 ? (
            <div className="text-center py-8">
              <p className="text-sm text-gray-500">No MCPs available</p>
              <p className="text-xs text-gray-600 mt-1">Add MCPs in Settings to see them here</p>
            </div>
          ) : (
            availableMCPs.map((mcp) => {
              const mcpId = mcp.id || mcp.name;
              return (
                <MCPItem
                  key={mcpId}
                  mcp={mcp}
                  isSelected={selectedMCPs.has(mcpId)}
                  onToggle={() => handleToggle(mcpId)}
                />
              );
            })
          )}
        </div>

        <div className="flex items-center justify-between pt-4 border-t border-white/10 mt-4">
          <p className="text-xs text-gray-400">
            {selectedMCPs.size} of {availableMCPs.length} MCPs selected
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
              className="bg-blue-600 hover:bg-blue-700"
            >
              Save
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
});

MCPsConfigDialog.displayName = "MCPsConfigDialog";
