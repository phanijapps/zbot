import { useEffect, useState } from "react";
import { SearchBar, type SearchMode } from "./SearchBar";
import { ScopeChips, type ContentType } from "./ScopeChips";
import { WardRail } from "./WardRail";
import { ContentDeck } from "./ContentDeck";
import { WriteRail } from "./WriteRail";
import { useWards, useWardContent, useHybridSearch } from "./hooks";
import { getTransport } from "@/services/transport";
import type { MemoryCategory } from "@/services/transport/types";

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

  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<SearchMode>("hybrid");
  const [types, setTypes] = useState<ContentType[]>(["facts", "wiki"]);

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
        <ScopeChips types={types} onChange={(v) => setTypes(v.types)} />
      </div>
      <div className="memory-tab-deck__grid">
        <WardRail
          wards={wards}
          activeId={activeId}
          onSelect={setActiveId}
        />
        <ContentDeck
          data={data}
          onOpenGraph={() => {
            /* wired in a follow-up */
          }}
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
    </div>
  );
}
