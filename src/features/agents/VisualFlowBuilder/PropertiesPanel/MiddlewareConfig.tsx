// ============================================================================
// VISUAL FLOW BUILDER - MIDDLEWARE CONFIGURATION
// Middleware configuration UI for agent nodes
// ============================================================================

import { memo, useState } from "react";
import { MIDDLEWARE_TYPES } from "../constants/agentResources";

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

interface MiddlewareConfig {
  id: string;
  type: string;
  enabled: boolean;
  config?: Record<string, unknown>;
}

interface MiddlewareConfigProps {
  middlewares: string[];
  onAdd: (type: string) => void;
  onRemove: (type: string) => void;
  onConfigure?: (type: string, config: Record<string, unknown>) => void;
}

// -----------------------------------------------------------------------------
// Icons
// -----------------------------------------------------------------------------

const PlusIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M12 5v14M5 12h14" />
  </svg>
);

const TrashIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="M3 6h18M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6m3 0V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
  </svg>
);

const SettingsIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="3" />
    <path d="M12 1v6m0 6v6m5.3-13.3-4.2 4.2m0 4.3 4.2 4.2M23 12h-6m-6 0H5m13.3 5.3-4.2-4.2m0-4.3-4.2-4.2" />
  </svg>
);

const ChevronDownIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2" viewBox="0 0 24 24">
    <path d="m6 9 6 6 6-6" />
  </svg>
);

// -----------------------------------------------------------------------------
// Middleware Item Component
// -----------------------------------------------------------------------------

interface MiddlewareItemProps {
  type: string;
  enabled: boolean;
  onToggle: () => void;
  onRemove: () => void;
  onConfigure?: () => void;
}

const MiddlewareItem = memo(({ type, enabled, onToggle, onRemove, onConfigure }: MiddlewareItemProps) => {
  const middleware = MIDDLEWARE_TYPES.find((m) => m.id === type);
  if (!middleware) return null;

  const [isExpanded, setIsExpanded] = useState(false);

  return (
    <div className={`p-2 rounded border transition-colors ${
      enabled ? "bg-white/5 border-white/20" : "bg-gray-500/5 border-gray-500/20 opacity-60"
    }`}>
      <div className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={enabled}
          onChange={onToggle}
          className="rounded"
        />
        <div className="flex-1 min-w-0">
          <p className="text-xs text-white font-medium">{middleware.name}</p>
          <p className="text-[10px] text-gray-500 truncate">{middleware.description}</p>
        </div>
        {onConfigure && (
          <button
            onClick={() => { setIsExpanded(!isExpanded); onConfigure(); }}
            className="p-1 rounded hover:bg-white/10 text-gray-400 hover:text-white transition-colors"
            title="Configure"
          >
            <SettingsIcon />
          </button>
        )}
        <button
          onClick={onRemove}
          className="p-1 rounded hover:bg-red-500/10 text-gray-400 hover:text-red-400 transition-colors"
          title="Remove"
        >
          <TrashIcon />
        </button>
        <button
          onClick={() => setIsExpanded(!isExpanded)}
          className={`p-1 transition-transform ${isExpanded ? "rotate-180" : ""}`}
        >
          <ChevronDownIcon />
        </button>
      </div>

      {/* Middleware Configuration */}
      {isExpanded && enabled && (
        <div className="mt-2 pt-2 border-t border-white/10">
          <MiddlewareSettings type={type} />
        </div>
      )}
    </div>
  );
});

MiddlewareItem.displayName = "MiddlewareItem";

// -----------------------------------------------------------------------------
// Middleware Settings Component
// -----------------------------------------------------------------------------

interface MiddlewareSettingsProps {
  type: string;
}

const MiddlewareSettings = memo(({ type }: MiddlewareSettingsProps) => {
  const renderSettings = () => {
    switch (type) {
      case "retry":
        return (
          <div className="space-y-2">
            <div>
              <label className="text-[10px] text-gray-400 block mb-1">Max Attempts</label>
              <input
                type="number"
                defaultValue={3}
                min={1}
                max={10}
                className="w-full bg-white/5 border border-white/10 rounded px-2 py-1 text-white text-xs"
              />
            </div>
            <div>
              <label className="text-[10px] text-gray-400 block mb-1">Backoff (ms)</label>
              <input
                type="number"
                defaultValue={1000}
                min={100}
                step={100}
                className="w-full bg-white/5 border border-white/10 rounded px-2 py-1 text-white text-xs"
              />
            </div>
          </div>
        );

      case "cache":
        return (
          <div className="space-y-2">
            <div>
              <label className="text-[10px] text-gray-400 block mb-1">TTL (seconds)</label>
              <input
                type="number"
                defaultValue={300}
                min={0}
                className="w-full bg-white/5 border border-white/10 rounded px-2 py-1 text-white text-xs"
              />
            </div>
            <div>
              <label className="text-[10px] text-gray-400 block mb-1">Max Size</label>
              <input
                type="number"
                defaultValue={100}
                min={1}
                className="w-full bg-white/5 border border-white/10 rounded px-2 py-1 text-white text-xs"
              />
            </div>
          </div>
        );

      case "rate_limit":
        return (
          <div className="space-y-2">
            <div>
              <label className="text-[10px] text-gray-400 block mb-1">Requests per minute</label>
              <input
                type="number"
                defaultValue={60}
                min={1}
                className="w-full bg-white/5 border border-white/10 rounded px-2 py-1 text-white text-xs"
              />
            </div>
            <div>
              <label className="text-[10px] text-gray-400 block mb-1">Burst size</label>
              <input
                type="number"
                defaultValue={10}
                min={1}
                className="w-full bg-white/5 border border-white/10 rounded px-2 py-1 text-white text-xs"
              />
            </div>
          </div>
        );

      case "timeout":
        return (
          <div className="space-y-2">
            <div>
              <label className="text-[10px] text-gray-400 block mb-1">Timeout (ms)</label>
              <input
                type="number"
                defaultValue={30000}
                min={1000}
                step={1000}
                className="w-full bg-white/5 border border-white/10 rounded px-2 py-1 text-white text-xs"
              />
            </div>
          </div>
        );

      case "logging":
        return (
          <div className="space-y-2">
            <label className="flex items-center gap-2 text-xs text-white cursor-pointer">
              <input type="checkbox" defaultChecked className="rounded" />
              <span>Log requests</span>
            </label>
            <label className="flex items-center gap-2 text-xs text-white cursor-pointer">
              <input type="checkbox" defaultChecked className="rounded" />
              <span>Log responses</span>
            </label>
            <label className="flex items-center gap-2 text-xs text-white cursor-pointer">
              <input type="checkbox" defaultChecked className="rounded" />
              <span>Log errors</span>
            </label>
          </div>
        );

      case "validation":
        return (
          <div className="space-y-2">
            <label className="flex items-center gap-2 text-xs text-white cursor-pointer">
              <input type="checkbox" defaultChecked className="rounded" />
              <span>Validate inputs</span>
            </label>
            <label className="flex items-center gap-2 text-xs text-white cursor-pointer">
              <input type="checkbox" defaultChecked className="rounded" />
              <span>Validate outputs</span>
            </label>
          </div>
        );

      default:
        return (
          <p className="text-[10px] text-gray-500">No configuration options for this middleware</p>
        );
    }
  };

  return <>{renderSettings()}</>;
});

MiddlewareSettings.displayName = "MiddlewareSettings";

// -----------------------------------------------------------------------------
// Add Middleware Dropdown
// -----------------------------------------------------------------------------

interface AddMiddlewareDropdownProps {
  availableMiddlewares: readonly (typeof MIDDLEWARE_TYPES)[number][];
  onAdd: (type: string) => void;
}

const AddMiddlewareDropdown = memo(({ availableMiddlewares, onAdd }: AddMiddlewareDropdownProps) => {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="relative">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="w-full flex items-center justify-center gap-1.5 px-3 py-1.5 bg-violet-600 hover:bg-violet-700 rounded text-xs font-medium text-white transition-colors"
      >
        <PlusIcon />
        <span>Add Middleware</span>
      </button>

      {isOpen && (
        <>
          <div
            className="fixed inset-0 z-10"
            onClick={() => setIsOpen(false)}
          />
          <div className="absolute z-20 bottom-full left-0 w-full mb-1 bg-[#1a1a1a] border border-white/10 rounded-lg shadow-xl overflow-hidden">
            {availableMiddlewares.map((middleware) => (
              <button
                key={middleware.id}
                onClick={() => { onAdd(middleware.id); setIsOpen(false); }}
                className="w-full px-3 py-2 text-left hover:bg-white/10 transition-colors"
              >
                <p className="text-xs text-white">{middleware.name}</p>
                <p className="text-[10px] text-gray-500">{middleware.description}</p>
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
});

AddMiddlewareDropdown.displayName = "AddMiddlewareDropdown";

// -----------------------------------------------------------------------------
// Main Component
// -----------------------------------------------------------------------------

export const MiddlewareConfig = memo(({ middlewares, onAdd, onRemove, onConfigure }: MiddlewareConfigProps) => {
  const availableMiddlewares = MIDDLEWARE_TYPES.filter((m) => !middlewares.includes(m.id));

  return (
    <div className="space-y-3">
      {/* Current Middlewares */}
      {middlewares.length > 0 ? (
        <div className="space-y-2">
          {middlewares.map((type) => (
            <MiddlewareItem
              key={type}
              type={type}
              enabled={true}
              onToggle={() => {/* Toggle logic */}}
              onRemove={() => onRemove(type)}
              onConfigure={onConfigure ? () => onConfigure?.(type, {}) : undefined}
            />
          ))}
        </div>
      ) : (
        <div className="p-3 rounded border border-dashed border-white/20 text-center">
          <p className="text-[10px] text-gray-500">No middleware configured</p>
          <p className="text-[10px] text-gray-600">Add middleware to enhance agent behavior</p>
        </div>
      )}

      {/* Add Middleware Button */}
      {availableMiddlewares.length > 0 && (
        <AddMiddlewareDropdown
          availableMiddlewares={availableMiddlewares}
          onAdd={onAdd}
        />
      )}

      {/* Info */}
      <div className="p-3 rounded-lg bg-purple-500/10 border border-purple-500/20">
        <p className="text-[10px] text-purple-300">
          Middleware intercepts and modifies agent requests/responses. Add retry, caching, rate limiting, and more.
        </p>
      </div>
    </div>
  );
});

MiddlewareConfig.displayName = "MiddlewareConfig";
