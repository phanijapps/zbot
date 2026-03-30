import { Search } from "lucide-react";
import type { ReactNode } from "react";

interface ActionBarProps {
  searchPlaceholder?: string;
  searchValue?: string;
  onSearchChange?: (value: string) => void;
  filters?: ReactNode;
  actions?: ReactNode;
}

export function ActionBar({ searchPlaceholder, searchValue, onSearchChange, filters, actions }: ActionBarProps) {
  return (
    <div className="action-bar">
      <div className="action-bar__left">
        {onSearchChange && (
          <div className="action-bar__search">
            <Search style={{ width: 14, height: 14 }} className="action-bar__search-icon" />
            <input
              className="action-bar__search-input"
              placeholder={searchPlaceholder || "Search..."}
              value={searchValue || ""}
              onChange={(e) => onSearchChange(e.target.value)}
            />
          </div>
        )}
        {filters}
      </div>
      {actions && <div className="action-bar__right">{actions}</div>}
    </div>
  );
}

interface FilterChipProps {
  label: string;
  active?: boolean;
  onClick: () => void;
}

export function FilterChip({ label, active, onClick }: FilterChipProps) {
  return (
    <button className={`filter-chip ${active ? "filter-chip--active" : ""}`} onClick={onClick}>
      {label}
    </button>
  );
}
