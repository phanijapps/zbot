import { MemoryItemCard } from "./MemoryItemCard";
import type {
  AgeBucket,
  MatchSource,
  MemoryCategory,
} from "@/services/transport/types";

export interface ContentListItem {
  id: string;
  content: string;
  category: MemoryCategory;
  confidence: number;
  created_at: string;
  age_bucket: AgeBucket;
  match_source?: MatchSource;
  ward_id?: string;
}

interface Props {
  items: ContentListItem[];
  timewarpDays?: number;
}

const LABEL: Record<AgeBucket, string> = {
  today: "TODAY",
  last_7_days: "LAST 7 DAYS",
  historical: "HISTORICAL",
};

function visibleBuckets(days: number | undefined): AgeBucket[] {
  if (days === undefined || days >= 8) return ["today", "last_7_days", "historical"];
  if (days >= 2) return ["today", "last_7_days"];
  return ["today"];
}

export function ContentList({ items, timewarpDays }: Props) {
  const visible = visibleBuckets(timewarpDays);
  const groups: Record<AgeBucket, ContentListItem[]> = {
    today: [],
    last_7_days: [],
    historical: [],
  };
  for (const it of items) groups[it.age_bucket].push(it);

  return (
    <div className="memory-list">
      {visible.map((b) =>
        groups[b].length > 0 ? (
          <section key={b} className="memory-list__group">
            <h3 className="memory-list__label">
              <span>{LABEL[b]}</span>
              <span>{groups[b].length} items</span>
            </h3>
            {groups[b].map((it) => (
              <MemoryItemCard key={it.id} {...it} />
            ))}
          </section>
        ) : null,
      )}
    </div>
  );
}
