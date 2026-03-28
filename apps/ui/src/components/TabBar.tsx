import type { ReactNode } from "react";

interface Tab {
  id: string;
  label: string;
  count?: number;
}

interface TabBarProps {
  tabs: Tab[];
  activeTab: string;
  onTabChange: (tabId: string) => void;
}

export function TabBar({ tabs, activeTab, onTabChange }: TabBarProps) {
  return (
    <div className="tab-bar" role="tablist">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          id={`tab-${tab.id}`}
          className={`tab-bar__tab ${activeTab === tab.id ? "tab-bar__tab--active" : ""}`}
          onClick={() => onTabChange(tab.id)}
          role="tab"
          aria-selected={activeTab === tab.id}
          aria-controls={`tabpanel-${tab.id}`}
        >
          {tab.label}
          {tab.count !== undefined && (
            <span className="tab-bar__count">{tab.count}</span>
          )}
        </button>
      ))}
    </div>
  );
}

interface TabPanelProps {
  id: string;
  activeTab: string;
  children: ReactNode;
}

export function TabPanel({ id, activeTab, children }: TabPanelProps) {
  if (activeTab !== id) return null;
  return (
    <div role="tabpanel" id={`tabpanel-${id}`} aria-labelledby={`tab-${id}`}>
      {children}
    </div>
  );
}
