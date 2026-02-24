# UI/UX Enhancement Plan

**Goal**: Simple, professional look with consistent themes and spacing

**Status**: Planned
**Estimated Effort**: 5-7 days

---

## Current State

### Strengths
- Comprehensive design token system (`theme.css`)
- Light/Dark mode variables already defined
- Radix UI primitives for accessibility
- Apple-inspired color palette
- CSS class architecture in `components.css`

### Gaps
- No theme switcher UI (dark mode defined but not accessible)
- Mixed styling approaches (CSS classes + Tailwind + inline styles)
- Dashboard lacks visual hierarchy
- Empty states inconsistent
- No stats/metrics cards

---

## Phase 1: Theme Switcher (0.5 day)

### 1.1 Theme Provider Hook

**File:** `apps/ui/src/hooks/useTheme.ts`

```typescript
type Theme = 'light' | 'dark' | 'system';

export function useTheme() {
  const [theme, setTheme] = useState<Theme>(() => {
    const stored = localStorage.getItem('agentzero-theme');
    return (stored as Theme) || 'system';
  });

  useEffect(() => {
    const root = document.documentElement;
    const isDark = theme === 'dark' ||
      (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);

    root.classList.toggle('dark', isDark);
    localStorage.setItem('agentzero-theme', theme);
  }, [theme]);

  return { theme, setTheme, isDark: document.documentElement.classList.contains('dark') };
}
```

### 1.2 Theme Toggle Component

**File:** `apps/ui/src/components/ThemeToggle.tsx`

```tsx
// Simple icon toggle in sidebar footer
// Sun icon (light) / Moon icon (dark)
// Cycles: light → dark → system → light
```

### 1.3 Integration

Add to `WebAppShell` sidebar footer, above connection status:

```
┌─────────────────────┐
│  ...nav items...    │
├─────────────────────┤
│  🌙 Dark  [toggle]  │  ← New
│  ● Connected        │
└─────────────────────┘
```

---

## Phase 2: Spacing Standardization (1 day)

### 2.1 Spacing Utility Classes

**File:** `apps/ui/src/styles/components.css` - Add:

```css
/* Spacing utilities */
.p-page { padding: var(--spacing-6); }
.p-card { padding: var(--spacing-4); }
.p-card-lg { padding: var(--spacing-6); }

.gap-section { gap: var(--spacing-6); }
.gap-item { gap: var(--spacing-3); }
.gap-inline { gap: var(--spacing-2); }

.mb-section { margin-bottom: var(--spacing-6); }
.mb-item { margin-bottom: var(--spacing-3); }
```

### 2.2 Standard Page Layout

All pages should follow:

```tsx
<div className="page">
  <div className="page-container">
    <div className="page-header">
      <h1 className="page-title">Title</h1>
      <p className="page-subtitle">Description</p>
    </div>

    <div className="flex flex-col gap-section">  {/* Sections */}
      <section>...</section>
      <section>...</section>
    </div>
  </div>
</div>
```

### 2.3 Audit & Fix

Review and fix spacing in:
- `WebMemoryPanel.tsx`
- `WebAgentsPanel.tsx`
- `WebSkillsPanel.tsx`
- `WebConnectorsPanel.tsx`
- `WebOpsDashboard.tsx`

---

## Phase 3: Card System Enhancement (1 day)

### 3.1 Card Variants

**File:** `apps/ui/src/styles/components.css` - Enhance:

```css
/* Card base */
.card {
  background: var(--card);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border);
}

/* Elevated - for important content */
.card--elevated {
  border: none;
  box-shadow: var(--shadow-card);
}

.card--elevated:hover {
  box-shadow: var(--shadow-card-hover);
}

/* Interactive - for clickable items */
.card--interactive {
  cursor: pointer;
  transition: transform 0.15s ease, box-shadow 0.15s ease;
}

.card--interactive:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-card-hover);
}

.card--interactive:active {
  transform: translateY(0);
}

/* Bordered - for form sections */
.card--bordered {
  border: 1px solid var(--border);
}

/* Stats card - for dashboard metrics */
.card--stat {
  display: flex;
  flex-direction: column;
  padding: var(--spacing-4);
  gap: var(--spacing-2);
}
```

### 3.2 Card Header Component

**File:** `apps/ui/src/shared/ui/Card.tsx` - Enhance:

```tsx
<Card>
  <CardHeader
    icon={Bot}
    iconVariant="primary"  // primary | success | warning | destructive
    title="Agents"
    description="Manage your AI agents"
    action={<Button>Add</Button>}
  />
  <CardContent>...</CardContent>
</Card>
```

---

## Phase 4: Dashboard Redesign (1.5 days)

### 4.1 Stats Cards Row

**File:** `apps/ui/src/features/ops/components/StatsCards.tsx`

```tsx
// 4-column stats row
// Sessions | Running | Agents | Workers
// Each with icon, count, trend/status

┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐
│ 📊 Sessions│ │ ▶ Running  │ │ 🤖 Agents  │ │ 🔌 Workers │
│    142     │ │     3      │ │     8      │ │     5      │
│   +12%     │ │   active   │ │  defined   │ │   online   │
└────────────┘ └────────────┘ └────────────┘ └────────────┘
```

### 4.2 Recent Activity Section

**File:** `apps/ui/src/features/ops/components/RecentActivity.tsx`

```tsx
// List of recent sessions with:
// - Source badge (web/cli/cron/connector)
// - Agent name
// - Time ago
// - Status indicator
```

### 4.3 Quick Actions Panel

**File:** `apps/ui/src/features/ops/components/QuickActions.tsx`

```tsx
// Sidebar-style panel with action buttons
// + New Agent | + New Skill | Schedule Job | View Logs
```

### 4.4 Dashboard Layout

```tsx
<div className="page">
  <div className="page-container">
    <div className="page-header">
      <h1 className="page-title">Dashboard</h1>
    </div>

    {/* Stats row */}
    <StatsCards stats={stats} className="mb-section" />

    {/* Two-column layout */}
    <div className="grid grid-cols-3 gap-section">
      {/* Main content - 2 cols */}
      <div className="col-span-2">
        <Card>
          <CardHeader title="Recent Activity" />
          <RecentActivity sessions={sessions} />
        </Card>
      </div>

      {/* Sidebar - 1 col */}
      <div>
        <Card className="mb-section">
          <CardHeader title="Quick Actions" />
          <QuickActions />
        </Card>

        <Card>
          <CardHeader title="System Status" />
          <SystemStatus />
        </Card>
      </div>
    </div>
  </div>
</div>
```

---

## Phase 5: Empty States Component (0.5 day)

### 5.1 Reusable Component

**File:** `apps/ui/src/shared/ui/EmptyState.tsx`

```tsx
interface EmptyStateProps {
  icon?: React.ComponentType;
  title: string;
  description?: string;
  action?: {
    label: string;
    onClick: () => void;
  };
}

export function EmptyState({ icon: Icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="empty-state">
      {Icon && (
        <div className="empty-state__icon">
          <Icon />
        </div>
      )}
      <h3 className="empty-state__title">{title}</h3>
      {description && <p className="empty-state__description">{description}</p>}
      {action && (
        <button className="btn btn--primary btn--md" onClick={action.onClick}>
          {action.label}
        </button>
      )}
    </div>
  );
}
```

### 5.2 Styles

**File:** `apps/ui/src/styles/components.css`:

```css
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-12) var(--spacing-6);
  text-align: center;
}

.empty-state__icon {
  width: 64px;
  height: 64px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--muted);
  border-radius: var(--radius-xl);
  margin-bottom: var(--spacing-4);
}

.empty-state__icon svg {
  width: 28px;
  height: 28px;
  color: var(--muted-foreground);
}

.empty-state__title {
  font-size: var(--text-lg);
  font-weight: 600;
  color: var(--foreground);
  margin-bottom: var(--spacing-2);
}

.empty-state__description {
  font-size: var(--text-sm);
  color: var(--muted-foreground);
  max-width: 320px;
  margin-bottom: var(--spacing-4);
}
```

### 5.3 Apply To

- `WebMemoryPanel` - "No memories yet"
- `WebAgentsPanel` - "No agents configured"
- `WebSkillsPanel` - "No skills installed"
- `WebConnectorsPanel` - "No workers connected"
- `WebLogsPanel` - "No sessions recorded"

---

## Phase 6: Navigation Polish (0.5 day)

### 6.1 Enhanced Active States

**File:** `apps/ui/src/styles/components.css`:

```css
.nav-link {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-2-5) var(--spacing-3);
  border-radius: var(--radius-md);
  color: var(--sidebar-muted);
  transition: all 0.15s ease;
}

.nav-link:hover {
  background: var(--sidebar-accent);
  color: var(--sidebar-foreground);
}

.nav-link--active {
  background: var(--sidebar-primary);
  color: var(--sidebar-primary-foreground);
}

.nav-link__icon {
  width: 18px;
  height: 18px;
  opacity: 0.7;
}

.nav-link--active .nav-link__icon {
  opacity: 1;
}
```

### 6.2 Navigation Groups

Add subtle separators between groups:

```css
.sidebar__group + .sidebar__group {
  margin-top: var(--spacing-4);
  padding-top: var(--spacing-4);
  border-top: 1px solid var(--sidebar-border);
}
```

---

## Phase 7: Chat Polish (1 day)

### 7.1 Message Bubbles

**File:** `apps/ui/src/features/agent/WebChatPanel.tsx`:

```tsx
// User messages: right-aligned, primary color background
// Assistant messages: left-aligned, card background

<div className={cn(
  "message",
  role === 'user' ? "message--user" : "message--assistant"
)}>
  {role === 'assistant' && (
    <div className="message__avatar">
      <Bot />
    </div>
  )}
  <div className="message__content">
    <ReactMarkdown>{content}</ReactMarkdown>
  </div>
  <div className="message__meta">
    <span className="message__time">2:34 PM</span>
    {tokens && <span className="message__tokens">245 tokens</span>}
  </div>
</div>
```

### 7.2 Message Styles

**File:** `apps/ui/src/styles/components.css`:

```css
.message {
  display: flex;
  flex-direction: column;
  max-width: 80%;
  margin-bottom: var(--spacing-4);
}

.message--user {
  margin-left: auto;
  align-items: flex-end;
}

.message--assistant {
  margin-right: auto;
  align-items: flex-start;
}

.message__content {
  padding: var(--spacing-3) var(--spacing-4);
  border-radius: var(--radius-lg);
}

.message--user .message__content {
  background: var(--primary);
  color: var(--primary-foreground);
  border-bottom-right-radius: var(--radius-sm);
}

.message--assistant .message__content {
  background: var(--card);
  border: 1px solid var(--border);
  border-bottom-left-radius: var(--radius-sm);
}

.message__meta {
  display: flex;
  gap: var(--spacing-2);
  margin-top: var(--spacing-1);
  font-size: var(--text-xs);
  color: var(--muted-foreground);
}
```

### 7.3 Code Block Styling

```css
.message__content pre {
  background: var(--muted);
  border-radius: var(--radius-md);
  padding: var(--spacing-3);
  overflow-x: auto;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  margin: var(--spacing-2) 0;
}

.message__content code:not(pre code) {
  background: var(--muted);
  padding: 2px var(--spacing-1);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}
```

---

## Files Summary

### New Files
| File | Description |
|------|-------------|
| `apps/ui/src/hooks/useTheme.ts` | Theme management hook |
| `apps/ui/src/components/ThemeToggle.tsx` | Theme toggle button |
| `apps/ui/src/shared/ui/EmptyState.tsx` | Reusable empty state |
| `apps/ui/src/features/ops/components/StatsCards.tsx` | Dashboard stats |
| `apps/ui/src/features/ops/components/RecentActivity.tsx` | Activity list |
| `apps/ui/src/features/ops/components/QuickActions.tsx` | Action buttons |

### Modified Files
| File | Changes |
|------|---------|
| `apps/ui/src/styles/components.css` | Card variants, spacing utils, message styles |
| `apps/ui/src/App.tsx` | Theme provider, toggle in sidebar |
| `apps/ui/src/features/ops/WebOpsDashboard.tsx` | New layout with stats |
| `apps/ui/src/features/agent/WebChatPanel.tsx` | Message bubbles |
| All panel components | Empty states, spacing |

---

## Visual Reference

### Before
```
┌─────────────────────────────────────────┐
│ Dashboard                               │
│                                         │
│ [basic session list with minimal style] │
│                                         │
└─────────────────────────────────────────┘
```

### After
```
┌─────────────────────────────────────────────────────────┐
│ Dashboard                                               │
├─────────────────────────────────────────────────────────┤
│ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐    │
│ │📊 Sessions│ │▶ Running │ │🤖 Agents │ │🔌 Workers│    │
│ │   142    │ │    3     │ │    8     │ │    5     │    │
│ │  +12%    │ │  active  │ │ defined  │ │  online  │    │
│ └──────────┘ └──────────┘ └──────────┘ └──────────┘    │
├─────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────┐ ┌───────────────────┐  │
│ │ Recent Activity              │ │ Quick Actions     │  │
│ │ ┌─────────────────────────┐ │ │ ┌───────────────┐ │  │
│ │ │ 🔌 slack-worker  2m ago │ │ │ │ + New Agent   │ │  │
│ │ │ ⏱️ cron: report  1h ago │ │ │ │ + New Skill   │ │  │
│ │ │ 🌐 web session  3h ago  │ │ │ │ Schedule Job  │ │  │
│ │ └─────────────────────────┘ │ │ └───────────────┘ │  │
│ └─────────────────────────────┘ └───────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## Implementation Order

1. **Phase 1**: Theme Switcher (quick win, high visibility)
2. **Phase 2**: Spacing Standardization (foundation for other changes)
3. **Phase 5**: Empty States (reusable component)
4. **Phase 3**: Card System (builds on spacing)
5. **Phase 4**: Dashboard Redesign (uses all above)
6. **Phase 6**: Navigation Polish (subtle enhancement)
7. **Phase 7**: Chat Polish (focused improvement)

---

## Success Criteria

- [ ] Theme switcher visible and functional in sidebar
- [ ] All pages use consistent spacing (6px increments)
- [ ] Dashboard shows stats cards with metrics
- [ ] Empty states are consistent and actionable
- [ ] Navigation has clear active states
- [ ] Chat messages are visually distinct (user vs assistant)
- [ ] No inline styles for spacing/colors
- [ ] Dark mode works across all components
