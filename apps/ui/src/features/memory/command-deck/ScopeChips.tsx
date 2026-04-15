export type ContentType = "facts" | "wiki" | "procedures" | "episodes";

interface Props {
  types: ContentType[];
  onChange: (v: { types: ContentType[] }) => void;
}

const ALL: ContentType[] = ["facts", "wiki", "procedures", "episodes"];

export function ScopeChips({ types, onChange }: Props) {
  const toggle = (t: ContentType) => {
    const next = types.includes(t) ? types.filter((x) => x !== t) : [...types, t];
    onChange({ types: next });
  };
  return (
    <div className="memory-chips" role="group" aria-label="Content type filter">
      <span className="memory-chips__label">TYPE</span>
      {ALL.map((t) => (
        <button
          key={t}
          type="button"
          className={`memory-chip ${types.includes(t) ? "is-on" : ""}`.trim()}
          aria-pressed={types.includes(t)}
          onClick={() => toggle(t)}
        >
          {t}
        </button>
      ))}
    </div>
  );
}
