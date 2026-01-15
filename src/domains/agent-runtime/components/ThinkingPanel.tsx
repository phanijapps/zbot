/**
 * ThinkingPanel - Right side panel showing agent thinking
 *
 * Displays:
 * - Execution plan (checklist) when planning module is active
 * - Tool calls with status
 * - Reasoning/blocks (if available)
 *
 * Responsive behavior:
 * - Desktop: Right side panel (30% width)
 * - Tablet: Bottom collapsible panel
 * - Mobile: Hidden (show modal on tap)
 */

import { X } from "lucide-react";
import { cn } from "@/shared/utils";
import { Button } from "@/shared/ui/button";
import { PlanSection } from "./PlanSection";
import { ToolCallsSection } from "./ToolCallsSection";
import type { ThinkingPanelProps } from "./types";

export function ThinkingPanel({
  isOpen,
  onClose,
  state,
}: ThinkingPanelProps) {
  return (
    <aside
      className={cn(
        "flex flex-col h-full bg-black/40",
        // Desktop: collapsible right panel - always render but animate width
        "transition-all duration-300 ease-in-out overflow-hidden",
        isOpen ? "w-80 shrink-0 opacity-100" : "w-0 opacity-0"
      )}
    >
      {/* Header - h-14 to match ConversationHeader */}
      <div className="h-14 flex items-center justify-between px-4 border-b border-white/10 shrink-0">
        <div className="flex items-center gap-2">
          <span className="text-lg">🧠</span>
          <span className="text-sm font-medium text-white">
            {state.isActive ? "Thinking" : "Thought Process"}
          </span>
          {state.isActive && (
            <span className="size-2 bg-purple-500 rounded-full animate-pulse" />
          )}
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={onClose}
          className="h-7 w-7 p-0 text-gray-400 hover:text-white"
        >
          <X className="size-4" />
        </Button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4 space-y-6">
        {/* Plan Section */}
        {state.hasPlan && state.planItems.length > 0 && (
          <PlanSection planItems={state.planItems} />
        )}

        {/* Tool Calls Section */}
        {state.toolCalls.length > 0 && (
          <ToolCallsSection toolCalls={state.toolCalls} />
        )}

        {/* Empty State - only show when agent was active but produced no output */}
        {state.isActive && !state.hasPlan && state.planItems.length === 0 && state.toolCalls.length === 0 && (
          <div className="text-center py-8">
            <div className="flex justify-center mb-3">
              <div className="size-8 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
            </div>
            <p className="text-sm text-gray-500">Agent is working...</p>
          </div>
        )}

        {/* Reasoning blocks (if available) */}
        {state.reasoning.length > 0 && (
          <div className="space-y-3">
            <div className="flex items-center gap-2 text-sm font-medium text-gray-300">
              <span>🤔</span>
              <span>Reasoning</span>
            </div>
            <div className="space-y-2">
              {state.reasoning.map((block, index) => (
                <div
                  key={index}
                  className="p-3 bg-white/5 rounded-lg text-sm text-gray-400 leading-relaxed"
                >
                  {block}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Footer - Status info */}
      <div className="px-4 py-3 border-t border-white/10">
        <div className="text-xs text-gray-500 text-center">
          {state.toolCalls.length > 0
            ? `${state.toolCalls.length} tool${state.toolCalls.length !== 1 ? "s" : ""} used`
            : state.isActive
            ? "Agent is working..."
            : "Ready"}
        </div>
      </div>
    </aside>
  );
}

// ============================================================================
// RESPONSIVE VARIANTS
// ============================================================================

/**
 * ThinkingPanel for tablet devices (bottom panel)
 */
export function ThinkingPanelTablet({
  isOpen,
  onClose,
  state,
}: ThinkingPanelProps) {
  if (!isOpen) {
    return null;
  }

  return (
    <div className="border-t border-white/10 bg-black/40 animate-panel-slide-up">
      {/* Collapsed header when closed */}
      {!isOpen && (
        <button
          onClick={onClose}
          className="w-full px-4 py-3 flex items-center justify-between text-sm text-gray-400 hover:text-white"
        >
          <span className="flex items-center gap-2">
            <span>🧠</span>
            {state.isActive ? (
              <span>Thinking...</span>
            ) : (
              <span>
                {state.toolCalls.length} tool
                {state.toolCalls.length !== 1 ? "s" : ""} used
              </span>
            )}
          </span>
          <span className="text-xs">▼</span>
        </button>
      )}

      {/* Expanded content */}
      {isOpen && (
        <div className="max-h-80 overflow-y-auto">
          <ThinkingPanel isOpen={isOpen} onClose={onClose} state={state} />
        </div>
      )}
    </div>
  );
}

/**
 * ThinkingPanel for mobile devices (modal)
 */
export function ThinkingPanelMobile({
  isOpen,
  onClose,
  state,
}: ThinkingPanelProps) {
  if (!isOpen) {
    return null;
  }

  return (
    <div
      className={cn(
        "fixed inset-0 z-50 flex items-end justify-center sm:items-center",
        "bg-black/60 backdrop-blur-sm",
        "animate-fade-in"
      )}
      onClick={onClose}
    >
      <div
        className={cn(
          "w-full max-w-lg bg-[#1a1a2e] rounded-t-2xl sm:rounded-2xl",
          "max-h-[80vh] overflow-hidden flex flex-col",
          "animate-slide-up"
        )}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
          <div className="flex items-center gap-2">
            <span className="text-lg">🧠</span>
            <span className="text-sm font-medium text-white">
              {state.isActive ? "Thinking" : "Thought Process"}
            </span>
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={onClose}
            className="h-7 w-7 p-0 text-gray-400 hover:text-white"
          >
            <X className="size-4" />
          </Button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          <ThinkingPanel isOpen={true} onClose={onClose} state={state} />
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// CSS ANIMATIONS
// Add these to your index.css
// ============================================================================

/*
@keyframes panel-slide-in {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes panel-slide-up {
  from {
    transform: translateY(100%);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

.animate-panel-slide-in {
  animation: panel-slide-in 0.3s ease-out;
}

.animate-panel-slide-up {
  animation: panel-slide-up 0.3s ease-out;
}

.animate-fade-in {
  animation: fade-in 0.2s ease-out;
}
*/
