/**
 * InlineToolCallsList - Container for inline tool call cards
 *
 * Displays a list of tool call cards in the message stream.
 * Each tool card is individually collapsible for details.
 */

import { InlineToolCard } from "./InlineToolCard";

interface InlineToolCallsListProps {
  tools: Array<{
    name: string;
    status: "pending" | "running" | "completed" | "failed";
    result?: string;
    error?: string;
  }>;
}

export function InlineToolCallsList({ tools }: InlineToolCallsListProps) {
  if (tools.length === 0) {
    return null;
  }

  return (
    <div className="space-y-1 my-2">
      {tools.map((tool, index) => (
        <InlineToolCard
          key={index}
          name={tool.name}
          status={tool.status}
          result={tool.result}
          error={tool.error}
        />
      ))}
    </div>
  );
}
