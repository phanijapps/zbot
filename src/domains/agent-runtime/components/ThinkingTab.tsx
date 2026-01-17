/**
 * ThinkingTab - Animated tab indicator for agent thinking
 *
 * Shows:
 * - Animated emoji when agent is working (isActive)
 * - Tool count badge when completed
 * - Click to toggle thinking panel
 */

import { cn } from "@/shared/utils";
import type { ThinkingTabProps } from "./types";

export function ThinkingTab({
  isActive,
  toolCount = 0,
  onClick,
}: ThinkingTabProps) {
  // Don't render if nothing to show
  if (!isActive && toolCount === 0) {
    return null;
  }

  return (
    <button
      onClick={onClick}
      className={cn(
        "relative flex items-center gap-2 px-3 py-1.5 rounded-lg",
        "text-sm font-medium transition-all duration-200",
        "hover:bg-white/5 active:scale-95",
        isActive
          ? "text-purple-400 bg-purple-500/10"
          : "text-gray-400 hover:text-gray-300"
      )}
      aria-label={isActive ? "Agent is thinking" : `Used ${toolCount} tools`}
    >
      {/* Brain emoji with animation */}
      <span
        className={cn(
          "text-base",
          isActive && "animate-thinking-pulse"
        )}
      >
        🧠
      </span>

      {/* Status text */}
      {isActive ? (
        <span className="thinking-breathe">Thinking</span>
      ) : toolCount > 0 ? (
        <span className="text-xs text-gray-500">
          Used {toolCount} tool{toolCount !== 1 ? "s" : ""}
        </span>
      ) : null}

      {/* Active indicator dot */}
      {isActive && (
        <span
          className="absolute top-1 right-1 size-2 bg-purple-500 rounded-full animate-pulse"
          aria-hidden="true"
        />
      )}
    </button>
  );
}

// ============================================================================
// CSS ANIMATIONS
// Add these to your index.css or a component-specific CSS file
// ============================================================================

/*
.thinking-pulse animation - gentle opacity pulse
@keyframes thinking-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.thinking-breathe animation - subtle scale breathe
@keyframes thinking-breathe {
  0%, 100% { transform: scale(1); }
  50% { transform: scale(1.05); }
}

.animate-thinking-pulse {
  animation: thinking-pulse 2s ease-in-out infinite;
}

.thinking-breathe {
  animation: thinking-breathe 2s ease-in-out infinite;
  display: inline-block;
}
*/
