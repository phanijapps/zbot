import { useState } from "react";

export type SearchMode = "hybrid" | "fts" | "semantic";
export interface SearchBarValue {
  query: string;
  mode: SearchMode;
}
interface Props {
  onChange: (v: SearchBarValue) => void;
}

const MODES: SearchMode[] = ["fts", "hybrid", "semantic"];
const QUOTE_RE = /"[^"]+"/;

export function SearchBar({ onChange }: Props) {
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<SearchMode>("hybrid");

  function emit(q: string, m: SearchMode) {
    const effective: SearchMode = QUOTE_RE.test(q) ? "fts" : m;
    onChange({ query: q, mode: effective });
  }

  return (
    <div className="memory-search">
      <span className="memory-search__icon" aria-hidden="true">⌕</span>
      <label className="sr-only" htmlFor="memory-search-input">Search memories</label>
      <input
        id="memory-search-input"
        className="memory-search__input"
        type="text"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          emit(e.target.value, mode);
        }}
        placeholder="search memories, wiki, procedures…"
      />
      <div className="memory-search__mode" role="tablist" aria-label="Search mode">
        {MODES.map((m) => (
          <button
            key={m}
            type="button"
            role="tab"
            aria-selected={mode === m}
            className={mode === m ? "is-active" : ""}
            onClick={() => {
              setMode(m);
              emit(query, m);
            }}
          >
            {m.toUpperCase()}
          </button>
        ))}
      </div>
    </div>
  );
}
