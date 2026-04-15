// ============================================================================
// SEARCH RESULTS — grouped hits from /api/memory/search
// Rendered in the center column when the search query is non-empty.
// ============================================================================

import type { HybridSearchResponse, MatchSource } from "@/services/transport/types";
import { MemoryItemCard } from "./MemoryItemCard";

interface Props {
  data: HybridSearchResponse | null;
  loading: boolean;
}

type TypeKey = "facts" | "wiki" | "procedures" | "episodes";
const TYPE_LABELS: Record<TypeKey, string> = {
  facts: "Facts",
  wiki: "Wiki Articles",
  procedures: "Procedures",
  episodes: "Episodes",
};

export function SearchResults({ data, loading }: Props) {
  if (loading && !data) {
    return <div className="memory-empty">Searching…</div>;
  }
  if (!data) {
    return <div className="memory-empty">Type to search.</div>;
  }

  const total =
    data.facts.hits.length +
    data.wiki.hits.length +
    data.procedures.hits.length +
    data.episodes.hits.length;

  if (total === 0) {
    return <div className="memory-empty">No matches.</div>;
  }

  return (
    <div className="memory-search-results">
      <TypeSection label={TYPE_LABELS.facts} block={data.facts}>
        {data.facts.hits.map((h) => (
          <MemoryItemCard
            key={h.id}
            id={h.id}
            content={h.content}
            category={h.category}
            confidence={h.confidence}
            created_at={h.created_at}
            age_bucket={"today"}
            match_source={h.match_source}
            ward_id={h.ward_id}
          />
        ))}
      </TypeSection>
      <TypeSection label={TYPE_LABELS.wiki} block={data.wiki}>
        {data.wiki.hits.map((h) => (
          <Row
            key={h.id}
            title={h.title}
            body={h.snippet}
            wardId={h.ward_id}
            matchSource={h.match_source}
            score={h.score}
          />
        ))}
      </TypeSection>
      <TypeSection label={TYPE_LABELS.procedures} block={data.procedures}>
        {data.procedures.hits.map((h) => (
          <Row
            key={h.id}
            title={h.name}
            body={h.description ?? ""}
            wardId={h.ward_id ?? undefined}
            matchSource={h.match_source}
          />
        ))}
      </TypeSection>
      <TypeSection label={TYPE_LABELS.episodes} block={data.episodes}>
        {data.episodes.hits.map((h) => (
          <Row
            key={h.id}
            title={h.title}
            body={h.content?.slice(0, 240) ?? ""}
            wardId={h.ward_id}
            matchSource={h.match_source}
          />
        ))}
      </TypeSection>
    </div>
  );
}

function TypeSection({
  label,
  block,
  children,
}: {
  label: string;
  block: { hits: unknown[]; latency_ms: number };
  children: React.ReactNode;
}) {
  if (block.hits.length === 0) return null;
  return (
    <section className="memory-search-results__group">
      <header className="memory-search-results__label">
        <span>{label}</span>
        <span>
          {block.hits.length} {block.hits.length === 1 ? "hit" : "hits"} ·{" "}
          {block.latency_ms} ms
        </span>
      </header>
      <div className="memory-search-results__rows">{children}</div>
    </section>
  );
}

function Row({
  title,
  body,
  wardId,
  matchSource,
  score,
}: {
  title: string;
  body: string;
  wardId?: string;
  matchSource?: MatchSource;
  score?: number;
}) {
  return (
    <div className="memory-item memory-search-results__row">
      <div className="memory-item__body">
        {wardId && <span className="memory-ward-tag">◆ {wardId}</span>}
        <strong>{title}</strong>
        {body && <p>{body}</p>}
      </div>
      <div className="memory-item__meta">
        {matchSource && (
          <span className={`memory-why memory-why--${matchSource}`}>{matchSource}</span>
        )}
        {typeof score === "number" && (
          <span className="memory-score">{score.toFixed(2)}</span>
        )}
      </div>
    </div>
  );
}
