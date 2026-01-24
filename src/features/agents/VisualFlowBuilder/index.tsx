// ============================================================================
// VISUAL FLOW BUILDER - MAIN COMPONENT
// Entry point for the visual workflow builder
// ============================================================================

import { useCallback, useEffect } from "react";
import { useCanvasState } from "./hooks/useCanvasState";
import { useAutoSave, loadSavedState } from "./hooks/useAutoSave";
import { useValidation } from "./hooks/useValidation";
import { InfiniteCanvas } from "./Canvas/InfiniteCanvas";
import { AssetsPanel } from "./Sidebar/AssetsPanel";
import { PropertiesPanel } from "./PropertiesPanel";
import { NODE_TEMPLATES } from "./constants";
import type { NodeType, BaseNode } from "./types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const FloppyIcon = () => (
  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" /><path d="M17 21v-8H7v8" /><path d="M7 3v5h8" />
  </svg>
);

// -----------------------------------------------------------------------------
// Helper: Generate unique ID
// -----------------------------------------------------------------------------

function generateId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

// -----------------------------------------------------------------------------
// Main Component Props
// -----------------------------------------------------------------------------

interface VisualFlowBuilderProps {
  agentId?: string;
  onSave?: (state: ReturnType<typeof useCanvasState>["state"]) => Promise<void> | void;
  initialNodes?: BaseNode[];
  readOnly?: boolean;
}

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export function VisualFlowBuilder({
  agentId,
  onSave,
  initialNodes,
  readOnly = false,
}: VisualFlowBuilderProps) {
  // Initialize state with saved data if available
  const getInitialState = useCallback(() => {
    const saved = loadSavedState();
    return {
      nodes: saved?.nodes ?? initialNodes ?? [],
      connections: saved?.connections ?? [],
      selectedNodeId: null,
      viewport: saved?.viewport ?? { x: 0, y: 0, zoom: 1 },
      validation: [],
    };
  }, [initialNodes]);

  const {
    state,
    addNode,
    updateNode,
    deleteNode,
    selectNode,
    setViewport,
  } = useCanvasState(getInitialState());

  // Auto-save hook
  const { saveStatus, forceSave } = useAutoSave(
    () => state,
    { onSave, enabled: !readOnly && !!onSave }
  );

  // Validation hook
  const { validationResults, overallStatus, counts } = useValidation(state);

  // Update validation when state changes
  useEffect(() => {
    // Could dispatch validation results here if needed
  }, [validationResults]);

  // -----------------------------------------------------------------------------
  // Add node handler
  // -----------------------------------------------------------------------------

  const handleAddNode = useCallback((type: NodeType, position: { x: number; y: number }) => {
    const template = NODE_TEMPLATES.find((t) => t.type === type);
    if (!template) return;

    const newNode: BaseNode = {
      id: generateId(type),
      type,
      position,
      data: { ...template.defaultData },
      selected: true,
      lastModified: Date.now(),
    };

    addNode(newNode);
    selectNode(newNode.id);
  }, [addNode, selectNode]);

  // -----------------------------------------------------------------------------
  // Update node handler
  // -----------------------------------------------------------------------------

  const handleUpdateNode = useCallback((nodeId: string, updates: Partial<BaseNode>) => {
    updateNode(nodeId, updates);
  }, [updateNode]);

  // -----------------------------------------------------------------------------
  // Save handler
  // -----------------------------------------------------------------------------

  const handleSave = useCallback(async () => {
    await forceSave();
  }, [forceSave]);

  // -----------------------------------------------------------------------------
  // Get selected node
  // -----------------------------------------------------------------------------

  const selectedNode = state.selectedNodeId
    ? state.nodes.find((n) => n.id === state.selectedNodeId) || null
    : null;

  // -----------------------------------------------------------------------------
  // Render
  // -----------------------------------------------------------------------------

  return (
    <div className="h-screen w-screen flex flex-col bg-[#0d0d0d] text-white overflow-hidden">
      {/* Top Bar */}
      <div className="h-12 border-b border-white/10 flex items-center justify-between px-4 bg-[#0d0d0d]">
        <div className="flex items-center gap-4">
          <h1 className="text-sm font-semibold">Visual Workflow Builder</h1>
          {agentId && (
            <span className="text-xs text-gray-500">
              Agent: {agentId}
            </span>
          )}
        </div>

        <div className="flex items-center gap-4">
          {/* Validation Status */}
          {overallStatus === "error" && (
            <div className="flex items-center gap-1.5 text-xs text-red-400">
              <span className="w-2 h-2 rounded-full bg-red-500" />
              {counts.errors} error{counts.errors !== 1 ? "s" : ""}
            </div>
          )}
          {overallStatus === "warning" && (
            <div className="flex items-center gap-1.5 text-xs text-yellow-400">
              <span className="w-2 h-2 rounded-full bg-yellow-500" />
              {counts.warnings} warning{counts.warnings !== 1 ? "s" : ""}
            </div>
          )}
          {overallStatus === "valid" && state.nodes.length > 0 && (
            <div className="flex items-center gap-1.5 text-xs text-green-400">
              <span className="w-2 h-2 rounded-full bg-green-500" />
              Valid
            </div>
          )}

          {/* Save Status */}
          <div className="flex items-center gap-1.5 text-xs">
            {saveStatus === "saved" && (
              <span className="text-gray-500">All changes saved</span>
            )}
            {saveStatus === "saving" && (
              <span className="text-blue-400 flex items-center gap-1.5">
                <span className="w-3 h-3 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
                Saving...
              </span>
            )}
            {saveStatus === "unsaved" && (
              <span className="text-yellow-400">Unsaved changes...</span>
            )}
          </div>

          {/* Save Button */}
          {!readOnly && (
            <button
              onClick={handleSave}
              disabled={saveStatus === "saving"}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-violet-600 hover:bg-violet-700 disabled:bg-violet-600/50 disabled:cursor-not-allowed rounded-lg text-xs font-medium transition-colors"
            >
              <FloppyIcon />
              Save
            </button>
          )}
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left Sidebar - Assets Panel */}
        {!readOnly && (
          <AssetsPanel />
        )}

        {/* Center - Infinite Canvas */}
        <InfiniteCanvas
          state={state}
          addNode={addNode}
          deleteNode={deleteNode}
          updateNode={updateNode}
          selectNode={selectNode}
          setViewport={setViewport}
          onAddNode={handleAddNode}
        />

        {/* Right Panel - Properties */}
        {!readOnly && (
          <PropertiesPanel
            node={selectedNode}
            onClose={() => selectNode(null)}
            onUpdate={(updates) => {
              if (selectedNode) {
                handleUpdateNode(selectedNode.id, updates);
              }
            }}
            validationResults={validationResults}
          />
        )}
      </div>

      {/* Keyboard Shortcuts Help */}
      <div className="absolute bottom-4 left-4 text-[10px] text-gray-600 space-y-0.5 pointer-events-none">
        <p>Space + Drag: Pan canvas</p>
        <p>Ctrl + Scroll: Zoom</p>
        <p>Delete: Remove selected node</p>
        <p>Escape: Deselect</p>
      </div>
    </div>
  );
}

// -----------------------------------------------------------------------------
// Export Component
// -----------------------------------------------------------------------------

export default VisualFlowBuilder;
export { FlowBuilderModal } from "./FlowBuilderModal";
