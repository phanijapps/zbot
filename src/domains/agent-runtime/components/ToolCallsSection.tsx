/**
 * ToolCallsSection - Displays tool calls in the thinking panel
 *
 * Shows:
 * - Tool name only (minimal format: "toolName")
 * - Status indicator (pending, running, completed, failed)
 * - Duration for completed tools
 */

import { Loader2, Check, X, Clock } from "lucide-react";
import { cn } from "@/shared/utils";
import type { ToolCallDisplay } from "./types";

interface ToolCallsSectionProps {
  toolCalls: ToolCallDisplay[];
  className?: string;
}

export function ToolCallsSection({
  toolCalls,
  className,
}: ToolCallsSectionProps) {
  if (toolCalls.length === 0) {
    return null;
  }

  return (
    <div className={cn("space-y-3", className)}>
      {/* Header */}
      <div className="flex items-center gap-2 text-sm font-medium text-gray-300">
        <span>🔧</span>
        <span>Calling Tools</span>
      </div>

      {/* Tool list */}
      <div className="space-y-1.5">
        {toolCalls.map((tool) => (
          <ToolCallItem key={tool.id} tool={tool} />
        ))}
      </div>
    </div>
  );
}

function ToolCallItem({ tool }: { tool: ToolCallDisplay }) {
  return (
    <div
      className={cn(
        "flex items-center gap-2.5 py-2 px-3 rounded-md",
        "transition-colors duration-150",
        tool.status === "completed" && "bg-green-500/5",
        tool.status === "failed" && "bg-red-500/5",
        tool.status === "running" && "bg-purple-500/5"
      )}
    >
      {/* Status Icon */}
      <StatusIcon status={tool.status} />

      {/* Tool name - minimal format as specified */}
      <span
        className={cn(
          "text-sm font-mono",
          tool.status === "completed" && "text-gray-400",
          tool.status === "failed" && "text-red-400",
          tool.status === "running" && "text-white",
          tool.status === "pending" && "text-gray-500"
        )}
      >
        {tool.name}
      </span>

      {/* Duration for completed tools */}
      {tool.status === "completed" && tool.duration !== undefined && (
        <span className="ml-auto text-xs text-gray-600 flex items-center gap-1">
          <Clock className="size-3" />
          {formatDuration(tool.duration)}
        </span>
      )}
    </div>
  );
}

function StatusIcon({ status }: { status: ToolCallDisplay["status"] }) {
  switch (status) {
    case "completed":
      return <Check className="size-4 text-green-500 shrink-0" />;
    case "running":
      return <Loader2 className="size-4 text-purple-500 shrink-0 animate-spin" />;
    case "failed":
      return <X className="size-4 text-red-500 shrink-0" />;
    case "pending":
      return <Clock className="size-4 text-gray-600 shrink-0" />;
  }
}

function formatDuration(seconds: number): string {
  if (seconds < 1) {
    return `${Math.round(seconds * 1000)}ms`;
  }
  return `${seconds.toFixed(1)}s`;
}

/**
 * Expanded tool call detail view
 * Shown when user clicks on a tool call for more info
 */
export interface ToolCallDetailProps {
  tool: ToolCallDisplay;
  args?: Record<string, unknown>;
  result?: string;
  error?: string;
}

export function ToolCallDetail({
  tool,
  args,
  result,
  error,
}: ToolCallDetailProps) {
  return (
    <div className="space-y-3 p-4 bg-white/5 rounded-lg border border-white/10">
      {/* Header */}
      <div className="flex items-center justify-between">
        <span className="text-sm font-mono font-medium text-white">
          {tool.name}
        </span>
        <StatusIcon status={tool.status} />
      </div>

      {/* Arguments */}
      {args && Object.keys(args).length > 0 && (
        <div className="space-y-1">
          <span className="text-xs text-gray-500 uppercase tracking-wide">
            Arguments
          </span>
          <pre className="text-xs text-gray-400 bg-black/20 p-2 rounded overflow-x-auto">
            {JSON.stringify(args, null, 2)}
          </pre>
        </div>
      )}

      {/* Result */}
      {result && tool.status === "completed" && (
        <div className="space-y-1">
          <span className="text-xs text-gray-500 uppercase tracking-wide">
            Result
          </span>
          <pre className="text-xs text-gray-400 bg-black/20 p-2 rounded overflow-x-auto max-h-32 overflow-y-auto">
            {truncateText(result, 500)}
          </pre>
        </div>
      )}

      {/* Error */}
      {error && tool.status === "failed" && (
        <div className="space-y-1">
          <span className="text-xs text-red-500 uppercase tracking-wide">
            Error
          </span>
          <p className="text-sm text-red-400">{error}</p>
        </div>
      )}

      {/* Duration */}
      {tool.duration !== undefined && (
        <div className="flex items-center gap-1 text-xs text-gray-500">
          <Clock className="size-3" />
          Completed in {formatDuration(tool.duration)}
        </div>
      )}
    </div>
  );
}

function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength) + "...";
}
