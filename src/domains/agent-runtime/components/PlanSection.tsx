/**
 * PlanSection - Displays agent execution plan as a checklist
 *
 * Shows when the planning module is active.
 * Displays items with status indicators (pending, in-progress, completed, failed)
 */

import { CheckCircle2, Circle, Loader2, XCircle } from "lucide-react";
import { cn } from "@/shared/utils";
import type { PlanItem } from "./types";

interface PlanSectionProps {
  planItems: PlanItem[];
  className?: string;
}

export function PlanSection({ planItems, className }: PlanSectionProps) {
  if (planItems.length === 0) {
    return null;
  }

  // Sort by order
  const sortedItems = [...planItems].sort((a, b) => a.order - b.order);

  return (
    <div className={cn("space-y-3", className)}>
      {/* Header */}
      <div className="flex items-center gap-2 text-sm font-medium text-gray-300">
        <span>📋</span>
        <span>Execution Plan</span>
      </div>

      {/* Checklist */}
      <div className="space-y-1.5">
        {sortedItems.map((item) => (
          <PlanItem key={item.id} item={item} />
        ))}
      </div>
    </div>
  );
}

function PlanItem({ item }: { item: PlanItem }) {
  return (
    <div
      className={cn(
        "flex items-start gap-2.5 py-1.5 px-2 rounded-md",
        "transition-colors duration-150",
        item.status === "completed" && "bg-green-500/5",
        item.status === "failed" && "bg-red-500/5",
        item.status === "in_progress" && "bg-purple-500/5"
      )}
    >
      {/* Status Icon */}
      <StatusIcon status={item.status} className="mt-0.5 shrink-0" />

      {/* Title */}
      <span
        className={cn(
          "text-sm leading-tight",
          item.status === "completed" && "text-gray-500 line-through",
          item.status === "failed" && "text-red-400",
          item.status === "in_progress" && "text-white",
          item.status === "pending" && "text-gray-400"
        )}
      >
        {item.title}
      </span>
    </div>
  );
}

function StatusIcon({
  status,
  className,
}: {
  status: PlanItem["status"];
  className?: string;
}) {
  switch (status) {
    case "completed":
      return <CheckCircle2 className={cn("size-4 text-green-500", className)} />;
    case "in_progress":
      return <Loader2 className={cn("size-4 text-purple-500 animate-spin", className)} />;
    case "failed":
      return <XCircle className={cn("size-4 text-red-500", className)} />;
    case "pending":
      return <Circle className={cn("size-4 text-gray-600", className)} />;
  }
}
