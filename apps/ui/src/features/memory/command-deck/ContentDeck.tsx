import { useState } from "react";
import { ContentList } from "./ContentList";
import type { WardContent } from "@/services/transport/types";

type Tab = "facts" | "wiki" | "procedures" | "episodes";
const TABS: Tab[] = ["facts", "wiki", "procedures", "episodes"];
const TAB_LABELS: Record<Tab, string> = {
  facts: "Facts",
  wiki: "Wiki",
  procedures: "Procedures",
  episodes: "Episodes",
};

interface Props {
  data: WardContent | null;
  onOpenGraph: () => void;
  timewarpDays?: number;
}

export function ContentDeck({ data, onOpenGraph, timewarpDays }: Props) {
  const [tab, setTab] = useState<Tab>("facts");
  if (!data)
    return (
      <div className="memory-deck-empty">Select a ward to view content.</div>
    );

  const counts = data.counts;

  return (
    <div className="memory-deck">
      <header className="memory-deck__head">
        <div className="memory-deck__crumb">◆ {data.ward_id}</div>
        {data.summary?.description && (
          <div className="memory-deck__summary">{data.summary.description}</div>
        )}
        <nav
          className="memory-deck__tabs"
          role="tablist"
          aria-label="Content tabs"
        >
          {TABS.map((t) => (
            <button
              key={t}
              type="button"
              role="tab"
              aria-selected={tab === t}
              className={tab === t ? "is-active" : ""}
              onClick={() => setTab(t)}
            >
              <span>{TAB_LABELS[t]}</span>
              <span className="memory-deck__tab-count">{counts[t]}</span>
            </button>
          ))}
          <button
            type="button"
            className="memory-deck__graph"
            onClick={onOpenGraph}
          >
            Graph ↗
          </button>
        </nav>
      </header>
      <div className="memory-deck__body">
        {tab === "facts" ? (
          <ContentList items={data.facts} timewarpDays={timewarpDays} />
        ) : tab === "wiki" ? (
          <WikiList items={data.wiki} />
        ) : tab === "procedures" ? (
          <ProcList items={data.procedures} />
        ) : (
          <EpisodeList items={data.episodes} />
        )}
      </div>
    </div>
  );
}

function WikiList({ items }: { items: WardContent["wiki"] }) {
  if (items.length === 0)
    return <div className="memory-empty">No wiki articles yet.</div>;
  return (
    <ul className="memory-list-simple">
      {items.map((a) => (
        <li key={a.id} className="memory-item">
          <strong>{a.title}</strong>
          <p>{a.content?.slice(0, 240)}</p>
        </li>
      ))}
    </ul>
  );
}

function ProcList({ items }: { items: WardContent["procedures"] }) {
  if (items.length === 0)
    return <div className="memory-empty">No procedures yet.</div>;
  return (
    <ul className="memory-list-simple">
      {items.map((p) => (
        <li key={p.id} className="memory-item">
          <strong>{p.name}</strong>
          {p.description && <p>{p.description}</p>}
        </li>
      ))}
    </ul>
  );
}

function EpisodeList({ items }: { items: WardContent["episodes"] }) {
  if (items.length === 0)
    return <div className="memory-empty">No episodes yet.</div>;
  return (
    <ul className="memory-list-simple">
      {items.map((e) => (
        <li key={e.id} className="memory-item">
          <strong>{e.title}</strong>
          <p>{e.content?.slice(0, 240)}</p>
        </li>
      ))}
    </ul>
  );
}
