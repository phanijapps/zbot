import { useState } from "react";
import type { MemoryFact, MemoryCategory } from "@/services/transport/types";

interface MemoryFactCardProps {
  fact: MemoryFact;
  onDelete: () => void;
  expanded?: boolean;
}

const CATEGORY_COLORS: Record<MemoryCategory, { bg: string; text: string; border: string }> = {
  correction: { bg: "var(--destructive-muted)", text: "var(--destructive)", border: "var(--destructive)" },
  instruction: { bg: "rgba(249, 115, 22, 0.15)", text: "#fb923c", border: "rgba(249, 115, 22, 0.5)" },
  user: { bg: "rgba(59, 130, 246, 0.15)", text: "#60a5fa", border: "rgba(59, 130, 246, 0.5)" },
  strategy: { bg: "rgba(139, 92, 246, 0.15)", text: "#a78bfa", border: "rgba(139, 92, 246, 0.5)" },
  domain: { bg: "rgba(34, 197, 94, 0.15)", text: "#4ade80", border: "rgba(34, 197, 94, 0.5)" },
  pattern: { bg: "rgba(34, 197, 94, 0.15)", text: "#4ade80", border: "rgba(34, 197, 94, 0.5)" },
  skill: { bg: "rgba(20, 184, 166, 0.15)", text: "#2dd4bf", border: "rgba(20, 184, 166, 0.5)" },
  agent: { bg: "rgba(20, 184, 166, 0.15)", text: "#2dd4bf", border: "rgba(20, 184, 166, 0.5)" },
  ward: { bg: "var(--primary-muted)", text: "var(--primary)", border: "var(--primary)" },
  preference: { bg: "var(--primary-muted)", text: "var(--primary)", border: "var(--primary)" },
  entity: { bg: "rgba(234, 179, 8, 0.15)", text: "#facc15", border: "rgba(234, 179, 8, 0.5)" },
  decision: { bg: "rgba(139, 92, 246, 0.15)", text: "#a78bfa", border: "rgba(139, 92, 246, 0.5)" },
};

function formatDate(dateStr: string): string {
  try {
    const date = new Date(dateStr);
    return date.toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return dateStr;
  }
}

export function MemoryFactCard({
  fact,
  onDelete,
  expanded = false,
}: MemoryFactCardProps) {
  const [isExpanded, setIsExpanded] = useState(expanded);
  const [isDeleting, setIsDeleting] = useState(false);

  const handleDelete = async () => {
    if (isDeleting) return;
    setIsDeleting(true);
    try {
      onDelete();
    } finally {
      setIsDeleting(false);
    }
  };

  const confidencePercent = Math.round(fact.confidence * 100);
  const categoryColor = CATEGORY_COLORS[fact.category] || CATEGORY_COLORS.entity;

  let confidenceColor: { bg: string; text: string };
  if (confidencePercent >= 80) {
    confidenceColor = { bg: "rgba(34, 197, 94, 0.15)", text: "#4ade80" };
  } else if (confidencePercent >= 50) {
    confidenceColor = { bg: "rgba(234, 179, 8, 0.15)", text: "#facc15" };
  } else {
    confidenceColor = { bg: "var(--destructive-muted)", text: "var(--destructive)" };
  }

  return (
    <div
      style={{
        border: "1px solid var(--border)",
        borderRadius: "var(--radius-md)",
        overflow: "hidden",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--spacing-2)",
          padding: "var(--spacing-3)",
          cursor: "pointer",
          transition: "background-color 0.15s ease",
        }}
        onClick={() => setIsExpanded(!isExpanded)}
        onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "var(--muted)")}
        onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "transparent")}
      >
        <span style={{ fontSize: "var(--text-xs)", color: "var(--muted-foreground)", userSelect: "none" }}>
          {isExpanded ? "\u25BC" : "\u25B6"}
        </span>
        <span
          style={{
            fontSize: "var(--text-xs)",
            padding: "2px var(--spacing-2)",
            borderRadius: "var(--radius-sm)",
            border: `1px solid ${categoryColor.border}`,
            backgroundColor: categoryColor.bg,
            color: categoryColor.text,
          }}
        >
          {fact.category}
        </span>
        <span
          style={{
            color: "var(--foreground)",
            fontSize: "var(--text-sm)",
            fontWeight: 500,
            flex: 1,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {fact.key}
        </span>
        <span
          style={{
            fontSize: "var(--text-xs)",
            padding: "2px var(--spacing-2)",
            borderRadius: "var(--radius-sm)",
            backgroundColor: confidenceColor.bg,
            color: confidenceColor.text,
          }}
          title="Confidence"
        >
          {confidencePercent}%
        </span>
      </div>

      {isExpanded && (
        <div
          style={{
            padding: "var(--spacing-3)",
            borderTop: "1px solid var(--border)",
          }}
        >
          <p
            style={{
              color: "var(--foreground)",
              fontSize: "var(--text-sm)",
              marginBottom: "var(--spacing-3)",
              whiteSpace: "pre-wrap",
            }}
          >
            {fact.content}
          </p>
          <div
            style={{
              display: "flex",
              flexWrap: "wrap",
              gap: "var(--spacing-4)",
              fontSize: "var(--text-xs)",
              color: "var(--muted-foreground)",
              marginBottom: "var(--spacing-3)",
            }}
          >
            <span>
              <span style={{ color: "var(--muted-foreground)", opacity: 0.7 }}>Scope:</span>{" "}
              {fact.scope}
            </span>
            <span>
              <span style={{ color: "var(--muted-foreground)", opacity: 0.7 }}>Mentions:</span>{" "}
              {fact.mention_count}
            </span>
            <span>
              <span style={{ color: "var(--muted-foreground)", opacity: 0.7 }}>Created:</span>{" "}
              {formatDate(fact.created_at)}
            </span>
          </div>
          {fact.source_summary && (
            <div
              style={{
                fontSize: "var(--text-xs)",
                color: "var(--muted-foreground)",
                marginBottom: "var(--spacing-3)",
                padding: "var(--spacing-2)",
                backgroundColor: "var(--muted)",
                borderRadius: "var(--radius-sm)",
              }}
            >
              <span style={{ opacity: 0.7 }}>Source:</span> {fact.source_summary}
            </div>
          )}
          <button
            style={{
              fontSize: "var(--text-xs)",
              color: "var(--destructive)",
              background: "none",
              border: "none",
              cursor: isDeleting ? "wait" : "pointer",
              opacity: isDeleting ? 0.5 : 1,
              padding: 0,
            }}
            onClick={(e) => {
              e.stopPropagation();
              handleDelete();
            }}
            disabled={isDeleting}
          >
            {isDeleting ? "Deleting..." : "Delete"}
          </button>
        </div>
      )}
    </div>
  );
}
