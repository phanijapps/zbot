import { useEffect, useState } from "react";
import { SearchBar, type SearchMode } from "./SearchBar";
import { ScopeChips, type ContentType } from "./ScopeChips";
import { WardRail } from "./WardRail";
import { ContentDeck } from "./ContentDeck";
import { WriteRail } from "./WriteRail";
import { useWards, useWardContent, useHybridSearch, useTimewarp } from "./hooks";
import { getTransport } from "@/services/transport";
import type { MemoryCategory } from "@/services/transport/types";
import { Slideover } from "@/components/Slideover";
import { GraphView } from "../GraphView";
import { Network } from "lucide-react";

interface Props {
  agentId: string;
}

interface SaveInput {
  category: MemoryCategory;
  content: string;
  ward_id: string;
}

/** Build a stable-ish key from the first words of the content. */
function buildFactKey(content: string): string {
  const slug = content
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, "")
    .trim()
    .split(/\s+/)
    .slice(0, 6)
    .join("-");
  return slug || `fact-${Date.now()}`;
}

export function MemoryTab({ agentId }: Props) {
  const wards = useWards();
  const [activeId, setActiveId] = useState<string>("");

  // Auto-select the first ward once wards load.
  useEffect(() => {
    if (!activeId && wards.length > 0) setActiveId(wards[0].id);
  }, [wards, activeId]);

  const { data, refresh } = useWardContent(activeId || null);

  const [showGraph, setShowGraph] = useState(false);
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<SearchMode>("hybrid");
  const [types, setTypes] = useState<ContentType[]>(["facts", "wiki"]);
  const { days, setDays } = useTimewarp();

  // Instantiated but not rendered yet — results UI is a v2 concern.
  // Keeping the hook active wires it for observability and primes cache.
  const _search = useHybridSearch(query, {
    mode,
    types,
    ward_ids: activeId ? [activeId] : [],
  });
  void _search;

  async function saveFact(v: SaveInput) {
    const transport = await getTransport();
    await transport.createMemory(agentId, {
      category: v.category,
      key: buildFactKey(v.content),
      content: v.content,
      ward_id: v.ward_id,
    });
    await refresh();
  }

  return (
    <div className="memory-tab-deck">
      <div className="memory-tab-deck__top">
        <SearchBar
          onChange={(v) => {
            setQuery(v.query);
            setMode(v.mode);
          }}
        />
        <div className="memory-tab-deck__row">
          <ScopeChips types={types} onChange={(v) => setTypes(v.types)} />
          <TimewarpSlider days={days} onChange={setDays} />
        </div>
      </div>
      <div className="memory-tab-deck__grid">
        <WardRail
          wards={wards}
          activeId={activeId}
          onSelect={setActiveId}
        />
        <ContentDeck
          data={data}
          onOpenGraph={() => setShowGraph(true)}
          timewarpDays={days}
        />
        <WriteRail
          wardId={activeId}
          counts={
            data?.counts ?? { facts: 0, wiki: 0, procedures: 0, episodes: 0 }
          }
          onSave={(v) => {
            void saveFact(v);
          }}
        />
      </div>
      <Slideover
        open={showGraph}
        onClose={() => setShowGraph(false)}
        title="Knowledge Graph"
        subtitle={activeId ? `Scoped to agent: ${agentId}` : "All entities"}
        icon={<Network size={18} />}
        className="memory-graph-slideover"
      >
        {showGraph && <GraphView agentId={agentId} />}
      </Slideover>
    </div>
  );
}

function TimewarpSlider({ days, onChange }: { days: number; onChange: (d: number) => void }) {
  return (
    <label className="memory-timewarp" aria-label="Time range">
      <span className="memory-timewarp__tick">NOW</span>
      <input
        type="range"
        min={0}
        max={30}
        step={1}
        value={days}
        onChange={(e) => onChange(Number.parseInt(e.target.value, 10))}
        className="memory-timewarp__slider"
      />
      <span className="memory-timewarp__tick">{days === 0 ? "0d" : `${days}d`}</span>
    </label>
  );
}
