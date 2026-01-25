/**
 * InlineToolCard - Compact tool call card displayed in message stream
 *
 * Shows tool calls as inline cards between user and assistant messages.
 * Distinct visual design with purple border, compact size, expandable details.
 */

import { CheckCircle, XCircle, Loader2, ChevronDown, ChevronRight } from "lucide-react";
import { useState } from "react";
import { cn } from "@/shared/utils";

export interface InlineToolCardProps {
  name: string;
  status: "pending" | "running" | "completed" | "failed";
  result?: string;
  error?: string;
}

// Tool icon mapping
const TOOL_ICONS: Record<string, string> = {
  "write": "📝",
  "read": "📖",
  "search": "🔍",
  "request_input": "📋",
  "bash": "💻",
  "grep": "🔎",
  "str_replace": "✏️",
};

export function InlineToolCard({ name, status, result, error }: InlineToolCardProps) {
  const [isExpanded, setIsExpanded] = useState(false);

  const getIcon = () => {
    switch (status) {
      case "pending":
        return <Loader2 className="size-4 text-gray-400" />;
      case "running":
        return <Loader2 className="size-4 text-purple-400 animate-spin" />;
      case "completed":
        return <CheckCircle className="size-4 text-green-400" />;
      case "failed":
        return <XCircle className="size-4 text-red-400" />;
    }
  };

  const getStatusText = () => {
    switch (status) {
      case "pending":
        return "Pending...";
      case "running":
        return "Running...";
      case "completed":
        return "Completed";
      case "failed":
        return "Failed";
    }
  };

  const getToolIcon = () => {
    return TOOL_ICONS[name] || "🔧";
  };

  const truncate = (str: string, maxLen = 50) => {
    if (!str) return "";
    return str.length > maxLen ? str.substring(0, maxLen) + "..." : str;
  };

  return (
    <div
      className={cn(
        "mx-auto my-2 max-w-[85%] rounded-lg border-l-4 bg-purple-500/10 transition-all",
        "hover:bg-purple-500/15 cursor-pointer",
        status === "pending" && "border-gray-500",
        status === "running" && "border-purple-500",
        status === "completed" && "border-green-500",
        status === "failed" && "border-red-500"
      )}
      onClick={() => setIsExpanded(!isExpanded)}
    >
      {/* Header (always visible) */}
      <div className="flex items-center justify-between px-3 py-2">
        <div className="flex items-center gap-2">
          <span className="text-lg">{getToolIcon()}</span>
          <span className="text-sm font-medium text-white">{name}</span>
          <span className="text-xs text-gray-400">• {getStatusText()}</span>
        </div>
        <div className="flex items-center gap-2">
          {getIcon()}
          <button className="text-gray-400 hover:text-white transition-colors">
            {isExpanded ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
          </button>
        </div>
      </div>

      {/* Details (expanded) */}
      {isExpanded && (
        <div className="border-t border-white/10 px-3 py-2 space-y-2">
          {result && (
            <div>
              <div className="text-xs text-gray-400 mb-1">Result:</div>
              <pre className="text-xs text-gray-300 bg-black/20 rounded p-2 overflow-x-auto">
                {truncate(result, 200)}
              </pre>
            </div>
          )}
          {error && (
            <div>
              <div className="text-xs text-red-400 mb-1">Error:</div>
              <pre className="text-xs text-red-300 bg-red-500/10 rounded p-2 overflow-x-auto">
                {truncate(error, 200)}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
