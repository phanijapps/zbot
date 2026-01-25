// ============================================================================
// ZERO IDE - SYSTEM INSTRUCTIONS DIALOG
// Modal dialog for editing system instructions with markdown support
// ============================================================================

import { memo, useCallback, useState, lazy, Suspense } from "react";
import { Dialog, DialogContent, DialogTitle } from "@/shared/ui/dialog";
import { Button } from "@/shared/ui/button";

// Lazy load MDEditor to reduce initial bundle size
const MDEditor = lazy(() =>
  import("@uiw/react-md-editor").then((module) => ({
    default: module.default,
  }))
);

// Import CSS for the markdown editor - these need to be imported eagerly
import "@uiw/react-md-editor/markdown-editor.css";
import "@uiw/react-markdown-preview/markdown.css";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface SystemInstructionsDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (instructions: string) => void;
  initialInstructions: string;
}

// -----------------------------------------------------------------------------
// Loading Component
// -----------------------------------------------------------------------------

const EditorLoading = () => (
  <div className="flex items-center justify-center h-[400px]">
    <div className="flex flex-col items-center gap-3">
      <div className="w-8 h-8 border-3 border-violet-500 border-t-transparent rounded-full animate-spin" />
      <p className="text-sm text-gray-500">Loading editor...</p>
    </div>
  </div>
);

// -----------------------------------------------------------------------------
// Main Dialog Component
// -----------------------------------------------------------------------------

export const SystemInstructionsDialog = memo(({
  open,
  onClose,
  onSave,
  initialInstructions,
}: SystemInstructionsDialogProps) => {
  const [content, setContent] = useState(initialInstructions);

  // Reset content when dialog opens
  const handleOpenChange = useCallback((isOpen: boolean) => {
    if (!isOpen) {
      onClose();
    } else {
      // Default to "You are a helpful AI Assistant" if empty
      setContent(initialInstructions || "You are a helpful AI Assistant");
    }
  }, [initialInstructions, onClose]);

  const handleSave = useCallback(() => {
    // Default to "You are a helpful AI Assistant" if empty
    const finalContent = content || "You are a helpful AI Assistant";
    onSave(finalContent);
    onClose();
  }, [content, onSave, onClose]);

  const hasUnsavedChanges = content !== (initialInstructions || "You are a helpful AI Assistant");

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="bg-[#141414] border-white/10 text-white w-[90vw] max-w-[90vw] sm:!max-w-[90vw] h-[90vh] sm:!max-h-[90vh] overflow-hidden flex flex-col p-0 [&>.absolute.top-4.right-4]:hidden">
        {/* Minimal header with title and actions */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10 flex-shrink-0 bg-[#141414]">
          <DialogTitle className="text-base font-semibold p-0">System Instructions</DialogTitle>
          <div className="flex items-center gap-2">
            {hasUnsavedChanges && (
              <span className="text-xs text-gray-400 mr-2">Unsaved changes</span>
            )}
            <Button
              variant="outline"
              size="sm"
              onClick={onClose}
              className="border-white/20 text-white hover:bg-white/5 h-7 px-3"
            >
              Cancel
            </Button>
            <Button
              size="sm"
              onClick={handleSave}
              className="bg-violet-600 hover:bg-violet-700 h-7 px-3"
              disabled={!hasUnsavedChanges}
            >
              Save
            </Button>
          </div>
        </div>

        {/* Editor */}
        <div className="flex-1 overflow-hidden flex flex-col min-h-0">
          <Suspense fallback={<EditorLoading />}>
            <div data-color-mode="dark" className="flex-1 flex flex-col">
              <MDEditor
                value={content}
                onChange={(val) => setContent(val || "")}
                height={500}
                preview="live"
                hideToolbar={false}
                visibleDragbar={false}
                textareaProps={{
                  placeholder: "You are a helpful AI assistant...\n\n# Markdown Support\nYou can use **bold**, *italic*, `code`, and more.\n\n## Examples\n\n- Use lists for clarity\n- Use code blocks for examples\n\n```javascript\nconsole.log('Hello');\n```",
                }}
                className="!bg-transparent !border-0"
              />
            </div>
          </Suspense>
        </div>
      </DialogContent>
    </Dialog>
  );
});

SystemInstructionsDialog.displayName = "SystemInstructionsDialog";
