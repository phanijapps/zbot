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
}

const BUCKETS: AgeBucket[] = ["today", "last_7_days", "historical"];
const LABEL: Record<AgeBucket, string> = {
  today: "TODAY",
  last_7_days: "LAST 7 DAYS",
  historical: "HISTORICAL",
};

export function ContentList({ items }: Props) {
  const groups: Record<AgeBucket, ContentListItem[]> = {
    today: [],
    last_7_days: [],
    historical: [],
  };
  for (const it of items) groups[it.age_bucket].push(it);

  return (
    <div className="memory-list">
      {BUCKETS.map((b) =>
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
