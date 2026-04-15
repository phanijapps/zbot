interface Ward {
  id: string;
  count: number;
}

interface Props {
  wards: Ward[];
  activeId: string;
  onSelect: (id: string) => void;
}

interface WardButtonProps {
  ward: Ward;
  activeId: string;
  onSelect: (id: string) => void;
  isGlobal?: boolean;
}

function WardButton({ ward, activeId, onSelect, isGlobal }: WardButtonProps) {
  const isActive = ward.id === activeId;
  const className = `memory-ward ${isActive ? "is-active" : ""}`.trim();
  const dotClass = isGlobal
    ? "memory-ward__dot memory-ward__dot--global"
    : "memory-ward__dot";
  return (
    <button
      type="button"
      className={className}
      aria-current={isActive ? "true" : undefined}
      onClick={() => onSelect(ward.id)}
    >
      <span className={dotClass} aria-hidden="true" />
      <span className="memory-ward__name">{ward.id}</span>
      <span className="memory-ward__badge">{ward.count}</span>
    </button>
  );
}

export function WardRail({ wards, activeId, onSelect }: Props) {
  const regular = wards.filter((w) => !w.id.startsWith("__"));
  const global = wards.filter((w) => w.id.startsWith("__"));
  return (
    <nav className="memory-wards" aria-label="Wards">
      <div className="memory-wards__title">
        <span>WARDS</span>
        <span>{regular.length}</span>
      </div>
      <ul>
        {regular.map((w) => (
          <li key={w.id}>
            <WardButton ward={w} activeId={activeId} onSelect={onSelect} />
          </li>
        ))}
      </ul>
      {global.length > 0 && (
        <>
          <div className="memory-wards__title">
            <span>GLOBAL</span>
            <span>∞</span>
          </div>
          <ul>
            {global.map((w) => (
              <li key={w.id}>
                <WardButton ward={w} activeId={activeId} onSelect={onSelect} isGlobal />
              </li>
            ))}
          </ul>
        </>
      )}
    </nav>
  );
}
