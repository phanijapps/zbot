/**
 * InlineToolCallsList - Container for inline tool call cards
 *
 * Displays a list of tool call cards in the message stream.
 * Each tool card is individually collapsible for details.
 */

import { memo } from "react";
import { InlineToolCard } from "./InlineToolCard";

interface InlineToolCallsListProps {
  tools: Array<{
    id: string; // Unique identifier for the tool call
    name: string;
    status: "pending" | "running" | "completed" | "failed";
    result?: string;
    error?: string;
  }>;
}

export const InlineToolCallsList = memo(function InlineToolCallsList({ tools }: InlineToolCallsListProps) {
  if (tools.length === 0) {
    return null;
  }

  return (
    <div className="space-y-1 my-2">
      {tools.map((tool) => (
        <InlineToolCard
          key={tool.id}
          name={tool.name}
          status={tool.status}
          result={tool.result}
          error={tool.error}
        />
      ))}
    </div>
  );
}, (prev, next) => {
  // Custom comparison to prevent unnecessary re-renders
  // Compare tools array by reference and length first
  if (prev.tools !== next.tools) {
    // If arrays are different, check if content is the same
    if (prev.tools.length !== next.tools.length) return false;
    
    // Deep compare tool properties that matter for rendering
    for (let i = 0; i < prev.tools.length; i++) {
      const p = prev.tools[i];
      const n = next.tools[i];
      if (
        p.name !== n.name ||
        p.status !== n.status ||
        p.result !== n.result ||
        p.error !== n.error
      ) {
        return false;
      }
    }
  }
  return true;
});
