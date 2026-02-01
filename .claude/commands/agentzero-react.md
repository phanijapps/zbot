---
description: React/TypeScript patterns and conventions for AgentZero UI development
---

# AgentZero React Development Guide

Use this skill when working on the React UI in `apps/ui/`. This captures the patterns, conventions, and architecture used in this codebase.

## Project Structure

```
apps/ui/src/
├── features/           # Feature-based modules
│   ├── chat/          # Chat interface
│   ├── agents/        # Agent management
│   ├── providers/     # LLM provider config
│   ├── skills/        # Skills management
│   ├── logs/          # Execution logs
│   ├── ops/           # Dashboard/monitoring
│   └── mcp/           # MCP server config
├── services/
│   └── transport/     # API communication layer
├── styles/            # Global CSS
└── App.tsx            # Main app with routing
```

## Naming Conventions

- **Panel Components**: `Web<Feature>Panel.tsx` (e.g., `WebLogsPanel.tsx`, `WebOpsDashboard.tsx`)
- **Types**: Defined in `services/transport/types.ts` for API-related types
- **Transport Layer**: All API calls go through `getTransport()` abstraction

## Component Structure Pattern

Follow this order within component files:

```tsx
// ============================================================================
// COMPONENT NAME
// Brief description
// ============================================================================

import { useEffect, useState, useCallback } from "react";
import { getTransport } from "../../services/transport";
import type { SomeType } from "../../services/transport/types";
import { Icon1, Icon2 } from "lucide-react";

// ============================================================================
// Sub-component 1
// ============================================================================

function SmallComponent({ prop }: { prop: string }) {
  return <div>{prop}</div>;
}

// ============================================================================
// Sub-component 2
// ============================================================================

interface LargerComponentProps {
  data: SomeType;
  onAction: () => void;
}

function LargerComponent({ data, onAction }: LargerComponentProps) {
  // Implementation
}

// ============================================================================
// Main Component
// ============================================================================

export function WebFeaturePanel() {
  const [data, setData] = useState<SomeType[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Load data with useCallback
  const loadData = useCallback(async () => {
    try {
      const transport = await getTransport();
      const result = await transport.listSomething();
      if (result.success && result.data) {
        setData(result.data);
      }
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  }, [/* dependencies */]);

  // Initial load and optional refresh
  useEffect(() => {
    loadData();
  }, [loadData]);

  // Render
  if (isLoading) return <LoadingState />;
  if (error) return <ErrorState error={error} />;
  return <div>...</div>;
}

export default WebFeaturePanel;
```

## React StrictMode Considerations

React StrictMode double-mounts components in development. For WebSocket connections or other side effects that shouldn't run twice:

```tsx
useEffect(() => {
  let cancelled = false;

  const initialize = async () => {
    if (cancelled) return;

    // Do async work
    await someAsyncOperation();

    if (cancelled) return;

    // More work after async...
  };

  initialize();

  return () => {
    cancelled = true;
    // Cleanup (disconnect, etc.)
  };
}, [dependencies]);
```

## Transport Layer Usage

Always use the transport abstraction for API calls:

```tsx
import { getTransport } from "../../services/transport";

// In component or hook
const transport = await getTransport();

// All methods return TransportResult<T>
const result = await transport.listAgents();
if (result.success && result.data) {
  // Use result.data
} else {
  console.error("Failed:", result.error);
}
```

### Adding New Transport Methods

1. Add type definitions to `services/transport/types.ts`
2. Add method signature to `services/transport/interface.ts`
3. Implement in `services/transport/http.ts`
4. Add stub in `services/transport/tauri.ts` (returns error)

## CSS Patterns

Use CSS custom properties defined in the theme:

```tsx
// Colors
style={{ color: "var(--primary)" }}
style={{ backgroundColor: "var(--muted)" }}
style={{ borderColor: "var(--border)" }}

// With alpha
style={{ backgroundColor: "color-mix(in srgb, var(--primary) 20%, transparent)" }}

// Common classes
className="card"
className="card__header"
className="btn btn--primary btn--md"
className="page"
className="page-container"
```

## Icon Usage

Use Lucide React icons consistently:

```tsx
import {
  Play, Pause, Square,      // Controls
  RefreshCw, Loader2,       // Loading/refresh
  AlertCircle, CheckCircle, // Status
  ChevronDown, ChevronRight // Expansion
} from "lucide-react";

// Standard sizes
<Icon size={14} />  // Small (buttons)
<Icon size={16} />  // Default
<Icon size={20} />  // Medium (cards)
<Icon size={48} />  // Large (empty states)

// Animated spinner
<Loader2 className="animate-spin" size={16} />
```

## State Management Patterns

### Loading States
```tsx
const [isLoading, setIsLoading] = useState(true);
const [error, setError] = useState<string | null>(null);
```

### Processing States (for actions)
```tsx
const [processingId, setProcessingId] = useState<string | null>(null);

const handleAction = async (id: string) => {
  setProcessingId(id);
  try {
    await doAction(id);
    await loadData(); // Refresh
  } finally {
    setProcessingId(null);
  }
};
```

### Auto-refresh Pattern
```tsx
const [autoRefresh, setAutoRefresh] = useState(true);

useEffect(() => {
  loadData();

  if (autoRefresh) {
    const interval = setInterval(loadData, 3000);
    return () => clearInterval(interval);
  }
}, [loadData, autoRefresh]);
```

## Common UI Patterns

### Empty State
```tsx
<div className="p-8 text-center text-muted-foreground">
  <SomeIcon size={48} className="mx-auto mb-4 opacity-50" />
  <p>No items found</p>
  <p className="text-sm mt-2">Items will appear here when available</p>
</div>
```

### Error Display
```tsx
{error && (
  <div className="card p-4 mb-6 border-destructive bg-destructive/10">
    <div className="flex items-center gap-2 text-destructive">
      <AlertCircle size={16} />
      <span>{error}</span>
    </div>
  </div>
)}
```

### Expandable Rows
```tsx
const [expandedId, setExpandedId] = useState<string | null>(null);

<div onClick={() => setExpandedId(expandedId === item.id ? null : item.id)}>
  {expandedId === item.id ? <ChevronDown /> : <ChevronRight />}
  {/* Row content */}
</div>
{expandedId === item.id && (
  <div className="bg-muted/30 p-3">
    {/* Expanded content */}
  </div>
)}
```

## File Organization Checklist

When creating a new feature:

1. Create `apps/ui/src/features/<feature>/` directory
2. Create main component: `Web<Feature>Panel.tsx`
3. Add types to `services/transport/types.ts` if needed
4. Add transport methods if API calls needed
5. Add route in `App.tsx`
6. Add navigation in sidebar

## Remember

- Always handle loading and error states
- Use `useCallback` for functions passed to `useEffect`
- Check `cancelled` flag after async operations in effects
- Use transport layer, never direct fetch
- Follow existing naming conventions
- Use CSS custom properties for colors
