/**
 * DaySeparator - Collapsible day header for message groups
 *
 * Discord-style day separator that can be expanded/collapsed to show/hide messages
 */

import { memo } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { cn } from "@/shared/utils";

interface DaySeparatorProps {
  date: string;        // Display date (e.g., "Today", "Yesterday", "Jan 15")
  messageCount: number; // Number of messages in this day
  isExpanded: boolean;  // Whether the day is expanded
  onToggle: () => void; // Toggle expand/collapse
  summary?: string;     // Optional day summary
  className?: string;
}

export const DaySeparator = memo(function DaySeparator({
  date,
  messageCount,
  isExpanded,
  onToggle,
  summary,
  className,
}: DaySeparatorProps) {
  return (
    <button
      onClick={onToggle}
      className={cn(
        "w-full flex items-center gap-2 py-2 text-left hover:bg-accent rounded transition-colors group",
        className
      )}
      aria-expanded={isExpanded}
      aria-label={`${isExpanded ? 'Collapse' : 'Expand'} ${date}`}
    >
      {/* Expand/collapse icon */}
      <span className="text-muted-foreground group-hover:text-foreground transition-colors">
        {isExpanded ? (
          <ChevronDown className="size-4" aria-hidden="true" />
        ) : (
          <ChevronRight className="size-4" aria-hidden="true" />
        )}
      </span>

      {/* Date label */}
      <span className="text-sm font-semibold text-muted-foreground group-hover:text-foreground transition-colors">
        {date}
      </span>

      {/* Message count */}
      <span className="text-xs text-muted-foreground/70 group-hover:text-muted-foreground transition-colors">
        {messageCount} message{messageCount !== 1 ? 's' : ''}
      </span>

      {/* Summary preview (when collapsed) */}
      {summary && !isExpanded && (
        <span className="text-xs text-muted-foreground/70 truncate flex-1" title={summary}>
          — {summary}
        </span>
      )}
    </button>
  );
}, (prev, next) => {
  // Custom comparison to prevent unnecessary re-renders
  return (
    prev.date === next.date &&
    prev.messageCount === next.messageCount &&
    prev.isExpanded === next.isExpanded &&
    prev.summary === next.summary
  );
});
