import { useState } from "react";
import { Trash2 } from "lucide-react";
import type { MemoryCategory, AgeBucket, MatchSource } from "@/services/transport/types";

export interface MemoryItemCardProps {
  id: string;
  content: string;
  category: MemoryCategory;
  confidence: number;
  created_at: string;
  age_bucket: AgeBucket;
  match_source?: MatchSource;
  ward_id?: string;
  onClick?: () => void;
  /** When provided, renders a trash button on the row that calls this with `id`. */
  onDelete?: (id: string) => void | Promise<void>;
}

const DECAY: Record<AgeBucket, string> = {
  today: "",
  last_7_days: "decay-1",
  historical: "decay-2",
};

export function MemoryItemCard(p: MemoryItemCardProps) {
  const decay = DECAY[p.age_bucket] ?? "";
  const [isDeleting, setIsDeleting] = useState(false);

  const handleDelete = async (e: React.MouseEvent | React.KeyboardEvent) => {
    e.stopPropagation();
    if (!p.onDelete || isDeleting) return;
    if (!window.confirm(`Delete this ${p.category} memory?`)) return;
    setIsDeleting(true);
    try {
      await p.onDelete(p.id);
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <div
      className={`memory-item ${decay}`.trim()}
      role="button"
      tabIndex={0}
      onClick={p.onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") p.onClick?.();
      }}
    >
      <div className="memory-item__body">
        <span className={`memory-kind memory-kind--${p.category}`}>{p.category}</span>
        {p.ward_id && <span className="memory-ward-tag">◆ {p.ward_id}</span>}
        <span className="memory-item__content">{p.content}</span>
      </div>
      <div className="memory-item__meta">
        {p.match_source && (
          <span className={`memory-why memory-why--${p.match_source}`}>{p.match_source}</span>
        )}
        <span className="memory-score">conf {p.confidence.toFixed(2)}</span>
        <span className="memory-age">{new Date(p.created_at).toLocaleDateString()}</span>
        {p.onDelete && (
          <button
            type="button"
            className="memory-item__delete"
            aria-label={`Delete ${p.category} memory`}
            title="Delete memory"
            disabled={isDeleting}
            onClick={handleDelete}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") void handleDelete(e);
            }}
          >
            <Trash2 size={14} />
          </button>
        )}
      </div>
    </div>
  );
}
