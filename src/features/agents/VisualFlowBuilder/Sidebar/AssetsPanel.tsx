// ============================================================================
// VISUAL FLOW BUILDER - ASSETS PANEL
// Left sidebar with draggable node templates
// ============================================================================

import React, { memo, useState, useCallback, useRef } from "react";
import { NODE_TEMPLATES, NODE_COLORS } from "../constants";
import type { NodeTemplate } from "../types";

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const SearchIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" />
  </svg>
);

const ChevronDownIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="m6 9 6 6 6-6" />
  </svg>
);

const PlusIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M12 5v14M5 12h14" />
  </svg>
);

const Icons: Record<string, React.FC<{ className?: string }>> = {
  Play: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <polygon points="5 3 19 12 5 21 5 3" />
    </svg>
  ),
  Bot: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="M12 8V4H8" /><rect width="16" height="12" x="4" y="8" rx="2" /><path d="M2 14h2" /><path d="M20 14h2" /><path d="M15 13v2" /><path d="M9 13v2" />
    </svg>
  ),
  Zap: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
    </svg>
  ),
  ArrowRight: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="M5 12h14" /><path d="m12 5 7 7-7 7" />
    </svg>
  ),
  GitBranch: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="M6 3v12" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><path d="M18 9a9 9 0 0 1-9 9" />
    </svg>
  ),
  Repeat: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="m17 2 4 4-4 4" /><path d="M3 11V9a4 4 0 0 1 4-4h14" /><path d="m7 22-4-4 4-4" /><path d="M21 13v2a4 4 0 0 1-4 4H3" />
    </svg>
  ),
  Merge: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="m6 8 6 6-6 6" /><path d="m18 8-6 6 6 6" />
    </svg>
  ),
  ListChecks: ({ className }) => (
    <svg className={className} fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
      <path d="M9 11 3 17l-2-2" /><path d="m21 9-5-5-5 5" /><path d="M11 14h10" /><path d="M11 18h7" />
    </svg>
  ),
};

// -----------------------------------------------------------------------------
// Category Section
// -----------------------------------------------------------------------------

interface CategorySectionProps {
  title: string;
  templates: NodeTemplate[];
  onNodeDragStart: (template: NodeTemplate, e: React.DragEvent) => void;
  expanded: boolean;
  onToggle: () => void;
}

const CategorySection = memo(({
  title,
  templates,
  onNodeDragStart,
  expanded,
  onToggle,
}: CategorySectionProps) => {
  return (
    <div className="mb-4">
      <button
        onClick={onToggle}
        className="w-full flex items-center justify-between px-3 py-2 text-xs font-semibold text-gray-400 hover:text-white transition-colors"
      >
        <span>{title}</span>
        <span className={`transition-transform ${expanded ? "rotate-180" : ""}`}>
          <ChevronDownIcon />
        </span>
      </button>

      {expanded && (
        <div className="space-y-1.5 px-2 pb-2">
          {templates.map((template) => {
            const nodeStyle = NODE_COLORS[template.type] || NODE_COLORS.subagent;
            const IconComponent = Icons[template.icon] || Icons.Bot;

            return (
              <div
                key={template.type}
                draggable
                onDragStart={(e) => onNodeDragStart(template, e)}
                className={`
                  flex items-center gap-2 px-3 py-2 rounded-lg cursor-grab
                  bg-white/5 hover:bg-white/10 border border-white/10 hover:border-white/20
                  transition-all group active:cursor-grabbing
                `}
              >
                <div className={`p-1.5 rounded ${nodeStyle.icon} bg-white/10`}>
                  <IconComponent className="w-3.5 h-3.5" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-xs font-medium text-white group-hover:text-violet-300 transition-colors">
                    {template.label}
                  </p>
                  <p className="text-[10px] text-gray-500 truncate">{template.description}</p>
                </div>
                <span className="w-3.5 h-3.5 text-gray-600 group-hover:text-violet-400 transition-colors flex items-center justify-center">
                  <PlusIcon />
                </span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
});

CategorySection.displayName = "CategorySection";

// -----------------------------------------------------------------------------
// Main Assets Panel Component
// -----------------------------------------------------------------------------

interface AssetsPanelProps {
  // No props needed - drag-and-drop uses HTML5 DataTransfer API
}

export const AssetsPanel = memo(({
}: AssetsPanelProps) => {
  const [searchQuery, setSearchQuery] = useState("");
  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(
    new Set(["basic", "flow"])
  );

  const dragTemplateRef = useRef<NodeTemplate | null>(null);

  // Group templates by category
  const categories = React.useMemo(() => {
    const grouped: Record<string, NodeTemplate[]> = {
      basic: [],
      flow: [],
      advanced: [],
    };

    NODE_TEMPLATES.forEach((template) => {
      if (template.category) {
        grouped[template.category].push(template);
      }
    });

    // Filter by search query
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      Object.keys(grouped).forEach((key) => {
        grouped[key] = grouped[key].filter((t) =>
          t.label.toLowerCase().includes(query) ||
          t.description.toLowerCase().includes(query)
        );
      });
    }

    return grouped;
  }, [searchQuery]);

  // Toggle category expansion
  const toggleCategory = useCallback((category: string) => {
    setExpandedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(category)) {
        next.delete(category);
      } else {
        next.add(category);
      }
      return next;
    });
  }, []);

  // Handle drag start
  const handleDragStart = useCallback((template: NodeTemplate, e: React.DragEvent) => {
    dragTemplateRef.current = template;
    e.dataTransfer.effectAllowed = "copy";
    e.dataTransfer.setData("application/node-type", template.type);
  }, []);

  return (
    <div className="w-[240px] bg-[#141414] border-r border-white/10 flex flex-col">
      {/* Header */}
      <div className="p-4 border-b border-white/10">
        <h2 className="text-sm font-semibold text-white mb-3">Assets</h2>

        {/* Search */}
        <div className="relative">
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search nodes..."
            className="w-full pl-8 pr-3 py-2 bg-white/5 border border-white/10 rounded-lg text-white text-sm placeholder:text-gray-600 focus:outline-none focus:ring-1 focus:ring-violet-500"
          />
          <div className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-500">
            <SearchIcon />
          </div>
        </div>
      </div>

      {/* Node Templates */}
      <div className="flex-1 overflow-y-auto p-3">
        <CategorySection
          title="Basic Nodes"
          templates={categories.basic}
          onNodeDragStart={handleDragStart}
          expanded={expandedCategories.has("basic")}
          onToggle={() => toggleCategory("basic")}
        />

        <CategorySection
          title="Flow Control"
          templates={categories.flow}
          onNodeDragStart={handleDragStart}
          expanded={expandedCategories.has("flow")}
          onToggle={() => toggleCategory("flow")}
        />

        <CategorySection
          title="Advanced"
          templates={categories.advanced}
          onNodeDragStart={handleDragStart}
          expanded={expandedCategories.has("advanced")}
          onToggle={() => toggleCategory("advanced")}
        />
      </div>

      {/* Drop Zone (invisible, covers the canvas) */}
      {/* This is handled by the parent component */}
    </div>
  );
});

AssetsPanel.displayName = "AssetsPanel";
